use std::collections::HashMap;
use std::path::{Path, PathBuf};

use crate::config::plugins::{PluginSettings, PluginsConfig};

use super::host::{LoadedPlugin, PluginHost};
use super::matcher::OutputMatcher;
use super::policy::{capability_from_name, capability_name, grant_with_consent, requires_consent};
use super::{
    Capability, MatchEvent, MenuContext, MenuItem, PluginInfo, PluginProfile, SettingEvent,
    SettingField, StatusItem,
};

pub const COMPONENT_FILE: &str = "plugin.wasm";

#[derive(Clone)]
pub struct ClickablePattern {
    pub plugin: String,
    pub pattern: String,
    pub regex: std::sync::Arc<regex::Regex>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ContributedCommand {
    pub plugin: String,
    pub source: String,
    pub id: String,
    pub title: String,
    pub default_key: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Status {
    Ready,
    Disabled,
    Retired(String),
}

enum Slot {
    Ready(Box<LoadedPlugin>, Box<OutputMatcher>),
    Disabled,
    Retired(String),
}

struct Entry {
    id: String,
    path: PathBuf,
    slot: Slot,
    info: Option<PluginInfo>,
    fields: Vec<SettingField>,
    status: Vec<StatusItem>,
    menu: Vec<MenuItem>,
    profiles: Vec<PluginProfile>,
}

impl Entry {
    fn remember(&mut self, host: &PluginHost) {
        if let Slot::Ready(plugin, _) = &mut self.slot {
            self.info = Some(plugin.info().clone());
            self.fields = plugin.contributions().settings.clone();
            self.status = plugin.contributions().status_items.clone();
            self.menu = plugin.contributions().menu_items.clone();
        } else if self.info.is_none()
            && let Ok((info, fields)) = host.inspect(&self.path)
        {
            self.info = Some(info);
            self.fields = fields;
        }
    }
}

#[derive(Clone)]
pub struct ProfileSource {
    pub id: String,
    pub path: PathBuf,
    consented: Vec<Capability>,
    config: HashMap<String, String>,
}

pub struct PluginRegistry {
    host: std::sync::Arc<PluginHost>,
    settings: PluginsConfig,
    entries: Vec<Entry>,
}

impl Slot {
    fn ready(plugin: LoadedPlugin) -> Self {
        let (matcher, rejected) = OutputMatcher::compile(&plugin.contributions().output_patterns);
        for (id, reason) in rejected {
            eprintln!("plugin pattern {id} rejected: {reason}");
        }
        Self::Ready(Box::new(plugin), Box::new(matcher))
    }
}

impl PluginRegistry {
    pub fn new(host: PluginHost, settings: PluginsConfig) -> Self {
        Self {
            host: std::sync::Arc::new(host),
            settings,
            entries: Vec::new(),
        }
    }

    pub fn settings(&self) -> &PluginsConfig {
        &self.settings
    }

    pub fn root(&self) -> &Path {
        self.host.root()
    }

    pub fn load_all(&mut self) {
        self.shutdown_all();
        self.entries.clear();
        self.host.forget_components();
        for (id, path) in discover(self.host.root()) {
            let settings = self.settings.get(&id).cloned().unwrap_or_default();
            let slot = if settings.enabled {
                Self::instantiate(&self.host, &id, &path, &settings)
            } else {
                Slot::Disabled
            };
            let mut entry = Entry {
                id,
                path,
                slot,
                info: None,
                fields: Vec::new(),
                status: Vec::new(),
                menu: Vec::new(),
                profiles: Vec::new(),
            };
            entry.remember(&self.host);
            self.entries.push(entry);
        }
    }

    fn instantiate(host: &PluginHost, id: &str, path: &Path, settings: &PluginSettings) -> Slot {
        let consented = consented_capabilities(settings);
        let policy = |info: &PluginInfo| grant_with_consent(info, &consented);
        let values: HashMap<String, String> = settings
            .settings
            .iter()
            .map(|(key, value)| (key.clone(), value.clone()))
            .collect();
        match host.load(id, path, values, &policy) {
            Ok(plugin) => Slot::ready(plugin),
            Err(err) => Slot::Retired(err.to_string()),
        }
    }

    pub fn ids(&self) -> impl Iterator<Item = &str> {
        self.entries.iter().map(|entry| entry.id.as_str())
    }

    pub fn status(&self, id: &str) -> Option<Status> {
        self.entry(id).map(|entry| match &entry.slot {
            Slot::Ready(plugin, _) => match plugin.failure() {
                Some(reason) => Status::Retired(reason.to_string()),
                None => Status::Ready,
            },
            Slot::Disabled => Status::Disabled,
            Slot::Retired(reason) => Status::Retired(reason.clone()),
        })
    }

    pub fn match_output(
        &mut self,
        pane: u64,
        line: &str,
        now: std::time::Instant,
    ) -> Vec<(String, MatchEvent)> {
        let mut events = Vec::new();
        for entry in &mut self.entries {
            let Slot::Ready(plugin, matcher) = &mut entry.slot else {
                continue;
            };
            if plugin.failure().is_some() || matcher.is_empty() {
                continue;
            }
            let found = matcher.hits(line, now);
            if let Some(dropped) = matcher.take_throttle_warning() {
                eprintln!(
                    "plugin {} exceeded the output match rate; {dropped} dropped so far",
                    entry.id
                );
            }
            for hit in found {
                events.push((
                    entry.id.clone(),
                    MatchEvent {
                        pane,
                        pattern: hit.pattern,
                        line: line.to_string(),
                        start: hit.start,
                        end: hit.end,
                    },
                ));
            }
        }
        events
    }

    pub fn has_ready(&self) -> bool {
        self.entries.iter().any(
            |entry| matches!(&entry.slot, Slot::Ready(plugin, _) if plugin.failure().is_none()),
        )
    }

    pub fn watches_output(&self) -> bool {
        self.entries.iter().any(|entry| {
            matches!(&entry.slot, Slot::Ready(plugin, matcher)
                if plugin.failure().is_none() && !matcher.is_empty())
        })
    }

    pub fn setting_fields(&self, id: &str) -> Vec<SettingField> {
        self.entry(id)
            .map(|entry| entry.fields.clone())
            .unwrap_or_default()
    }

    pub fn status_items(&self) -> Vec<(String, StatusItem)> {
        self.entries
            .iter()
            .filter(
                |entry| matches!(&entry.slot, Slot::Ready(plugin, _) if plugin.failure().is_none()),
            )
            .flat_map(|entry| {
                entry
                    .status
                    .iter()
                    .map(move |item| (entry.id.clone(), item.clone()))
            })
            .collect()
    }

    pub fn clickable_patterns(&self) -> Vec<ClickablePattern> {
        self.entries
            .iter()
            .filter_map(|entry| match &entry.slot {
                Slot::Ready(plugin, matcher) if plugin.failure().is_none() => {
                    Some((entry, matcher))
                }
                _ => None,
            })
            .flat_map(|(entry, matcher)| {
                matcher
                    .clickable()
                    .map(|(id, regex)| ClickablePattern {
                        plugin: entry.id.clone(),
                        pattern: id.to_string(),
                        regex,
                    })
                    .collect::<Vec<_>>()
            })
            .collect()
    }

    pub fn host(&self) -> std::sync::Arc<PluginHost> {
        std::sync::Arc::clone(&self.host)
    }

    pub fn profile_sources(&self) -> Vec<ProfileSource> {
        self.entries
            .iter()
            .filter(
                |entry| matches!(&entry.slot, Slot::Ready(plugin, _) if plugin.failure().is_none()),
            )
            .map(|entry| {
                let settings = self.settings.get(&entry.id).cloned().unwrap_or_default();
                ProfileSource {
                    id: entry.id.clone(),
                    path: entry.path.clone(),
                    consented: consented_capabilities(&settings),
                    config: settings.settings.into_iter().collect(),
                }
            })
            .collect()
    }

    pub fn set_profiles(&mut self, id: &str, profiles: Vec<PluginProfile>) {
        if let Some(entry) = self.entry_mut(id) {
            entry.profiles = profiles;
        }
    }

    pub fn profiles(&self) -> Vec<(String, PluginProfile)> {
        self.entries
            .iter()
            .filter(
                |entry| matches!(&entry.slot, Slot::Ready(plugin, _) if plugin.failure().is_none()),
            )
            .flat_map(|entry| {
                entry
                    .profiles
                    .iter()
                    .map(move |profile| (entry.id.clone(), profile.clone()))
            })
            .collect()
    }

    pub fn menu_items(&self, context: MenuContext) -> Vec<(String, MenuItem)> {
        self.entries
            .iter()
            .filter(
                |entry| matches!(&entry.slot, Slot::Ready(plugin, _) if plugin.failure().is_none()),
            )
            .flat_map(|entry| {
                entry
                    .menu
                    .iter()
                    .filter(move |item| item.context == context)
                    .map(move |item| (entry.id.clone(), item.clone()))
            })
            .collect()
    }

    pub fn set_status(&mut self, id: &str, item: &str, text: String) -> bool {
        let Some(entry) = self.entry_mut(id) else {
            return false;
        };
        match entry.status.iter_mut().find(|slot| slot.id == item) {
            Some(slot) => {
                slot.text = text;
                true
            }
            None => false,
        }
    }

    pub fn info(&self, id: &str) -> Option<&PluginInfo> {
        self.entry(id)?.info.as_ref()
    }

    pub fn is_enabled(&self, id: &str) -> bool {
        self.settings
            .get(id)
            .is_none_or(|settings| settings.enabled)
    }

    pub fn granted(&self, id: &str) -> Vec<Capability> {
        let Some(info) = self.info(id) else {
            return Vec::new();
        };
        let consented = self
            .settings
            .get(id)
            .map(consented_capabilities)
            .unwrap_or_default();
        grant_with_consent(info, &consented)
    }

    pub fn setting_value(&self, id: &str, key: &str) -> Option<String> {
        let stored = self
            .settings
            .get(id)
            .and_then(|settings| settings.settings.get(key))
            .cloned();
        stored.or_else(|| {
            self.setting_fields(id)
                .into_iter()
                .find(|field| field.key == key)
                .map(|field| field.default_value)
        })
    }

    pub fn set_setting(&mut self, id: &str, key: &str, value: String) -> Option<SettingEvent> {
        if self.setting_value(id, key).as_deref() == Some(value.as_str()) {
            return None;
        }
        self.settings
            .entry(id.to_string())
            .or_default()
            .settings
            .insert(key.to_string(), value.clone());
        Some(SettingEvent {
            key: key.to_string(),
            value,
        })
    }

    pub fn contributed_commands(&self) -> Vec<ContributedCommand> {
        self.entries
            .iter()
            .filter_map(|entry| match &entry.slot {
                Slot::Ready(plugin, _) if plugin.failure().is_none() => Some((entry, plugin)),
                _ => None,
            })
            .flat_map(|(entry, plugin)| {
                let source = plugin.info().name.clone();
                plugin
                    .contributions()
                    .commands
                    .iter()
                    .map(move |command| ContributedCommand {
                        plugin: entry.id.clone(),
                        source: source.clone(),
                        id: command.id.clone(),
                        title: command.title.clone(),
                        default_key: command.default_key.clone(),
                    })
            })
            .collect()
    }

    pub fn pending_consent(&self) -> Vec<(String, Vec<Capability>)> {
        self.entries
            .iter()
            .filter_map(|entry| match &entry.slot {
                Slot::Ready(plugin, _) => {
                    let wanted = requires_consent(plugin.info());
                    let granted = plugin.granted();
                    let missing: Vec<Capability> = wanted
                        .into_iter()
                        .filter(|cap| !granted.contains(cap))
                        .collect();
                    (!missing.is_empty()).then(|| (entry.id.clone(), missing))
                }
                _ => None,
            })
            .collect()
    }

    pub fn get_mut(&mut self, id: &str) -> Option<&mut LoadedPlugin> {
        match &mut self.entry_mut(id)?.slot {
            Slot::Ready(plugin, _) if plugin.failure().is_none() => Some(plugin),
            _ => None,
        }
    }

    pub fn ready_mut(&mut self) -> impl Iterator<Item = (&str, &mut LoadedPlugin)> {
        self.entries
            .iter_mut()
            .filter_map(|entry| match &mut entry.slot {
                Slot::Ready(plugin, _) if plugin.failure().is_none() => {
                    Some((entry.id.as_str(), plugin.as_mut()))
                }
                _ => None,
            })
    }

    pub fn shutdown_all(&mut self) -> Vec<(String, String)> {
        let mut failures = Vec::new();
        for entry in &mut self.entries {
            if let Slot::Ready(plugin, _) = &mut entry.slot
                && let Err(err) = plugin.shutdown()
            {
                failures.push((entry.id.clone(), err.to_string()));
            }
        }
        failures
    }

    pub fn disable(&mut self, id: &str) -> bool {
        match self.entry_mut(id) {
            Some(entry) => {
                if let Slot::Ready(plugin, _) = &mut entry.slot {
                    let _ = plugin.shutdown();
                }
                entry.slot = Slot::Disabled;
                self.settings.entry(id.to_string()).or_default().enabled = false;
                true
            }
            None => false,
        }
    }

    pub fn enable(&mut self, id: &str) -> Result<(), String> {
        let Some(index) = self.entries.iter().position(|entry| entry.id == id) else {
            return Err(format!("no plugin named {id}"));
        };
        let settings = self.settings.entry(id.to_string()).or_default();
        settings.enabled = true;
        let settings = settings.clone();

        if let Slot::Ready(plugin, _) = &mut self.entries[index].slot {
            let _ = plugin.shutdown();
        }
        let entry = &self.entries[index];
        let slot = Self::instantiate(&self.host, &entry.id, &entry.path, &settings);
        let outcome = match &slot {
            Slot::Retired(reason) => Err(reason.clone()),
            _ => Ok(()),
        };
        self.entries[index].slot = slot;
        self.entries[index].remember(&self.host);
        outcome
    }

    pub fn consent(&mut self, id: &str, capability: Capability) {
        let settings = self.settings.entry(id.to_string()).or_default();
        let name = capability_name(capability).to_string();
        if !settings.consented.contains(&name) {
            settings.consented.push(name);
        }
    }

    pub fn revoke(&mut self, id: &str, capability: Capability) {
        let name = capability_name(capability);
        if let Some(settings) = self.settings.get_mut(id) {
            settings.consented.retain(|granted| granted != name);
        }
    }

    pub fn retire_failed(&mut self) -> Vec<(String, String)> {
        let mut retired = Vec::new();
        for entry in &mut self.entries {
            if let Slot::Ready(plugin, _) = &entry.slot
                && let Some(reason) = plugin.failure()
            {
                let reason = reason.to_string();
                retired.push((entry.id.clone(), reason.clone()));
                entry.slot = Slot::Retired(reason);
            }
        }
        retired
    }

    fn entry(&self, id: &str) -> Option<&Entry> {
        self.entries.iter().find(|entry| entry.id == id)
    }

    fn entry_mut(&mut self, id: &str) -> Option<&mut Entry> {
        self.entries.iter_mut().find(|entry| entry.id == id)
    }
}

fn discover(root: &Path) -> Vec<(String, PathBuf)> {
    let Ok(dir) = std::fs::read_dir(root) else {
        return Vec::new();
    };
    let mut found: Vec<(String, PathBuf)> = dir
        .flatten()
        .filter_map(|entry| {
            let id = entry.file_name().to_str()?.to_string();
            let component = entry.path().join(COMPONENT_FILE);
            component.is_file().then_some((id, component))
        })
        .collect();
    found.sort_by(|a, b| a.0.cmp(&b.0));
    found
}

fn consented_capabilities(settings: &PluginSettings) -> Vec<Capability> {
    settings
        .consented
        .iter()
        .filter_map(|name| capability_from_name(name))
        .collect()
}

/// Runs on a worker thread: the live instance's `Store` is `!Sync`, so profile
/// enumeration gets an instance of its own that is dropped when it answers.
pub fn fetch_profiles_blocking(
    host: &PluginHost,
    source: &ProfileSource,
) -> Result<Vec<PluginProfile>, String> {
    let consented = source.consented.clone();
    let policy = |info: &PluginInfo| grant_with_consent(info, &consented);
    host.fetch_profiles(&source.id, &source.path, source.config.clone(), &policy)
        .map_err(|err| err.to_string())
}

/// A source that never answers would otherwise pin the caller forever, so the
/// call is handed to a thread we are willing to abandon. Fuel cannot help here:
/// it counts instructions, and a guest blocked in host I/O burns none.
pub fn fetch_profiles_with_deadline(
    host: &std::sync::Arc<PluginHost>,
    source: &ProfileSource,
    deadline: std::time::Duration,
) -> Option<Vec<PluginProfile>> {
    let (tx, rx) = std::sync::mpsc::channel();
    let host = std::sync::Arc::clone(host);
    let worker = source.clone();
    let id = source.id.clone();
    std::thread::spawn(move || {
        let _ = tx.send(fetch_profiles_blocking(&host, &worker));
    });

    match rx.recv_timeout(deadline) {
        Ok(Ok(profiles)) => Some(profiles),
        Ok(Err(reason)) => {
            eprintln!("plugin {id} failed to list profiles: {reason}");
            None
        }
        Err(_) => {
            eprintln!("plugin {id} did not list profiles in time; abandoning the call");
            None
        }
    }
}

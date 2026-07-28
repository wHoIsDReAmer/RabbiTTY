use std::collections::HashMap;
use std::path::{Path, PathBuf};

use crate::config::plugins::{PluginSettings, PluginsConfig};

use super::host::{LoadedPlugin, PluginHost};
use super::matcher::OutputMatcher;
use super::policy::{capability_from_name, capability_name, grant_with_consent, requires_consent};
use super::{Capability, MatchEvent, PluginInfo, SettingEvent, SettingField};

pub const COMPONENT_FILE: &str = "plugin.wasm";

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
}

pub struct PluginRegistry {
    host: PluginHost,
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
            host,
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
        for (id, path) in discover(self.host.root()) {
            let settings = self.settings.get(&id).cloned().unwrap_or_default();
            let slot = if settings.enabled {
                Self::instantiate(&self.host, &id, &path, &settings)
            } else {
                Slot::Disabled
            };
            self.entries.push(Entry { id, path, slot });
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

    pub fn watches_output(&self) -> bool {
        self.entries.iter().any(|entry| {
            matches!(&entry.slot, Slot::Ready(plugin, matcher)
                if plugin.failure().is_none() && !matcher.is_empty())
        })
    }

    pub fn setting_fields(&self, id: &str) -> Vec<SettingField> {
        match self.entry(id).map(|entry| &entry.slot) {
            Some(Slot::Ready(plugin, _)) => plugin.contributions().settings.clone(),
            _ => Vec::new(),
        }
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

    pub fn contributed_commands(&self) -> Vec<(String, Vec<String>)> {
        self.entries
            .iter()
            .filter_map(|entry| match &entry.slot {
                Slot::Ready(plugin, _) => {
                    let ids: Vec<String> = plugin
                        .contributions()
                        .commands
                        .iter()
                        .map(|command| command.id.clone())
                        .collect();
                    (!ids.is_empty()).then(|| (entry.id.clone(), ids))
                }
                _ => None,
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
        outcome
    }

    pub fn consent(&mut self, id: &str, capability: Capability) {
        let settings = self.settings.entry(id.to_string()).or_default();
        let name = capability_name(capability).to_string();
        if !settings.consented.contains(&name) {
            settings.consented.push(name);
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

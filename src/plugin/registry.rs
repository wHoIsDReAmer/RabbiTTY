use std::collections::HashMap;
use std::path::{Path, PathBuf};

use crate::config::plugins::{PluginSettings, PluginsConfig};

use super::host::{
    COMMAND_DEADLINE, EVENT_DEADLINE, LoadedPlugin, PROFILE_DEADLINE, PluginError, PluginHost,
    SHUTDOWN_DEADLINE, START_DEADLINE,
};
use super::matcher::OutputMatcher;
use super::policy::{capability_from_name, capability_name, grant_with_consent, requires_consent};
use super::{
    Capability, Contributions, Event, MatchEvent, MenuContext, MenuItem, PluginInfo, PluginProfile,
    PluginRequest, SettingEvent, SettingField, StatusItem,
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
    Ready(Box<PluginWorker>, Box<OutputMatcher>),
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
    fn remember(&mut self, host: &std::sync::Arc<PluginHost>) {
        if let Slot::Ready(plugin, _) = &mut self.slot {
            self.info = Some(plugin.info().clone());
            self.fields = plugin.contributions().settings.clone();
            self.status = plugin.contributions().status_items.clone();
            self.menu = plugin.contributions().menu_items.clone();
        } else if self.info.is_none()
            && let Ok((info, fields)) = PluginWorker::inspect(host, &self.path)
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
    fn ready(plugin: PluginWorker) -> Self {
        let (matcher, rejected) = OutputMatcher::compile(&plugin.contributions().output_patterns);
        for (id, reason) in rejected {
            eprintln!("plugin pattern {id} rejected: {reason}");
        }
        Self::Ready(Box::new(plugin), Box::new(matcher))
    }
}

/// What the plugin's own thread is asked to do. One job is outstanding at a
/// time: `PluginWorker` takes `&mut self` for every call, so a reply always
/// belongs to the job the caller just sent.
enum Job {
    Command(String),
    Event(Event),
    Profiles,
    Shutdown,
}

enum Answer {
    Done(Result<(), PluginError>),
    Profiles(Result<Vec<PluginProfile>, PluginError>),
}

/// Every reply carries whatever the guest asked for while it ran, so draining
/// requests costs no second round trip, and the instance's latched failure, so
/// the worker mirrors a trap exactly as the instance saw it.
struct Reply {
    answer: Answer,
    requests: Vec<PluginRequest>,
    failure: Option<String>,
}

/// What starting a plugin yields. `init` and `contributions` are guest calls,
/// so even a load has to be something the caller can give up on.
struct Boot {
    info: PluginInfo,
    contributions: Contributions,
    granted: Vec<Capability>,
    requests: Vec<PluginRequest>,
}

/// A `LoadedPlugin` on a thread of its own, reached only through a channel with
/// a deadline on it. The instance itself cannot be bounded: a guest parked in a
/// host or WASI call reaches no epoch check and burns no fuel, so nothing short
/// of not waiting for it keeps the UI thread moving.
pub struct PluginWorker {
    id: String,
    /// Dropped once the worker is unusable, which also releases the thread as
    /// soon as whatever it is running returns.
    jobs: Option<std::sync::mpsc::Sender<Job>>,
    replies: std::sync::mpsc::Receiver<Reply>,
    pending: Vec<PluginRequest>,
    info: PluginInfo,
    contributions: Contributions,
    granted: Vec<Capability>,
    failure: Option<String>,
    profile_deadline: std::time::Duration,
}

impl PluginWorker {
    pub(super) fn load(
        host: &std::sync::Arc<PluginHost>,
        id: &str,
        path: &Path,
        config: HashMap<String, String>,
        consented: Vec<Capability>,
    ) -> Result<Self, String> {
        host.precompile(path).map_err(|err| err.to_string())?;

        let (boot_tx, boot_rx) = std::sync::mpsc::channel::<Result<Boot, String>>();
        let (jobs_tx, jobs_rx) = std::sync::mpsc::channel::<Job>();
        let (reply_tx, reply_rx) = std::sync::mpsc::channel::<Reply>();

        let host = std::sync::Arc::clone(host);
        let owned_id = id.to_string();
        let owned_path = path.to_path_buf();
        std::thread::Builder::new()
            .name(format!("plugin-{id}"))
            .spawn(move || {
                let policy = |info: &PluginInfo| grant_with_consent(info, &consented);
                let mut plugin = match host.load(&owned_id, &owned_path, config, &policy) {
                    Ok(plugin) => plugin,
                    Err(err) => {
                        let _ = boot_tx.send(Err(err.to_string()));
                        return;
                    }
                };
                let boot = Boot {
                    info: plugin.info().clone(),
                    contributions: plugin.contributions().clone(),
                    granted: plugin.granted().to_vec(),
                    requests: plugin.drain_requests(),
                };
                if boot_tx.send(Ok(boot)).is_ok() {
                    serve(&mut plugin, &jobs_rx, &reply_tx);
                }
            })
            .map_err(|err| err.to_string())?;

        let boot = match boot_rx.recv_timeout(START_DEADLINE) {
            Ok(boot) => boot?,
            Err(_) => {
                return Err(format!("did not start within {START_DEADLINE:?}"));
            }
        };

        Ok(Self {
            id: id.to_string(),
            jobs: Some(jobs_tx),
            replies: reply_rx,
            pending: boot.requests,
            info: boot.info,
            contributions: boot.contributions,
            granted: boot.granted,
            failure: None,
            profile_deadline: PROFILE_DEADLINE,
        })
    }

    /// Reading a component means running it: `manifest` and `contributions` are
    /// guest calls. So inspection gets a worker too, kept only long enough to
    /// answer and then dropped, exactly like the throwaway probe it replaces.
    pub(super) fn inspect(
        host: &std::sync::Arc<PluginHost>,
        path: &Path,
    ) -> Result<(PluginInfo, Vec<SettingField>), String> {
        let probe = Self::load(host, "", path, HashMap::new(), Vec::new())?;
        Ok((probe.info().clone(), probe.contributions().settings.clone()))
    }

    pub fn info(&self) -> &PluginInfo {
        &self.info
    }

    pub fn contributions(&self) -> &Contributions {
        &self.contributions
    }

    pub fn granted(&self) -> &[Capability] {
        &self.granted
    }

    pub fn failure(&self) -> Option<&str> {
        self.failure.as_deref()
    }

    /// A source the user just asked to enumerate may be given longer than the
    /// budget the registry hands its own instance.
    pub(super) fn set_profile_deadline(&mut self, deadline: std::time::Duration) {
        self.profile_deadline = deadline;
    }

    pub fn run_command(&mut self, id: &str) -> Result<(), PluginError> {
        self.settle(Job::Command(id.to_string()), COMMAND_DEADLINE)
    }

    pub fn on_event(&mut self, event: Event) -> Result<(), PluginError> {
        self.settle(Job::Event(event), EVENT_DEADLINE)
    }

    pub fn list_profiles(&mut self) -> Result<Vec<PluginProfile>, PluginError> {
        let deadline = self.profile_deadline;
        match self.dispatch(Job::Profiles, deadline)? {
            Answer::Profiles(listed) => listed,
            Answer::Done(other) => other.map(|()| Vec::new()),
        }
    }

    pub fn shutdown(&mut self) -> Result<(), PluginError> {
        if self.failure.is_some() || self.jobs.is_none() {
            return Ok(());
        }
        let outcome = self.settle(Job::Shutdown, SHUTDOWN_DEADLINE);
        // The thread stops serving once it has answered a shutdown, so a second
        // one would only find a closed channel.
        self.jobs = None;
        outcome
    }

    pub fn drain_requests(&mut self) -> Vec<PluginRequest> {
        std::mem::take(&mut self.pending)
    }

    fn settle(&mut self, job: Job, deadline: std::time::Duration) -> Result<(), PluginError> {
        match self.dispatch(job, deadline)? {
            Answer::Done(outcome) => outcome,
            Answer::Profiles(listed) => listed.map(|_| ()),
        }
    }

    fn dispatch(&mut self, job: Job, deadline: std::time::Duration) -> Result<Answer, PluginError> {
        if let Some(reason) = &self.failure {
            return Err(PluginError::Retired(reason.clone()));
        }
        let sent = match &self.jobs {
            Some(jobs) => jobs.send(job).is_ok(),
            None => false,
        };
        if !sent {
            let reason = format!("{} stopped answering", self.id);
            self.retire(reason.clone());
            return Err(PluginError::Retired(reason));
        }

        match self.replies.recv_timeout(deadline) {
            Ok(reply) => {
                self.pending.extend(reply.requests);
                self.failure = reply.failure;
                Ok(reply.answer)
            }
            // The call is still running on a thread we have stopped listening
            // to, so a later job would interleave with it; the worker is done.
            // The thread is abandoned on purpose: a compute-bound guest hits its
            // epoch deadline, traps, and lets the thread exit on its own, and
            // one parked in a host call cannot be killed at all.
            Err(_) => {
                let reason = format!("{} did not answer within {deadline:?}", self.id);
                self.retire(reason.clone());
                Err(PluginError::TimedOut(reason))
            }
        }
    }

    /// Latched exactly like a trap, so `retire_failed` demotes a worker that
    /// missed its deadline the same way it demotes one that crashed.
    fn retire(&mut self, reason: String) {
        self.failure = Some(reason);
        self.jobs = None;
    }
}

/// The whole of the guest's working life runs here, one job at a time.
fn serve(
    plugin: &mut LoadedPlugin,
    jobs: &std::sync::mpsc::Receiver<Job>,
    replies: &std::sync::mpsc::Sender<Reply>,
) {
    while let Ok(job) = jobs.recv() {
        let last = matches!(job, Job::Shutdown);
        let answer = match job {
            Job::Command(id) => Answer::Done(plugin.run_command(&id)),
            Job::Event(event) => Answer::Done(plugin.on_event(event)),
            Job::Profiles => Answer::Profiles(plugin.list_profiles()),
            Job::Shutdown => Answer::Done(plugin.shutdown()),
        };
        let reply = Reply {
            answer,
            requests: plugin.drain_requests(),
            failure: plugin.failure().map(str::to_string),
        };
        if replies.send(reply).is_err() || last {
            break;
        }
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

    fn instantiate(
        host: &std::sync::Arc<PluginHost>,
        id: &str,
        path: &Path,
        settings: &PluginSettings,
    ) -> Slot {
        let consented = consented_capabilities(settings);
        let values: HashMap<String, String> = settings
            .settings
            .iter()
            .map(|(key, value)| (key.clone(), value.clone()))
            .collect();
        match PluginWorker::load(host, id, path, values, consented) {
            Ok(plugin) => Slot::ready(plugin),
            Err(reason) => Slot::Retired(reason),
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

    /// Validates the file first: an unreadable or wrong-ABI component is
    /// rejected before anything lands in the plugin directory.
    pub fn preview(&self, source: &Path) -> Result<PluginInfo, String> {
        PluginWorker::inspect(&self.host, source).map(|(info, _)| info)
    }

    pub fn install(&mut self, source: &Path) -> Result<String, String> {
        let (info, _) = PluginWorker::inspect(&self.host, source)?;
        let dir = super::host::dir_name(&info.name)
            .ok_or_else(|| format!("{} is not a usable plugin name", info.name))?;

        let target = self.host.root().join(&dir);
        std::fs::create_dir_all(&target).map_err(|err| err.to_string())?;
        std::fs::copy(source, target.join(COMPONENT_FILE)).map_err(|err| err.to_string())?;

        self.load_all();
        Ok(info.name)
    }

    pub fn uninstall(&mut self, id: &str) -> Result<(), String> {
        let Some(entry) = self.entry(id) else {
            return Err(format!("no plugin named {id}"));
        };
        let dir = entry
            .path
            .parent()
            .ok_or_else(|| format!("{id} has no install directory"))?
            .to_path_buf();

        if !dir.starts_with(self.host.root()) {
            return Err(format!("{id} lives outside the plugin directory"));
        }
        std::fs::remove_dir_all(&dir).map_err(|err| err.to_string())?;
        self.settings.remove(id);
        self.load_all();
        Ok(())
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

    pub fn get_mut(&mut self, id: &str) -> Option<&mut PluginWorker> {
        match &mut self.entry_mut(id)?.slot {
            Slot::Ready(plugin, _) if plugin.failure().is_none() => Some(plugin),
            _ => None,
        }
    }

    pub fn ready_mut(&mut self) -> impl Iterator<Item = (&str, &mut PluginWorker)> {
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

/// Enumeration gets an instance of its own, dropped once it answers, so it
/// never contends with the one serving the UI. Deliberately unbounded: the
/// caller decides how long it is willing to wait.
pub fn fetch_profiles_blocking(
    host: &PluginHost,
    source: &ProfileSource,
) -> Result<Vec<PluginProfile>, String> {
    let consented = source.consented.clone();
    let policy = |info: &PluginInfo| grant_with_consent(info, &consented);
    host.fetch_profiles(&source.id, &source.path, source.config.clone(), &policy)
        .map_err(|err| err.to_string())
}

/// The registry's own instance is busy serving the UI, so enumeration gets a
/// worker of its own; the worker's deadline is the only thing that bounds a
/// source that never answers.
pub fn fetch_profiles_with_deadline(
    host: &std::sync::Arc<PluginHost>,
    source: &ProfileSource,
    deadline: std::time::Duration,
) -> Option<Vec<PluginProfile>> {
    let id = &source.id;
    let mut worker = match PluginWorker::load(
        host,
        id,
        &source.path,
        source.config.clone(),
        source.consented.clone(),
    ) {
        Ok(worker) => worker,
        Err(reason) => {
            eprintln!("plugin {id} failed to start for profile enumeration: {reason}");
            return None;
        }
    };
    worker.set_profile_deadline(deadline);

    let listed = worker.list_profiles();
    let _ = worker.shutdown();
    match listed {
        Ok(profiles) => Some(profiles),
        Err(reason) => {
            eprintln!("plugin {id} failed to list profiles: {reason}");
            None
        }
    }
}

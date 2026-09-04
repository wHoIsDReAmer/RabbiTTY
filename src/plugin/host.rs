use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};
use std::time::Duration;

use wasmtime::component::{Component, Linker, ResourceTable};
use wasmtime::{Config, Engine, Store, StoreLimitsBuilder};
use wasmtime_wasi::{DirPerms, FilePerms, WasiCtx, WasiCtxBuilder};

use super::policy::CapabilityPolicy;
use super::state::{PluginRequest, PluginState};
use super::{Capability, Contributions, Event, Plugin, PluginInfo, PluginProfile, SettingField};

/// Kept in step with the package line in `wit/world.wit`; a test enforces it.
pub const PLUGIN_ABI_VERSION: &str = "0.4.0";
const ABI_PACKAGE: &str = "rabbitty:plugin/";

const CALL_FUEL: u64 = 10_000_000;
const MAX_MEMORY: usize = 64 * 1024 * 1024;
const MAX_TABLE_ELEMENTS: usize = 100_000;
const MAX_INSTANCES: usize = 64;

/// How often the engine's epoch counter advances. Every budget below is
/// expressed in these ticks, so it is also the resolution of all of them.
const EPOCH_TICK_MS: u64 = 10;

/// What a guest call is allowed to take on the wall clock. Two mechanisms hold
/// the line together: the epoch deadline traps a guest spinning inside wasm,
/// and the caller's worker gives up on one parked inside a host or WASI call,
/// where no epoch check is ever reached.
pub(super) const START_DEADLINE: Duration = Duration::from_secs(2);
pub(super) const EVENT_DEADLINE: Duration = Duration::from_millis(250);
pub(super) const COMMAND_DEADLINE: Duration = Duration::from_secs(2);
pub const PROFILE_DEADLINE: Duration = Duration::from_secs(5);
pub(super) const SHUTDOWN_DEADLINE: Duration = Duration::from_millis(500);

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum PluginError {
    Trapped(String),
    Reported(String),
    Retired(String),
    TimedOut(String),
}

impl std::fmt::Display for PluginError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Trapped(reason) => write!(f, "trapped: {reason}"),
            Self::Reported(reason) => write!(f, "{reason}"),
            Self::Retired(reason) => write!(f, "retired earlier: {reason}"),
            Self::TimedOut(reason) => write!(f, "timed out: {reason}"),
        }
    }
}

pub struct PluginHost {
    engine: Engine,
    linker: Linker<PluginState>,
    root: PathBuf,
    /// Compiling a component is far more expensive than instantiating one, and
    /// profile enumeration instantiates on every refresh.
    components: std::sync::RwLock<HashMap<PathBuf, Component>>,
    /// Cleared on drop so the epoch ticker stops with the host that owns it.
    ticking: Arc<AtomicBool>,
}

impl PluginHost {
    pub fn new() -> wasmtime::Result<Self> {
        Self::with_root(default_root()?)
    }

    pub fn with_root(root: PathBuf) -> wasmtime::Result<Self> {
        let mut config = Config::new();
        config.wasm_component_model(true);
        config.consume_fuel(true);
        // Fuel counts instructions, so a guest that spins for a millisecond and
        // one that spins forever look alike until the counter drains. Only the
        // epoch deadline turns a wall-clock budget into a trap.
        config.epoch_interruption(true);

        // The mach port handler thread aborts on any interrupted receive, and every
        // PTY registers a SIGCHLD handler. Signals have no such thread.
        if cfg!(target_os = "macos") {
            config.macos_use_mach_ports(false);
        }
        let engine = Engine::new(&config)?;

        let mut linker = Linker::new(&engine);
        wasmtime_wasi::p2::add_to_linker_sync(&mut linker)?;
        Plugin::add_to_linker::<_, wasmtime::component::HasSelf<_>>(&mut linker, |state| state)?;

        let ticking = Arc::new(AtomicBool::new(true));
        spawn_ticker(engine.clone(), Arc::clone(&ticking))?;

        Ok(Self {
            engine,
            linker,
            root,
            components: std::sync::RwLock::new(HashMap::new()),
            ticking,
        })
    }

    fn component(&self, path: &Path) -> wasmtime::Result<Component> {
        if let Ok(cache) = self.components.read()
            && let Some(found) = cache.get(path)
        {
            return Ok(found.clone());
        }
        let compiled = Component::from_file(&self.engine, path)?;
        self.check_abi(&compiled)?;
        if let Ok(mut cache) = self.components.write() {
            cache.insert(path.to_path_buf(), compiled.clone());
        }
        Ok(compiled)
    }

    /// Dropped when the plugin directory is rescanned, so a replaced `.wasm`
    /// is picked up instead of served from the cache.
    pub fn forget_components(&self) {
        if let Ok(mut cache) = self.components.write() {
            cache.clear();
        }
    }

    /// Compiling has no guest code in it, so a caller that puts a deadline on
    /// starting a plugin gets the compile — the slowest part, and the only part
    /// the plugin cannot stretch — out of the way first. Also where a
    /// wrong-ABI or unreadable component is rejected.
    pub fn precompile(&self, path: &Path) -> wasmtime::Result<()> {
        self.component(path)?;
        Ok(())
    }

    pub fn load(
        &self,
        id: &str,
        path: &Path,
        config: HashMap<String, String>,
        policy: CapabilityPolicy<'_>,
    ) -> wasmtime::Result<LoadedPlugin> {
        let component = self.component(path)?;
        let info = self.read_manifest(&component)?;
        let granted = policy(&info);

        self.start(id, &component, info, granted, config)
    }

    pub fn inspect(&self, path: &Path) -> wasmtime::Result<(PluginInfo, Vec<SettingField>)> {
        let component = self.component(path)?;
        let info = self.read_manifest(&component)?;
        let probe = self.start("", &component, info.clone(), Vec::new(), HashMap::new())?;
        let fields = probe.contributions().settings.clone();
        Ok((info, fields))
    }

    pub fn fetch_profiles(
        &self,
        id: &str,
        path: &Path,
        config: HashMap<String, String>,
        policy: CapabilityPolicy<'_>,
    ) -> wasmtime::Result<Vec<PluginProfile>> {
        let mut plugin = self.load(id, path, config, policy)?;
        let profiles = plugin
            .list_profiles()
            .map_err(|err| wasmtime::Error::msg(err.to_string()))?;
        let _ = plugin.shutdown();
        Ok(profiles)
    }

    /// The guest's import names carry the WIT package version it was built
    /// against, so a mismatch is reported precisely instead of surfacing as a
    /// wasmtime type-checking error.
    fn check_abi(&self, component: &Component) -> wasmtime::Result<()> {
        let ty = component.component_type();
        let found = ty
            .imports(&self.engine)
            .find_map(|(name, _)| abi_version_of(name));

        abi_verdict(found).map_err(wasmtime::Error::msg)
    }

    fn read_manifest(&self, component: &Component) -> wasmtime::Result<PluginInfo> {
        let mut store = self.store(WasiCtxBuilder::new().build(), Vec::new(), HashMap::new())?;
        let bindings = Plugin::instantiate(&mut store, component, &self.linker)?;
        // Re-armed after instantiation so the budget covers the call rather
        // than the host work that set it up.
        arm(&mut store, START_DEADLINE)?;
        bindings.call_manifest(&mut store)
    }

    fn start(
        &self,
        id: &str,
        component: &Component,
        info: PluginInfo,
        mut granted: Vec<Capability>,
        config: HashMap<String, String>,
    ) -> wasmtime::Result<LoadedPlugin> {
        let mut builder = WasiCtxBuilder::new();

        if granted.contains(&Capability::Network) {
            builder.inherit_network().allow_ip_name_lookup(true);
        }

        if granted.contains(&Capability::Filesystem) {
            match self.data_dir(id) {
                Some(dir) => {
                    std::fs::create_dir_all(&dir)?;
                    builder.preopened_dir(&dir, ".", DirPerms::all(), FilePerms::all())?;
                }
                None => granted.retain(|cap| *cap != Capability::Filesystem),
            }
        }

        let mut store = self.store(builder.build(), granted, config)?;
        let bindings = Plugin::instantiate(&mut store, component, &self.linker)?;
        arm(&mut store, START_DEADLINE)?;
        bindings
            .call_init(&mut store)?
            .map_err(wasmtime::Error::msg)?;
        arm(&mut store, START_DEADLINE)?;
        let contributions = bindings
            .call_contributions(&mut store)?
            .map_err(wasmtime::Error::msg)?;

        Ok(LoadedPlugin {
            store,
            bindings,
            info,
            contributions,
            failure: None,
        })
    }

    fn store(
        &self,
        wasi: WasiCtx,
        granted: Vec<Capability>,
        config: HashMap<String, String>,
    ) -> wasmtime::Result<Store<PluginState>> {
        let limits = StoreLimitsBuilder::new()
            .memory_size(MAX_MEMORY)
            .table_elements(MAX_TABLE_ELEMENTS)
            .instances(MAX_INSTANCES)
            .build();
        let mut store = Store::new(
            &self.engine,
            PluginState {
                limits,
                wasi,
                table: ResourceTable::new(),
                granted,
                config,
                requests: Vec::new(),
            },
        );
        store.limiter(|state| &mut state.limits);
        arm(&mut store, START_DEADLINE)?;
        Ok(store)
    }

    fn data_dir(&self, id: &str) -> Option<PathBuf> {
        Some(self.root.join(dir_name(id)?).join("data"))
    }

    pub fn root(&self) -> &Path {
        &self.root
    }
}

/// One thread advances the epoch for every store the engine owns, so a host
/// costs one ticker rather than one per plugin. It is detached on purpose:
/// joining it would stall each host drop for up to a tick.
fn spawn_ticker(engine: Engine, ticking: Arc<AtomicBool>) -> wasmtime::Result<()> {
    std::thread::Builder::new()
        .name("plugin-epoch".to_string())
        .spawn(move || {
            while ticking.load(Ordering::Relaxed) {
                std::thread::sleep(Duration::from_millis(EPOCH_TICK_MS));
                engine.increment_epoch();
            }
        })?;
    Ok(())
}

impl Drop for PluginHost {
    fn drop(&mut self) {
        self.ticking.store(false, Ordering::Relaxed);
    }
}

pub(super) fn abi_verdict(found: Option<&str>) -> Result<(), String> {
    match found {
        Some(version) if version == PLUGIN_ABI_VERSION => Ok(()),
        Some(version) => Err(format!(
            "built for plugin API {version}, but this build provides {PLUGIN_ABI_VERSION}"
        )),
        None => Err("not a Rabbitty plugin: it imports no rabbitty:plugin interface".to_string()),
    }
}

pub(super) fn abi_version_of(interface: &str) -> Option<&str> {
    interface
        .strip_prefix(ABI_PACKAGE)?
        .split_once('@')
        .map(|(_, version)| version)
}

/// Components are binaries, so they live under the data directory rather than
/// beside `config.toml`. Only Linux differs — on macOS and Windows both paths
/// resolve to the same directory.
fn default_root() -> wasmtime::Result<PathBuf> {
    dirs::data_dir()
        .map(|dir| dir.join("rabbitty").join("plugins"))
        .ok_or_else(|| wasmtime::Error::msg("no data directory"))
}

pub(super) fn dir_name(plugin_name: &str) -> Option<String> {
    let mapped: String = plugin_name
        .chars()
        .map(|c| {
            if c.is_ascii_alphanumeric() || c == '-' || c == '_' {
                c.to_ascii_lowercase()
            } else {
                '_'
            }
        })
        .collect();
    let trimmed = mapped.trim_matches('_');
    (!trimmed.is_empty()).then(|| trimmed.to_string())
}

/// Both bounds are re-armed for every call: fuel stops a guest that computes
/// too much, the epoch deadline one that computes for too long.
fn arm(store: &mut Store<PluginState>, deadline: Duration) -> wasmtime::Result<()> {
    store.set_fuel(CALL_FUEL)?;
    store.set_epoch_deadline(epoch_ticks(deadline));
    Ok(())
}

/// A budget shorter than a tick would expire before the ticker ever ran, so it
/// is rounded up to one.
fn epoch_ticks(deadline: Duration) -> u64 {
    (deadline.as_millis() as u64).div_ceil(EPOCH_TICK_MS).max(1)
}

pub struct LoadedPlugin {
    store: Store<PluginState>,
    bindings: Plugin,
    info: PluginInfo,
    contributions: Contributions,
    failure: Option<String>,
}

impl LoadedPlugin {
    pub fn info(&self) -> &PluginInfo {
        &self.info
    }

    pub fn contributions(&self) -> &Contributions {
        &self.contributions
    }

    pub fn granted(&self) -> &[Capability] {
        &self.store.data().granted
    }

    pub fn failure(&self) -> Option<&str> {
        self.failure.as_deref()
    }

    pub fn shutdown(&mut self) -> Result<(), PluginError> {
        if self.failure.is_some() {
            return Ok(());
        }
        self.settle(SHUTDOWN_DEADLINE, |bindings, store| {
            bindings.call_shutdown(store)
        })
    }

    pub fn run_command(&mut self, id: &str) -> Result<(), PluginError> {
        self.guard()?;
        let id = id.to_string();
        self.settle(COMMAND_DEADLINE, move |bindings, store| {
            bindings.call_run_command(store, &id)
        })
    }

    pub fn on_event(&mut self, event: Event) -> Result<(), PluginError> {
        self.guard()?;
        self.settle(EVENT_DEADLINE, move |bindings, store| {
            bindings.call_on_event(store, &event)
        })
    }

    fn guard(&self) -> Result<(), PluginError> {
        match &self.failure {
            Some(reason) => Err(PluginError::Retired(reason.clone())),
            None => Ok(()),
        }
    }

    fn settle<F>(&mut self, deadline: Duration, call: F) -> Result<(), PluginError>
    where
        F: FnOnce(&Plugin, &mut Store<PluginState>) -> wasmtime::Result<Result<(), String>>,
    {
        self.settle_with(deadline, call)
    }

    fn settle_with<T, F>(&mut self, deadline: Duration, call: F) -> Result<T, PluginError>
    where
        F: FnOnce(&Plugin, &mut Store<PluginState>) -> wasmtime::Result<Result<T, String>>,
    {
        if let Err(err) = arm(&mut self.store, deadline) {
            return Err(PluginError::Trapped(err.to_string()));
        }
        match call(&self.bindings, &mut self.store) {
            Err(trap) => {
                let reason = trap.to_string();
                self.failure = Some(reason.clone());
                Err(PluginError::Trapped(reason))
            }
            Ok(Err(reported)) => Err(PluginError::Reported(reported)),
            Ok(Ok(value)) => Ok(value),
        }
    }

    pub fn list_profiles(&mut self) -> Result<Vec<PluginProfile>, PluginError> {
        self.guard()?;
        self.settle_with(PROFILE_DEADLINE, move |bindings, store| {
            bindings.call_list_profiles(store)
        })
    }

    pub fn drain_requests(&mut self) -> Vec<PluginRequest> {
        std::mem::take(&mut self.store.data_mut().requests)
    }
}

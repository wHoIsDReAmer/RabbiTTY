use std::collections::HashMap;
use std::path::{Path, PathBuf};

use wasmtime::component::{Component, Linker, ResourceTable};
use wasmtime::{Config, Engine, Store};
use wasmtime_wasi::{DirPerms, FilePerms, WasiCtx, WasiCtxBuilder};

use super::policy::CapabilityPolicy;
use super::state::{PluginRequest, PluginState};
use super::{Capability, Contributions, Event, Plugin, PluginInfo, SettingField};

const CALL_FUEL: u64 = 10_000_000;

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum PluginError {
    Trapped(String),
    Reported(String),
    Retired(String),
}

impl std::fmt::Display for PluginError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Trapped(reason) => write!(f, "trapped: {reason}"),
            Self::Reported(reason) => write!(f, "{reason}"),
            Self::Retired(reason) => write!(f, "retired earlier: {reason}"),
        }
    }
}

pub struct PluginHost {
    engine: Engine,
    linker: Linker<PluginState>,
    root: PathBuf,
}

impl PluginHost {
    pub fn new() -> wasmtime::Result<Self> {
        Self::with_root(default_root()?)
    }

    pub fn with_root(root: PathBuf) -> wasmtime::Result<Self> {
        let mut config = Config::new();
        config.wasm_component_model(true);
        config.consume_fuel(true);
        let engine = Engine::new(&config)?;

        let mut linker = Linker::new(&engine);
        wasmtime_wasi::p2::add_to_linker_sync(&mut linker)?;
        Plugin::add_to_linker::<_, wasmtime::component::HasSelf<_>>(&mut linker, |state| state)?;

        Ok(Self {
            engine,
            linker,
            root,
        })
    }

    pub fn load(
        &self,
        id: &str,
        path: &Path,
        config: HashMap<String, String>,
        policy: CapabilityPolicy<'_>,
    ) -> wasmtime::Result<LoadedPlugin> {
        let component = Component::from_file(&self.engine, path)?;
        let info = self.read_manifest(&component)?;
        let granted = policy(&info);

        self.start(id, &component, info, granted, config)
    }

    pub fn inspect(&self, path: &Path) -> wasmtime::Result<(PluginInfo, Vec<SettingField>)> {
        let component = Component::from_file(&self.engine, path)?;
        let info = self.read_manifest(&component)?;
        let probe = self.start("", &component, info.clone(), Vec::new(), HashMap::new())?;
        let fields = probe.contributions().settings.clone();
        Ok((info, fields))
    }

    fn read_manifest(&self, component: &Component) -> wasmtime::Result<PluginInfo> {
        let mut store = self.store(WasiCtxBuilder::new().build(), Vec::new(), HashMap::new())?;
        let bindings = Plugin::instantiate(&mut store, component, &self.linker)?;
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

        bindings
            .call_init(&mut store)?
            .map_err(wasmtime::Error::msg)?;
        refuel(&mut store)?;
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
        let mut store = Store::new(
            &self.engine,
            PluginState {
                wasi,
                table: ResourceTable::new(),
                granted,
                config,
                requests: Vec::new(),
            },
        );
        store.set_fuel(CALL_FUEL)?;
        Ok(store)
    }

    fn data_dir(&self, id: &str) -> Option<PathBuf> {
        Some(self.root.join(dir_name(id)?).join("data"))
    }

    pub fn root(&self) -> &Path {
        &self.root
    }
}

fn default_root() -> wasmtime::Result<PathBuf> {
    dirs::config_dir()
        .map(|dir| dir.join("rabbitty").join("plugins"))
        .ok_or_else(|| wasmtime::Error::msg("no config directory"))
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

fn refuel(store: &mut Store<PluginState>) -> wasmtime::Result<()> {
    store.set_fuel(CALL_FUEL)?;
    Ok(())
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
        self.settle(|bindings, store| bindings.call_shutdown(store))
    }

    pub fn run_command(&mut self, id: &str) -> Result<(), PluginError> {
        self.guard()?;
        let id = id.to_string();
        self.settle(move |bindings, store| bindings.call_run_command(store, &id))
    }

    pub fn on_event(&mut self, event: Event) -> Result<(), PluginError> {
        self.guard()?;
        self.settle(move |bindings, store| bindings.call_on_event(store, &event))
    }

    fn guard(&self) -> Result<(), PluginError> {
        match &self.failure {
            Some(reason) => Err(PluginError::Retired(reason.clone())),
            None => Ok(()),
        }
    }

    fn settle<F>(&mut self, call: F) -> Result<(), PluginError>
    where
        F: FnOnce(&Plugin, &mut Store<PluginState>) -> wasmtime::Result<Result<(), String>>,
    {
        if let Err(err) = refuel(&mut self.store) {
            return Err(PluginError::Trapped(err.to_string()));
        }
        match call(&self.bindings, &mut self.store) {
            Err(trap) => {
                let reason = trap.to_string();
                self.failure = Some(reason.clone());
                Err(PluginError::Trapped(reason))
            }
            Ok(Err(reported)) => Err(PluginError::Reported(reported)),
            Ok(Ok(())) => Ok(()),
        }
    }

    pub fn drain_requests(&mut self) -> Vec<PluginRequest> {
        std::mem::take(&mut self.store.data_mut().requests)
    }
}

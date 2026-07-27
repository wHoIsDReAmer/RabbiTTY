use std::collections::HashMap;
use std::path::{Path, PathBuf};

use wasmtime::component::{Component, Linker, ResourceTable};
use wasmtime::{Config, Engine, Store};
use wasmtime_wasi::{DirPerms, FilePerms, WasiCtx, WasiCtxBuilder};

use super::policy::CapabilityPolicy;
use super::state::{PluginRequest, PluginState};
use super::{Capability, Contributions, Event, Plugin, PluginInfo};

const CALL_FUEL: u64 = 10_000_000;

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

        bindings.call_init(&mut store)?;
        refuel(&mut store)?;
        let contributions = bindings.call_contributions(&mut store)?;

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

    pub fn run_command(&mut self, id: &str) -> wasmtime::Result<()> {
        self.guard()?;
        refuel(&mut self.store)?;
        let result = self.bindings.call_run_command(&mut self.store, id);
        self.record(result)
    }

    pub fn on_event(&mut self, event: Event) -> wasmtime::Result<()> {
        self.guard()?;
        refuel(&mut self.store)?;
        let result = self.bindings.call_on_event(&mut self.store, &event);
        self.record(result)
    }

    fn guard(&self) -> wasmtime::Result<()> {
        match &self.failure {
            Some(reason) => Err(wasmtime::Error::msg(format!("plugin retired: {reason}"))),
            None => Ok(()),
        }
    }

    fn record(&mut self, result: wasmtime::Result<()>) -> wasmtime::Result<()> {
        if let Err(err) = &result {
            self.failure = Some(err.to_string());
        }
        result
    }

    pub fn drain_requests(&mut self) -> Vec<PluginRequest> {
        std::mem::take(&mut self.store.data_mut().requests)
    }
}

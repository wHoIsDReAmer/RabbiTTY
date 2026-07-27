use std::collections::HashMap;
use std::path::{Path, PathBuf};

use wasmtime::component::{Component, Linker, ResourceTable};
use wasmtime::{Config, Engine, Store};
use wasmtime_wasi::{DirPerms, FilePerms, WasiCtx, WasiCtxBuilder};

use super::policy::CapabilityPolicy;
use super::state::{HostRequest, PluginState};
use super::{Capability, Contributions, Event, Plugin, PluginInfo};

const CALL_FUEL: u64 = 10_000_000;

pub struct PluginHost {
    engine: Engine,
    linker: Linker<PluginState>,
    data_root: PathBuf,
}

impl PluginHost {
    pub fn new() -> wasmtime::Result<Self> {
        Self::with_data_root(default_data_root()?)
    }

    pub fn with_data_root(data_root: PathBuf) -> wasmtime::Result<Self> {
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
            data_root,
        })
    }

    pub fn load(
        &self,
        path: &Path,
        config: HashMap<String, String>,
        policy: CapabilityPolicy,
    ) -> wasmtime::Result<LoadedPlugin> {
        let component = Component::from_file(&self.engine, path)?;
        let info = self.read_manifest(&component)?;
        let granted = policy(&info);
        self.start(&component, info, granted, config)
    }

    /// The WASI context is fixed when the store is created, so the manifest is
    /// read from a throwaway instance that is granted nothing at all.
    fn read_manifest(&self, component: &Component) -> wasmtime::Result<PluginInfo> {
        let mut store = self.store(WasiCtxBuilder::new().build(), Vec::new(), HashMap::new())?;
        let bindings = Plugin::instantiate(&mut store, component, &self.linker)?;
        bindings.call_manifest(&mut store)
    }

    fn start(
        &self,
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
            match self.data_dir(&info.name) {
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

    fn data_dir(&self, plugin_name: &str) -> Option<PathBuf> {
        Some(self.data_root.join(dir_name(plugin_name)?))
    }
}

fn default_data_root() -> wasmtime::Result<PathBuf> {
    dirs::config_dir()
        .map(|dir| dir.join("rabbitty").join("plugins"))
        .ok_or_else(|| wasmtime::Error::msg("no config directory"))
}

/// Collapses a plugin name to one path segment. Separators and `.` cannot
/// survive, so the result can never escape the data root.
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

    pub fn run_command(&mut self, id: &str) -> wasmtime::Result<()> {
        refuel(&mut self.store)?;
        self.bindings.call_run_command(&mut self.store, id)
    }

    pub fn on_event(&mut self, event: Event) -> wasmtime::Result<()> {
        refuel(&mut self.store)?;
        self.bindings.call_on_event(&mut self.store, &event)
    }

    pub fn drain_requests(&mut self) -> Vec<HostRequest> {
        std::mem::take(&mut self.store.data_mut().requests)
    }
}

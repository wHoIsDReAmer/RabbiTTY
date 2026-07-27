use std::collections::HashMap;
use std::path::Path;

use wasmtime::component::{Component, Linker, ResourceTable};
use wasmtime::{Config, Engine, Store};
use wasmtime_wasi::WasiCtxBuilder;

use super::policy::CapabilityPolicy;
use super::state::{HostRequest, PluginState};
use super::{Capability, Contributions, Event, Plugin, PluginInfo};

const CALL_FUEL: u64 = 10_000_000;

pub struct PluginHost {
    engine: Engine,
    linker: Linker<PluginState>,
}

impl PluginHost {
    pub fn new() -> wasmtime::Result<Self> {
        let mut config = Config::new();
        config.wasm_component_model(true);
        config.consume_fuel(true);
        let engine = Engine::new(&config)?;

        let mut linker = Linker::new(&engine);
        wasmtime_wasi::p2::add_to_linker_sync(&mut linker)?;
        Plugin::add_to_linker::<_, wasmtime::component::HasSelf<_>>(&mut linker, |state| state)?;

        Ok(Self { engine, linker })
    }

    pub fn load(
        &self,
        path: &Path,
        config: HashMap<String, String>,
        policy: CapabilityPolicy,
    ) -> wasmtime::Result<LoadedPlugin> {
        let component = Component::from_file(&self.engine, path)?;
        self.instantiate(component, config, policy)
    }

    fn instantiate(
        &self,
        component: Component,
        config: HashMap<String, String>,
        policy: CapabilityPolicy,
    ) -> wasmtime::Result<LoadedPlugin> {
        let state = PluginState {
            wasi: WasiCtxBuilder::new().build(),
            table: ResourceTable::new(),
            granted: Vec::new(),
            config,
            requests: Vec::new(),
        };
        let mut store = Store::new(&self.engine, state);
        store.set_fuel(CALL_FUEL)?;

        let bindings = Plugin::instantiate(&mut store, &component, &self.linker)?;

        let info = bindings.call_manifest(&mut store)?;
        store.data_mut().granted = policy(&info);

        refuel(&mut store)?;
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

use std::collections::HashMap;

use wasmtime::StoreLimits;
use wasmtime::component::ResourceTable;
use wasmtime_wasi::{WasiCtx, WasiCtxView, WasiView};

use super::Capability;

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum PluginRequest {
    WritePty { pane: u64, data: Vec<u8> },
    Notify { message: String },
    OpenUrl { url: String },
    SetStatus { id: String, text: String },
}

pub(super) struct PluginState {
    pub(super) limits: StoreLimits,
    pub(super) wasi: WasiCtx,
    pub(super) table: ResourceTable,
    pub(super) granted: Vec<Capability>,
    pub(super) config: HashMap<String, String>,
    pub(super) requests: Vec<PluginRequest>,
}

impl PluginState {
    fn allows(&self, cap: Capability) -> bool {
        self.granted.contains(&cap)
    }
}

impl WasiView for PluginState {
    fn ctx(&mut self) -> WasiCtxView<'_> {
        WasiCtxView {
            ctx: &mut self.wasi,
            table: &mut self.table,
        }
    }
}

impl super::rabbitty::plugin::types::Host for PluginState {}

impl super::rabbitty::plugin::host::Host for PluginState {
    fn write_pty(&mut self, pane: u64, data: Vec<u8>) {
        if self.allows(Capability::WritePty) {
            self.requests.push(PluginRequest::WritePty { pane, data });
        }
    }

    fn notify(&mut self, message: String) {
        if self.allows(Capability::Notify) {
            self.requests.push(PluginRequest::Notify { message });
        }
    }

    fn open_url(&mut self, url: String) {
        if self.allows(Capability::OpenUrl) {
            self.requests.push(PluginRequest::OpenUrl { url });
        }
    }

    fn set_status(&mut self, id: String, text: String) {
        self.requests.push(PluginRequest::SetStatus { id, text });
    }

    fn read_config(&mut self, key: String) -> Option<String> {
        if !self.allows(Capability::ReadConfig) {
            return None;
        }
        self.config.get(&key).cloned()
    }
}

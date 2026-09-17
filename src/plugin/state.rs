use std::collections::HashMap;

use wasmtime::StoreLimits;
use wasmtime::component::ResourceTable;
use wasmtime_wasi::{WasiCtx, WasiCtxView, WasiView};

use super::{Action, Capability, ConnectTarget, Query};

pub(super) struct PluginState {
    pub(super) limits: StoreLimits,
    pub(super) wasi: WasiCtx,
    pub(super) table: ResourceTable,
    pub(super) granted: Vec<Capability>,
    pub(super) config: HashMap<String, String>,
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
    fn read_config(&mut self, key: String) -> Option<String> {
        if !self.allows(Capability::ReadConfig) {
            return None;
        }
        self.config.get(&key).cloned()
    }
}

pub(super) fn required_capability(action: &Action) -> Option<Capability> {
    match action {
        Action::WritePty(_) => Some(Capability::WritePty),
        Action::Notify(_) => Some(Capability::Notify),
        Action::OpenUrl(_) => Some(Capability::OpenUrl),
        Action::OpenTab(_) | Action::FocusPane(_) | Action::ClosePane(_) => {
            Some(Capability::Control)
        }
        Action::Connect(request) => Some(match request.target {
            ConnectTarget::Tcp(_) => Capability::Network,
            ConnectTarget::Local(_) => Capability::LocalIpc,
        }),
        Action::Query(Query::Scrollback(_) | Query::Selection(_)) => Some(Capability::ReadScreen),
        Action::SetStatus(_)
        | Action::Schedule(_)
        | Action::CancelTimer(_)
        | Action::Query(Query::Panes)
        | Action::Send(_)
        | Action::Close(_) => None,
    }
}

pub(super) fn permitted(granted: &[Capability], actions: Vec<Action>) -> Vec<Action> {
    actions
        .into_iter()
        .filter(|action| required_capability(action).is_none_or(|cap| granted.contains(&cap)))
        .collect()
}

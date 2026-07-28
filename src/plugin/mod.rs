mod host;
mod lines;
mod matcher;
mod policy;
mod registry;
mod state;
#[cfg(test)]
mod tests;

wasmtime::component::bindgen!({
    path: "wit",
    world: "plugin",
});

pub(crate) use self::rabbitty::plugin::types::{
    Capability, MatchEvent, MenuContext, MenuEvent, MenuItem, OutputPattern, SelectionEvent,
    SettingEvent, SettingField, SettingKind, StatusItem, TitleEvent,
};
pub use host::{PluginError, PluginHost};
pub use lines::LineReader;
pub use policy::{capability_from_name, capability_name, requires_consent};
pub use registry::{PluginRegistry, Status};
pub use state::PluginRequest;

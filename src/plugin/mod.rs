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
    Capability, CwdEvent, MatchEvent, MenuContext, MenuEvent, MenuItem, OutputPattern,
    ProfileTarget, SelectionEvent, SettingEvent, SettingField, SettingKind, StatusItem, TitleEvent,
};
pub use host::{PluginError, PluginHost};
pub use lines::LineReader;
pub use matcher::span_at;
pub use policy::{capability_from_name, capability_name, requires_consent};
pub use registry::{
    ClickablePattern, PluginRegistry, ProfileSource, Status, fetch_profiles_blocking,
};
pub use state::PluginRequest;

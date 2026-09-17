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
    additional_derives: [PartialEq],
});

pub(crate) use self::rabbitty::plugin::types::{
    Capability, ConnectTarget, CwdEvent, IoClosed, IoFrame, MatchEvent, MenuContext, MenuEvent,
    MenuItem, OutputPattern, PaneInfo, ProfileTarget, Query, ScrollbackChunk, ScrollbackRange,
    SelectionEvent, SettingEvent, SettingField, SettingKind, StatusItem, TcpTarget, Timer,
    TitleEvent,
};
pub use host::{PLUGIN_ABI_VERSION, PROFILE_DEADLINE, PluginError, PluginHost};
pub use lines::LineReader;
pub use matcher::span_at;
pub use policy::{
    ALL as ALL_CAPABILITIES, capability_from_name, capability_name, requires_consent,
};
pub use registry::{
    ClickablePattern, PluginRegistry, ProfileSource, Status, fetch_profiles_blocking,
    fetch_profiles_with_deadline,
};

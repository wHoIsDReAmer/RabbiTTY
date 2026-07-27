mod host;
mod policy;
mod registry;
mod state;
#[cfg(test)]
mod tests;

wasmtime::component::bindgen!({
    path: "wit",
    world: "plugin",
});

pub use self::rabbitty::plugin::types::{Capability, Command, CwdEvent, LineEvent, MenuItem};
pub use host::{LoadedPlugin, PluginHost};
pub use policy::{
    CapabilityPolicy, capability_from_name, capability_name, grant_nothing, grant_supported,
    grant_with_consent, requires_consent,
};
pub use registry::{PluginRegistry, Status};
pub use state::PluginRequest;

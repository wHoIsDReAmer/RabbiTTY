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

pub(crate) use self::rabbitty::plugin::types::Capability;
pub use host::PluginHost;
pub use policy::capability_name;
pub use registry::{PluginRegistry, Status};
pub use state::PluginRequest;

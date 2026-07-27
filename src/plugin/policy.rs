use super::{Capability, PluginInfo};

pub type CapabilityPolicy = fn(&PluginInfo) -> Vec<Capability>;

pub fn grant_supported(info: &PluginInfo) -> Vec<Capability> {
    info.capabilities
        .iter()
        .copied()
        .filter(|cap| {
            matches!(
                cap,
                Capability::WritePty | Capability::ReadConfig | Capability::Notify
            )
        })
        .collect()
}

pub fn grant_nothing(_info: &PluginInfo) -> Vec<Capability> {
    Vec::new()
}

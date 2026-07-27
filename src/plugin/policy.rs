use super::{Capability, PluginInfo};

pub type CapabilityPolicy<'a> = &'a dyn Fn(&PluginInfo) -> Vec<Capability>;

const AUTO: [Capability; 3] = [
    Capability::Notify,
    Capability::ReadConfig,
    Capability::Filesystem,
];

const NEEDS_CONSENT: [Capability; 2] = [Capability::WritePty, Capability::Network];

pub fn grant_supported(info: &PluginInfo) -> Vec<Capability> {
    info.capabilities
        .iter()
        .copied()
        .filter(|cap| AUTO.contains(cap))
        .collect()
}

pub fn requires_consent(info: &PluginInfo) -> Vec<Capability> {
    info.capabilities
        .iter()
        .copied()
        .filter(|cap| NEEDS_CONSENT.contains(cap))
        .collect()
}

pub fn grant_with_consent(info: &PluginInfo, consented: &[Capability]) -> Vec<Capability> {
    info.capabilities
        .iter()
        .copied()
        .filter(|cap| {
            AUTO.contains(cap) || (NEEDS_CONSENT.contains(cap) && consented.contains(cap))
        })
        .collect()
}

pub fn grant_nothing(_info: &PluginInfo) -> Vec<Capability> {
    Vec::new()
}

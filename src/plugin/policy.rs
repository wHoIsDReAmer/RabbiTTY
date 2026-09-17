use super::{Capability, PluginInfo};

pub type CapabilityPolicy<'a> = &'a dyn Fn(&PluginInfo) -> Vec<Capability>;

const AUTO: [Capability; 3] = [
    Capability::Notify,
    Capability::ReadConfig,
    Capability::Filesystem,
];

const NEEDS_CONSENT: [Capability; 6] = [
    Capability::WritePty,
    Capability::ReadScreen,
    Capability::Network,
    Capability::LocalIpc,
    Capability::Control,
    Capability::OpenUrl,
];

pub const ALL: [Capability; 9] = [
    Capability::Notify,
    Capability::ReadConfig,
    Capability::Filesystem,
    Capability::WritePty,
    Capability::ReadScreen,
    Capability::Network,
    Capability::LocalIpc,
    Capability::Control,
    Capability::OpenUrl,
];

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

pub fn capability_name(cap: Capability) -> &'static str {
    match cap {
        Capability::WritePty => "write-pty",
        Capability::ReadConfig => "read-config",
        Capability::ReadScreen => "read-screen",
        Capability::Notify => "notify",
        Capability::Network => "network",
        Capability::LocalIpc => "local-ipc",
        Capability::Control => "control",
        Capability::Filesystem => "filesystem",
        Capability::OpenUrl => "open-url",
    }
}

pub fn capability_from_name(name: &str) -> Option<Capability> {
    ALL.iter()
        .copied()
        .find(|cap| capability_name(*cap) == name)
}

use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;

pub type PluginsConfig = BTreeMap<String, PluginSettings>;

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(default)]
pub struct PluginSettings {
    pub enabled: bool,
    pub consented: Vec<String>,
}

impl Default for PluginSettings {
    fn default() -> Self {
        Self {
            enabled: true,
            consented: Vec::new(),
        }
    }
}

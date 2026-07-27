use std::collections::HashMap;
use std::path::{Path, PathBuf};

use super::host::{LoadedPlugin, PluginHost};
use super::policy::CapabilityPolicy;

pub const COMPONENT_FILE: &str = "plugin.wasm";

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Status {
    Ready,
    Disabled,
    Retired(String),
}

enum Slot {
    Ready(Box<LoadedPlugin>),
    Disabled,
    Retired(String),
}

struct Entry {
    id: String,
    path: PathBuf,
    slot: Slot,
}

pub struct PluginRegistry {
    host: PluginHost,
    entries: Vec<Entry>,
}

impl PluginRegistry {
    pub fn new(host: PluginHost) -> Self {
        Self {
            host,
            entries: Vec::new(),
        }
    }

    pub fn root(&self) -> &Path {
        self.host.root()
    }

    pub fn load_all(&mut self, policy: CapabilityPolicy<'_>) {
        self.entries.clear();
        for (id, path) in discover(self.host.root()) {
            let slot = match self.host.load(&id, &path, HashMap::new(), policy) {
                Ok(plugin) => Slot::Ready(Box::new(plugin)),
                Err(err) => Slot::Retired(err.to_string()),
            };
            self.entries.push(Entry { id, path, slot });
        }
    }

    pub fn ids(&self) -> impl Iterator<Item = &str> {
        self.entries.iter().map(|entry| entry.id.as_str())
    }

    pub fn status(&self, id: &str) -> Option<Status> {
        self.entry(id).map(|entry| match &entry.slot {
            Slot::Ready(plugin) => match plugin.failure() {
                Some(reason) => Status::Retired(reason.to_string()),
                None => Status::Ready,
            },
            Slot::Disabled => Status::Disabled,
            Slot::Retired(reason) => Status::Retired(reason.clone()),
        })
    }

    pub fn get_mut(&mut self, id: &str) -> Option<&mut LoadedPlugin> {
        match &mut self.entry_mut(id)?.slot {
            Slot::Ready(plugin) if plugin.failure().is_none() => Some(plugin),
            _ => None,
        }
    }

    pub fn ready_mut(&mut self) -> impl Iterator<Item = (&str, &mut LoadedPlugin)> {
        self.entries
            .iter_mut()
            .filter_map(|entry| match &mut entry.slot {
                Slot::Ready(plugin) if plugin.failure().is_none() => {
                    Some((entry.id.as_str(), plugin.as_mut()))
                }
                _ => None,
            })
    }

    pub fn disable(&mut self, id: &str) -> bool {
        match self.entry_mut(id) {
            Some(entry) => {
                entry.slot = Slot::Disabled;
                true
            }
            None => false,
        }
    }

    pub fn enable(&mut self, id: &str, policy: CapabilityPolicy<'_>) -> Result<(), String> {
        let Some(index) = self.entries.iter().position(|entry| entry.id == id) else {
            return Err(format!("no plugin named {id}"));
        };
        let entry = &self.entries[index];
        let slot = match self
            .host
            .load(&entry.id, &entry.path, HashMap::new(), policy)
        {
            Ok(plugin) => Slot::Ready(Box::new(plugin)),
            Err(err) => {
                let reason = err.to_string();
                self.entries[index].slot = Slot::Retired(reason.clone());
                return Err(reason);
            }
        };
        self.entries[index].slot = slot;
        Ok(())
    }

    pub fn retire_failed(&mut self) -> Vec<(String, String)> {
        let mut retired = Vec::new();
        for entry in &mut self.entries {
            if let Slot::Ready(plugin) = &entry.slot
                && let Some(reason) = plugin.failure()
            {
                let reason = reason.to_string();
                retired.push((entry.id.clone(), reason.clone()));
                entry.slot = Slot::Retired(reason);
            }
        }
        retired
    }

    fn entry(&self, id: &str) -> Option<&Entry> {
        self.entries.iter().find(|entry| entry.id == id)
    }

    fn entry_mut(&mut self, id: &str) -> Option<&mut Entry> {
        self.entries.iter_mut().find(|entry| entry.id == id)
    }
}

fn discover(root: &Path) -> Vec<(String, PathBuf)> {
    let Ok(dir) = std::fs::read_dir(root) else {
        return Vec::new();
    };
    let mut found: Vec<(String, PathBuf)> = dir
        .flatten()
        .filter_map(|entry| {
            let id = entry.file_name().to_str()?.to_string();
            let component = entry.path().join(COMPONENT_FILE);
            component.is_file().then_some((id, component))
        })
        .collect();
    found.sort_by(|a, b| a.0.cmp(&b.0));
    found
}

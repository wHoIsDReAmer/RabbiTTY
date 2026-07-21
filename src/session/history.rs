use crate::gui::tab::Profile;
use serde::{Deserialize, Serialize};
use std::path::PathBuf;
use std::time::{SystemTime, UNIX_EPOCH};

const MAX_ENTRIES: usize = 10;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SessionHistoryEntry {
    pub profile: Profile,
    pub display_name: String,
    pub timestamp: u64,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct SessionHistory {
    pub entries: Vec<SessionHistoryEntry>,
}

impl SessionHistory {
    pub fn load() -> Self {
        let Some(path) = history_path() else {
            return Self::default();
        };
        std::fs::read_to_string(&path)
            .ok()
            .and_then(|s| toml::from_str(&s).ok())
            .unwrap_or_default()
    }

    pub fn save(&self) {
        let Some(path) = history_path() else { return };
        if let Some(parent) = path.parent() {
            let _ = std::fs::create_dir_all(parent);
        }
        if let Ok(s) = toml::to_string_pretty(self) {
            let _ = std::fs::write(&path, s);
        }
    }

    pub fn record(&mut self, profile: Profile, display_name: String) {
        self.entries.retain(|e| e.display_name != display_name);
        self.entries.insert(
            0,
            SessionHistoryEntry {
                profile,
                display_name,
                timestamp: SystemTime::now()
                    .duration_since(UNIX_EPOCH)
                    .unwrap_or_default()
                    .as_secs(),
            },
        );
        self.entries.truncate(MAX_ENTRIES);
        self.save();
    }
}

fn history_path() -> Option<PathBuf> {
    Some(
        dirs::config_dir()?
            .join("rabbitty")
            .join("session_history.toml"),
    )
}

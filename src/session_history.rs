use serde::{Deserialize, Serialize};
use std::path::PathBuf;
use std::time::{SystemTime, UNIX_EPOCH};

const MAX_ENTRIES: usize = 10;

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(tag = "type")]
pub enum SessionKind {
    Default,
    Shell {
        name: String,
        path: String,
    },
    Ssh {
        host: String,
        port: u16,
        user: String,
    },
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SessionHistoryEntry {
    pub kind: SessionKind,
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

    pub fn record(&mut self, kind: SessionKind, display_name: String) {
        self.entries.retain(|e| e.kind != kind);
        self.entries.insert(
            0,
            SessionHistoryEntry {
                kind,
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

impl From<&crate::gui::tab::ShellKind> for SessionKind {
    fn from(shell: &crate::gui::tab::ShellKind) -> Self {
        use crate::gui::tab::ShellKind;
        match shell {
            ShellKind::Default => SessionKind::Default,
            ShellKind::Shell { name, path } => SessionKind::Shell {
                name: name.clone(),
                path: path.clone(),
            },
            ShellKind::Ssh(p) => SessionKind::Ssh {
                host: p.host.clone(),
                port: p.port,
                user: p.user.clone(),
            },
        }
    }
}

impl SessionKind {
    pub fn to_shell_kind(
        &self,
        ssh_profiles: &[crate::config::SshProfile],
    ) -> Option<crate::gui::tab::ShellKind> {
        use crate::gui::tab::ShellKind;
        match self {
            SessionKind::Default => Some(ShellKind::Default),
            SessionKind::Shell { name, path } => Some(ShellKind::Shell {
                name: name.clone(),
                path: path.clone(),
            }),
            SessionKind::Ssh { host, port, user } => ssh_profiles
                .iter()
                .find(|p| p.host == *host && p.port == *port && p.user == *user)
                .map(|p| ShellKind::Ssh(p.clone())),
        }
    }
}

fn history_path() -> Option<PathBuf> {
    Some(
        dirs::config_dir()?
            .join("rabbitty")
            .join("session_history.toml"),
    )
}

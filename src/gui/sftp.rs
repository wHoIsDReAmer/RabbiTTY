//! GUI state for the SFTP drawer attached to an SSH terminal tab.

use crate::ssh::sftp;
use iced::Animation;
use iced::futures::channel::mpsc;

pub const DEFAULT_PATH: &str = ".";
pub const DEFAULT_HEIGHT_RATIO: f32 = 0.45;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TransferState {
    Running,
    Done,
    Cancelled,
    Failed,
}

impl TransferState {
    pub fn is_terminal(self) -> bool {
        self != TransferState::Running
    }
}

#[derive(Debug, Clone)]
pub struct TransferRow {
    pub path: String,
    pub transferred: u64,
    pub total: u64,
    pub state: TransferState,
}

#[derive(Debug)]
pub struct SftpDrawerState {
    pub open: bool,
    pub opening: bool,
    pub loading: bool,
    pub error: Option<String>,
    pub current_path: String,
    pub entries: Vec<sftp::Entry>,
    pub transfers: Vec<TransferRow>,
    pub command_tx: Option<mpsc::UnboundedSender<sftp::Command>>,
    pub height_ratio: f32,
    pub anim: Animation<bool>,
}

impl Default for SftpDrawerState {
    fn default() -> Self {
        Self::new()
    }
}

pub fn parent_path(path: &str) -> Option<String> {
    match path {
        "/" => None,
        "" | "." => Some("..".to_string()),
        _ if path == ".." || path.ends_with("/..") => Some(format!("{path}/..")),
        _ => {
            let trimmed = path.trim_end_matches('/');
            match trimmed.rfind('/') {
                None => Some(".".to_string()),
                Some(0) => Some("/".to_string()),
                Some(i) => Some(trimmed[..i].to_string()),
            }
        }
    }
}

pub fn join_path(base: &str, name: &str) -> String {
    if base.is_empty() {
        name.to_string()
    } else if base == "/" {
        format!("/{name}")
    } else {
        format!("{}/{name}", base.trim_end_matches('/'))
    }
}

impl SftpDrawerState {
    pub fn new() -> Self {
        Self {
            open: false,
            opening: false,
            loading: false,
            error: None,
            current_path: DEFAULT_PATH.to_string(),
            entries: Vec::new(),
            transfers: Vec::new(),
            command_tx: None,
            height_ratio: DEFAULT_HEIGHT_RATIO,
            anim: Animation::new(false)
                .duration(std::time::Duration::from_millis(220))
                .easing(iced::animation::Easing::EaseOutQuint),
        }
    }

    pub fn reset(&mut self) {
        self.open = false;
        self.opening = false;
        self.loading = false;
        self.error = None;
        self.entries.clear();
        self.transfers.clear();
        self.command_tx = None;
    }
}

#[cfg(test)]
mod tests {
    use super::{join_path, parent_path};

    #[test]
    fn parent_handles_common_cases() {
        assert_eq!(parent_path("/"), None);
        assert_eq!(parent_path("."), Some("..".to_string()));
        assert_eq!(parent_path(".."), Some("../..".to_string()));
        assert_eq!(parent_path("../.."), Some("../../..".to_string()));
        assert_eq!(parent_path("/home/me"), Some("/home".to_string()));
        assert_eq!(parent_path("/home"), Some("/".to_string()));
        assert_eq!(parent_path("/home/me/docs/"), Some("/home/me".to_string()));
        assert_eq!(parent_path("foo"), Some(".".to_string()));
        assert_eq!(parent_path("./foo"), Some(".".to_string()));
        assert_eq!(parent_path("../foo"), Some("..".to_string()));
    }

    #[test]
    fn join_handles_common_cases() {
        assert_eq!(join_path("/", "foo"), "/foo");
        assert_eq!(join_path("/home", "me"), "/home/me");
        assert_eq!(join_path("/home/", "me"), "/home/me");
        assert_eq!(join_path("", "foo"), "foo");
    }
}

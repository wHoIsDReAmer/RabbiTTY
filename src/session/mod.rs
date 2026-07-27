pub mod history;
mod transport;

use iced::futures::channel::mpsc;
use std::io::Write;
use std::path::PathBuf;
use std::sync::{Arc, Mutex};

use transport::{LocalPty, SshTransport, Transport};

pub struct LaunchSpec {
    pub program: String,
    pub args: Vec<String>,
    pub env: Vec<(String, String)>,
    pub rows: u16,
    pub cols: u16,
    pub cwd: Option<PathBuf>,
}

/// A terminal session. A thin facade over a [`Transport`] backend (local PTY or
/// SSH); the connection type is chosen at spawn time and hidden behind the trait.
pub struct Session {
    backend: Box<dyn Transport>,
}

#[derive(Debug, Clone)]
pub enum SessionError {
    Spawn(String),
    Io(String),
}

#[derive(Debug, Clone)]
pub enum OutputEvent {
    Data { tab_id: u64, bytes: Vec<u8> },
    Closed { tab_id: u64 },
}

impl Session {
    pub fn spawn(
        spec: LaunchSpec,
        tab_id: u64,
        output_tx: mpsc::UnboundedSender<OutputEvent>,
    ) -> Result<Self, SessionError> {
        Ok(Self {
            backend: Box::new(LocalPty::spawn(spec, tab_id, output_tx)?),
        })
    }

    pub fn spawn_ssh(
        profile: crate::config::SshProfile,
        tab_id: u64,
        rows: u16,
        cols: u16,
        output_tx: mpsc::UnboundedSender<OutputEvent>,
    ) -> Self {
        Self {
            backend: Box::new(SshTransport::spawn(profile, tab_id, rows, cols, output_tx)),
        }
    }

    /// Returns the underlying SSH session handle when this session was spawned
    /// via `spawn_ssh`. Local PTY sessions return `None`.
    pub fn ssh_handle(&self) -> Option<&crate::ssh::SshSessionHandle> {
        self.backend.ssh_handle()
    }

    pub fn send_bytes(&self, bytes: &[u8]) -> Result<(), SessionError> {
        let writer = self.backend.writer();
        let mut guard = writer
            .lock()
            .map_err(|err| SessionError::Io(format!("writer lock failed: {err}")))?;
        guard
            .write_all(bytes)
            .map_err(|err| SessionError::Io(format!("write failed: {err}")))
    }

    pub fn writer(&self) -> Arc<Mutex<Box<dyn Write + Send>>> {
        self.backend.writer()
    }

    pub fn working_directory(&self) -> Option<PathBuf> {
        self.backend.working_directory()
    }

    pub fn resize(&mut self, rows: u16, cols: u16) -> Result<(), SessionError> {
        self.backend.resize(rows, cols)
    }
}

pub(super) fn send_output_event(
    output_tx: &mut mpsc::UnboundedSender<OutputEvent>,
    event: OutputEvent,
) -> bool {
    output_tx.unbounded_send(event).is_ok()
}

pub(super) fn default_working_directory() -> Option<PathBuf> {
    default_working_directory_from_env(
        std::env::var_os("HOME").as_deref(),
        std::env::var_os("USERPROFILE").as_deref(),
        std::env::var_os("HOMEDRIVE").as_deref(),
        std::env::var_os("HOMEPATH").as_deref(),
        dirs::home_dir(),
    )
}

fn default_working_directory_from_env(
    home: Option<&std::ffi::OsStr>,
    user_profile: Option<&std::ffi::OsStr>,
    home_drive: Option<&std::ffi::OsStr>,
    home_path: Option<&std::ffi::OsStr>,
    fallback: Option<PathBuf>,
) -> Option<PathBuf> {
    home.filter(|value| !value.is_empty())
        .map(PathBuf::from)
        .or_else(|| {
            user_profile
                .filter(|value| !value.is_empty())
                .map(PathBuf::from)
        })
        .or_else(|| match (home_drive, home_path) {
            (Some(drive), Some(path)) if !drive.is_empty() && !path.is_empty() => {
                let mut combined = std::ffi::OsString::from(drive);
                combined.push(path);
                Some(PathBuf::from(combined))
            }
            _ => None,
        })
        .or(fallback)
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::path::PathBuf;

    #[test]
    fn default_working_directory_prefers_unix_home_env() {
        let home = PathBuf::from("/tmp/rabbitty-home");

        assert_eq!(
            default_working_directory_from_env(
                Some(home.as_os_str()),
                None,
                None,
                None,
                Some(PathBuf::from("/fallback"))
            ),
            Some(home)
        );
    }

    #[test]
    fn default_working_directory_prefers_windows_user_profile() {
        let profile = PathBuf::from(r"C:\Users\rabbitty");

        assert_eq!(
            default_working_directory_from_env(
                None,
                Some(profile.as_os_str()),
                None,
                None,
                Some(PathBuf::from(r"C:\fallback"))
            ),
            Some(profile)
        );
    }

    #[test]
    fn default_working_directory_builds_windows_home_drive_path() {
        assert_eq!(
            default_working_directory_from_env(
                None,
                None,
                Some(std::ffi::OsStr::new("C:")),
                Some(std::ffi::OsStr::new(r"\Users\rabbitty")),
                None
            ),
            Some(PathBuf::from(r"C:\Users\rabbitty"))
        );
    }
}

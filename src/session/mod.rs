pub mod history;
mod transport;

use iced::futures::channel::mpsc;
use std::io::{ErrorKind, Write};
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
        write_all_retrying(&mut **guard, bytes)
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

/// The local pty master is opened `O_NONBLOCK` and the writer is a `dup` that
/// shares those flags, so a paste larger than the tty input buffer returns
/// after a partial write. `write_all` would report that as an error the callers
/// discard, silently losing the tail of the user's input. Retry while the child
/// drains, but give up rather than freezing the UI on a child that never reads.
const WRITE_RETRY_BUDGET: std::time::Duration = std::time::Duration::from_millis(250);

fn write_all_retrying<W: Write + ?Sized>(writer: &mut W, bytes: &[u8]) -> Result<(), SessionError> {
    let deadline = std::time::Instant::now() + WRITE_RETRY_BUDGET;
    let mut backoff = std::time::Duration::from_micros(100);
    let mut written = 0;

    while written < bytes.len() {
        match writer.write(&bytes[written..]) {
            Ok(0) => {
                return Err(SessionError::Io(format!(
                    "write accepted {written} of {} bytes and then stopped",
                    bytes.len()
                )));
            }
            Ok(count) => written += count,
            Err(err) if err.kind() == ErrorKind::Interrupted => {}
            Err(err) if err.kind() == ErrorKind::WouldBlock => {
                if std::time::Instant::now() >= deadline {
                    return Err(SessionError::Io(format!(
                        "write blocked with {} of {} bytes unsent",
                        bytes.len() - written,
                        bytes.len()
                    )));
                }
                std::thread::sleep(backoff);
                backoff = (backoff * 2).min(std::time::Duration::from_millis(2));
            }
            Err(err) => return Err(SessionError::Io(format!("write failed: {err}"))),
        }
    }

    Ok(())
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

    /// Accepts `chunk` bytes per call and reports `WouldBlock` in between, the
    /// way a nonblocking pty master behaves while the child drains its input.
    struct StutteringWriter {
        accepted: Vec<u8>,
        chunk: usize,
        block_next: bool,
    }

    impl Write for StutteringWriter {
        fn write(&mut self, buf: &[u8]) -> std::io::Result<usize> {
            if self.block_next {
                self.block_next = false;
                return Err(std::io::Error::from(ErrorKind::WouldBlock));
            }
            self.block_next = true;
            let take = self.chunk.min(buf.len());
            self.accepted.extend_from_slice(&buf[..take]);
            Ok(take)
        }

        fn flush(&mut self) -> std::io::Result<()> {
            Ok(())
        }
    }

    #[test]
    fn a_paste_larger_than_the_tty_buffer_is_written_in_full() {
        let payload: Vec<u8> = (0..8192u32).map(|i| (i % 251) as u8).collect();
        let mut writer = StutteringWriter {
            accepted: Vec::new(),
            chunk: 4096,
            block_next: false,
        };

        write_all_retrying(&mut writer, &payload).expect("a draining pty must not lose bytes");

        assert_eq!(writer.accepted, payload);
    }

    #[test]
    fn a_child_that_never_reads_is_reported_instead_of_losing_input_silently() {
        struct Stalled;
        impl Write for Stalled {
            fn write(&mut self, _buf: &[u8]) -> std::io::Result<usize> {
                Err(std::io::Error::from(ErrorKind::WouldBlock))
            }
            fn flush(&mut self) -> std::io::Result<()> {
                Ok(())
            }
        }

        let err = write_all_retrying(&mut Stalled, b"hello").expect_err("must not report success");
        let SessionError::Io(message) = err else {
            panic!("expected an io error");
        };
        assert!(message.contains("5 of 5 bytes unsent"), "{message}");
    }
}

pub mod history;

use alacritty_terminal::event::{OnResize, WindowSize};
use alacritty_terminal::tty::{self, Options, Shell};
#[cfg(windows)]
use alacritty_terminal::tty::{ChildEvent, EventedPty, EventedReadWrite};
use iced::futures::channel::mpsc;
#[cfg(unix)]
use std::io::ErrorKind;
use std::io::{Read, Write};
use std::path::PathBuf;
#[cfg(windows)]
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex};
use std::thread;
use std::thread::JoinHandle;
use std::time::Duration;

pub struct LaunchSpec {
    pub program: String,
    pub args: Vec<String>,
    pub env: Vec<(String, String)>,
    pub rows: u16,
    pub cols: u16,
}

pub struct Session {
    writer: Arc<Mutex<Box<dyn Write + Send>>>,
    #[cfg(unix)]
    pty: Option<tty::Pty>,
    #[cfg(windows)]
    pty: Option<Arc<Mutex<tty::Pty>>>,
    #[cfg(windows)]
    shutdown: Option<Arc<AtomicBool>>,
    reader: Option<JoinHandle<()>>,
    /// For native SSH sessions: send resize events to the async task.
    resize_tx: Option<tokio::sync::mpsc::UnboundedSender<(u16, u16)>>,
    /// For native SSH sessions: handle that can open additional channels
    /// (e.g., SFTP subsystem) on the active connection.
    ssh: Option<crate::ssh::SshSessionHandle>,
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

#[cfg(windows)]
struct PtyWriter {
    pty: Arc<Mutex<tty::Pty>>,
}

#[cfg(windows)]
impl Write for PtyWriter {
    fn write(&mut self, buf: &[u8]) -> std::io::Result<usize> {
        let mut guard = self
            .pty
            .lock()
            .map_err(|_| std::io::Error::other("pty mutex poisoned"))?;
        guard.writer().write(buf)
    }

    fn flush(&mut self) -> std::io::Result<()> {
        let mut guard = self
            .pty
            .lock()
            .map_err(|_| std::io::Error::other("pty mutex poisoned"))?;
        guard.writer().flush()
    }
}

impl Session {
    #[cfg(unix)]
    pub fn spawn(
        spec: LaunchSpec,
        tab_id: u64,
        mut output_tx: mpsc::UnboundedSender<OutputEvent>,
    ) -> Result<Self, SessionError> {
        tty::setup_env();

        // Override process env for keys specified by the launch spec.
        // This ensures the forked child inherits the correct value
        // even if setup_env set something else.
        for (key, value) in &spec.env {
            // SAFETY: no other threads mutate env concurrently on the main thread.
            unsafe { std::env::set_var(key, value) };
        }

        let options = Options {
            shell: Some(Shell::new(spec.program, spec.args)),
            env: spec.env.into_iter().collect(),
            working_directory: default_working_directory(),
            ..Default::default()
        };

        let window_size = WindowSize {
            num_lines: spec.rows,
            num_cols: spec.cols,
            cell_width: 1,
            cell_height: 1,
        };

        let pty = tty::new(&options, window_size, tab_id)
            .map_err(|err| SessionError::Spawn(format!("pty spawn failed: {err}")))?;

        let reader_file = pty
            .file()
            .try_clone()
            .map_err(|err| SessionError::Spawn(format!("reader clone failed: {err}")))?;

        let writer_file = pty
            .file()
            .try_clone()
            .map_err(|err| SessionError::Spawn(format!("writer clone failed: {err}")))?;

        let writer: Arc<Mutex<Box<dyn Write + Send>>> = Arc::new(Mutex::new(Box::new(writer_file)));

        let reader_handle = thread::spawn(move || {
            let mut reader = reader_file;
            let mut buf = [0u8; 2048];
            loop {
                match reader.read(&mut buf) {
                    Ok(0) => {
                        let _ = send_output_event(&mut output_tx, OutputEvent::Closed { tab_id });
                        break;
                    }
                    Ok(n) => {
                        if !send_output_event(
                            &mut output_tx,
                            OutputEvent::Data {
                                tab_id,
                                bytes: buf[..n].to_vec(),
                            },
                        ) {
                            break;
                        }
                    }
                    Err(err) if err.kind() == ErrorKind::Interrupted => continue,
                    Err(err) if err.kind() == ErrorKind::WouldBlock => {
                        thread::sleep(Duration::from_millis(1));
                        continue;
                    }
                    Err(_) => {
                        let _ = send_output_event(&mut output_tx, OutputEvent::Closed { tab_id });
                        break;
                    }
                }
            }
        });

        Ok(Self {
            writer,
            pty: Some(pty),
            reader: Some(reader_handle),
            resize_tx: None,
            ssh: None,
        })
    }

    #[cfg(windows)]
    pub fn spawn(
        spec: LaunchSpec,
        tab_id: u64,
        mut output_tx: mpsc::UnboundedSender<OutputEvent>,
    ) -> Result<Self, SessionError> {
        tty::setup_env();

        let options = Options {
            shell: Some(Shell::new(spec.program, spec.args)),
            env: spec.env.into_iter().collect(),
            working_directory: default_working_directory(),
            ..Default::default()
        };

        let window_size = WindowSize {
            num_lines: spec.rows,
            num_cols: spec.cols,
            cell_width: 1,
            cell_height: 1,
        };

        let pty = tty::new(&options, window_size, tab_id)
            .map_err(|err| SessionError::Spawn(format!("pty spawn failed: {err}")))?;
        let pty = Arc::new(Mutex::new(pty));
        let shutdown = Arc::new(AtomicBool::new(false));

        let writer: Arc<Mutex<Box<dyn Write + Send>>> = Arc::new(Mutex::new(Box::new(PtyWriter {
            pty: Arc::clone(&pty),
        })));

        let reader_pty = Arc::clone(&pty);
        let reader_shutdown = Arc::clone(&shutdown);
        let reader_handle = thread::spawn(move || {
            let mut buf = [0u8; 2048];
            while !reader_shutdown.load(Ordering::Acquire) {
                let (bytes, exited) = {
                    let mut guard = match reader_pty.lock() {
                        Ok(g) => g,
                        Err(_) => break,
                    };
                    let n = guard.reader().read(&mut buf).unwrap_or(0);
                    let bytes = if n > 0 { Some(buf[..n].to_vec()) } else { None };
                    let exited = matches!(guard.next_child_event(), Some(ChildEvent::Exited(_)));
                    (bytes, exited)
                };

                if let Some(bytes) = bytes {
                    if !send_output_event(&mut output_tx, OutputEvent::Data { tab_id, bytes }) {
                        break;
                    }
                    continue;
                }

                if exited {
                    let _ = send_output_event(&mut output_tx, OutputEvent::Closed { tab_id });
                    break;
                }

                thread::sleep(Duration::from_millis(5));
            }
        });

        Ok(Self {
            writer,
            pty: Some(pty),
            shutdown: Some(shutdown),
            reader: Some(reader_handle),
            resize_tx: None,
            ssh: None,
        })
    }

    pub fn spawn_ssh(
        profile: crate::config::SshProfile,
        tab_id: u64,
        rows: u16,
        cols: u16,
        output_tx: mpsc::UnboundedSender<OutputEvent>,
    ) -> Self {
        let handle = crate::ssh::spawn_ssh_session(profile, tab_id, rows, cols, output_tx);
        Self {
            writer: Arc::clone(&handle.writer),
            #[cfg(unix)]
            pty: None,
            #[cfg(windows)]
            pty: None,
            #[cfg(windows)]
            shutdown: None,
            reader: None,
            resize_tx: Some(handle.resize_tx.clone()),
            ssh: Some(handle),
        }
    }

    /// Returns the underlying SSH session handle when this session was spawned
    /// via `spawn_ssh`. Local PTY sessions return `None`.
    pub fn ssh_handle(&self) -> Option<&crate::ssh::SshSessionHandle> {
        self.ssh.as_ref()
    }

    pub fn send_bytes(&self, bytes: &[u8]) -> Result<(), SessionError> {
        let mut guard = self
            .writer
            .lock()
            .map_err(|err| SessionError::Io(format!("writer lock failed: {err}")))?;
        guard
            .write_all(bytes)
            .map_err(|err| SessionError::Io(format!("write failed: {err}")))
    }

    pub fn writer(&self) -> Arc<Mutex<Box<dyn Write + Send>>> {
        Arc::clone(&self.writer)
    }

    #[cfg(unix)]
    pub fn resize(&mut self, rows: u16, cols: u16) -> Result<(), SessionError> {
        if let Some(ref tx) = self.resize_tx {
            let _ = tx.send((rows, cols));
            return Ok(());
        }
        if let Some(ref mut pty) = self.pty {
            let window_size = WindowSize {
                num_lines: rows,
                num_cols: cols,
                cell_width: 1,
                cell_height: 1,
            };
            pty.on_resize(window_size);
            Ok(())
        } else {
            Err(SessionError::Io("no pty".into()))
        }
    }

    #[cfg(windows)]
    pub fn resize(&mut self, rows: u16, cols: u16) -> Result<(), SessionError> {
        if let Some(ref tx) = self.resize_tx {
            let _ = tx.send((rows, cols));
            return Ok(());
        }
        let pty = self
            .pty
            .as_ref()
            .ok_or_else(|| SessionError::Io("no pty".into()))?;
        let window_size = WindowSize {
            num_lines: rows,
            num_cols: cols,
            cell_width: 1,
            cell_height: 1,
        };
        let mut guard = pty
            .lock()
            .map_err(|err| SessionError::Io(format!("pty lock failed: {err}")))?;
        guard.on_resize(window_size);
        Ok(())
    }
}

fn send_output_event(
    output_tx: &mut mpsc::UnboundedSender<OutputEvent>,
    event: OutputEvent,
) -> bool {
    output_tx.unbounded_send(event).is_ok()
}

fn default_working_directory() -> Option<PathBuf> {
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

#[cfg(unix)]
impl Drop for Session {
    fn drop(&mut self) {
        // Drop the PTY first — kills the child process, causing
        // the slave side to close. The reader thread will then get
        // EIO on its cloned master fd and exit.
        self.pty.take();

        if let Some(handle) = self.reader.take() {
            let _ = handle.join();
        }
    }
}

#[cfg(windows)]
impl Drop for Session {
    fn drop(&mut self) {
        if let Some(ref shutdown) = self.shutdown {
            shutdown.store(true, Ordering::Release);
        }
        if let Some(handle) = self.reader.take() {
            let _ = handle.join();
        }
    }
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

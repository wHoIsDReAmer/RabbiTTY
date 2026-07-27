use super::{LaunchSpec, OutputEvent, SessionError, default_working_directory, send_output_event};
use crate::ssh::SshSessionHandle;
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

/// A connection backend behind a [`Session`](super::Session): a local PTY or an
/// SSH channel. Native only (Tier 1) — never crosses the plugin/WASM boundary.
pub(super) trait Transport: Send {
    fn writer(&self) -> Arc<Mutex<Box<dyn Write + Send>>>;
    fn resize(&mut self, rows: u16, cols: u16) -> Result<(), SessionError>;
    fn working_directory(&self) -> Option<PathBuf> {
        None
    }
    fn ssh_handle(&self) -> Option<&SshSessionHandle> {
        None
    }
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

// ── Local PTY (unix) ────────────────────────────────────────────────────────
#[cfg(unix)]
pub(super) struct LocalPty {
    writer: Arc<Mutex<Box<dyn Write + Send>>>,
    pty: Option<tty::Pty>,
    reader: Option<JoinHandle<()>>,
}

#[cfg(unix)]
impl LocalPty {
    pub(super) fn spawn(
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
            working_directory: spec.cwd.or_else(default_working_directory),
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
        })
    }
}

#[cfg(unix)]
impl Transport for LocalPty {
    fn writer(&self) -> Arc<Mutex<Box<dyn Write + Send>>> {
        Arc::clone(&self.writer)
    }

    fn resize(&mut self, rows: u16, cols: u16) -> Result<(), SessionError> {
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

    fn working_directory(&self) -> Option<PathBuf> {
        let pid = self.pty.as_ref()?.child().id();
        process_cwd(pid)
    }
}

#[cfg(unix)]
impl Drop for LocalPty {
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

// ── Local PTY (windows) ─────────────────────────────────────────────────────
#[cfg(windows)]
pub(super) struct LocalPty {
    writer: Arc<Mutex<Box<dyn Write + Send>>>,
    pty: Option<Arc<Mutex<tty::Pty>>>,
    shutdown: Option<Arc<AtomicBool>>,
    reader: Option<JoinHandle<()>>,
}

#[cfg(windows)]
impl LocalPty {
    pub(super) fn spawn(
        spec: LaunchSpec,
        tab_id: u64,
        mut output_tx: mpsc::UnboundedSender<OutputEvent>,
    ) -> Result<Self, SessionError> {
        tty::setup_env();

        let options = Options {
            shell: Some(Shell::new(spec.program, spec.args)),
            env: spec.env.into_iter().collect(),
            working_directory: spec.cwd.or_else(default_working_directory),
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
        })
    }
}

#[cfg(windows)]
impl Transport for LocalPty {
    fn writer(&self) -> Arc<Mutex<Box<dyn Write + Send>>> {
        Arc::clone(&self.writer)
    }

    fn resize(&mut self, rows: u16, cols: u16) -> Result<(), SessionError> {
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

#[cfg(windows)]
impl Drop for LocalPty {
    fn drop(&mut self) {
        if let Some(ref shutdown) = self.shutdown {
            shutdown.store(true, Ordering::Release);
        }
        if let Some(handle) = self.reader.take() {
            let _ = handle.join();
        }
    }
}

// ── SSH ─────────────────────────────────────────────────────────────────────
pub(super) struct SshTransport {
    handle: SshSessionHandle,
}

impl SshTransport {
    pub(super) fn spawn(
        profile: crate::config::SshProfile,
        tab_id: u64,
        rows: u16,
        cols: u16,
        output_tx: mpsc::UnboundedSender<OutputEvent>,
    ) -> Self {
        let handle = crate::ssh::spawn_ssh_session(profile, tab_id, rows, cols, output_tx);
        Self { handle }
    }
}

impl Transport for SshTransport {
    fn writer(&self) -> Arc<Mutex<Box<dyn Write + Send>>> {
        Arc::clone(&self.handle.writer)
    }

    fn resize(&mut self, rows: u16, cols: u16) -> Result<(), SessionError> {
        let _ = self.handle.resize_tx.send((rows, cols));
        Ok(())
    }

    fn ssh_handle(&self) -> Option<&SshSessionHandle> {
        Some(&self.handle)
    }
}

// ── cwd resolution (unix) ───────────────────────────────────────────────────
#[cfg(target_os = "linux")]
fn process_cwd(pid: u32) -> Option<PathBuf> {
    std::fs::read_link(format!("/proc/{pid}/cwd")).ok()
}

#[cfg(target_os = "macos")]
fn process_cwd(pid: u32) -> Option<PathBuf> {
    use std::os::unix::ffi::OsStrExt;
    let mut info: libc::proc_vnodepathinfo = unsafe { std::mem::zeroed() };
    let size = std::mem::size_of::<libc::proc_vnodepathinfo>() as libc::c_int;
    let ret = unsafe {
        libc::proc_pidinfo(
            pid as libc::c_int,
            libc::PROC_PIDVNODEPATHINFO,
            0,
            &mut info as *mut _ as *mut libc::c_void,
            size,
        )
    };
    if ret != size {
        return None;
    }
    let path = &info.pvi_cdir.vip_path;
    let bytes = unsafe { std::slice::from_raw_parts(path.as_ptr() as *const u8, path.len()) };
    let end = bytes.iter().position(|&b| b == 0).unwrap_or(bytes.len());
    if end == 0 {
        return None;
    }
    Some(PathBuf::from(std::ffi::OsStr::from_bytes(&bytes[..end])))
}

#[cfg(all(unix, not(target_os = "linux"), not(target_os = "macos")))]
fn process_cwd(_pid: u32) -> Option<PathBuf> {
    None
}

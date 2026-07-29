use super::{LaunchSpec, OutputEvent, SessionError, default_working_directory, send_output_event};
use crate::ssh::SshSessionHandle;
use alacritty_terminal::event::{OnResize, WindowSize};
use alacritty_terminal::tty::{self, Options, Shell};
#[cfg(windows)]
use alacritty_terminal::tty::{ChildEvent, EventedPty, EventedReadWrite};
use iced::futures::channel::mpsc;
use std::collections::HashMap;
#[cfg(unix)]
use std::io::ErrorKind;
use std::io::{Read, Write};
use std::path::PathBuf;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex, OnceLock};
use std::thread;
use std::thread::JoinHandle;
use std::time::Duration;

/// A connection backend behind a [`Session`](super::Session)
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

fn child_env(spec_env: Vec<(String, String)>) -> HashMap<String, String> {
    let mut env: HashMap<String, String> = spec_env.into_iter().collect();
    env.entry("TERM".to_string())
        .or_insert_with(|| default_term().to_string());
    env.entry("COLORTERM".to_string())
        .or_insert_with(|| "truecolor".to_string());
    env
}

fn default_term() -> &'static str {
    static TERM: OnceLock<&'static str> = OnceLock::new();
    TERM.get_or_init(|| {
        if terminfo_exists("alacritty") {
            "alacritty"
        } else {
            "xterm-256color"
        }
    })
}

/// Mirrors alacritty's private `terminfo_exists`.
fn terminfo_exists(terminfo: &str) -> bool {
    let first = terminfo.get(..1).unwrap_or_default();
    let first_hex = format!("{:x}", first.chars().next().unwrap_or_default() as usize);

    macro_rules! check_path {
        ($path:expr) => {
            if $path.join(first).join(terminfo).exists()
                || $path.join(&first_hex).join(terminfo).exists()
            {
                return true;
            }
        };
    }

    if let Some(dir) = std::env::var_os("TERMINFO") {
        check_path!(PathBuf::from(&dir));
    } else if let Some(home) = dirs::home_dir() {
        check_path!(home.join(".terminfo"));
    }

    if let Ok(dirs) = std::env::var("TERMINFO_DIRS") {
        for dir in dirs.split(':') {
            check_path!(PathBuf::from(dir));
        }
    }

    if let Ok(prefix) = std::env::var("PREFIX") {
        let path = PathBuf::from(prefix);
        check_path!(path.join("etc/terminfo"));
        check_path!(path.join("lib/terminfo"));
        check_path!(path.join("share/terminfo"));
    }

    check_path!(PathBuf::from("/etc/terminfo"));
    check_path!(PathBuf::from("/lib/terminfo"));
    check_path!(PathBuf::from("/usr/share/terminfo"));
    check_path!(PathBuf::from("/boot/system/data/terminfo"));

    false
}

// ── Local PTY (unix) ────────────────────────────────────────────────────────
#[cfg(unix)]
pub(super) struct LocalPty {
    writer: Arc<Mutex<Box<dyn Write + Send>>>,
    pty: Option<tty::Pty>,
    shutdown: Arc<AtomicBool>,
    reader: Option<JoinHandle<()>>,
}

#[cfg(unix)]
impl LocalPty {
    pub(super) fn spawn(
        spec: LaunchSpec,
        tab_id: u64,
        mut output_tx: mpsc::UnboundedSender<OutputEvent>,
    ) -> Result<Self, SessionError> {
        let options = Options {
            shell: Some(Shell::new(spec.program, spec.args)),
            env: child_env(spec.env),
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

        let shutdown = Arc::new(AtomicBool::new(false));
        let reader_shutdown = Arc::clone(&shutdown);

        let reader_handle = thread::spawn(move || {
            let mut reader = reader_file;
            let mut buf = [0u8; 2048];

            // Stopping the reads is not an option even once nobody wants the output:
            // the shell fills the tty buffer, and then it cannot finish exiting, which
            // leaves `Pty::drop` waiting on it forever. Drain and discard instead.
            let mut listening = true;
            loop {
                match reader.read(&mut buf) {
                    Ok(0) => {
                        let _ = send_output_event(&mut output_tx, OutputEvent::Closed { tab_id });
                        break;
                    }
                    Ok(n) => {
                        if listening {
                            listening = send_output_event(
                                &mut output_tx,
                                OutputEvent::Data {
                                    tab_id,
                                    bytes: buf[..n].to_vec(),
                                },
                            );
                        }
                    }
                    Err(err) if err.kind() == ErrorKind::Interrupted => continue,
                    Err(err) if err.kind() == ErrorKind::WouldBlock => {
                        if reader_shutdown.load(Ordering::Relaxed) {
                            break;
                        }
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
            shutdown,
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
        // Drop the PTY first — kills the child, so the reader drains what is left
        // and sees EOF. The flag then covers the case where EOF never comes.
        self.pty.take();
        self.shutdown.store(true, Ordering::Relaxed);

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
        let options = Options {
            shell: Some(Shell::new(spec.program, spec.args)),
            env: child_env(spec.env),
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

#[cfg(test)]
mod tests {
    use super::child_env;

    #[test]
    fn child_env_supplies_term_without_touching_the_process() {
        let env = child_env(Vec::new());
        assert!(matches!(
            env.get("TERM").map(String::as_str),
            Some("alacritty") | Some("xterm-256color")
        ));
        assert_eq!(env.get("COLORTERM").map(String::as_str), Some("truecolor"));
    }

    #[test]
    fn child_env_keeps_launch_spec_entries_and_lets_them_override() {
        let env = child_env(vec![
            ("TERM".to_string(), "dumb".to_string()),
            ("PROMPT_COMMAND".to_string(), "true".to_string()),
        ]);
        assert_eq!(env.get("TERM").map(String::as_str), Some("dumb"));
        assert_eq!(env.get("PROMPT_COMMAND").map(String::as_str), Some("true"));
    }
}

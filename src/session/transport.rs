use super::{
    LaunchSpec, OutputEvent, PluginInput, PtySize, SessionError, default_working_directory,
    send_output_event,
};
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
#[cfg(unix)]
use std::os::fd::AsRawFd;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex, OnceLock};
use std::thread;
use std::thread::JoinHandle;
#[cfg(windows)]
use std::time::Duration;

/// A connection backend behind a [`Session`](super::Session)
pub(super) trait Transport: Send {
    fn writer(&self) -> Arc<Mutex<Box<dyn Write + Send>>>;
    fn resize(&mut self, size: PtySize) -> Result<(), SessionError>;
    fn working_directory(&self) -> Option<PathBuf> {
        None
    }
    fn ssh_handle(&self) -> Option<&SshSessionHandle> {
        None
    }
}

fn window_size(size: PtySize) -> WindowSize {
    WindowSize {
        num_lines: size.rows,
        num_cols: size.cols,
        cell_width: size.cell_width,
        cell_height: size.cell_height,
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
    if needs_locale(&env) {
        env.insert("LANG".to_string(), default_locale().to_string());
    }
    env
}

/// A GUI launch inherits no locale from launchd, and in the C locale programs
/// writing to a tty replace non-ASCII with `?`.
fn needs_locale(env: &HashMap<String, String>) -> bool {
    ["LC_ALL", "LC_CTYPE", "LANG"]
        .iter()
        .all(|key| !env.contains_key(*key) && std::env::var_os(key).is_none())
}

fn default_locale() -> &'static str {
    static LOCALE: OnceLock<String> = OnceLock::new();
    LOCALE.get_or_init(|| {
        utf8_locale(system_locale().as_deref(), |name| {
            Path::new("/usr/share/locale").join(name).exists()
        })
    })
}

fn system_locale() -> Option<String> {
    if !cfg!(target_os = "macos") {
        return None;
    }
    let out = std::process::Command::new("defaults")
        .args(["read", "-g", "AppleLocale"])
        .output()
        .ok()?;
    let raw = String::from_utf8(out.stdout).ok()?.trim().to_string();
    (!raw.is_empty()).then_some(raw)
}

fn utf8_locale(raw: Option<&str>, exists: impl Fn(&str) -> bool) -> String {
    const FALLBACK: &str = "en_US.UTF-8";
    let Some(raw) = raw else {
        return FALLBACK.to_string();
    };
    let base = raw
        .split(['@', '.'])
        .next()
        .unwrap_or_default()
        .replace('-', "_");
    if base.is_empty() {
        return FALLBACK.to_string();
    }
    let candidate = format!("{base}.UTF-8");
    if exists(&candidate) {
        candidate
    } else {
        FALLBACK.to_string()
    }
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

#[cfg(unix)]
const POLL_TIMEOUT_MS: i32 = 50;
const READ_BUF: usize = 64 * 1024;
const COALESCE_CAP: usize = 256 * 1024;

/// Blocks until the fd has something to read, it hangs up, or the timeout
/// expires. The timeout only exists so shutdown is noticed promptly.
#[cfg(unix)]
fn wait_readable(fd: std::os::fd::RawFd, timeout_ms: i32) {
    let mut poll_fd = libc::pollfd {
        fd,
        events: libc::POLLIN,
        revents: 0,
    };

    // SAFETY: one initialised pollfd is passed with a matching count.
    unsafe {
        libc::poll(&mut poll_fd, 1, timeout_ms);
    }
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

        let window_size = window_size(spec.size);

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
            let mut buf = [0u8; READ_BUF];

            let mut pending: Vec<u8> = Vec::new();

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
                        pending.extend_from_slice(&buf[..n]);
                        if pending.len() < COALESCE_CAP {
                            continue;
                        }

                        if listening {
                            listening = send_output_event(
                                &mut output_tx,
                                OutputEvent::Data {
                                    tab_id,
                                    bytes: std::mem::take(&mut pending),
                                },
                            );
                        }

                        pending.clear();
                    }
                    Err(err) if err.kind() == ErrorKind::Interrupted => continue,
                    Err(err) if err.kind() == ErrorKind::WouldBlock => {
                        if !pending.is_empty() {
                            if listening {
                                listening = send_output_event(
                                    &mut output_tx,
                                    OutputEvent::Data {
                                        tab_id,
                                        bytes: std::mem::take(&mut pending),
                                    },
                                );
                            }

                            pending.clear();
                            continue;
                        }
                        if reader_shutdown.load(Ordering::Relaxed) {
                            break;
                        }

                        wait_readable(reader.as_raw_fd(), POLL_TIMEOUT_MS);
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

    fn resize(&mut self, size: PtySize) -> Result<(), SessionError> {
        if let Some(ref mut pty) = self.pty {
            pty.on_resize(window_size(size));
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
        let pty = self.pty.take();
        let shutdown = Arc::clone(&self.shutdown);
        let reader = self.reader.take();
        let _ = thread::Builder::new()
            .name("pty-cleanup".to_string())
            .spawn(move || {
                // Drop the PTY first — kills the child, so the reader drains what is left
                // and sees EOF. The flag then covers the case where EOF never comes.
                drop(pty);
                shutdown.store(true, Ordering::Relaxed);
                if let Some(handle) = reader {
                    let _ = handle.join();
                }
            });
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

        let window_size = window_size(spec.size);

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
            let mut buf = [0u8; READ_BUF];
            // Reads here never block, so a read returning nothing means the pty is
            // empty right now. Accumulating until that happens turns many small
            // reads into one message, and each read still takes the lock on its own
            // so resizes and writes are not held up.
            let mut pending: Vec<u8> = Vec::new();
            while !reader_shutdown.load(Ordering::Acquire) {
                let (read, exited) = {
                    let mut guard = match reader_pty.lock() {
                        Ok(g) => g,
                        Err(_) => break,
                    };
                    let n = guard.reader().read(&mut buf).unwrap_or(0);
                    let exited = matches!(guard.next_child_event(), Some(ChildEvent::Exited(_)));
                    (n, exited)
                };

                if read > 0 {
                    pending.extend_from_slice(&buf[..read]);
                    if pending.len() < COALESCE_CAP {
                        continue;
                    }
                }

                if !pending.is_empty() {
                    let bytes = std::mem::take(&mut pending);
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

    fn resize(&mut self, size: PtySize) -> Result<(), SessionError> {
        let pty = self
            .pty
            .as_ref()
            .ok_or_else(|| SessionError::Io("no pty".into()))?;
        let mut guard = pty
            .lock()
            .map_err(|err| SessionError::Io(format!("pty lock failed: {err}")))?;
        guard.on_resize(window_size(size));
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
        size: PtySize,
        output_tx: mpsc::UnboundedSender<OutputEvent>,
    ) -> Self {
        let handle = crate::ssh::spawn_ssh_session(profile, tab_id, size, output_tx);
        Self { handle }
    }
}

impl Transport for SshTransport {
    fn writer(&self) -> Arc<Mutex<Box<dyn Write + Send>>> {
        Arc::clone(&self.handle.writer)
    }

    fn resize(&mut self, size: PtySize) -> Result<(), SessionError> {
        let _ = self.handle.resize_tx.send(size);
        Ok(())
    }

    fn ssh_handle(&self) -> Option<&SshSessionHandle> {
        Some(&self.handle)
    }
}

pub(super) struct PluginTransport {
    input: mpsc::UnboundedSender<PluginInput>,
    writer: Arc<Mutex<Box<dyn Write + Send>>>,
}

struct PluginWriter {
    input: mpsc::UnboundedSender<PluginInput>,
}

impl Write for PluginWriter {
    fn write(&mut self, buf: &[u8]) -> std::io::Result<usize> {
        self.input
            .unbounded_send(PluginInput::Data(buf.to_vec()))
            .map_err(|_| {
                std::io::Error::new(std::io::ErrorKind::BrokenPipe, "plugin tab closed")
            })?;
        Ok(buf.len())
    }

    fn flush(&mut self) -> std::io::Result<()> {
        Ok(())
    }
}

impl PluginTransport {
    pub(super) fn spawn(input: mpsc::UnboundedSender<PluginInput>, rows: u16, cols: u16) -> Self {
        let _ = input.unbounded_send(PluginInput::Resize { rows, cols });
        let writer: Box<dyn Write + Send> = Box::new(PluginWriter {
            input: input.clone(),
        });
        Self {
            input,
            writer: Arc::new(Mutex::new(writer)),
        }
    }
}

impl Transport for PluginTransport {
    fn writer(&self) -> Arc<Mutex<Box<dyn Write + Send>>> {
        Arc::clone(&self.writer)
    }

    fn resize(&mut self, size: PtySize) -> Result<(), SessionError> {
        let _ = self.input.unbounded_send(PluginInput::Resize {
            rows: size.rows,
            cols: size.cols,
        });
        Ok(())
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
    use super::{child_env, needs_locale, utf8_locale};
    use std::collections::HashMap;

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

    #[test]
    fn a_system_locale_becomes_its_utf8_variant() {
        assert_eq!(
            utf8_locale(Some("ko_KR"), |n| n == "ko_KR.UTF-8"),
            "ko_KR.UTF-8"
        );
        assert_eq!(
            utf8_locale(Some("ko-KR"), |n| n == "ko_KR.UTF-8"),
            "ko_KR.UTF-8"
        );
        assert_eq!(
            utf8_locale(Some("en_US@rg=krzzzz"), |n| n == "en_US.UTF-8"),
            "en_US.UTF-8"
        );
    }

    #[test]
    fn an_unavailable_or_missing_locale_falls_back_to_utf8() {
        assert_eq!(utf8_locale(Some("zh-Hans-CN"), |_| false), "en_US.UTF-8");
        assert_eq!(utf8_locale(None, |_| true), "en_US.UTF-8");
        assert_eq!(utf8_locale(Some(""), |_| true), "en_US.UTF-8");
    }

    #[test]
    fn a_locale_the_profile_already_sets_is_left_alone() {
        for key in ["LC_ALL", "LC_CTYPE", "LANG"] {
            let env = HashMap::from([(key.to_string(), "C".to_string())]);
            assert!(!needs_locale(&env), "{key}");
        }
    }

    #[cfg(unix)]
    #[test]
    fn a_child_reads_the_grid_and_its_pixel_size_from_the_pty() {
        use super::{LocalPty, Transport};
        use crate::session::{LaunchSpec, PtySize};
        use std::os::fd::AsRawFd;

        fn winsize(pty: &LocalPty) -> (u16, u16, u16, u16) {
            let fd = pty.pty.as_ref().expect("pty").file().as_raw_fd();
            let mut size = libc::winsize {
                ws_row: 0,
                ws_col: 0,
                ws_xpixel: 0,
                ws_ypixel: 0,
            };
            let status = unsafe { libc::ioctl(fd, libc::TIOCGWINSZ, &mut size) };
            assert_eq!(status, 0, "TIOCGWINSZ failed");
            (size.ws_row, size.ws_col, size.ws_xpixel, size.ws_ypixel)
        }

        let (tx, _rx) = iced::futures::channel::mpsc::unbounded();
        let spec = LaunchSpec {
            program: "/bin/sh".to_string(),
            args: Vec::new(),
            env: Vec::new(),
            size: PtySize {
                rows: 24,
                cols: 80,
                cell_width: 9,
                cell_height: 18,
            },
            cwd: None,
            shell_integration: false,
        };
        let mut pty = LocalPty::spawn(spec, 1, tx).expect("sh starts");
        assert_eq!(winsize(&pty), (24, 80, 720, 432));

        pty.resize(PtySize {
            rows: 30,
            cols: 100,
            cell_width: 10,
            cell_height: 20,
        })
        .expect("resize");
        assert_eq!(winsize(&pty), (30, 100, 1000, 600));
    }

    #[cfg(unix)]
    #[test]
    fn closing_a_pty_does_not_wait_for_a_program_that_ignores_hangup() {
        use super::LocalPty;
        use crate::session::{LaunchSpec, OutputEvent, PtySize};
        use std::time::{Duration, Instant};

        let (tx, mut rx) = iced::futures::channel::mpsc::unbounded();
        let spec = LaunchSpec {
            program: "/bin/sh".to_string(),
            args: vec![
                "-c".to_string(),
                "trap '' HUP; echo ready; sleep 5".to_string(),
            ],
            env: Vec::new(),
            size: PtySize {
                rows: 24,
                cols: 80,
                cell_width: 9,
                cell_height: 18,
            },
            cwd: None,
            shell_integration: false,
        };
        let pty = LocalPty::spawn(spec, 1, tx).expect("sh starts");

        let deadline = Instant::now() + Duration::from_secs(10);
        let mut output = Vec::new();
        while !String::from_utf8_lossy(&output).contains("ready") {
            assert!(Instant::now() < deadline, "sh never reported its trap");
            match rx.try_recv() {
                Ok(OutputEvent::Data { bytes, .. }) => output.extend(bytes),
                Ok(_) => panic!("sh exited before it trapped SIGHUP"),
                Err(_) => std::thread::sleep(Duration::from_millis(10)),
            }
        }

        let (closed_tx, closed_rx) = std::sync::mpsc::channel();
        std::thread::spawn(move || {
            drop(pty);
            let _ = closed_tx.send(());
        });
        assert!(
            closed_rx.recv_timeout(Duration::from_secs(2)).is_ok(),
            "closing the pty waited for a program that ignores SIGHUP"
        );
    }
}

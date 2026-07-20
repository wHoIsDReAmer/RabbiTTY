mod ansi;
pub mod sftp;
pub mod user_config;

use crate::config::{SshAuthMethod, SshProfile};
use crate::session::OutputEvent;
use async_trait::async_trait;
use iced::futures::channel::mpsc as futures_mpsc;
use russh::keys::*;
use russh::*;
use std::io::Write;
use std::pin::Pin;
use std::process::Stdio;
use std::sync::{Arc, Mutex};
use std::task::{Context, Poll};
use tokio::io::{AsyncRead, AsyncWrite, ReadBuf};
use tokio::process::{Child, ChildStdin, ChildStdout, Command};
use tokio::sync::mpsc as tokio_mpsc;

fn ssh_badge() -> String {
    ansi::badge("SSH")
}

// ── Host key verification ───────────────────────────────────────────

#[derive(Debug, Clone)]
pub enum HostKeyStatus {
    Known,
    Recorded,
    RecordFailed(String),
    CheckFailed(String),
    Changed { line: usize },
}

#[derive(Debug, Clone)]
pub struct HostKeyInfo {
    pub fingerprint: String,
    pub status: HostKeyStatus,
}

impl HostKeyStatus {
    fn accepts(&self) -> bool {
        !matches!(self, Self::Changed { .. } | Self::CheckFailed(_))
    }
}

fn known_hosts_path() -> Option<std::path::PathBuf> {
    Some(dirs::home_dir()?.join(".ssh").join("known_hosts"))
}

fn verify_host_key(
    host: &str,
    port: u16,
    key: &ssh_key::PublicKey,
    path: &std::path::Path,
) -> HostKeyStatus {
    use russh::keys::known_hosts::{check_known_hosts_path, learn_known_hosts_path};

    // russh maps every File::open failure to "not recorded", which would turn an
    // unreadable known_hosts into silent auto-accept. Detect that case first.
    if path.exists()
        && let Err(err) = std::fs::File::open(path)
    {
        return HostKeyStatus::CheckFailed(err.to_string());
    }

    match check_known_hosts_path(host, port, key, path) {
        Ok(true) => HostKeyStatus::Known,
        Ok(false) => match learn_known_hosts_path(host, port, key, path) {
            Ok(()) => HostKeyStatus::Recorded,
            Err(err) => HostKeyStatus::RecordFailed(err.to_string()),
        },
        Err(russh::keys::Error::KeyChanged { line }) => HostKeyStatus::Changed { line },
        Err(err) => HostKeyStatus::CheckFailed(err.to_string()),
    }
}

fn host_key_rejection(info: Option<HostKeyInfo>) -> Option<String> {
    match info?.status {
        HostKeyStatus::Changed { line } => Some(format!(
            "Host key verification failed: the key recorded at ~/.ssh/known_hosts line {line} \
             does not match the key this server presented. If you did not intentionally \
             change the server, the connection may be intercepted."
        )),
        HostKeyStatus::CheckFailed(err) => Some(format!(
            "Host key verification failed: ~/.ssh/known_hosts could not be read ({err}). \
             Fix its permissions, or remove it to start over."
        )),
        _ => None,
    }
}

// ── SSH client handler ──────────────────────────────────────────────
struct SshHandler {
    host: String,
    port: u16,
    host_key_tx: Option<tokio::sync::oneshot::Sender<HostKeyInfo>>,
}

#[async_trait]
impl client::Handler for SshHandler {
    type Error = russh::Error;

    async fn check_server_key(
        &mut self,
        server_public_key: &ssh_key::PublicKey,
    ) -> Result<bool, Self::Error> {
        let fingerprint = server_public_key
            .fingerprint(ssh_key::HashAlg::Sha256)
            .to_string();

        let status = match known_hosts_path() {
            Some(path) => verify_host_key(&self.host, self.port, server_public_key, &path),
            None => HostKeyStatus::CheckFailed("no home directory".to_string()),
        };

        let accepts = status.accepts();
        if let Some(tx) = self.host_key_tx.take() {
            let _ = tx.send(HostKeyInfo {
                fingerprint,
                status,
            });
        }
        Ok(accepts)
    }
}

// ── Sync Write → async tokio channel bridge ─────────────────────────
struct SshWriter {
    tx: tokio_mpsc::UnboundedSender<Vec<u8>>,
}

impl Write for SshWriter {
    fn write(&mut self, buf: &[u8]) -> std::io::Result<usize> {
        self.tx.send(buf.to_vec()).map_err(|_| {
            std::io::Error::new(std::io::ErrorKind::BrokenPipe, "SSH channel closed")
        })?;
        Ok(buf.len())
    }

    fn flush(&mut self) -> std::io::Result<()> {
        Ok(())
    }
}

struct ProxyCommandStream {
    child: Child,
    stdout: ChildStdout,
    stdin: ChildStdin,
}

impl AsyncRead for ProxyCommandStream {
    fn poll_read(
        mut self: Pin<&mut Self>,
        cx: &mut Context<'_>,
        buf: &mut ReadBuf<'_>,
    ) -> Poll<std::io::Result<()>> {
        Pin::new(&mut self.stdout).poll_read(cx, buf)
    }
}

impl AsyncWrite for ProxyCommandStream {
    fn poll_write(
        mut self: Pin<&mut Self>,
        cx: &mut Context<'_>,
        buf: &[u8],
    ) -> Poll<std::io::Result<usize>> {
        Pin::new(&mut self.stdin).poll_write(cx, buf)
    }

    fn poll_flush(mut self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<std::io::Result<()>> {
        Pin::new(&mut self.stdin).poll_flush(cx)
    }

    fn poll_shutdown(mut self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<std::io::Result<()>> {
        Pin::new(&mut self.stdin).poll_shutdown(cx)
    }
}

impl Drop for ProxyCommandStream {
    fn drop(&mut self) {
        let _ = self.child.start_kill();
    }
}

// ── Public API ──────────────────────────────────────────────────────
type SharedSession = Arc<client::Handle<SshHandler>>;
type SessionSlot = Arc<Mutex<Option<SharedSession>>>;

#[derive(Clone)]
pub struct SshSessionHandle {
    pub writer: Arc<Mutex<Box<dyn Write + Send>>>,
    pub resize_tx: tokio_mpsc::UnboundedSender<(u16, u16)>,
    session_handle: SessionSlot,
}

impl SshSessionHandle {
    /// Open a fresh SFTP subsystem channel on the active session.
    /// Errors out if the SSH session has not yet authenticated.
    pub async fn open_sftp(&self) -> Result<sftp::SftpHandle, String> {
        let cloned = self
            .session_handle
            .lock()
            .map_err(|_| "session slot poisoned".to_string())?
            .clone();
        let Some(handle) = cloned else {
            return Err("ssh session is not connected".into());
        };
        let mut channel = handle
            .channel_open_session()
            .await
            .map_err(|e| format!("open channel: {e}"))?;
        sftp::request_sftp(&mut channel).await?;
        sftp::spawn_worker(channel).await
    }
}

pub fn spawn_ssh_session(
    profile: SshProfile,
    tab_id: u64,
    rows: u16,
    cols: u16,
    output_tx: futures_mpsc::UnboundedSender<OutputEvent>,
) -> SshSessionHandle {
    let (initial_write_tx, _initial_write_rx) = tokio_mpsc::unbounded_channel::<Vec<u8>>();
    let (resize_tx, resize_rx) = tokio_mpsc::unbounded_channel::<(u16, u16)>();

    let writer: Arc<Mutex<Box<dyn Write + Send>>> = Arc::new(Mutex::new(Box::new(SshWriter {
        tx: initial_write_tx,
    })));
    let writer_handle = Arc::clone(&writer);
    let session_handle: SessionSlot = Arc::new(Mutex::new(None));
    let slot_for_task = Arc::clone(&session_handle);

    tokio::spawn(async move {
        let mut otx = output_tx;
        let badge = ssh_badge();
        let mut resize_rx = resize_rx;
        // Last remote working directory seen via OSC 7; restored on reconnect.
        let mut last_cwd: Option<String> = None;

        loop {
            let (attempt_write_tx, attempt_write_rx) = tokio_mpsc::unbounded_channel();

            if let Ok(mut guard) = writer_handle.lock() {
                *guard = Box::new(SshWriter {
                    tx: attempt_write_tx,
                });
            }

            let result = ssh_task(
                profile.clone(),
                tab_id,
                rows,
                cols,
                attempt_write_rx,
                &mut resize_rx,
                &mut otx,
                &slot_for_task,
                &mut last_cwd,
            )
            .await;

            if let Ok(mut guard) = slot_for_task.lock() {
                *guard = None;
            }

            let msg = match &result {
                Ok(()) => format!(
                    "\r\n  {badge}  {}\r\n  {badge}  {}\r\n",
                    ansi::yellow("Session disconnected."),
                    ansi::cyan("Press any key to reconnect...")
                ),
                Err(e) => format!(
                    "\r\n  {badge}  {}\r\n  {badge}  {}\r\n",
                    ansi::red_bold(&e.to_string()),
                    ansi::cyan("Press any key to reconnect...")
                ),
            };
            let _ = otx.unbounded_send(OutputEvent::Data {
                tab_id,
                bytes: msg.into_bytes(),
            });

            let (wait_tx, mut wait_rx) = tokio_mpsc::unbounded_channel();
            if let Ok(mut guard) = writer_handle.lock() {
                *guard = Box::new(SshWriter { tx: wait_tx });
            }

            if wait_rx.recv().await.is_none() {
                break;
            }
        }

        let _ = otx.unbounded_send(OutputEvent::Closed { tab_id });
    });

    SshSessionHandle {
        writer,
        resize_tx,
        session_handle,
    }
}

pub async fn test_ssh_connection(
    mut profile: SshProfile,
    timeout: std::time::Duration,
) -> Result<(), String> {
    match tokio::time::timeout(timeout, async move {
        test_ssh_connection_inner(&mut profile).await
    })
    .await
    {
        Ok(Ok(())) => Ok(()),
        Ok(Err(err)) => Err(err.to_string()),
        Err(_) => Err(format!(
            "Connection timed out after {} seconds.",
            timeout.as_secs()
        )),
    }
}

async fn test_ssh_connection_inner(
    profile: &mut SshProfile,
) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
    if matches!(profile.auth_method, SshAuthMethod::Password) && profile.password.is_none() {
        profile.password = crate::keychain::get_password(&profile.host, &profile.user);
    }

    let config = Arc::new(interactive_ssh_config());

    let (fp_tx, fp_rx) = tokio::sync::oneshot::channel();
    let handler = SshHandler {
        host: profile.host.clone(),
        port: profile.port,
        host_key_tx: Some(fp_tx),
    };

    let connected = if let Some(ref proxy_command) = profile.proxy_command {
        let stream = spawn_proxy_command(proxy_command, &profile.host, profile.port)?;
        client::connect_stream(config, stream, handler).await
    } else {
        let addr = format!("{}:{}", profile.host, profile.port);
        client::connect(config, &*addr, handler).await
    };

    let mut session = match connected {
        Ok(session) => session,
        Err(err) => {
            return Err(match host_key_rejection(fp_rx.await.ok()) {
                Some(reason) => reason.into(),
                None => err.into(),
            });
        }
    };

    let user = ssh_user(&profile.user);
    let authenticated = authenticate_session(&mut session, profile, &user).await?;

    if !authenticated {
        return Err("Authentication failed".into());
    }

    let _ = session
        .disconnect(Disconnect::ByApplication, "Connection test complete", "")
        .await;
    Ok(())
}

// ── Status message helper ───────────────────────────────────────────
fn send_status(output_tx: &mut futures_mpsc::UnboundedSender<OutputEvent>, tab_id: u64, msg: &str) {
    let _ = output_tx.unbounded_send(OutputEvent::Data {
        tab_id,
        bytes: msg.as_bytes().to_vec(),
    });
}

// ── Main SSH task ───────────────────────────────────────────────────
#[allow(clippy::too_many_arguments)]
async fn ssh_task(
    mut profile: SshProfile,
    tab_id: u64,
    rows: u16,
    cols: u16,
    mut write_rx: tokio_mpsc::UnboundedReceiver<Vec<u8>>,
    resize_rx: &mut tokio_mpsc::UnboundedReceiver<(u16, u16)>,
    output_tx: &mut futures_mpsc::UnboundedSender<OutputEvent>,
    session_slot: &SessionSlot,
    last_cwd: &mut Option<String>,
) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
    let badge = ssh_badge();

    // --- Status: Connecting ---
    let dest = if profile.user.is_empty() {
        profile.host.to_string()
    } else {
        format!("{}@{}", profile.user, profile.host)
    };
    let port_info = if profile.port != 22 {
        format!(":{}", profile.port)
    } else {
        String::new()
    };
    send_status(
        output_tx,
        tab_id,
        &format!(
            "\r\n  {badge}  {}\r\n",
            ansi::bold(&format!("Connecting to {dest}{port_info}"))
        ),
    );

    // Load password from OS keychain on demand (not at app startup)
    if matches!(profile.auth_method, SshAuthMethod::Password) && profile.password.is_none() {
        profile.password = crate::keychain::get_password(&profile.host, &profile.user);
    }

    // Auth method hint
    match profile.auth_method {
        SshAuthMethod::KeyFile => {
            if let Some(ref identity) = profile.identity_file {
                send_status(
                    output_tx,
                    tab_id,
                    &format!(
                        "         {}  {}\r\n",
                        ansi::cyan("Using private key from"),
                        ansi::bold_underline(identity)
                    ),
                );
            }
        }
        SshAuthMethod::Password => {
            if profile.password.is_some() {
                send_status(
                    output_tx,
                    tab_id,
                    &format!("         {}\r\n", ansi::cyan("Using saved password")),
                );
            }
        }
    }

    // --- TCP + SSH handshake ---
    let config = Arc::new(interactive_ssh_config());

    let (fp_tx, fp_rx) = tokio::sync::oneshot::channel();
    let handler = SshHandler {
        host: profile.host.clone(),
        port: profile.port,
        host_key_tx: Some(fp_tx),
    };

    let connect_timeout = std::time::Duration::from_secs(15);

    let connected = if let Some(ref proxy_command) = profile.proxy_command {
        send_status(
            output_tx,
            tab_id,
            &format!("         {}\r\n", ansi::cyan("Using ProxyCommand")),
        );
        let stream = spawn_proxy_command(proxy_command, &profile.host, profile.port)?;
        tokio::time::timeout(
            connect_timeout,
            client::connect_stream(config, stream, handler),
        )
        .await
    } else {
        let addr = format!("{}:{}", profile.host, profile.port);
        tokio::time::timeout(connect_timeout, client::connect(config, &*addr, handler)).await
    };

    let mut session = match connected {
        Ok(Ok(session)) => session,
        Ok(Err(err)) => {
            return Err(match host_key_rejection(fp_rx.await.ok()) {
                Some(reason) => reason.into(),
                None => err.into(),
            });
        }
        Err(_) => return Err("Connection timed out (15s).".into()),
    };

    if let Ok(info) = fp_rx.await {
        send_status(
            output_tx,
            tab_id,
            &format!("         {}\r\n", ansi::cyan("Host key fingerprint:")),
        );
        send_status(
            output_tx,
            tab_id,
            &format!("         {}\r\n", ansi::badge(&info.fingerprint)),
        );
        let note = match info.status {
            HostKeyStatus::Known => None,
            HostKeyStatus::Recorded => {
                Some("New host - recorded to ~/.ssh/known_hosts".to_string())
            }
            HostKeyStatus::RecordFailed(err) => {
                Some(format!("New host - could not record to known_hosts: {err}"))
            }
            HostKeyStatus::Changed { .. } | HostKeyStatus::CheckFailed(_) => None,
        };
        if let Some(note) = note {
            send_status(
                output_tx,
                tab_id,
                &format!("         {}\r\n", ansi::cyan(&note)),
            );
        }
    }

    // --- Authenticate ---
    send_status(
        output_tx,
        tab_id,
        &format!("  {badge}  {}\r\n", ansi::yellow("Authenticating...")),
    );

    let user = ssh_user(&profile.user);

    let auth_timeout = std::time::Duration::from_secs(15);
    let authenticated = match tokio::time::timeout(
        auth_timeout,
        authenticate_session(&mut session, &profile, &user),
    )
    .await
    {
        Ok(result) => result?,
        Err(_) => return Err("Authentication timed out (15s).".into()),
    };

    if !authenticated {
        return Err("Authentication failed".into());
    }

    let session = Arc::new(session);
    if let Ok(mut guard) = session_slot.lock() {
        *guard = Some(Arc::clone(&session));
    }

    // --- Connected ---
    send_status(
        output_tx,
        tab_id,
        &format!(
            "  {badge}  {}\r\n\r\n",
            ansi::green_bold("\u{2713} Connected!")
        ),
    );

    // --- Open channel with PTY + shell ---
    let mut channel = session.channel_open_session().await?;
    channel
        .request_pty(false, "xterm-256color", cols as u32, rows as u32, 0, 0, &[])
        .await?;
    channel.request_shell(false).await?;

    // On reconnect, return to the directory captured before the drop. The
    // leading space keeps it out of history where `ignorespace` is set.
    if let Some(dir) = last_cwd.as_deref() {
        let cmd = format!(" cd -- {}\r", shell_single_quote(dir));
        channel.data(cmd.as_bytes()).await?;
    }

    // --- I/O bridge ---
    loop {
        tokio::select! {
            msg = channel.wait() => {
                match msg {
                    Some(ChannelMsg::Data { data }) => {
                        if let Some(dir) = parse_osc7_cwd(&data) {
                            *last_cwd = Some(dir);
                        }
                        let _ = output_tx.unbounded_send(OutputEvent::Data {
                            tab_id,
                            bytes: data.to_vec(),
                        });
                    }
                    Some(ChannelMsg::Eof)
                    | Some(ChannelMsg::Close)
                    | Some(ChannelMsg::ExitStatus { .. })
                    | None => break,
                    _ => {}
                }
            }
            bytes = write_rx.recv() => {
                match bytes {
                    Some(bytes) => channel.data(&bytes[..]).await?,
                    None => break,
                }
            }
            resize = resize_rx.recv() => {
                match resize {
                    Some((r, c)) => channel.window_change(c as u32, r as u32, 0, 0).await?,
                    None => break,
                }
            }
        }
    }

    Ok(())
}

fn parse_osc7_cwd(bytes: &[u8]) -> Option<String> {
    const PREFIX: &[u8] = b"\x1b]7;";
    let mut latest = None;
    let mut base = 0;
    while base < bytes.len() {
        let Some(rel) = find_subslice(&bytes[base..], PREFIX) else {
            break;
        };
        let payload_start = base + rel + PREFIX.len();
        let Some(term) = find_osc_terminator(&bytes[payload_start..]) else {
            break; // incomplete; wait for more data
        };

        if let Some(path) = decode_osc7_payload(&bytes[payload_start..payload_start + term]) {
            latest = Some(path);
        }
        base = payload_start + term + 1;
    }
    latest
}

fn find_subslice(haystack: &[u8], needle: &[u8]) -> Option<usize> {
    if needle.is_empty() || haystack.len() < needle.len() {
        return None;
    }
    haystack.windows(needle.len()).position(|w| w == needle)
}

/// Offset of the OSC terminator (BEL, or ESC `\`) within `bytes`.
fn find_osc_terminator(bytes: &[u8]) -> Option<usize> {
    bytes
        .iter()
        .position(|&b| b == 0x07)
        .or_else(|| bytes.windows(2).position(|w| w == [0x1b, b'\\']))
}

/// Decodes an OSC 7 payload (`file://host/path`) into the absolute path.
fn decode_osc7_payload(payload: &[u8]) -> Option<String> {
    let text = std::str::from_utf8(payload).ok()?;
    let rest = text.strip_prefix("file://")?;
    let slash = rest.find('/')?;
    Some(percent_decode(&rest[slash..]))
}

fn percent_decode(s: &str) -> String {
    let bytes = s.as_bytes();
    let mut out = Vec::with_capacity(bytes.len());
    let mut i = 0;
    while i < bytes.len() {
        if bytes[i] == b'%'
            && i + 2 < bytes.len()
            && let (Some(hi), Some(lo)) = (hex_val(bytes[i + 1]), hex_val(bytes[i + 2]))
        {
            out.push(hi * 16 + lo);
            i += 3;
        } else {
            out.push(bytes[i]);
            i += 1;
        }
    }
    String::from_utf8_lossy(&out).into_owned()
}

fn hex_val(b: u8) -> Option<u8> {
    match b {
        b'0'..=b'9' => Some(b - b'0'),
        b'a'..=b'f' => Some(b - b'a' + 10),
        b'A'..=b'F' => Some(b - b'A' + 10),
        _ => None,
    }
}

/// Wraps a string in single quotes for safe use as one shell word.
fn shell_single_quote(s: &str) -> String {
    let mut out = String::with_capacity(s.len() + 2);
    out.push('\'');
    for ch in s.chars() {
        if ch == '\'' {
            out.push_str("'\\''");
        } else {
            out.push(ch);
        }
    }
    out.push('\'');
    out
}

fn ssh_user(configured_user: &str) -> String {
    if configured_user.is_empty() {
        std::env::var("USER")
            .or_else(|_| std::env::var("USERNAME"))
            .unwrap_or_else(|_| "root".into())
    } else {
        configured_user.to_string()
    }
}

fn interactive_ssh_config() -> client::Config {
    client::Config {
        inactivity_timeout: None,
        keepalive_interval: Some(std::time::Duration::from_secs(15)),
        keepalive_max: 3,
        ..<_>::default()
    }
}

async fn authenticate_session<H: client::Handler>(
    session: &mut client::Handle<H>,
    profile: &SshProfile,
    user: &str,
) -> Result<bool, Box<dyn std::error::Error + Send + Sync>> {
    Ok(match profile.auth_method {
        SshAuthMethod::KeyFile => {
            let Some(ref identity_path) = profile.identity_file else {
                return Err(
                    "Key file authentication selected but no key file is configured".into(),
                );
            };
            let expanded = if identity_path.starts_with("~/") {
                dirs::home_dir()
                    .map(|h| h.join(&identity_path[2..]).to_string_lossy().to_string())
                    .unwrap_or_else(|| identity_path.clone())
            } else {
                identity_path.clone()
            };
            let key_pair = load_secret_key(&expanded, None)?;
            session
                .authenticate_publickey(user, Arc::new(key_pair))
                .await?
        }
        SshAuthMethod::Password => {
            let Some(ref password) = profile.password else {
                return Err(
                    "Password authentication selected but no password is configured".into(),
                );
            };
            session.authenticate_password(user, password).await?
        }
    })
}

fn expand_proxy_command(command: &str, host: &str, port: u16) -> String {
    command.replace("%h", host).replace("%p", &port.to_string())
}

fn spawn_proxy_command(
    command: &str,
    host: &str,
    port: u16,
) -> Result<ProxyCommandStream, Box<dyn std::error::Error + Send + Sync>> {
    let command = expand_proxy_command(command, host, port);

    #[cfg(target_os = "windows")]
    let mut child = Command::new("cmd")
        .arg("/C")
        .arg(&command)
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::inherit())
        .spawn()?;

    #[cfg(not(target_os = "windows"))]
    let mut child = Command::new("sh")
        .arg("-c")
        .arg(&command)
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::inherit())
        .spawn()?;

    let stdin = child.stdin.take().ok_or("ProxyCommand stdin unavailable")?;
    let stdout = child
        .stdout
        .take()
        .ok_or("ProxyCommand stdout unavailable")?;

    Ok(ProxyCommandStream {
        child,
        stdout,
        stdin,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    const KEY_A: &str =
        "ssh-ed25519 AAAAC3NzaC1lZDI1NTE5AAAAIC4ciWsqk8eXCH9xnqpoj6bPqZoHijtF2ij2mSdUlZ+l";
    const KEY_B: &str =
        "ssh-ed25519 AAAAC3NzaC1lZDI1NTE5AAAAIJAEGFk+lTsD1tIUfUxpmCYcgkUqSfYoRuMDrvnybLjs";

    fn key(encoded: &str) -> ssh_key::PublicKey {
        encoded.parse().expect("test key should parse")
    }

    struct TempDir(std::path::PathBuf);

    impl TempDir {
        fn new(tag: &str) -> Self {
            let unique = format!(
                "rabbitty-{tag}-{}-{:?}",
                std::process::id(),
                std::thread::current().id()
            );
            let path = std::env::temp_dir().join(unique);
            let _ = std::fs::remove_dir_all(&path);
            std::fs::create_dir_all(&path).expect("temp dir");
            Self(path)
        }

        fn known_hosts(&self) -> std::path::PathBuf {
            self.0.join("known_hosts")
        }
    }

    impl Drop for TempDir {
        fn drop(&mut self) {
            let _ = std::fs::remove_dir_all(&self.0);
        }
    }

    #[test]
    fn unknown_host_is_recorded() {
        let dir = TempDir::new("unknown");
        let path = dir.known_hosts();
        assert!(!path.exists());

        let status = verify_host_key("example.com", 22, &key(KEY_A), &path);

        assert!(matches!(status, HostKeyStatus::Recorded), "{status:?}");
        assert!(status.accepts());
        let written = std::fs::read_to_string(&path).expect("known_hosts written");
        assert!(written.contains("example.com"), "{written}");
    }

    #[test]
    fn recorded_key_is_accepted_on_reconnect() {
        let dir = TempDir::new("known");
        let path = dir.known_hosts();

        verify_host_key("example.com", 22, &key(KEY_A), &path);
        let status = verify_host_key("example.com", 22, &key(KEY_A), &path);

        assert!(matches!(status, HostKeyStatus::Known), "{status:?}");
        assert!(status.accepts());
    }

    #[test]
    fn changed_key_is_rejected() {
        let dir = TempDir::new("changed");
        let path = dir.known_hosts();

        verify_host_key("example.com", 22, &key(KEY_A), &path);
        let status = verify_host_key("example.com", 22, &key(KEY_B), &path);

        assert!(
            matches!(status, HostKeyStatus::Changed { .. }),
            "{status:?}"
        );
        assert!(!status.accepts());
        assert!(
            host_key_rejection(Some(HostKeyInfo {
                fingerprint: String::new(),
                status,
            }))
            .is_some()
        );
    }

    #[test]
    fn non_default_port_is_tracked_separately() {
        let dir = TempDir::new("port");
        let path = dir.known_hosts();

        verify_host_key("example.com", 22, &key(KEY_A), &path);
        let status = verify_host_key("example.com", 2222, &key(KEY_B), &path);

        assert!(matches!(status, HostKeyStatus::Recorded), "{status:?}");
    }

    #[test]
    fn verifiable_outcomes_are_accepted_without_a_rejection_message() {
        for status in [
            HostKeyStatus::Known,
            HostKeyStatus::Recorded,
            HostKeyStatus::RecordFailed("disk full".into()),
        ] {
            assert!(status.accepts(), "{status:?}");
            assert!(
                host_key_rejection(Some(HostKeyInfo {
                    fingerprint: String::new(),
                    status,
                }))
                .is_none()
            );
        }
        assert!(host_key_rejection(None).is_none());
    }

    #[test]
    fn unverifiable_known_hosts_is_rejected() {
        let status = HostKeyStatus::CheckFailed("permission denied".into());
        assert!(!status.accepts());
        assert!(
            host_key_rejection(Some(HostKeyInfo {
                fingerprint: String::new(),
                status,
            }))
            .is_some()
        );
    }

    #[cfg(unix)]
    #[test]
    fn unreadable_known_hosts_does_not_pass_as_unknown_host() {
        use std::os::unix::fs::PermissionsExt;

        let dir = TempDir::new("unreadable");
        let path = dir.known_hosts();
        verify_host_key("example.com", 22, &key(KEY_A), &path);

        std::fs::set_permissions(&path, std::fs::Permissions::from_mode(0o000)).expect("chmod");
        if std::fs::File::open(&path).is_ok() {
            return; // running as root; permissions are not enforced
        }

        let status = verify_host_key("example.com", 22, &key(KEY_A), &path);

        assert!(
            matches!(status, HostKeyStatus::CheckFailed(_)),
            "{status:?}"
        );
        assert!(!status.accepts());

        let _ = std::fs::set_permissions(&path, std::fs::Permissions::from_mode(0o600));
    }

    #[test]
    fn interactive_ssh_config_does_not_close_idle_sessions() {
        let config = interactive_ssh_config();

        assert_eq!(config.inactivity_timeout, None);
        assert_eq!(
            config.keepalive_interval,
            Some(std::time::Duration::from_secs(15))
        );
        assert_eq!(config.keepalive_max, 3);
    }

    #[test]
    fn proxy_command_replaces_host_and_port_tokens() {
        let command = expand_proxy_command(
            "cloudflared access ssh --hostname %h --url localhost:%p",
            "myyrakle-remote.chainshift.co",
            2222,
        );

        assert_eq!(
            command,
            "cloudflared access ssh --hostname myyrakle-remote.chainshift.co --url localhost:2222"
        );
    }

    #[test]
    fn osc7_parses_path_with_bel_and_st_terminators() {
        let bel = b"prompt\x1b]7;file://host/home/user/proj\x07$ ";
        assert_eq!(parse_osc7_cwd(bel), Some("/home/user/proj".to_string()));

        let st = b"\x1b]7;file://host/var/log\x1b\\";
        assert_eq!(parse_osc7_cwd(st), Some("/var/log".to_string()));
    }

    #[test]
    fn osc7_percent_decodes_and_keeps_latest() {
        let bytes = b"\x1b]7;file://h/tmp\x07\x1b]7;file://h/a%20b/c\x07";
        assert_eq!(parse_osc7_cwd(bytes), Some("/a b/c".to_string()));
    }

    #[test]
    fn osc7_ignores_absent_or_incomplete_sequences() {
        assert_eq!(parse_osc7_cwd(b"no escape here"), None);
        assert_eq!(parse_osc7_cwd(b"\x1b]7;file://host/partial"), None);
    }

    #[test]
    fn shell_single_quote_escapes_quotes() {
        assert_eq!(shell_single_quote("/a/b"), "'/a/b'");
        assert_eq!(shell_single_quote("/it's/here"), "'/it'\\''s/here'");
    }
}

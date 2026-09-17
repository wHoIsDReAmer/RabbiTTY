mod ansi;
mod auth;
mod cwd;
mod host_key;
mod proxy;
pub mod sftp;
pub mod user_config;

use crate::config::{SshAuthMethod, SshProfile};
use crate::session::OutputEvent;
use async_trait::async_trait;
use auth::authenticate_session;
use cwd::{parse_osc7_cwd, shell_single_quote};
pub use host_key::{HostKeyInfo, HostKeyStatus};
use host_key::{host_key_rejection, known_hosts_path, verify_host_key};
use iced::futures::channel::mpsc as futures_mpsc;
use proxy::spawn_proxy_command;
use russh::keys::*;
use russh::*;
use std::io::Write;
use std::sync::{Arc, Mutex};
use tokio::sync::mpsc as tokio_mpsc;

fn ssh_badge() -> String {
    ansi::badge("SSH")
}

// ── Host key verification ───────────────────────────────────────────

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

            // Ordered before the status text so it lands on the main screen.
            let _ = otx.unbounded_send(OutputEvent::Disconnected { tab_id });

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
        SshAuthMethod::Agent => {
            send_status(
                output_tx,
                tab_id,
                &format!("         {}\r\n", ansi::cyan("Using SSH agent")),
            );
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

#[cfg(test)]
mod tests {
    use super::*;

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
}

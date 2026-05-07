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

use crate::ansi;

fn ssh_badge() -> String {
    ansi::badge("SSH")
}

// ── SSH client handler ──────────────────────────────────────────────
struct SshHandler {
    fingerprint_tx: Option<tokio::sync::oneshot::Sender<String>>,
}

#[async_trait]
impl client::Handler for SshHandler {
    type Error = russh::Error;

    async fn check_server_key(
        &mut self,
        server_public_key: &ssh_key::PublicKey,
    ) -> Result<bool, Self::Error> {
        if let Some(tx) = self.fingerprint_tx.take() {
            let fp = server_public_key
                .fingerprint(ssh_key::HashAlg::Sha256)
                .to_string();
            let _ = tx.send(fp);
        }
        // TODO: proper host key verification against known_hosts
        Ok(true)
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
pub struct SshSessionHandle {
    pub writer: Arc<Mutex<Box<dyn Write + Send>>>,
    pub resize_tx: tokio_mpsc::UnboundedSender<(u16, u16)>,
}

pub fn spawn_ssh_session(
    profile: SshProfile,
    tab_id: u64,
    rows: u16,
    cols: u16,
    output_tx: futures_mpsc::UnboundedSender<OutputEvent>,
) -> SshSessionHandle {
    let (initial_write_tx, _) = tokio_mpsc::unbounded_channel::<Vec<u8>>();
    let (resize_tx, _) = tokio_mpsc::unbounded_channel::<(u16, u16)>();

    let writer: Arc<Mutex<Box<dyn Write + Send>>> = Arc::new(Mutex::new(Box::new(SshWriter {
        tx: initial_write_tx,
    })));
    let writer_handle = Arc::clone(&writer);

    tokio::spawn(async move {
        let mut otx = output_tx;
        let badge = ssh_badge();

        loop {
            let (attempt_write_tx, attempt_write_rx) = tokio_mpsc::unbounded_channel();
            let (_, attempt_resize_rx) = tokio_mpsc::unbounded_channel();

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
                attempt_resize_rx,
                &mut otx,
            )
            .await;

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

    SshSessionHandle { writer, resize_tx }
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

    let (fp_tx, _fp_rx) = tokio::sync::oneshot::channel();
    let handler = SshHandler {
        fingerprint_tx: Some(fp_tx),
    };

    let mut session = if let Some(ref proxy_command) = profile.proxy_command {
        let stream = spawn_proxy_command(proxy_command, &profile.host, profile.port)?;
        client::connect_stream(config, stream, handler).await?
    } else {
        let addr = format!("{}:{}", profile.host, profile.port);
        client::connect(config, &*addr, handler).await?
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
async fn ssh_task(
    mut profile: SshProfile,
    tab_id: u64,
    rows: u16,
    cols: u16,
    mut write_rx: tokio_mpsc::UnboundedReceiver<Vec<u8>>,
    mut resize_rx: tokio_mpsc::UnboundedReceiver<(u16, u16)>,
    output_tx: &mut futures_mpsc::UnboundedSender<OutputEvent>,
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
        fingerprint_tx: Some(fp_tx),
    };

    let connect_timeout = std::time::Duration::from_secs(15);

    let mut session = if let Some(ref proxy_command) = profile.proxy_command {
        send_status(
            output_tx,
            tab_id,
            &format!("         {}\r\n", ansi::cyan("Using ProxyCommand")),
        );
        let stream = spawn_proxy_command(proxy_command, &profile.host, profile.port)?;
        match tokio::time::timeout(
            connect_timeout,
            client::connect_stream(config, stream, handler),
        )
        .await
        {
            Ok(result) => result?,
            Err(_) => return Err("Connection timed out (15s).".into()),
        }
    } else {
        let addr = format!("{}:{}", profile.host, profile.port);
        match tokio::time::timeout(connect_timeout, client::connect(config, &*addr, handler)).await
        {
            Ok(result) => result?,
            Err(_) => return Err("Connection timed out (15s).".into()),
        }
    };

    // Display host key fingerprint
    if let Ok(fp) = fp_rx.await {
        send_status(
            output_tx,
            tab_id,
            &format!("         {}\r\n", ansi::cyan("Host key fingerprint:")),
        );
        send_status(
            output_tx,
            tab_id,
            &format!("         {}\r\n", ansi::badge(&fp)),
        );
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

    // --- I/O bridge ---
    loop {
        tokio::select! {
            msg = channel.wait() => {
                match msg {
                    Some(ChannelMsg::Data { data }) => {
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
}

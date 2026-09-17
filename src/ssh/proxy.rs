use std::pin::Pin;
use std::process::Stdio;
use std::task::{Context, Poll};
use tokio::io::{AsyncRead, AsyncWrite, ReadBuf};
use tokio::process::{Child, ChildStdin, ChildStdout, Command};

pub(super) struct ProxyCommandStream {
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

fn expand_proxy_command(command: &str, host: &str, port: u16) -> String {
    command.replace("%h", host).replace("%p", &port.to_string())
}

pub(super) fn spawn_proxy_command(
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

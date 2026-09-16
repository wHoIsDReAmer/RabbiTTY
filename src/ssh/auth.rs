use crate::config::{SshAuthMethod, SshProfile};
use russh::keys::*;
use russh::*;
use std::sync::Arc;

pub(super) async fn authenticate_session<H: client::Handler>(
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
            let Some(password) = &profile.password else {
                return Err(
                    "Password authentication selected but no password is configured".into(),
                );
            };
            session.authenticate_password(user, password).await?
        }
        SshAuthMethod::Agent => authenticate_with_agent(session, user).await?,
    })
}

async fn authenticate_with_agent<H: client::Handler>(
    session: &mut client::Handle<H>,
    user: &str,
) -> Result<bool, Box<dyn std::error::Error + Send + Sync>> {
    let Some(socket) = agent_socket() else {
        return Err("SSH agent authentication selected but no agent socket was found".into());
    };
    let mut agent = connect_agent(&socket).await.map_err(|err| {
        format!(
            "Could not reach the SSH agent at {}: {err}",
            socket.display()
        )
    })?;
    let identities = agent.request_identities().await?;
    if identities.is_empty() {
        return Err("The SSH agent holds no identities".into());
    }

    let mut offered = 0;
    for key in identities {
        offered += 1;
        if session
            .authenticate_publickey_with(user, key, &mut agent)
            .await?
        {
            return Ok(true);
        }
    }
    Err(
        format!("The server accepted none of the {offered} identities the SSH agent offered")
            .into(),
    )
}

#[cfg(unix)]
type AgentClient = agent::client::AgentClient<tokio::net::UnixStream>;
#[cfg(windows)]
type AgentClient = agent::client::AgentClient<tokio::net::windows::named_pipe::NamedPipeClient>;

#[cfg(unix)]
async fn connect_agent(
    socket: &std::path::Path,
) -> Result<AgentClient, Box<dyn std::error::Error + Send + Sync>> {
    Ok(agent::client::AgentClient::connect_uds(socket).await?)
}

#[cfg(windows)]
async fn connect_agent(
    socket: &std::path::Path,
) -> Result<AgentClient, Box<dyn std::error::Error + Send + Sync>> {
    let pipe = tokio::net::windows::named_pipe::ClientOptions::new().open(socket)?;
    Ok(agent::client::AgentClient::connect(pipe))
}

const WINDOWS_AGENT_PIPE: &str = r"\\.\pipe\openssh-ssh-agent";

fn agent_socket() -> Option<std::path::PathBuf> {
    if let Some(path) = std::env::var_os("SSH_AUTH_SOCK").filter(|p| !p.is_empty()) {
        return Some(path.into());
    }
    if cfg!(target_os = "macos") {
        let out = std::process::Command::new("launchctl")
            .args(["getenv", "SSH_AUTH_SOCK"])
            .output()
            .ok()?;
        let path = String::from_utf8_lossy(&out.stdout).trim().to_string();
        if !path.is_empty() {
            return Some(path.into());
        }
    }
    if cfg!(windows) {
        return Some(WINDOWS_AGENT_PIPE.into());
    }
    None
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn an_empty_ssh_auth_sock_does_not_count_as_a_socket() {
        let saved = std::env::var_os("SSH_AUTH_SOCK");
        unsafe { std::env::set_var("SSH_AUTH_SOCK", "/tmp/rabbitty-test-agent.sock") };
        assert_eq!(
            agent_socket().as_deref(),
            Some(std::path::Path::new("/tmp/rabbitty-test-agent.sock"))
        );
        unsafe { std::env::set_var("SSH_AUTH_SOCK", "") };
        let fallback = agent_socket();
        assert!(fallback.is_none_or(|p| !p.as_os_str().is_empty()));
        match saved {
            Some(v) => unsafe { std::env::set_var("SSH_AUTH_SOCK", v) },
            None => unsafe { std::env::remove_var("SSH_AUTH_SOCK") },
        }
    }
}

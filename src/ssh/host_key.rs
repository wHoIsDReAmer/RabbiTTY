use russh::keys::*;

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
    pub(super) fn accepts(&self) -> bool {
        !matches!(self, Self::Changed { .. } | Self::CheckFailed(_))
    }
}

pub(super) fn known_hosts_path() -> Option<std::path::PathBuf> {
    Some(dirs::home_dir()?.join(".ssh").join("known_hosts"))
}

pub(super) fn verify_host_key(
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

pub(super) fn host_key_rejection(info: Option<HostKeyInfo>) -> Option<String> {
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
}

//! SFTP worker over a russh `sftp` subsystem channel.
#![allow(dead_code)]

use std::collections::HashMap;
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};

use iced::futures::StreamExt;
use iced::futures::channel::mpsc;
use iced::futures::stream::FuturesUnordered;
use russh::client::Msg;
use russh_sftp::client::SftpSession;
use russh_sftp::protocol::FileType;
use std::path::PathBuf;
use tokio::io::{AsyncReadExt, AsyncWriteExt};

const TRANSFER_CHUNK: usize = 32 * 1024;

#[derive(Debug, Clone)]
pub enum Command {
    List(String),
    Mkdir(String),
    Rename { from: String, to: String },
    Delete { path: String, is_dir: bool },
    Upload { local: PathBuf, remote: String },
    Download { remote: String, local: PathBuf },
    Cancel { path: String },
}

#[derive(Debug, Clone)]
pub struct Entry {
    pub name: String,
    pub size: u64,
    pub mtime: Option<i64>,
    pub mode: Option<u32>,
    pub is_dir: bool,
    pub is_symlink: bool,
}

#[derive(Debug, Clone)]
pub enum Event {
    Listed {
        path: String,
        entries: Vec<Entry>,
    },
    TransferStarted {
        path: String,
        total: u64,
    },
    TransferProgress {
        path: String,
        transferred: u64,
        total: u64,
    },
    TransferEnded {
        path: String,
        outcome: Outcome,
    },
    Mutated {
        path: String,
    },
    Error {
        message: String,
    },
    Closed,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Outcome {
    Done,
    Cancelled,
    Failed(String),
}

pub struct SftpHandle {
    pub tx: mpsc::UnboundedSender<Command>,
    pub rx: mpsc::UnboundedReceiver<Event>,
}

pub async fn spawn_worker(channel: russh::Channel<Msg>) -> Result<SftpHandle, String> {
    let (cmd_tx, cmd_rx) = mpsc::unbounded::<Command>();
    let (evt_tx, evt_rx) = mpsc::unbounded::<Event>();

    let sftp = SftpSession::new(channel.into_stream())
        .await
        .map_err(|e| format!("failed to start SFTP session: {e}"))?;

    tokio::spawn(run_worker(sftp, cmd_rx, evt_tx));

    Ok(SftpHandle {
        tx: cmd_tx,
        rx: evt_rx,
    })
}

async fn run_worker(
    sftp: SftpSession,
    mut cmd_rx: mpsc::UnboundedReceiver<Command>,
    evt_tx: mpsc::UnboundedSender<Event>,
) {
    let mut tokens: HashMap<String, Arc<AtomicBool>> = HashMap::new();
    let mut running = FuturesUnordered::new();

    loop {
        let cmd = tokio::select! {
            cmd = cmd_rx.next() => match cmd {
                Some(cmd) => cmd,
                None => break,
            },
            Some(done) = running.next(), if !running.is_empty() => {
                tokens.remove(&done);
                continue;
            }
        };

        match cmd {
            Command::Cancel { path } => {
                if let Some(token) = tokens.get(&path) {
                    token.store(true, Ordering::SeqCst);
                }
            }
            Command::List(path) => match list_dir(&sftp, &path).await {
                Ok(entries) => {
                    let _ = evt_tx.unbounded_send(Event::Listed {
                        path: path.clone(),
                        entries,
                    });
                }
                Err(e) => {
                    let _ = evt_tx.unbounded_send(Event::Error {
                        message: format!("list {path}: {e}"),
                    });
                }
            },
            Command::Mkdir(path) => match sftp.create_dir(&path).await {
                Ok(()) => {
                    let _ = evt_tx.unbounded_send(Event::Mutated { path });
                }
                Err(e) => {
                    let _ = evt_tx.unbounded_send(Event::Error {
                        message: format!("mkdir {path}: {e}"),
                    });
                }
            },
            Command::Rename { from, to } => match sftp.rename(&from, &to).await {
                Ok(()) => {
                    let _ = evt_tx.unbounded_send(Event::Mutated { path: to });
                }
                Err(e) => {
                    let _ = evt_tx.unbounded_send(Event::Error {
                        message: format!("rename {from} -> {to}: {e}"),
                    });
                }
            },
            Command::Delete { path, is_dir } => {
                let result = if is_dir {
                    sftp.remove_dir(&path).await
                } else {
                    sftp.remove_file(&path).await
                };
                match result {
                    Ok(()) => {
                        let _ = evt_tx.unbounded_send(Event::Mutated { path });
                    }
                    Err(e) => {
                        let _ = evt_tx.unbounded_send(Event::Error {
                            message: format!("delete {path}: {e}"),
                        });
                    }
                }
            }
            Command::Upload { local, remote } => {
                let token = arm_token(&mut tokens, &remote);
                running.push(run_transfer(
                    &sftp,
                    Job::Upload { local, remote },
                    &evt_tx,
                    token,
                ));
            }
            Command::Download { remote, local } => {
                let token = arm_token(&mut tokens, &remote);
                running.push(run_transfer(
                    &sftp,
                    Job::Download { remote, local },
                    &evt_tx,
                    token,
                ));
            }
        }
    }

    let _ = evt_tx.unbounded_send(Event::Closed);
}

/// A queued cancel must not leak into the next transfer of the same path.
fn arm_token(tokens: &mut HashMap<String, Arc<AtomicBool>>, path: &str) -> Arc<AtomicBool> {
    let token = Arc::new(AtomicBool::new(false));
    tokens.insert(path.to_string(), Arc::clone(&token));
    token
}

enum Job {
    Upload { local: PathBuf, remote: String },
    Download { remote: String, local: PathBuf },
}

async fn run_transfer(
    sftp: &SftpSession,
    job: Job,
    evt_tx: &mpsc::UnboundedSender<Event>,
    token: Arc<AtomicBool>,
) -> String {
    let (path, result) = match job {
        Job::Upload { local, remote } => {
            let result = upload(sftp, &local, &remote, evt_tx, &token).await;
            (remote, result)
        }
        Job::Download { remote, local } => {
            let result = download(sftp, &remote, &local, evt_tx, &token).await;
            (remote, result)
        }
    };

    let outcome = match result {
        Ok(outcome) => outcome,
        Err(message) => Outcome::Failed(message),
    };
    let _ = evt_tx.unbounded_send(Event::TransferEnded {
        path: path.clone(),
        outcome,
    });
    path
}

async fn list_dir(sftp: &SftpSession, path: &str) -> Result<Vec<Entry>, String> {
    let dir = sftp.read_dir(path).await.map_err(|e| e.to_string())?;
    let mut out = Vec::new();
    for entry in dir {
        let metadata = entry.metadata();
        let is_dir = matches!(metadata.file_type(), FileType::Dir);
        let is_symlink = matches!(metadata.file_type(), FileType::Symlink);
        out.push(Entry {
            name: entry.file_name(),
            size: metadata.size.unwrap_or(0),
            mtime: metadata.mtime.map(|v| v as i64),
            mode: metadata.permissions,
            is_dir,
            is_symlink,
        });
    }
    out.sort_by(|a, b| match (a.is_dir, b.is_dir) {
        (true, false) => std::cmp::Ordering::Less,
        (false, true) => std::cmp::Ordering::Greater,
        _ => a
            .name
            .to_ascii_lowercase()
            .cmp(&b.name.to_ascii_lowercase()),
    });
    Ok(out)
}

async fn upload(
    sftp: &SftpSession,
    local: &std::path::Path,
    remote: &str,
    evt_tx: &mpsc::UnboundedSender<Event>,
    cancelled: &AtomicBool,
) -> Result<Outcome, String> {
    let mut local_file = tokio::fs::File::open(local)
        .await
        .map_err(|e| format!("open local: {e}"))?;
    let total = local_file.metadata().await.map(|m| m.len()).unwrap_or(0);

    let mut remote_file = sftp
        .create(remote)
        .await
        .map_err(|e| format!("create remote: {e}"))?;

    let _ = evt_tx.unbounded_send(Event::TransferStarted {
        path: remote.to_string(),
        total,
    });

    let mut buf = vec![0u8; TRANSFER_CHUNK];
    let mut transferred = 0u64;
    loop {
        if cancelled.load(Ordering::SeqCst) {
            drop(remote_file);
            let _ = sftp.remove_file(remote).await;
            return Ok(Outcome::Cancelled);
        }
        let n = local_file
            .read(&mut buf)
            .await
            .map_err(|e| format!("read local: {e}"))?;
        if n == 0 {
            break;
        }
        remote_file
            .write_all(&buf[..n])
            .await
            .map_err(|e| format!("write remote: {e}"))?;
        transferred += n as u64;
        let _ = evt_tx.unbounded_send(Event::TransferProgress {
            path: remote.to_string(),
            transferred,
            total,
        });
    }
    remote_file
        .shutdown()
        .await
        .map_err(|e| format!("close remote: {e}"))?;

    Ok(Outcome::Done)
}

async fn download(
    sftp: &SftpSession,
    remote: &str,
    local: &std::path::Path,
    evt_tx: &mpsc::UnboundedSender<Event>,
    cancelled: &AtomicBool,
) -> Result<Outcome, String> {
    let metadata = sftp
        .metadata(remote)
        .await
        .map_err(|e| format!("stat remote: {e}"))?;
    let total = metadata.size.unwrap_or(0);

    let mut remote_file = sftp
        .open(remote)
        .await
        .map_err(|e| format!("open remote: {e}"))?;
    let mut local_file = tokio::fs::File::create(local)
        .await
        .map_err(|e| format!("create local: {e}"))?;

    let _ = evt_tx.unbounded_send(Event::TransferStarted {
        path: remote.to_string(),
        total,
    });

    let mut buf = vec![0u8; TRANSFER_CHUNK];
    let mut transferred = 0u64;
    loop {
        if cancelled.load(Ordering::SeqCst) {
            drop(local_file);
            let _ = tokio::fs::remove_file(local).await;
            return Ok(Outcome::Cancelled);
        }
        let n = remote_file
            .read(&mut buf)
            .await
            .map_err(|e| format!("read remote: {e}"))?;
        if n == 0 {
            break;
        }
        local_file
            .write_all(&buf[..n])
            .await
            .map_err(|e| format!("write local: {e}"))?;
        transferred += n as u64;
        let _ = evt_tx.unbounded_send(Event::TransferProgress {
            path: remote.to_string(),
            transferred,
            total,
        });
    }
    local_file
        .flush()
        .await
        .map_err(|e| format!("flush local: {e}"))?;

    Ok(Outcome::Done)
}

pub async fn request_sftp(channel: &mut russh::Channel<Msg>) -> Result<(), String> {
    channel
        .request_subsystem(true, "sftp")
        .await
        .map_err(|e| format!("request sftp subsystem: {e}"))?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn entry_sort_dirs_first_then_alpha() {
        fn entry(name: &str, is_dir: bool) -> Entry {
            Entry {
                name: name.into(),
                size: 0,
                mtime: None,
                mode: None,
                is_dir,
                is_symlink: false,
            }
        }
        let mut entries = [
            entry("zfile", false),
            entry("Bdir", true),
            entry("afile", false),
            entry("adir", true),
        ];
        entries.sort_by(|a, b| match (a.is_dir, b.is_dir) {
            (true, false) => std::cmp::Ordering::Less,
            (false, true) => std::cmp::Ordering::Greater,
            _ => a
                .name
                .to_ascii_lowercase()
                .cmp(&b.name.to_ascii_lowercase()),
        });
        let names: Vec<&str> = entries.iter().map(|e| e.name.as_str()).collect();
        assert_eq!(names, ["adir", "Bdir", "afile", "zfile"]);
    }

    use russh_sftp::protocol::{
        Attrs, Data, File as NameFile, FileAttributes, Handle, Name, OpenFlags, Status, StatusCode,
    };

    const FILE_SIZE: u64 = (TRANSFER_CHUNK * 64) as u64;

    /// Serves one file of `FILE_SIZE` and a directory holding one entry.
    /// Requests are answered one at a time, as a real server does.
    #[derive(Default)]
    struct FakeServer {
        drained: std::collections::HashSet<String>,
    }

    fn ok(id: u32) -> Status {
        Status {
            id,
            status_code: StatusCode::Ok,
            error_message: String::new(),
            language_tag: "en-US".to_string(),
        }
    }

    impl russh_sftp::server::Handler for FakeServer {
        type Error = StatusCode;

        fn unimplemented(&self) -> StatusCode {
            StatusCode::OpUnsupported
        }

        async fn open(
            &mut self,
            id: u32,
            filename: String,
            _pflags: OpenFlags,
            _attrs: FileAttributes,
        ) -> Result<Handle, StatusCode> {
            Ok(Handle {
                id,
                handle: filename,
            })
        }

        async fn close(&mut self, id: u32, _handle: String) -> Result<Status, StatusCode> {
            Ok(ok(id))
        }

        async fn stat(&mut self, id: u32, _path: String) -> Result<Attrs, StatusCode> {
            Ok(Attrs {
                id,
                attrs: FileAttributes {
                    size: Some(FILE_SIZE),
                    permissions: Some(0o100_644),
                    ..Default::default()
                },
            })
        }

        async fn read(
            &mut self,
            id: u32,
            _handle: String,
            offset: u64,
            len: u32,
        ) -> Result<Data, StatusCode> {
            if offset >= FILE_SIZE {
                return Err(StatusCode::Eof);
            }
            let n = (FILE_SIZE - offset).min(len as u64) as usize;
            Ok(Data {
                id,
                data: vec![7u8; n],
            })
        }

        async fn opendir(&mut self, id: u32, path: String) -> Result<Handle, StatusCode> {
            Ok(Handle { id, handle: path })
        }

        async fn readdir(&mut self, id: u32, handle: String) -> Result<Name, StatusCode> {
            if !self.drained.insert(handle) {
                return Err(StatusCode::Eof);
            }
            Ok(Name {
                id,
                files: vec![NameFile {
                    filename: "note.txt".to_string(),
                    longname: "-rw-r--r-- 1 me me 3 Jan 1 00:00 note.txt".to_string(),
                    attrs: FileAttributes {
                        size: Some(3),
                        permissions: Some(0o100_644),
                        ..Default::default()
                    },
                }],
            })
        }
    }

    struct Harness {
        handle: SftpHandle,
        dir: PathBuf,
    }

    async fn harness(tag: &str) -> Harness {
        let (client, server) = tokio::io::duplex(256 * 1024);
        russh_sftp::server::run(server, FakeServer::default()).await;

        let sftp = SftpSession::new(client).await.expect("client session");
        let (cmd_tx, cmd_rx) = mpsc::unbounded::<Command>();
        let (evt_tx, evt_rx) = mpsc::unbounded::<Event>();
        tokio::spawn(run_worker(sftp, cmd_rx, evt_tx));

        let dir = std::env::temp_dir().join(format!("rabbitty-sftp-{tag}-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).expect("temp dir");

        Harness {
            handle: SftpHandle {
                tx: cmd_tx,
                rx: evt_rx,
            },
            dir,
        }
    }

    impl Drop for Harness {
        fn drop(&mut self) {
            let _ = std::fs::remove_dir_all(&self.dir);
        }
    }

    async fn next_event(rx: &mut mpsc::UnboundedReceiver<Event>) -> Event {
        tokio::time::timeout(std::time::Duration::from_secs(5), rx.next())
            .await
            .expect("worker went quiet")
            .expect("worker closed")
    }

    #[tokio::test]
    async fn a_cancel_stops_a_transfer_that_is_already_running() {
        let mut h = harness("cancel").await;
        let local = h.dir.join("copy.bin");

        h.handle
            .tx
            .unbounded_send(Command::Download {
                remote: "/remote/big.bin".to_string(),
                local: local.clone(),
            })
            .expect("queued");

        loop {
            if let Event::TransferProgress { .. } = next_event(&mut h.handle.rx).await {
                break;
            }
        }
        h.handle
            .tx
            .unbounded_send(Command::Cancel {
                path: "/remote/big.bin".to_string(),
            })
            .expect("queued");

        let mut transferred = 0;
        let outcome = loop {
            match next_event(&mut h.handle.rx).await {
                Event::TransferProgress { transferred: n, .. } => transferred = n,
                Event::TransferEnded { outcome, .. } => break outcome,
                _ => {}
            }
        };

        assert_eq!(outcome, Outcome::Cancelled);
        assert!(
            transferred < FILE_SIZE,
            "it should have stopped short of {FILE_SIZE}, got {transferred}"
        );
        assert!(
            !local.exists(),
            "a cancelled download must not leave a partial file behind"
        );
    }

    #[tokio::test]
    async fn the_drawer_can_still_browse_while_a_transfer_runs() {
        let mut h = harness("browse").await;

        h.handle
            .tx
            .unbounded_send(Command::Download {
                remote: "/remote/big.bin".to_string(),
                local: h.dir.join("copy.bin"),
            })
            .expect("queued");
        loop {
            if let Event::TransferProgress { .. } = next_event(&mut h.handle.rx).await {
                break;
            }
        }

        h.handle
            .tx
            .unbounded_send(Command::List("/remote".to_string()))
            .expect("queued");

        let listed = loop {
            match next_event(&mut h.handle.rx).await {
                Event::Listed { entries, .. } => break entries,
                Event::TransferEnded { .. } => panic!("the transfer finished before the listing"),
                _ => {}
            }
        };
        assert_eq!(listed.len(), 1);
        assert_eq!(listed[0].name, "note.txt");
    }

    #[tokio::test]
    async fn a_cancel_does_not_carry_over_into_the_next_transfer() {
        let mut h = harness("carryover").await;

        h.handle
            .tx
            .unbounded_send(Command::Cancel {
                path: "/remote/big.bin".to_string(),
            })
            .expect("queued");
        h.handle
            .tx
            .unbounded_send(Command::Download {
                remote: "/remote/big.bin".to_string(),
                local: h.dir.join("copy.bin"),
            })
            .expect("queued");

        let outcome = loop {
            if let Event::TransferEnded { outcome, .. } = next_event(&mut h.handle.rx).await {
                break outcome;
            }
        };
        assert_eq!(outcome, Outcome::Done);
    }
}

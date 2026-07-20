//! SFTP drawer wiring.

use super::super::{App, Message, SETTINGS_TAB_INDEX, SftpMessage};
use crate::gui::sftp::{SftpDrawerState, TransferRow};
use crate::gui::tab::ShellKind;
use crate::ssh::SshSessionHandle;
use crate::ssh::sftp;
use iced::Task;
use iced::futures::StreamExt;
use iced::futures::channel::mpsc;
use iced::futures::stream;
use iced::time::Instant;

impl App {
    pub(super) fn update_sftp(&mut self, message: SftpMessage) -> Task<Message> {
        match message {
            SftpMessage::ToggleDrawer => {
                if self.active_tab != SETTINGS_TAB_INDEX
                    && let Some(tab) = self.tabs.get_mut(self.active_tab)
                    && matches!(tab.shell, ShellKind::Ssh(_))
                {
                    let was_open = tab.sftp.open;
                    tab.sftp.anim.go_mut(!was_open, Instant::now());
                    if !was_open {
                        tab.sftp.open = true;
                    }
                    if !was_open
                        && tab.sftp.command_tx.is_none()
                        && !tab.sftp.opening
                        && let crate::gui::tab::TerminalSession::Active(session) = &tab.session
                        && let Some(ssh) = session.ssh_handle()
                    {
                        tab.sftp.opening = true;
                        tab.sftp.error = None;
                        let tab_id = tab.id;
                        return open_sftp_stream(ssh.clone(), tab_id);
                    }
                }
            }
            SftpMessage::OpenSucceeded { tab_id, command_tx } => {
                if let Some(tab) = self.tabs.iter_mut().find(|t| t.id == tab_id) {
                    tab.sftp.opening = false;
                    tab.sftp.command_tx = Some(command_tx.clone());
                    tab.sftp.loading = true;
                    tab.sftp.error = None;
                    let path = tab.sftp.current_path.clone();
                    let _ = command_tx.unbounded_send(sftp::Command::List(path));
                }
            }
            SftpMessage::OpenFailed { tab_id, error } => {
                if let Some(tab) = self.tabs.iter_mut().find(|t| t.id == tab_id) {
                    tab.sftp.opening = false;
                    tab.sftp.error = Some(error);
                }
            }
            SftpMessage::Event { tab_id, event } => {
                let finished_path = if let sftp::Event::TransferFinished { path } = &event {
                    Some(path.clone())
                } else {
                    None
                };
                if let Some(tab) = self.tabs.iter_mut().find(|t| t.id == tab_id) {
                    apply_sftp_event(&mut tab.sftp, event);
                }
                if let Some(path) = finished_path {
                    return Task::perform(
                        async {
                            std::thread::sleep(std::time::Duration::from_millis(1500));
                        },
                        move |()| {
                            Message::Sftp(SftpMessage::DismissTransfer {
                                tab_id,
                                path: path.clone(),
                            })
                        },
                    );
                }
            }
            SftpMessage::DismissTransfer { tab_id, path } => {
                if let Some(tab) = self.tabs.iter_mut().find(|t| t.id == tab_id) {
                    tab.sftp
                        .transfers
                        .retain(|t| !(t.finished && t.path == path));
                }
            }
            SftpMessage::Navigate { tab_id, path } => {
                if let Some(tab) = self.tabs.iter_mut().find(|t| t.id == tab_id)
                    && let Some(tx) = tab.sftp.command_tx.clone()
                {
                    tab.sftp.loading = true;
                    tab.sftp.error = None;
                    let _ = tx.unbounded_send(sftp::Command::List(path));
                }
            }
            SftpMessage::Refresh => {
                if self.active_tab != SETTINGS_TAB_INDEX
                    && let Some(tab) = self.tabs.get_mut(self.active_tab)
                    && let Some(tx) = tab.sftp.command_tx.clone()
                {
                    tab.sftp.loading = true;
                    tab.sftp.error = None;
                    let _ = tx.unbounded_send(sftp::Command::List(tab.sftp.current_path.clone()));
                }
            }
            SftpMessage::RequestUpload => {
                if self.active_tab != SETTINGS_TAB_INDEX
                    && let Some(tab) = self.tabs.get(self.active_tab)
                    && matches!(tab.shell, ShellKind::Ssh(_))
                {
                    let tab_id = tab.id;
                    return Task::perform(
                        async move {
                            rfd::AsyncFileDialog::new()
                                .pick_files()
                                .await
                                .map(|files| {
                                    files
                                        .into_iter()
                                        .map(|f| f.path().to_path_buf())
                                        .collect::<Vec<_>>()
                                })
                                .unwrap_or_default()
                        },
                        move |files| Message::Sftp(SftpMessage::UploadPicked { tab_id, files }),
                    );
                }
            }
            SftpMessage::UploadPicked { tab_id, files } => {
                if files.is_empty() {
                    return Task::none();
                }
                if let Some(tab) = self.tabs.iter_mut().find(|t| t.id == tab_id)
                    && let Some(tx) = tab.sftp.command_tx.clone()
                {
                    let base = tab.sftp.current_path.clone();
                    for local in files {
                        let name = local
                            .file_name()
                            .and_then(|s| s.to_str())
                            .unwrap_or("upload")
                            .to_string();
                        let remote = crate::gui::sftp::join_path(&base, &name);
                        let _ = tx.unbounded_send(sftp::Command::Upload { local, remote });
                    }
                }
            }
            SftpMessage::RequestDownload {
                tab_id,
                remote,
                suggested_name,
            } => {
                return Task::perform(
                    async move {
                        let local = rfd::AsyncFileDialog::new()
                            .set_file_name(&suggested_name)
                            .save_file()
                            .await
                            .map(|f| f.path().to_path_buf());
                        (remote, local)
                    },
                    move |(remote, local)| match local {
                        Some(local) => Message::Sftp(SftpMessage::DownloadPicked {
                            tab_id,
                            remote,
                            local,
                        }),
                        None => Message::Noop,
                    },
                );
            }
            SftpMessage::DownloadPicked {
                tab_id,
                remote,
                local,
            } => {
                if let Some(tab) = self.tabs.iter_mut().find(|t| t.id == tab_id)
                    && let Some(tx) = tab.sftp.command_tx.clone()
                {
                    let _ = tx.unbounded_send(sftp::Command::Download { remote, local });
                }
            }
            SftpMessage::CancelTransfer => {
                if self.active_tab != SETTINGS_TAB_INDEX
                    && let Some(tab) = self.tabs.get_mut(self.active_tab)
                    && let Some(tx) = tab.sftp.command_tx.clone()
                {
                    let _ = tx.unbounded_send(sftp::Command::Cancel);
                }
            }
        }
        Task::none()
    }
}

enum OpenState {
    Initial(SshSessionHandle),
    Streaming(mpsc::UnboundedReceiver<sftp::Event>),
    Done,
}

fn open_sftp_stream(ssh: SshSessionHandle, tab_id: u64) -> Task<Message> {
    let s = stream::unfold(OpenState::Initial(ssh), move |state| async move {
        match state {
            OpenState::Initial(ssh) => match ssh.open_sftp().await {
                Ok(handle) => {
                    let tx = handle.tx.clone();
                    Some((
                        Message::Sftp(SftpMessage::OpenSucceeded {
                            tab_id,
                            command_tx: tx,
                        }),
                        OpenState::Streaming(handle.rx),
                    ))
                }
                Err(error) => Some((
                    Message::Sftp(SftpMessage::OpenFailed { tab_id, error }),
                    OpenState::Done,
                )),
            },
            OpenState::Streaming(mut rx) => rx.next().await.map(|event| {
                (
                    Message::Sftp(SftpMessage::Event { tab_id, event }),
                    OpenState::Streaming(rx),
                )
            }),
            OpenState::Done => None,
        }
    });
    Task::stream(s)
}

fn apply_sftp_event(state: &mut SftpDrawerState, event: sftp::Event) {
    match event {
        sftp::Event::Listed { path, entries } => {
            state.current_path = path;
            state.entries = entries;
            state.loading = false;
            state.error = None;
        }
        sftp::Event::TransferStarted { path, total } => {
            state.transfers.insert(
                0,
                TransferRow {
                    path,
                    transferred: 0,
                    total,
                    finished: false,
                },
            );
            const TRANSFER_HISTORY_CAP: usize = 8;
            if state.transfers.len() > TRANSFER_HISTORY_CAP {
                state.transfers.truncate(TRANSFER_HISTORY_CAP);
            }
        }
        sftp::Event::TransferProgress {
            path,
            transferred,
            total,
        } => {
            if let Some(row) = state.transfers.iter_mut().find(|row| row.path == path) {
                row.transferred = transferred;
                row.total = total;
            }
        }
        sftp::Event::TransferFinished { path } => {
            if let Some(row) = state.transfers.iter_mut().find(|row| row.path == path) {
                row.finished = true;
            }
            if let Some(tx) = state.command_tx.clone() {
                let _ = tx.unbounded_send(sftp::Command::List(state.current_path.clone()));
                state.loading = true;
            }
        }
        sftp::Event::Mutated { .. } => {
            if let Some(tx) = state.command_tx.clone() {
                let _ = tx.unbounded_send(sftp::Command::List(state.current_path.clone()));
                state.loading = true;
            }
        }
        sftp::Event::Error { message } => {
            state.error = Some(message);
            state.loading = false;
        }
        sftp::Event::Closed => {
            state.reset();
        }
    }
}

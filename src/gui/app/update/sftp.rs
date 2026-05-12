//! SFTP drawer wiring.

use super::super::Message;
use crate::gui::sftp::{SftpDrawerState, TransferRow};
use crate::ssh::SshSessionHandle;
use crate::ssh::sftp;
use iced::Task;
use iced::futures::StreamExt;
use iced::futures::channel::mpsc;
use iced::futures::stream;

enum OpenState {
    Initial(SshSessionHandle),
    Streaming(mpsc::UnboundedReceiver<sftp::Event>),
    Done,
}

pub(super) fn open_sftp_stream(ssh: SshSessionHandle, tab_id: u64) -> Task<Message> {
    let s = stream::unfold(OpenState::Initial(ssh), move |state| async move {
        match state {
            OpenState::Initial(ssh) => match ssh.open_sftp().await {
                Ok(handle) => {
                    let tx = handle.tx.clone();
                    Some((
                        Message::SftpOpenSucceeded {
                            tab_id,
                            command_tx: tx,
                        },
                        OpenState::Streaming(handle.rx),
                    ))
                }
                Err(error) => Some((Message::SftpOpenFailed { tab_id, error }, OpenState::Done)),
            },
            OpenState::Streaming(mut rx) => rx.next().await.map(|event| {
                (
                    Message::SftpEvent { tab_id, event },
                    OpenState::Streaming(rx),
                )
            }),
            OpenState::Done => None,
        }
    });
    Task::stream(s)
}

pub(super) fn apply_sftp_event(state: &mut SftpDrawerState, event: sftp::Event) {
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

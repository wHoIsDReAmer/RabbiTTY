use iced::futures::channel::mpsc;
use iced::futures::executor;
use iced::futures::sink::SinkExt;
use portable_pty::{Child, CommandBuilder, MasterPty, PtySize, native_pty_system};
use std::io::{ErrorKind, Read, Write};
use std::sync::{Arc, Mutex};
use std::thread;
use std::thread::JoinHandle;
use std::time::Duration;

pub struct LaunchSpec<'a> {
    pub program: &'a str,
    pub args: &'a [&'a str],
    pub rows: u16,
    pub cols: u16,
}

pub struct Session {
    writer: Arc<Mutex<Box<dyn Write + Send>>>,
    child: Option<Box<dyn Child + Send>>,
    master: Option<Box<dyn MasterPty + Send>>,
    reader: Option<JoinHandle<()>>,
}

#[derive(Debug, Clone)]
pub enum SessionError {
    Spawn(String),
    Io(String),
}

#[derive(Debug, Clone)]
pub enum OutputEvent {
    Data { tab_id: u64, bytes: Vec<u8> },
    Closed { tab_id: u64 },
}

impl Session {
    pub fn spawn(
        spec: LaunchSpec<'_>,
        tab_id: u64,
        mut output_tx: mpsc::Sender<OutputEvent>,
    ) -> Result<Self, SessionError> {
        let pty_system = native_pty_system();
        let pair = pty_system
            .openpty(PtySize {
                rows: spec.rows,
                cols: spec.cols,
                pixel_width: 0,
                pixel_height: 0,
            })
            .map_err(|err| SessionError::Spawn(format!("openpty failed: {err}")))?;

        let mut cmd = CommandBuilder::new(spec.program);
        for arg in spec.args {
            cmd.arg(arg);
        }

        let child = pair
            .slave
            .spawn_command(cmd)
            .map_err(|err| SessionError::Spawn(format!("spawn failed: {err}")))?;

        let mut reader = pair
            .master
            .try_clone_reader()
            .map_err(|err| SessionError::Spawn(format!("reader clone failed: {err}")))?;

        let writer = pair
            .master
            .take_writer()
            .map_err(|err| SessionError::Spawn(format!("writer unavailable: {err}")))?;

        let writer = Arc::new(Mutex::new(writer));

        let _writer_for_reader = Arc::clone(&writer);
        let reader_handle = thread::spawn(move || {
            let mut buf = [0u8; 4096];
            loop {
                match reader.read(&mut buf) {
                    Ok(0) => {
                        let _ = executor::block_on(output_tx.send(OutputEvent::Closed { tab_id }));
                        break;
                    }
                    Ok(n) => {
                        let chunk = buf[..n].to_vec();
                        if chunk.is_empty() {
                            continue;
                        }
                        let _ = executor::block_on(output_tx.send(OutputEvent::Data {
                            tab_id,
                            bytes: chunk,
                        }));
                    }
                    Err(err) if err.kind() == ErrorKind::Interrupted => continue,
                    Err(err) if err.kind() == ErrorKind::WouldBlock => {
                        thread::sleep(Duration::from_millis(5));
                        continue;
                    }
                    Err(_) => {
                        let _ = executor::block_on(output_tx.send(OutputEvent::Closed { tab_id }));
                        break;
                    }
                }
            }
        });

        Ok(Self {
            writer,
            child: Some(child),
            master: Some(pair.master),
            reader: Some(reader_handle),
        })
    }

    pub fn send_bytes(&self, bytes: &[u8]) -> Result<(), SessionError> {
        let mut guard = self
            .writer
            .lock()
            .map_err(|err| SessionError::Io(format!("writer lock failed: {err}")))?;
        guard
            .write_all(bytes)
            .and_then(|_| guard.flush())
            .map_err(|err| SessionError::Io(format!("write failed: {err}")))
    }

    pub fn writer(&self) -> Arc<Mutex<Box<dyn Write + Send>>> {
        Arc::clone(&self.writer)
    }

    /// 세션(자식 프로세스)이 아직 살아있는지 확인
    pub fn is_alive(&mut self) -> bool {
        if let Some(ref mut child) = self.child {
            // try_wait: None이면 아직 실행 중, Some이면 종료됨
            match child.try_wait() {
                Ok(Some(_exit_status)) => false, // 종료됨
                Ok(None) => true,                // 아직 실행 중
                Err(_) => false,                 // 에러 = 죽은 것으로 간주
            }
        } else {
            false
        }
    }

    /// PTY 크기 조정
    pub fn resize(&self, rows: u16, cols: u16) -> Result<(), SessionError> {
        if let Some(ref master) = self.master {
            master
                .resize(PtySize {
                    rows,
                    cols,
                    pixel_width: 0,
                    pixel_height: 0,
                })
                .map_err(|err| SessionError::Io(format!("resize failed: {err}")))
        } else {
            Err(SessionError::Io("no master pty".into()))
        }
    }
}

impl Drop for Session {
    fn drop(&mut self) {
        if let Some(mut child) = self.child.take() {
            let _ = child.kill();
            let _ = child.wait();
        }

        self.master.take();

        if let Some(handle) = self.reader.take() {
            let _ = handle.join();
        }
    }
}

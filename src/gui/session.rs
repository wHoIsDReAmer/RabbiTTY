use portable_pty::{Child, CommandBuilder, MasterPty, PtySize, native_pty_system};
use std::io::{ErrorKind, Read, Write};
use std::sync::mpsc::{self, Receiver};
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
    output_rx: Receiver<Vec<u8>>,
    child: Option<Box<dyn Child + Send>>,
    master: Option<Box<dyn MasterPty + Send>>,
    reader: Option<JoinHandle<()>>,
}

#[derive(Debug, Clone)]
pub enum SessionError {
    Spawn(String),
    Io(String),
}

impl Session {
    pub fn spawn(spec: LaunchSpec<'_>) -> Result<Self, SessionError> {
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
        let (tx, rx) = mpsc::channel();

        let _writer_for_reader = Arc::clone(&writer);
        let reader_handle = thread::spawn(move || {
            let mut buf = [0u8; 4096];
            loop {
                match reader.read(&mut buf) {
                    Ok(0) => break,
                    Ok(n) => {
                        let chunk = buf[..n].to_vec();
                        if chunk.is_empty() {
                            continue;
                        }
                        if tx.send(chunk).is_err() {
                            break;
                        }
                    }
                    Err(err) if err.kind() == ErrorKind::Interrupted => continue,
                    Err(err) if err.kind() == ErrorKind::WouldBlock => {
                        thread::sleep(Duration::from_millis(5));
                        continue;
                    }
                    Err(_) => break,
                }
            }
        });

        Ok(Self {
            writer,
            output_rx: rx,
            child: Some(child),
            master: Some(pair.master),
            reader: Some(reader_handle),
        })
    }

    pub fn send_line(&self, line: &str) -> Result<(), SessionError> {
        let mut guard = self
            .writer
            .lock()
            .map_err(|err| SessionError::Io(format!("writer lock failed: {err}")))?;
        guard
            .write_all(line.as_bytes())
            .and_then(|_| guard.write_all(b"\n"))
            .and_then(|_| guard.flush())
            .map_err(|err| SessionError::Io(format!("write failed: {err}")))
    }

    pub fn drain_output(&self) -> Vec<Vec<u8>> {
        let mut chunks = Vec::new();
        while let Ok(chunk) = self.output_rx.try_recv() {
            chunks.push(chunk);
        }
        chunks
    }

    pub fn writer(&self) -> Arc<Mutex<Box<dyn Write + Send>>> {
        Arc::clone(&self.writer)
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

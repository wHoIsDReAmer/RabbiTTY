use alacritty_terminal::event::{OnResize, WindowSize};
use alacritty_terminal::tty::{self, Options, Shell};
use iced::futures::SinkExt;
use iced::futures::channel::mpsc;
use std::io::{ErrorKind, Read, Write};
use std::sync::{Arc, Mutex};
use std::thread;
use std::thread::JoinHandle;
use std::time::Duration;

pub struct LaunchSpec {
    pub program: String,
    pub args: Vec<String>,
    pub env: Vec<(String, String)>,
    pub rows: u16,
    pub cols: u16,
}

pub struct Session {
    writer: Arc<Mutex<Box<dyn Write + Send>>>,
    pty: Option<tty::Pty>,
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
        spec: LaunchSpec,
        tab_id: u64,
        mut output_tx: mpsc::Sender<OutputEvent>,
    ) -> Result<Self, SessionError> {
        tty::setup_env();

        let options = Options {
            shell: Some(Shell::new(spec.program, spec.args)),
            env: spec.env.into_iter().collect(),
            ..Default::default()
        };

        let window_size = WindowSize {
            num_lines: spec.rows,
            num_cols: spec.cols,
            cell_width: 1,
            cell_height: 1,
        };

        let pty = tty::new(&options, window_size, tab_id)
            .map_err(|err| SessionError::Spawn(format!("pty spawn failed: {err}")))?;

        let reader_file = pty
            .file()
            .try_clone()
            .map_err(|err| SessionError::Spawn(format!("reader clone failed: {err}")))?;

        let writer_file = pty
            .file()
            .try_clone()
            .map_err(|err| SessionError::Spawn(format!("writer clone failed: {err}")))?;

        let writer: Arc<Mutex<Box<dyn Write + Send>>> = Arc::new(Mutex::new(Box::new(writer_file)));

        let reader_handle = thread::spawn(move || {
            let mut reader = reader_file;
            let mut buf = [0u8; 2048];
            loop {
                match reader.read(&mut buf) {
                    Ok(0) => {
                        let _ = send_output_event(&mut output_tx, OutputEvent::Closed { tab_id });
                        break;
                    }
                    Ok(n) => {
                        if !send_output_event(
                            &mut output_tx,
                            OutputEvent::Data {
                                tab_id,
                                bytes: buf[..n].to_vec(),
                            },
                        ) {
                            break;
                        }
                    }
                    Err(err) if err.kind() == ErrorKind::Interrupted => continue,
                    Err(err) if err.kind() == ErrorKind::WouldBlock => {
                        thread::sleep(Duration::from_millis(1));
                        continue;
                    }
                    Err(_) => {
                        let _ = send_output_event(&mut output_tx, OutputEvent::Closed { tab_id });
                        break;
                    }
                }
            }
        });

        Ok(Self {
            writer,
            pty: Some(pty),
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
            .map_err(|err| SessionError::Io(format!("write failed: {err}")))
    }

    pub fn writer(&self) -> Arc<Mutex<Box<dyn Write + Send>>> {
        Arc::clone(&self.writer)
    }

    pub fn resize(&mut self, rows: u16, cols: u16) -> Result<(), SessionError> {
        if let Some(ref mut pty) = self.pty {
            let window_size = WindowSize {
                num_lines: rows,
                num_cols: cols,
                cell_width: 1,
                cell_height: 1,
            };
            pty.on_resize(window_size);
            Ok(())
        } else {
            Err(SessionError::Io("no pty".into()))
        }
    }
}

fn send_output_event(output_tx: &mut mpsc::Sender<OutputEvent>, event: OutputEvent) -> bool {
    iced::futures::executor::block_on(output_tx.send(event)).is_ok()
}

impl Drop for Session {
    fn drop(&mut self) {
        // Drop the PTY first — kills the child process, causing
        // the slave side to close. The reader thread will then get
        // EIO on its cloned master fd and exit.
        self.pty.take();

        if let Some(handle) = self.reader.take() {
            let _ = handle.join();
        }
    }
}

use crate::session::{LaunchSpec, Session, SessionError};
use crate::terminal::{TerminalEngine, TerminalSize};
use iced::keyboard::{Key, Modifiers, key::Named};
use std::fmt::{Display, Formatter};
use std::io::Write;
use std::sync::{Arc, Mutex};

pub struct TerminalTab {
    pub title: String,
    pub shell: ShellKind,
    pub session: TerminalSession,
    engine: TerminalEngine,
}

pub enum TerminalSession {
    Active(Session),
    Failed(String),
}

impl TerminalTab {
    pub fn from_shell(shell: ShellKind, columns: usize, lines: usize) -> Self {
        Self::launch(shell, columns, lines)
    }

    fn launch(shell: ShellKind, columns: usize, lines: usize) -> Self {
        let size = TerminalSize::new(columns, lines);
        let (session, writer) = match Session::spawn(shell.launch_spec(size)) {
            Ok(session) => {
                let writer = session.writer();
                (TerminalSession::Active(session), writer)
            }
            Err(err) => (
                TerminalSession::Failed(err.to_string()),
                Arc::new(Mutex::new(
                    Box::new(std::io::sink()) as Box<dyn Write + Send>
                )),
            ),
        };

        Self {
            title: shell.to_string(),
            shell,
            session,
            engine: TerminalEngine::new(size, 10_000, writer),
        }
    }

    pub fn pull_output(&mut self) {
        if let TerminalSession::Active(session) = &self.session {
            for chunk in session.drain_output() {
                self.engine.feed_bytes(&chunk);
            }
        }
    }

    pub fn status_text(&self) -> String {
        match &self.session {
            TerminalSession::Active(_) => "Session: live".into(),
            TerminalSession::Failed(err) => format!("Session error: {err}"),
        }
    }

    pub fn rendered_text(&self) -> String {
        self.engine.render_lines().join("\n")
    }

    pub fn size(&self) -> TerminalSize {
        self.engine.size()
    }

    pub fn is_alive(&mut self) -> bool {
        match &mut self.session {
            TerminalSession::Active(session) => session.is_alive(),
            TerminalSession::Failed(_) => false,
        }
    }

    pub fn resize(&mut self, columns: usize, lines: usize) {
        let new_size = TerminalSize::new(columns, lines);
        self.engine.resize(new_size);

        if let TerminalSession::Active(session) = &self.session {
            let _ = session.resize(lines as u16, columns as u16);
        }
    }

    pub fn handle_key(&mut self, key: &Key, modifiers: Modifiers, text: Option<&str>) {
        if let TerminalSession::Active(session) = &self.session
            && let Some(bytes) = self.key_to_bytes(key, modifiers, text)
            && let Err(err) = session.send_bytes(&bytes)
        {
            eprintln!("Failed to send key to session: {err}")
        }
    }

    fn key_to_bytes(&self, key: &Key, modifiers: Modifiers, text: Option<&str>) -> Option<Vec<u8>> {
        match key {
            Key::Named(named) => match named {
                Named::Enter => Some(b"\r".to_vec()),
                Named::Backspace => Some(b"\x7f".to_vec()),
                Named::Tab => {
                    if modifiers.shift() {
                        Some(b"\x1b[Z".to_vec())
                    } else {
                        Some(b"\t".to_vec())
                    }
                }
                Named::Escape => Some(b"\x1b".to_vec()),
                Named::ArrowUp => Some(b"\x1b[A".to_vec()),
                Named::ArrowDown => Some(b"\x1b[B".to_vec()),
                Named::ArrowRight => Some(b"\x1b[C".to_vec()),
                Named::ArrowLeft => Some(b"\x1b[D".to_vec()),
                Named::Home => Some(b"\x1b[H".to_vec()),
                Named::End => Some(b"\x1b[F".to_vec()),
                Named::Delete => Some(b"\x1b[3~".to_vec()),
                Named::PageUp => Some(b"\x1b[5~".to_vec()),
                Named::PageDown => Some(b"\x1b[6~".to_vec()),
                Named::Insert => Some(b"\x1b[2~".to_vec()),
                Named::F1 => Some(b"\x1bOP".to_vec()),
                Named::F2 => Some(b"\x1bOQ".to_vec()),
                Named::F3 => Some(b"\x1bOR".to_vec()),
                Named::F4 => Some(b"\x1bOS".to_vec()),
                Named::F5 => Some(b"\x1b[15~".to_vec()),
                Named::F6 => Some(b"\x1b[17~".to_vec()),
                Named::F7 => Some(b"\x1b[18~".to_vec()),
                Named::F8 => Some(b"\x1b[19~".to_vec()),
                Named::F9 => Some(b"\x1b[20~".to_vec()),
                Named::F10 => Some(b"\x1b[21~".to_vec()),
                Named::F11 => Some(b"\x1b[23~".to_vec()),
                Named::F12 => Some(b"\x1b[24~".to_vec()),
                Named::Space => {
                    if modifiers.control() {
                        Some(vec![0])
                    } else {
                        Some(b" ".to_vec())
                    }
                }
                _ => None,
            },

            Key::Character(c) if modifiers.control() => c.chars().next().and_then(|ch| {
                let upper = ch.to_ascii_uppercase();
                if upper.is_ascii_alphabetic() {
                    Some(vec![(upper as u8) - b'A' + 1])
                } else {
                    None
                }
            }),

            Key::Character(_) => text.map(|t| t.as_bytes().to_vec()),
            _ => None,
        }
    }
}

#[derive(Debug, Clone, Copy)]
pub enum ShellKind {
    #[cfg(target_family = "unix")]
    Zsh,
    Cmd,
    PowerShell,
}

impl ShellKind {
    fn launch_spec(self, size: TerminalSize) -> LaunchSpec<'static> {
        let (program, args): (&str, &[&str]) = match self {
            #[cfg(target_family = "unix")]
            ShellKind::Zsh => ("zsh", &["-i"]),
            ShellKind::Cmd => ("cmd", &["/Q", "/K"]),
            ShellKind::PowerShell => ("powershell", &["-NoLogo", "-ExecutionPolicy", "Bypass"]),
        };

        LaunchSpec {
            program,
            args,
            rows: size.lines as u16,
            cols: size.columns as u16,
        }
    }
}

impl Display for ShellKind {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        match self {
            #[cfg(target_family = "unix")]
            ShellKind::Zsh => write!(f, "zsh"),
            ShellKind::Cmd => write!(f, "cmd"),
            ShellKind::PowerShell => write!(f, "powershell"),
        }
    }
}

impl Display for SessionError {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        match self {
            SessionError::Spawn(err) => write!(f, "{err}"),
            SessionError::Io(err) => write!(f, "{err}"),
        }
    }
}

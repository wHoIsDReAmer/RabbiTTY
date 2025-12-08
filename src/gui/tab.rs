use crate::gui::session::{LaunchSpec, Session, SessionError};
use crate::gui::terminal::{DEFAULT_COLUMNS, DEFAULT_LINES, TerminalEngine, TerminalSize};
use std::fmt::{Display, Formatter};
use std::io::Write;
use std::sync::{Arc, Mutex};

pub struct TerminalTab {
    pub title: String,
    pub shell: ShellKind,
    pub session: TerminalSession,
    pub input: String,
    engine: TerminalEngine,
}

pub enum TerminalSession {
    Active(Session),
    Failed(String),
}

impl TerminalTab {
    pub fn zsh() -> Self {
        Self::launch(ShellKind::Zsh)
    }

    pub fn cmd() -> Self {
        Self::launch(ShellKind::Cmd)
    }

    pub fn powershell() -> Self {
        Self::launch(ShellKind::PowerShell)
    }

    fn launch(shell: ShellKind) -> Self {
        let size = TerminalSize::new(DEFAULT_COLUMNS, DEFAULT_LINES);
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
            input: String::new(),
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
}

#[derive(Debug, Clone, Copy)]
pub enum ShellKind {
    Zsh,
    Cmd,
    PowerShell,
}

impl ShellKind {
    fn launch_spec(self, size: TerminalSize) -> LaunchSpec<'static> {
        let (program, args): (&str, &[&str]) = match self {
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

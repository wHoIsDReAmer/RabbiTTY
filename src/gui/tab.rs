use crate::config::SshProfile;
use crate::session::{LaunchSpec, OutputEvent, Session, SessionError};
use crate::terminal::{CellVisual, TerminalEngine, TerminalSize, TerminalTheme};
use iced::futures::channel::mpsc;
use iced::keyboard::{Key, Modifiers, key::Named};
use std::fmt::{Display, Formatter};
use std::io::Write;
use std::path::Path;
use std::sync::{Arc, Mutex};

pub struct TerminalTab {
    pub id: u64,
    pub title: String,
    #[allow(dead_code)]
    pub shell: ShellKind,
    pub session: TerminalSession,
    engine: TerminalEngine,
}

pub enum TerminalSession {
    Active(Session),
    #[allow(dead_code)]
    Failed(String),
}

impl TerminalTab {
    pub fn from_shell(
        shell: ShellKind,
        columns: usize,
        lines: usize,
        theme: TerminalTheme,
        id: u64,
        output_tx: mpsc::Sender<OutputEvent>,
    ) -> Self {
        Self::launch(shell, columns, lines, theme, id, output_tx)
    }

    fn launch(
        shell: ShellKind,
        columns: usize,
        lines: usize,
        theme: TerminalTheme,
        id: u64,
        output_tx: mpsc::Sender<OutputEvent>,
    ) -> Self {
        let size = TerminalSize::new(columns, lines);
        let launch_spec = shell.launch_spec(size);
        let title = shell.title_from_program(&launch_spec.program);
        let (session, writer) = match Session::spawn(launch_spec, id, output_tx) {
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
            id,
            title,
            shell,
            session,
            engine: TerminalEngine::new(size, 10_000, writer, theme),
        }
    }

    pub fn feed_bytes(&mut self, bytes: &[u8]) {
        self.engine.feed_bytes(bytes);
        if let Some(new_title) = self.engine.take_title() {
            self.title = new_title;
        }
    }

    #[allow(dead_code)]
    pub fn status_text(&self) -> String {
        match &self.session {
            TerminalSession::Active(_) => "Session: live".into(),
            TerminalSession::Failed(err) => format!("Session error: {err}"),
        }
    }

    pub fn render_cells(&self) -> std::sync::Arc<Vec<CellVisual>> {
        self.engine.render_cells()
    }

    pub fn set_theme(&mut self, theme: TerminalTheme) {
        self.engine.set_theme(theme);
    }

    pub fn size(&self) -> TerminalSize {
        self.engine.size()
    }

    #[allow(dead_code)]
    pub fn is_alive(&mut self) -> bool {
        match &mut self.session {
            TerminalSession::Active(session) => session.is_alive(),
            TerminalSession::Failed(_) => false,
        }
    }

    pub fn scroll(&mut self, delta: i32) {
        self.engine.scroll(delta);
    }

    /// Returns (display_offset, total_history_lines).
    pub fn scroll_position(&self) -> (usize, usize) {
        self.engine.scroll_position()
    }

    pub fn scroll_to_relative(&mut self, rel: f32) {
        self.engine.scroll_to_relative(rel);
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

#[derive(Debug, Clone)]
pub enum ShellKind {
    Default,
    Shell { name: String, path: String },
    Ssh(SshProfile),
}

impl ShellKind {
    fn launch_spec(&self, size: TerminalSize) -> LaunchSpec {
        let (program, args) = match self {
            ShellKind::Ssh(profile) => return profile.launch_spec(size),
            ShellKind::Default => resolve_default_shell(),
            ShellKind::Shell { path, .. } => (path.clone(), vec!["-l".to_string()]),
        };

        let env = title_env_for_shell(&program);

        LaunchSpec {
            program,
            args,
            env,
            rows: size.lines as u16,
            cols: size.columns as u16,
        }
    }

    fn title_from_program(&self, program: &str) -> String {
        if let ShellKind::Ssh(profile) = self {
            return profile.tab_title();
        }

        Path::new(program)
            .file_name()
            .and_then(|name| name.to_str())
            .filter(|name| !name.trim().is_empty())
            .unwrap_or("shell")
            .to_string()
    }

    pub fn display_name(&self) -> String {
        match self {
            ShellKind::Default => {
                let (program, _) = resolve_default_shell();
                let name = Path::new(&program)
                    .file_name()
                    .and_then(|n| n.to_str())
                    .unwrap_or("shell");
                format!("{name} (Default)")
            }
            ShellKind::Shell { name, .. } => name.clone(),
            ShellKind::Ssh(profile) => format!("SSH: {}", profile.tab_title()),
        }
    }
}

impl SshProfile {
    fn launch_spec(&self, size: TerminalSize) -> LaunchSpec {
        let mut args = Vec::new();

        if self.port != 22 {
            args.push("-p".to_string());
            args.push(self.port.to_string());
        }

        if let Some(ref identity) = self.identity_file {
            let expanded = if identity.starts_with("~/") {
                dirs::home_dir()
                    .map(|h| h.join(&identity[2..]).to_string_lossy().to_string())
                    .unwrap_or_else(|| identity.clone())
            } else {
                identity.clone()
            };
            args.push("-i".to_string());
            args.push(expanded);
        }

        let destination = if self.user.is_empty() {
            self.host.clone()
        } else {
            format!("{}@{}", self.user, self.host)
        };
        args.push(destination);

        LaunchSpec {
            program: "ssh".to_string(),
            args,
            env: vec![],
            rows: size.lines as u16,
            cols: size.columns as u16,
        }
    }

    fn tab_title(&self) -> String {
        if self.name.is_empty() {
            if self.user.is_empty() {
                self.host.clone()
            } else {
                format!("{}@{}", self.user, self.host)
            }
        } else {
            self.name.clone()
        }
    }
}

/// Set PROMPT_COMMAND for bash to emit OSC 0 title with cwd.
/// zsh handles this natively (precmd), fish uses fish_title.
fn title_env_for_shell(program: &str) -> Vec<(String, String)> {
    let name = Path::new(program)
        .file_name()
        .and_then(|n| n.to_str())
        .unwrap_or("");

    match name {
        "bash" => vec![(
            "PROMPT_COMMAND".to_string(),
            r#"printf "\033]0;%s\007" "${PWD/#$HOME/~}""#.to_string(),
        )],
        _ => vec![],
    }
}

fn resolve_default_shell() -> (String, Vec<String>) {
    #[cfg(target_family = "unix")]
    {
        if let Ok(shell) = std::env::var("SHELL") {
            let shell = shell.trim();
            if !shell.is_empty() {
                return (shell.to_string(), vec!["-l".to_string()]);
            }
        }

        const FALLBACKS: &[&str] = &["zsh", "bash", "fish", "sh"];
        if let Some(candidate) = FALLBACKS.iter().find(|c| command_exists(c)) {
            return ((*candidate).to_string(), vec!["-l".to_string()]);
        }

        ("sh".to_string(), vec!["-l".to_string()])
    }

    #[cfg(target_family = "windows")]
    {
        (
            "powershell".to_string(),
            vec![
                "-NoLogo".to_string(),
                "-ExecutionPolicy".to_string(),
                "Bypass".to_string(),
            ],
        )
    }
}

#[cfg(target_family = "unix")]
fn command_exists(program: &str) -> bool {
    std::process::Command::new("sh")
        .arg("-lc")
        .arg(format!("command -v {program} >/dev/null 2>&1"))
        .status()
        .is_ok_and(|status| status.success())
}

/// Discover available shells from `/etc/shells` (Unix) or known Windows shells.
pub fn discover_available_shells() -> Vec<ShellKind> {
    let mut shells = vec![ShellKind::Default];

    #[cfg(target_family = "unix")]
    {
        let default_path = std::env::var("SHELL").unwrap_or_default();
        let default_path = default_path.trim();

        let etc_shells = std::fs::read_to_string("/etc/shells")
            .or_else(|_| std::fs::read_to_string("/usr/share/defaults/etc/shells"))
            .unwrap_or_default();

        for line in etc_shells.lines() {
            let line = line.trim();
            if line.is_empty() || line.starts_with('#') {
                continue;
            }
            // Skip if same as default shell (already listed)
            if line == default_path {
                continue;
            }
            // Skip non-interactive shells
            let name = Path::new(line)
                .file_name()
                .and_then(|n| n.to_str())
                .unwrap_or("");
            if name.is_empty() || matches!(name, "nologin" | "false") {
                continue;
            }
            shells.push(ShellKind::Shell {
                name: name.to_string(),
                path: line.to_string(),
            });
        }
    }

    #[cfg(target_family = "windows")]
    {
        shells.push(ShellKind::Shell {
            name: "cmd".to_string(),
            path: "cmd".to_string(),
        });
    }

    shells
}

impl Display for ShellKind {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        match self {
            ShellKind::Default => write!(f, "shell"),
            ShellKind::Shell { name, .. } => write!(f, "{name}"),
            ShellKind::Ssh(profile) => write!(f, "ssh: {}", profile.tab_title()),
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

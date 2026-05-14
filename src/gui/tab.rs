use crate::config::SshProfile;
use crate::gui::sftp::SftpDrawerState;
use crate::session::{LaunchSpec, OutputEvent, Session, SessionError};
use crate::terminal::{CellVisual, Selection, TerminalEngine, TerminalSize, TerminalTheme};
use iced::futures::channel::mpsc;
use iced::keyboard::{Key, Modifiers, key::Named};
use std::borrow::Cow;
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
    pub selection: Option<Selection>,
    pub sftp: SftpDrawerState,
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
        output_tx: mpsc::UnboundedSender<OutputEvent>,
    ) -> Self {
        Self::launch(shell, columns, lines, theme, id, output_tx)
    }

    fn launch(
        shell: ShellKind,
        columns: usize,
        lines: usize,
        theme: TerminalTheme,
        id: u64,
        output_tx: mpsc::UnboundedSender<OutputEvent>,
    ) -> Self {
        let size = TerminalSize::new(columns, lines);

        let title = shell.display_name();

        let (session, writer) = if let ShellKind::Ssh(ref profile) = shell {
            let s =
                Session::spawn_ssh(profile.clone(), id, lines as u16, columns as u16, output_tx);

            let w = s.writer();
            (TerminalSession::Active(s), w)
        } else {
            let spec = shell.launch_spec(size);
            match Session::spawn(spec, id, output_tx) {
                Ok(s) => {
                    let w = s.writer();
                    (TerminalSession::Active(s), w)
                }
                Err(err) => {
                    let sink = Arc::new(Mutex::new(
                        Box::new(std::io::sink()) as Box<dyn Write + Send>
                    ));
                    (TerminalSession::Failed(err.to_string()), sink)
                }
            }
        };

        let engine = TerminalEngine::new(size, 10_000, writer, theme);

        Self {
            id,
            title,
            shell,
            session,
            selection: None,
            sftp: SftpDrawerState::new(),
            engine,
        }
    }

    pub fn feed_bytes(&mut self, bytes: &[u8]) {
        self.engine.feed_bytes(bytes);
        if let Some(new_title) = self.engine.take_title() {
            self.title = new_title;
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

    pub fn scroll(&mut self, delta: i32) {
        self.engine.scroll(delta);
    }

    /// Returns (display_offset, total_history_lines).
    pub fn scroll_position(&self) -> (usize, usize) {
        self.engine.scroll_position()
    }

    pub fn cursor_position(&self) -> (usize, usize) {
        self.engine.cursor_position()
    }

    pub fn selected_text(&self) -> Option<String> {
        let sel = self.selection.as_ref().filter(|s| !s.is_empty())?;
        let cells = self.engine.render_cells();
        let size = self.engine.size();
        let (current_offset, _) = self.engine.scroll_position();
        let delta = sel.delta(current_offset);
        let (start, end) = sel.ordered();
        let mut result = String::new();
        for row in start.row..=end.row {
            // Selection rows live in the anchor frame; translate to the
            // current viewport before reading cells.
            let viewport_row = row + delta;
            let row_in_view = (0..size.lines as i64).contains(&viewport_row);
            if row_in_view {
                let viewport_row = viewport_row as usize;
                let col_start = if row == start.row { start.col } else { 0 };
                let col_end = if row == end.row {
                    end.col
                } else {
                    size.columns.saturating_sub(1)
                };
                for col in col_start..=col_end {
                    let idx = viewport_row * size.columns + col;
                    if let Some(cell) = cells.get(idx) {
                        result.push(cell.ch);
                    }
                }
                let trimmed_len = result.trim_end_matches(' ').len();
                result.truncate(trimmed_len);
            }
            if row != end.row {
                result.push('\n');
            }
        }
        let trimmed_len = result.trim_end_matches(' ').len();
        result.truncate(trimmed_len);
        if result.is_empty() {
            None
        } else {
            Some(result)
        }
    }

    pub fn clear_selection(&mut self) {
        self.selection = None;
    }

    pub fn scroll_to_relative(&mut self, rel: f32) {
        self.engine.scroll_to_relative(rel);
    }

    /// Returns true when the terminal program has enabled mouse reporting.
    pub fn mouse_mode(&self) -> bool {
        self.engine.mouse_mode()
    }

    /// Returns true when the terminal is in the alternate screen buffer.
    pub fn alt_screen(&self) -> bool {
        self.engine.alt_screen()
    }

    /// Returns true when the running program has enabled bracketed paste.
    pub fn bracketed_paste(&self) -> bool {
        self.engine.bracketed_paste()
    }

    /// Send scroll as arrow key sequences (for alt screen without mouse mode).
    pub fn send_scroll_as_arrows(&self, lines: i32) {
        let TerminalSession::Active(session) = &self.session else {
            return;
        };
        let arrow = if lines > 0 { b'A' } else { b'B' }; // Up / Down
        let seq = [b'\x1b', b'[', arrow];
        for _ in 0..lines.unsigned_abs() {
            let _ = session.send_bytes(&seq);
        }
    }

    /// Send a mouse event to the PTY using SGR or legacy encoding.
    pub fn send_mouse_event(&self, button: u8, col: usize, row: usize, pressed: bool) {
        let TerminalSession::Active(session) = &self.session else {
            return;
        };
        // SGR encoding: \x1b[<btn;col;row;M/m  (M=press, m=release)
        // Columns and rows are 1-based in the protocol.
        if self.engine.sgr_mouse() {
            let suffix = if pressed { 'M' } else { 'm' };
            let seq = format!("\x1b[<{};{};{}{}", button, col + 1, row + 1, suffix);
            let _ = session.send_bytes(seq.as_bytes());
        } else {
            // Legacy X10/normal encoding: only sends press, limited to 223 cols/rows
            if pressed {
                let cb = 32 + button;
                let cx = 32 + (col as u8 + 1);
                let cy = 32 + (row as u8 + 1);
                let seq = [b'\x1b', b'[', b'M', cb, cx, cy];
                let _ = session.send_bytes(&seq);
            }
        }
    }

    pub fn resize(&mut self, columns: usize, lines: usize) {
        let new_size = TerminalSize::new(columns, lines);
        self.engine.resize(new_size);

        if let TerminalSession::Active(session) = &mut self.session {
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

    fn key_to_bytes<'a>(
        &self,
        key: &Key,
        modifiers: Modifiers,
        text: Option<&'a str>,
    ) -> Option<Cow<'a, [u8]>> {
        match key {
            Key::Named(named) => match named {
                Named::Enter => Some(Cow::Borrowed(b"\r")),
                Named::Backspace => Some(Cow::Borrowed(b"\x7f")),
                Named::Tab => {
                    if modifiers.shift() {
                        Some(Cow::Borrowed(b"\x1b[Z"))
                    } else {
                        Some(Cow::Borrowed(b"\t"))
                    }
                }
                Named::Escape => Some(Cow::Borrowed(b"\x1b")),
                Named::ArrowUp => Some(Cow::Borrowed(b"\x1b[A")),
                Named::ArrowDown => Some(Cow::Borrowed(b"\x1b[B")),
                Named::ArrowRight => Some(Cow::Borrowed(b"\x1b[C")),
                Named::ArrowLeft => Some(Cow::Borrowed(b"\x1b[D")),
                Named::Home => Some(Cow::Borrowed(b"\x1b[H")),
                Named::End => Some(Cow::Borrowed(b"\x1b[F")),
                Named::Delete => Some(Cow::Borrowed(b"\x1b[3~")),
                Named::PageUp => Some(Cow::Borrowed(b"\x1b[5~")),
                Named::PageDown => Some(Cow::Borrowed(b"\x1b[6~")),
                Named::Insert => Some(Cow::Borrowed(b"\x1b[2~")),
                Named::F1 => Some(Cow::Borrowed(b"\x1bOP")),
                Named::F2 => Some(Cow::Borrowed(b"\x1bOQ")),
                Named::F3 => Some(Cow::Borrowed(b"\x1bOR")),
                Named::F4 => Some(Cow::Borrowed(b"\x1bOS")),
                Named::F5 => Some(Cow::Borrowed(b"\x1b[15~")),
                Named::F6 => Some(Cow::Borrowed(b"\x1b[17~")),
                Named::F7 => Some(Cow::Borrowed(b"\x1b[18~")),
                Named::F8 => Some(Cow::Borrowed(b"\x1b[19~")),
                Named::F9 => Some(Cow::Borrowed(b"\x1b[20~")),
                Named::F10 => Some(Cow::Borrowed(b"\x1b[21~")),
                Named::F11 => Some(Cow::Borrowed(b"\x1b[23~")),
                Named::F12 => Some(Cow::Borrowed(b"\x1b[24~")),
                Named::Space => {
                    if modifiers.control() {
                        Some(Cow::Borrowed(b"\0"))
                    } else {
                        Some(Cow::Borrowed(b" "))
                    }
                }
                _ => None,
            },

            Key::Character(c) if modifiers.control() => c.chars().next().and_then(|ch| {
                let upper = ch.to_ascii_uppercase();
                if upper.is_ascii_alphabetic() {
                    Some(Cow::Owned(vec![(upper as u8) - b'A' + 1]))
                } else {
                    None
                }
            }),

            Key::Character(_) => text.map(|t| Cow::Borrowed(t.as_bytes())),
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
            ShellKind::Ssh(_) => unreachable!("SSH uses native russh, not launch_spec"),
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

    pub fn display_name(&self) -> String {
        match self {
            ShellKind::Default => default_shell_display_name(),
            ShellKind::Shell { name, .. } => name.clone(),
            ShellKind::Ssh(profile) => format!("SSH: {}", profile.tab_title()),
        }
    }
}

impl SshProfile {
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

fn default_shell_display_name() -> String {
    use std::sync::OnceLock;
    static CACHED: OnceLock<String> = OnceLock::new();
    CACHED
        .get_or_init(|| {
            let (program, _) = resolve_default_shell();
            let name = Path::new(&program)
                .file_name()
                .and_then(|n| n.to_str())
                .unwrap_or("shell");
            format!("{name} (Default)")
        })
        .clone()
}

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

        let mut seen_names = std::collections::HashSet::new();
        // Mark the default shell name as seen to avoid duplicates
        if let Some(default_name) = Path::new(default_path).file_name().and_then(|n| n.to_str()) {
            seen_names.insert(default_name.to_string());
        }

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
            // Skip duplicate shell names (e.g. /bin/bash and /usr/bin/bash)
            if !seen_names.insert(name.to_string()) {
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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn ssh_profile_tab_title() {
        let with_name = SshProfile {
            name: "Production".into(),
            host: "prod.example.com".into(),
            port: 22,
            user: "deploy".into(),
            auth_method: crate::config::SshAuthMethod::Password,
            identity_file: None,
            password: None,
            proxy_command: None,
        };
        assert_eq!(with_name.tab_title(), "Production");

        let no_name = SshProfile {
            name: String::new(),
            host: "dev.example.com".into(),
            port: 22,
            user: "user".into(),
            auth_method: crate::config::SshAuthMethod::Password,
            identity_file: None,
            password: None,
            proxy_command: None,
        };
        assert_eq!(no_name.tab_title(), "user@dev.example.com");

        let no_name_no_user = SshProfile {
            name: String::new(),
            host: "bare.host".into(),
            port: 22,
            user: String::new(),
            auth_method: crate::config::SshAuthMethod::Password,
            identity_file: None,
            password: None,
            proxy_command: None,
        };
        assert_eq!(no_name_no_user.tab_title(), "bare.host");
    }
}

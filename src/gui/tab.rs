use crate::config::SshProfile;
use crate::gui::pane::{Axis, Direction, PaneNode, neighbour};
use crate::gui::sftp::SftpDrawerState;
use crate::session::{LaunchSpec, OutputEvent, Session, SessionError};
use crate::terminal::{CellVisual, Selection, TerminalEngine, TerminalSize, TerminalTheme};
use iced::futures::channel::mpsc;
use iced::keyboard::{Key, Modifiers, key::Named};
use serde::{Deserialize, Serialize};
use std::borrow::Cow;
use std::fmt::{Display, Formatter};
use std::io::Write;
use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex};

pub struct Pane {
    pub id: u64,
    pub title: String,
    pub profile: Profile,
    pub session: TerminalSession,
    pub selection: Option<Selection>,
    pub sftp: SftpDrawerState,
    engine: TerminalEngine,
}

pub struct TerminalTab {
    pub id: u64,
    pub layout: PaneNode,
    pub focused: u64,
    pub panes: Vec<Pane>,
}

pub enum TerminalSession {
    Active(Session),
    #[allow(dead_code)]
    Failed(String),
}

pub struct PaneSpawn {
    pub profile: Profile,
    pub columns: usize,
    pub lines: usize,
    pub theme: TerminalTheme,
    pub id: u64,
    pub output_tx: mpsc::UnboundedSender<OutputEvent>,
    pub scrollback_lines: usize,
    pub cwd: Option<PathBuf>,
}

impl Pane {
    pub fn from_profile(spec: PaneSpawn) -> Self {
        let PaneSpawn {
            profile,
            columns,
            lines,
            theme,
            id,
            output_tx,
            scrollback_lines,
            cwd,
        } = spec;

        let size = TerminalSize::new(columns, lines);

        let title = profile.display_name();

        let (session, writer) = if let Some(ssh) = profile.ssh_profile() {
            let s = Session::spawn_ssh(ssh.clone(), id, lines as u16, columns as u16, output_tx);

            let w = s.writer();
            (TerminalSession::Active(s), w)
        } else {
            let spec = profile.launch_spec(size, cwd);
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

        // scrollback_lines is read from config at tab creation time;
        // changing the setting later applies only to newly created tabs.
        let engine = TerminalEngine::new(size, scrollback_lines, writer, theme);

        Self {
            id,
            title,
            profile,
            session,
            selection: None,
            sftp: SftpDrawerState::new(),
            engine,
        }
    }

    /// Feeds PTY bytes to the terminal engine. Returns `true` if a bell rang.
    pub fn feed_bytes(&mut self, bytes: &[u8]) -> bool {
        self.engine.feed_bytes(bytes);
        if let Some(new_title) = self.engine.take_title() {
            self.title = new_title;
        }
        self.engine.take_bell()
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

    pub fn cursor_cell(&self) -> Option<(usize, usize)> {
        self.engine.cursor_cell()
    }

    pub fn cursor_color(&self) -> [f32; 4] {
        self.engine.cursor_color()
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

    pub fn scroll_to_bottom(&mut self) {
        self.engine.scroll_to_bottom();
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

    pub fn working_directory(&self) -> Option<PathBuf> {
        match &self.session {
            TerminalSession::Active(session) => session.working_directory(),
            _ => None,
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

/// A launchable session descriptor: a local shell (default or a specific
/// program) or an SSH connection. The unifying type behind every tab.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Profile {
    pub name: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub icon: Option<String>,
    pub kind: ProfileKind,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum ProfileKind {
    /// A local PTY. `program: None` resolves the user's default shell.
    Local {
        program: Option<String>,
        #[serde(default)]
        args: Vec<String>,
    },
    Ssh(SshProfile),
}

impl Profile {
    pub fn default_shell() -> Self {
        Self {
            name: default_shell_display_name(),
            icon: None,
            kind: ProfileKind::Local {
                program: None,
                args: Vec::new(),
            },
        }
    }

    pub fn shell(name: String, path: String) -> Self {
        Self {
            name,
            icon: None,
            kind: ProfileKind::Local {
                program: Some(path),
                args: vec!["-l".to_string()],
            },
        }
    }

    pub fn ssh(profile: SshProfile) -> Self {
        Self {
            name: profile.tab_title(),
            icon: None,
            kind: ProfileKind::Ssh(profile),
        }
    }

    /// The SSH connection this profile launches, if any.
    pub fn ssh_profile(&self) -> Option<&SshProfile> {
        match &self.kind {
            ProfileKind::Ssh(p) => Some(p),
            ProfileKind::Local { .. } => None,
        }
    }

    fn launch_spec(&self, size: TerminalSize, cwd: Option<PathBuf>) -> LaunchSpec {
        let (program, args) = match &self.kind {
            ProfileKind::Ssh(_) => unreachable!("SSH uses native russh, not launch_spec"),
            ProfileKind::Local { program: None, .. } => resolve_default_shell(),
            ProfileKind::Local {
                program: Some(path),
                args,
            } => (path.clone(), args.clone()),
        };

        let env = title_env_for_shell(&program);

        LaunchSpec {
            program,
            args,
            env,
            rows: size.lines as u16,
            cols: size.columns as u16,
            cwd,
        }
    }

    pub fn display_name(&self) -> String {
        match &self.kind {
            ProfileKind::Ssh(profile) => format!("SSH: {}", profile.tab_title()),
            _ => self.name.clone(),
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
pub fn discover_available_shells() -> Vec<Profile> {
    let mut shells = vec![Profile::default_shell()];

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
            shells.push(Profile::shell(name.to_string(), line.to_string()));
        }
    }

    #[cfg(target_family = "windows")]
    {
        shells.push(Profile::shell("cmd".to_string(), "cmd".to_string()));
    }

    shells
}

impl Display for Profile {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        match &self.kind {
            ProfileKind::Ssh(profile) => write!(f, "ssh: {}", profile.tab_title()),
            _ => write!(f, "{}", self.name),
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

impl TerminalTab {
    pub fn new(id: u64, pane: Pane) -> Self {
        let focused = pane.id;
        Self {
            id,
            layout: PaneNode::Leaf(focused),
            focused,
            panes: vec![pane],
        }
    }

    pub fn focused(&self) -> &Pane {
        self.panes
            .iter()
            .find(|p| p.id == self.focused)
            .unwrap_or(&self.panes[0])
    }

    pub fn focused_mut(&mut self) -> &mut Pane {
        let id = self.focused;
        if let Some(index) = self.panes.iter().position(|p| p.id == id) {
            &mut self.panes[index]
        } else {
            &mut self.panes[0]
        }
    }

    pub fn pane_mut(&mut self, id: u64) -> Option<&mut Pane> {
        self.panes.iter_mut().find(|p| p.id == id)
    }

    pub fn title(&self) -> &str {
        &self.focused().title
    }

    pub fn split(&mut self, axis: Axis, pane: Pane) {
        let new_id = pane.id;
        if self.layout.split(self.focused, axis, new_id) {
            self.panes.push(pane);
            self.focused = new_id;
        }
    }

    pub fn close_focused(&mut self) -> bool {
        self.close_pane(self.focused)
    }

    pub fn close_pane(&mut self, target: u64) -> bool {
        if !self.layout.remove(target) {
            return false;
        }
        self.panes.retain(|p| p.id != target);
        if self.focused == target {
            self.focused = self.layout.leaves().first().copied().unwrap_or(target);
        }
        true
    }

    pub fn focus_direction(&mut self, direction: Direction, area: iced::Rectangle) {
        let regions = self.layout.regions(area);
        if let Some(next) = neighbour(&regions, self.focused, direction) {
            self.focused = next;
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

    #[test]
    fn ssh_profile_snapshot_never_serializes_password() {
        let profile = Profile::ssh(SshProfile {
            name: "Prod".into(),
            host: "prod.example.com".into(),
            port: 2222,
            user: "deploy".into(),
            auth_method: crate::config::SshAuthMethod::Password,
            identity_file: None,
            password: Some("hunter2".into()),
            proxy_command: None,
        });

        let toml = toml::to_string(&profile).expect("serialize");
        assert!(!toml.contains("hunter2"), "{toml}");
        assert!(!toml.contains("password ="), "{toml}");

        let restored: Profile = toml::from_str(&toml).expect("deserialize");
        assert_eq!(restored.ssh_profile().unwrap().host, "prod.example.com");
        assert_eq!(restored.ssh_profile().unwrap().port, 2222);
        assert!(restored.ssh_profile().unwrap().password.is_none());
    }

    #[test]
    fn local_profile_round_trips() {
        let default = Profile::default_shell();
        let toml = toml::to_string(&default).expect("serialize");
        let restored: Profile = toml::from_str(&toml).expect("deserialize");
        assert!(matches!(
            restored.kind,
            ProfileKind::Local { program: None, .. }
        ));

        let shell = Profile::shell("fish".into(), "/opt/bin/fish".into());
        let restored: Profile =
            toml::from_str(&toml::to_string(&shell).expect("serialize")).expect("deserialize");
        assert!(matches!(
            restored.kind,
            ProfileKind::Local { program: Some(p), .. } if p == "/opt/bin/fish"
        ));
    }
}

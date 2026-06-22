use ab_glyph::{Font, FontArc, PxScale, ScaleFont, point};
use serde::{Deserialize, Serialize};
use std::fs;
use std::path::{Path, PathBuf};

pub const DEFAULT_WINDOW_WIDTH: f32 = 600.0;
pub const DEFAULT_WINDOW_HEIGHT: f32 = 350.0;
pub const FONT_SCALE_FACTOR: f32 = 0.85;
pub const DEFAULT_THEME_FOREGROUND: [u8; 3] = [0xcd, 0xd6, 0xf4];
pub const DEFAULT_THEME_BACKGROUND: [u8; 3] = [0x1e, 0x1e, 0x2e];
pub const DEFAULT_THEME_CURSOR: [u8; 3] = [0x89, 0xb4, 0xfa];
pub const DEFAULT_THEME_BG_OPACITY: f32 = 1.0;
pub const DEFAULT_BLUR_ENABLED: bool = true;
pub const DEFAULT_MACOS_BLUR_RADIUS: i32 = 20;

#[cfg(target_os = "macos")]
pub const DEFAULT_SHORTCUT_NEW_TAB: &str = "Command+T";
#[cfg(not(target_os = "macos"))]
pub const DEFAULT_SHORTCUT_NEW_TAB: &str = "Ctrl+T";

#[cfg(target_os = "macos")]
pub const DEFAULT_SHORTCUT_CLOSE_TAB: &str = "Command+W";
#[cfg(not(target_os = "macos"))]
pub const DEFAULT_SHORTCUT_CLOSE_TAB: &str = "Ctrl+W";

#[cfg(target_os = "macos")]
pub const DEFAULT_SHORTCUT_OPEN_SETTINGS: &str = "Command+Comma";
#[cfg(not(target_os = "macos"))]
pub const DEFAULT_SHORTCUT_OPEN_SETTINGS: &str = "Ctrl+Comma";

#[cfg(target_os = "macos")]
pub const DEFAULT_SHORTCUT_NEXT_TAB: &str = "Command+PageDown";
#[cfg(not(target_os = "macos"))]
pub const DEFAULT_SHORTCUT_NEXT_TAB: &str = "Ctrl+PageDown";

#[cfg(target_os = "macos")]
pub const DEFAULT_SHORTCUT_PREV_TAB: &str = "Command+PageUp";
#[cfg(not(target_os = "macos"))]
pub const DEFAULT_SHORTCUT_PREV_TAB: &str = "Ctrl+PageUp";

#[cfg(target_os = "macos")]
pub const DEFAULT_SHORTCUT_QUIT: &str = "Command+Q";
#[cfg(not(target_os = "macos"))]
pub const DEFAULT_SHORTCUT_QUIT: &str = "Ctrl+Q";

#[cfg(target_os = "macos")]
pub const DEFAULT_SHORTCUT_FONT_SIZE_INCREASE: &str = "Command+=";
#[cfg(not(target_os = "macos"))]
pub const DEFAULT_SHORTCUT_FONT_SIZE_INCREASE: &str = "Ctrl+=";

#[cfg(target_os = "macos")]
pub const DEFAULT_SHORTCUT_FONT_SIZE_DECREASE: &str = "Command+-";
#[cfg(not(target_os = "macos"))]
pub const DEFAULT_SHORTCUT_FONT_SIZE_DECREASE: &str = "Ctrl+-";

#[cfg(target_os = "macos")]
pub const DEFAULT_SHORTCUT_FONT_SIZE_RESET: &str = "Command+0";
#[cfg(not(target_os = "macos"))]
pub const DEFAULT_SHORTCUT_FONT_SIZE_RESET: &str = "Ctrl+0";

#[cfg(target_os = "macos")]
pub const DEFAULT_SHORTCUT_DUPLICATE_TAB: &str = "Command+D";
#[cfg(not(target_os = "macos"))]
pub const DEFAULT_SHORTCUT_DUPLICATE_TAB: &str = "Ctrl+Shift+D";

pub const DEFAULT_TERMINAL_FONT_SIZE: f32 = 14.0;
pub const DEFAULT_TERMINAL_PADDING_X: f32 = 4.0;
pub const DEFAULT_TERMINAL_PADDING_Y: f32 = 4.0;
pub const DEFAULT_TERMINAL_SCROLLBACK: usize = 10_000;
pub const DEFAULT_BRACKETED_PASTE: bool = true;
pub const DEFAULT_MULTILINE_PASTE_CONFIRM: bool = false;
pub const DEFAULT_TERMINAL_SCROLL_MULTIPLIER: f32 = 1.0;
pub const DEFAULT_CURSOR_BLINK: bool = true;
pub const DEFAULT_ANIMATIONS_ENABLED: bool = true;
const DEJAVU_SANS_MONO: &[u8] = include_bytes!("../fonts/DejaVuSansMono.ttf");

#[derive(Debug, Clone, Copy, PartialEq, Eq, Deserialize, Serialize, Default)]
#[serde(rename_all = "snake_case")]
pub enum SshAuthMethod {
    KeyFile,
    #[default]
    Password,
}

/// Visual shape of the terminal text cursor.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Deserialize, Serialize, Default)]
#[serde(rename_all = "snake_case")]
pub enum CursorShape {
    #[default]
    Block,
    Bar,
    Underline,
}

impl CursorShape {
    pub const ALL: [Self; 3] = [Self::Block, Self::Bar, Self::Underline];
}

impl std::fmt::Display for CursorShape {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let label = match self {
            Self::Block => crate::t!("settings.terminal.cursor_shape.block"),
            Self::Bar => crate::t!("settings.terminal.cursor_shape.bar"),
            Self::Underline => crate::t!("settings.terminal.cursor_shape.underline"),
        };
        f.write_str(label)
    }
}

/// Behavior when the terminal receives a bell (`\a`, 0x07).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Deserialize, Serialize, Default)]
#[serde(rename_all = "snake_case")]
pub enum BellMode {
    Off,
    Visual,
    #[default]
    Sound,
}

impl BellMode {
    pub const ALL: [Self; 3] = [Self::Off, Self::Visual, Self::Sound];
}

impl std::fmt::Display for BellMode {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let label = match self {
            Self::Off => crate::t!("settings.terminal.bell_mode.off"),
            Self::Visual => crate::t!("settings.terminal.bell_mode.visual"),
            Self::Sound => crate::t!("settings.terminal.bell_mode.sound"),
        };
        f.write_str(label)
    }
}

/// Where the tab bar (which doubles as the title bar) is anchored.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Deserialize, Serialize, Default)]
#[serde(rename_all = "snake_case")]
pub enum TabBarPosition {
    #[default]
    Top,
    Bottom,
}

impl TabBarPosition {
    pub const ALL: [Self; 2] = [Self::Top, Self::Bottom];
}

/// Action taken when the terminal area is right-clicked.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Deserialize, Serialize, Default)]
#[serde(rename_all = "snake_case")]
pub enum RightClickAction {
    #[default]
    Paste,
    Menu,
    None,
}

impl RightClickAction {
    pub const ALL: [Self; 3] = [Self::Paste, Self::Menu, Self::None];
}

impl std::fmt::Display for RightClickAction {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let label = match self {
            Self::Paste => crate::t!("settings.terminal.right_click_action.paste"),
            Self::Menu => crate::t!("settings.terminal.right_click_action.menu"),
            Self::None => crate::t!("settings.terminal.right_click_action.none"),
        };
        f.write_str(label)
    }
}

#[derive(Debug, Clone)]
pub struct SshProfile {
    pub name: String,
    pub host: String,
    pub port: u16,
    pub user: String,
    pub auth_method: SshAuthMethod,
    pub identity_file: Option<String>,
    pub password: Option<String>,
    pub proxy_command: Option<String>,
}

impl Default for SshProfile {
    fn default() -> Self {
        Self {
            name: String::new(),
            host: String::new(),
            port: 22,
            user: String::new(),
            auth_method: SshAuthMethod::Password,
            identity_file: None,
            password: None,
            proxy_command: None,
        }
    }
}

#[derive(Debug, Clone)]
pub struct AppConfig {
    pub ui: UiConfig,
    pub terminal: TerminalConfig,
    pub theme: ThemeConfig,
    pub shortcuts: ShortcutsConfig,
    pub ssh_profiles: Vec<SshProfile>,
}

#[derive(Debug, Clone)]
pub struct UiConfig {
    pub window_width: f32,
    pub window_height: f32,
    /// `None` = follow `LANG` env; otherwise a tag from `AVAILABLE_LOCALES`.
    pub language: Option<String>,
    /// Whether smooth hover transition animations are enabled.
    pub animations_enabled: bool,
    /// Where the tab bar / title bar is anchored.
    pub tab_bar_position: TabBarPosition,
}

#[derive(Debug, Clone)]
pub struct TerminalConfig {
    pub cell_width: f32,
    pub cell_height: f32,
    pub font_selection: Option<String>,
    pub font_size: f32,
    pub padding_x: f32,
    pub padding_y: f32,
    pub scrollback_lines: usize,
    pub bracketed_paste: bool,
    pub multiline_paste_confirm: bool,
    pub scroll_multiplier: f32,
    pub cursor_shape: CursorShape,
    pub cursor_blink: bool,
    pub bell_mode: BellMode,
    pub right_click_action: RightClickAction,
}

#[derive(Debug, Clone)]
pub struct ThemeConfig {
    pub color_scheme: String,
    pub foreground: [u8; 3],
    pub background: [u8; 3],
    pub cursor: [u8; 3],
    pub ansi_colors: Option<[[u8; 3]; 16]>,
    pub background_opacity: f32,
    pub blur_enabled: bool,
    pub macos_blur_radius: i32,
}

#[derive(Debug, Clone)]
pub struct ShortcutsConfig {
    pub new_tab: String,
    pub close_tab: String,
    pub open_settings: String,
    pub next_tab: String,
    pub prev_tab: String,
    pub quit: String,
    pub font_size_increase: String,
    pub font_size_decrease: String,
    pub font_size_reset: String,
    pub duplicate_tab: String,
}

#[derive(Debug, Deserialize, Serialize)]
struct SshProfileFileConfig {
    name: Option<String>,
    host: Option<String>,
    port: Option<u16>,
    user: Option<String>,
    auth_method: Option<SshAuthMethod>,
    identity_file: Option<String>,
    proxy_command: Option<String>,
}

#[derive(Debug, Deserialize, Serialize)]
struct FileConfig {
    ui: Option<UiFileConfig>,
    terminal: Option<TerminalFileConfig>,
    theme: Option<ThemeFileConfig>,
    shortcuts: Option<ShortcutsFileConfig>,
    ssh_profiles: Option<Vec<SshProfileFileConfig>>,
}

#[derive(Debug, Deserialize, Serialize)]
struct UiFileConfig {
    window_width: Option<f32>,
    window_height: Option<f32>,
    language: Option<String>,
    animations_enabled: Option<bool>,
    tab_bar_position: Option<TabBarPosition>,
}

#[derive(Debug, Deserialize, Serialize)]
struct TerminalFileConfig {
    cell_width: Option<f32>,
    cell_height: Option<f32>,
    font_selection: Option<String>,
    #[serde(alias = "font_path")]
    legacy_font_path: Option<String>,
    font_size: Option<f32>,
    padding_x: Option<f32>,
    padding_y: Option<f32>,
    scrollback_lines: Option<usize>,
    bracketed_paste: Option<bool>,
    multiline_paste_confirm: Option<bool>,
    scroll_multiplier: Option<f32>,
    cursor_shape: Option<CursorShape>,
    cursor_blink: Option<bool>,
    bell_mode: Option<BellMode>,
    right_click_action: Option<RightClickAction>,
}

#[derive(Debug, Deserialize, Serialize)]
struct ThemeFileConfig {
    color_scheme: Option<String>,
    foreground: Option<String>,
    background: Option<String>,
    cursor: Option<String>,
    ansi_colors: Option<Vec<String>>,
    background_opacity: Option<f32>,
    blur_enabled: Option<bool>,
    macos_blur_radius: Option<i32>,
}

#[derive(Debug, Deserialize, Serialize)]
struct ShortcutsFileConfig {
    new_tab: Option<String>,
    close_tab: Option<String>,
    open_settings: Option<String>,
    next_tab: Option<String>,
    prev_tab: Option<String>,
    quit: Option<String>,
    font_size_increase: Option<String>,
    font_size_decrease: Option<String>,
    font_size_reset: Option<String>,
    duplicate_tab: Option<String>,
}

#[derive(Debug, Default, Clone)]
pub struct AppConfigUpdates {
    pub window_width: Option<f32>,
    pub window_height: Option<f32>,
    /// `None` = no change; `Some("auto"|"")` = clear; `Some("<tag>")` = set.
    pub language: Option<String>,
    pub animations_enabled: Option<bool>,
    pub tab_bar_position: Option<TabBarPosition>,
    pub terminal_font_selection: Option<String>,
    pub terminal_font_size: Option<f32>,
    pub terminal_padding_x: Option<f32>,
    pub terminal_padding_y: Option<f32>,
    pub color_scheme: Option<String>,
    pub foreground: Option<[u8; 3]>,
    pub background: Option<[u8; 3]>,
    pub cursor: Option<[u8; 3]>,
    pub ansi_colors: Option<[[u8; 3]; 16]>,
    pub background_opacity: Option<f32>,
    pub blur_enabled: Option<bool>,
    pub macos_blur_radius: Option<i32>,
    pub shortcut_new_tab: Option<String>,
    pub shortcut_close_tab: Option<String>,
    pub shortcut_open_settings: Option<String>,
    pub shortcut_next_tab: Option<String>,
    pub shortcut_prev_tab: Option<String>,
    pub shortcut_quit: Option<String>,
    pub shortcut_font_size_increase: Option<String>,
    pub shortcut_font_size_decrease: Option<String>,
    pub shortcut_font_size_reset: Option<String>,
    pub shortcut_duplicate_tab: Option<String>,
    pub terminal_scrollback: Option<usize>,
    pub terminal_bracketed_paste: Option<bool>,
    pub terminal_multiline_paste_confirm: Option<bool>,
    pub terminal_scroll_multiplier: Option<f32>,
    pub terminal_cursor_shape: Option<CursorShape>,
    pub terminal_cursor_blink: Option<bool>,
    pub terminal_bell_mode: Option<BellMode>,
    pub terminal_right_click_action: Option<RightClickAction>,
}

impl Default for AppConfig {
    fn default() -> Self {
        let (cell_width, cell_height) = default_cell_metrics();
        Self {
            ui: UiConfig {
                window_width: DEFAULT_WINDOW_WIDTH,
                window_height: DEFAULT_WINDOW_HEIGHT,
                language: None,
                animations_enabled: DEFAULT_ANIMATIONS_ENABLED,
                tab_bar_position: TabBarPosition::default(),
            },
            terminal: TerminalConfig {
                cell_width,
                cell_height,
                font_selection: None,
                font_size: DEFAULT_TERMINAL_FONT_SIZE,
                padding_x: DEFAULT_TERMINAL_PADDING_X,
                padding_y: DEFAULT_TERMINAL_PADDING_Y,
                scrollback_lines: DEFAULT_TERMINAL_SCROLLBACK,
                bracketed_paste: DEFAULT_BRACKETED_PASTE,
                multiline_paste_confirm: DEFAULT_MULTILINE_PASTE_CONFIRM,
                scroll_multiplier: DEFAULT_TERMINAL_SCROLL_MULTIPLIER,
                cursor_shape: CursorShape::default(),
                cursor_blink: DEFAULT_CURSOR_BLINK,
                bell_mode: BellMode::default(),
                right_click_action: RightClickAction::default(),
            },
            theme: ThemeConfig {
                color_scheme: "Catppuccin Mocha".to_string(),
                foreground: DEFAULT_THEME_FOREGROUND,
                background: DEFAULT_THEME_BACKGROUND,
                cursor: DEFAULT_THEME_CURSOR,
                ansi_colors: None,
                background_opacity: DEFAULT_THEME_BG_OPACITY,
                blur_enabled: DEFAULT_BLUR_ENABLED,
                macos_blur_radius: DEFAULT_MACOS_BLUR_RADIUS,
            },
            shortcuts: ShortcutsConfig {
                new_tab: DEFAULT_SHORTCUT_NEW_TAB.to_string(),
                close_tab: DEFAULT_SHORTCUT_CLOSE_TAB.to_string(),
                open_settings: DEFAULT_SHORTCUT_OPEN_SETTINGS.to_string(),
                next_tab: DEFAULT_SHORTCUT_NEXT_TAB.to_string(),
                prev_tab: DEFAULT_SHORTCUT_PREV_TAB.to_string(),
                quit: DEFAULT_SHORTCUT_QUIT.to_string(),
                font_size_increase: DEFAULT_SHORTCUT_FONT_SIZE_INCREASE.to_string(),
                font_size_decrease: DEFAULT_SHORTCUT_FONT_SIZE_DECREASE.to_string(),
                font_size_reset: DEFAULT_SHORTCUT_FONT_SIZE_RESET.to_string(),
                duplicate_tab: DEFAULT_SHORTCUT_DUPLICATE_TAB.to_string(),
            },
            ssh_profiles: vec![],
        }
    }
}

impl AppConfig {
    pub fn load() -> Self {
        let mut config = Self::default();
        if let Some(path) = config_path() {
            let _ = ensure_config_file(&path);
            if let Ok(contents) = fs::read_to_string(&path)
                && let Ok(file) = toml::from_str::<FileConfig>(&contents)
            {
                config.apply_file(file);
            }
        }
        config
    }

    pub fn apply_updates(&mut self, updates: AppConfigUpdates) {
        if let Some(width) = updates.window_width {
            self.ui.window_width = sanitize_positive(width, self.ui.window_width);
        }
        if let Some(height) = updates.window_height {
            self.ui.window_height = sanitize_positive(height, self.ui.window_height);
        }
        if let Some(lang) = updates.language.as_deref() {
            self.ui.language = sanitize_language(lang);
        }
        if let Some(enabled) = updates.animations_enabled {
            self.ui.animations_enabled = enabled;
        }
        if let Some(position) = updates.tab_bar_position {
            self.ui.tab_bar_position = position;
        }
        let old_font = self.terminal.font_selection.clone();
        if let Some(selection) = updates.terminal_font_selection {
            self.terminal.font_selection = sanitize_terminal_font_selection(&selection);
        }
        if let Some(size) = updates.terminal_font_size {
            self.terminal.font_size = sanitize_terminal_font_size(size, self.terminal.font_size);
        }
        // Recalculate cell metrics if font or size changed
        if self.terminal.font_selection != old_font || updates.terminal_font_size.is_some() {
            let (cw, ch) = cell_metrics_for_selection(
                self.terminal.font_selection.as_deref(),
                self.terminal.font_size,
            );
            self.terminal.cell_width = cw;
            self.terminal.cell_height = ch;
        }
        if let Some(px) = updates.terminal_padding_x {
            self.terminal.padding_x = sanitize_padding(px);
        }
        if let Some(py) = updates.terminal_padding_y {
            self.terminal.padding_y = sanitize_padding(py);
        }
        if let Some(lines) = updates.terminal_scrollback {
            self.terminal.scrollback_lines =
                sanitize_scrollback(lines, self.terminal.scrollback_lines);
        }
        if let Some(enabled) = updates.terminal_bracketed_paste {
            self.terminal.bracketed_paste = enabled;
        }
        if let Some(enabled) = updates.terminal_multiline_paste_confirm {
            self.terminal.multiline_paste_confirm = enabled;
        }
        if let Some(mult) = updates.terminal_scroll_multiplier {
            self.terminal.scroll_multiplier =
                sanitize_scroll_multiplier(mult, self.terminal.scroll_multiplier);
        }
        if let Some(shape) = updates.terminal_cursor_shape {
            self.terminal.cursor_shape = shape;
        }
        if let Some(enabled) = updates.terminal_cursor_blink {
            self.terminal.cursor_blink = enabled;
        }
        if let Some(mode) = updates.terminal_bell_mode {
            self.terminal.bell_mode = mode;
        }
        if let Some(action) = updates.terminal_right_click_action {
            self.terminal.right_click_action = action;
        }
        if let Some(scheme) = updates.color_scheme {
            self.theme.color_scheme = scheme;
        }
        if let Some(foreground) = updates.foreground {
            self.theme.foreground = foreground;
        }
        if let Some(background) = updates.background {
            self.theme.background = background;
        }
        if let Some(cursor) = updates.cursor {
            self.theme.cursor = cursor;
        }
        if let Some(ansi) = updates.ansi_colors {
            self.theme.ansi_colors = Some(ansi);
        }
        if let Some(opacity) = updates.background_opacity {
            self.theme.background_opacity =
                sanitize_opacity(opacity, self.theme.background_opacity);
        }
        if let Some(enabled) = updates.blur_enabled {
            self.theme.blur_enabled = enabled;
        }
        if let Some(radius) = updates.macos_blur_radius {
            self.theme.macos_blur_radius = radius.clamp(0, 100);
        }

        if let Some(value) = updates.shortcut_new_tab {
            self.shortcuts.new_tab = sanitize_shortcut(&value, &self.shortcuts.new_tab);
        }
        if let Some(value) = updates.shortcut_close_tab {
            self.shortcuts.close_tab = sanitize_shortcut(&value, &self.shortcuts.close_tab);
        }
        if let Some(value) = updates.shortcut_open_settings {
            self.shortcuts.open_settings = sanitize_shortcut(&value, &self.shortcuts.open_settings);
        }
        if let Some(value) = updates.shortcut_next_tab {
            self.shortcuts.next_tab = sanitize_shortcut(&value, &self.shortcuts.next_tab);
        }
        if let Some(value) = updates.shortcut_prev_tab {
            self.shortcuts.prev_tab = sanitize_shortcut(&value, &self.shortcuts.prev_tab);
        }
        if let Some(value) = updates.shortcut_quit {
            self.shortcuts.quit = sanitize_shortcut(&value, &self.shortcuts.quit);
        }
        if let Some(value) = updates.shortcut_font_size_increase {
            self.shortcuts.font_size_increase =
                sanitize_shortcut(&value, &self.shortcuts.font_size_increase);
        }
        if let Some(value) = updates.shortcut_font_size_decrease {
            self.shortcuts.font_size_decrease =
                sanitize_shortcut(&value, &self.shortcuts.font_size_decrease);
        }
        if let Some(value) = updates.shortcut_font_size_reset {
            self.shortcuts.font_size_reset =
                sanitize_shortcut(&value, &self.shortcuts.font_size_reset);
        }
        if let Some(value) = updates.shortcut_duplicate_tab {
            self.shortcuts.duplicate_tab = sanitize_shortcut(&value, &self.shortcuts.duplicate_tab);
        }
    }

    pub fn save(&self) -> std::io::Result<()> {
        let Some(path) = config_path() else {
            return Ok(());
        };

        if let Some(parent) = path.parent() {
            fs::create_dir_all(parent)?;
        }

        let file = FileConfig::from(self);
        let contents = toml::to_string_pretty(&file).map_err(std::io::Error::other)?;
        fs::write(path, contents.as_bytes())
    }

    fn apply_file(&mut self, file: FileConfig) {
        if let Some(ui) = file.ui {
            if let Some(width) = ui.window_width {
                self.ui.window_width = sanitize_positive(width, self.ui.window_width);
            }
            if let Some(height) = ui.window_height {
                self.ui.window_height = sanitize_positive(height, self.ui.window_height);
            }
            if let Some(lang) = ui.language.as_deref() {
                self.ui.language = sanitize_language(lang);
            }
            if let Some(enabled) = ui.animations_enabled {
                self.ui.animations_enabled = enabled;
            }
            if let Some(position) = ui.tab_bar_position {
                self.ui.tab_bar_position = position;
            }
        }

        if let Some(term) = file.terminal {
            self.terminal.font_selection = term
                .font_selection
                .as_deref()
                .or(term.legacy_font_path.as_deref())
                .and_then(sanitize_terminal_font_selection);
            if let Some(size) = term.font_size {
                self.terminal.font_size =
                    sanitize_terminal_font_size(size, self.terminal.font_size);
            }

            // Cell dimensions derived from selected font + size
            let (cw, ch) = cell_metrics_for_selection(
                self.terminal.font_selection.as_deref(),
                self.terminal.font_size,
            );
            self.terminal.cell_width = cw;
            self.terminal.cell_height = ch;

            if let Some(px) = term.padding_x {
                self.terminal.padding_x = sanitize_padding(px);
            }
            if let Some(py) = term.padding_y {
                self.terminal.padding_y = sanitize_padding(py);
            }
            if let Some(lines) = term.scrollback_lines {
                self.terminal.scrollback_lines =
                    sanitize_scrollback(lines, self.terminal.scrollback_lines);
            }
            if let Some(enabled) = term.bracketed_paste {
                self.terminal.bracketed_paste = enabled;
            }
            if let Some(enabled) = term.multiline_paste_confirm {
                self.terminal.multiline_paste_confirm = enabled;
            }
            if let Some(mult) = term.scroll_multiplier {
                self.terminal.scroll_multiplier =
                    sanitize_scroll_multiplier(mult, self.terminal.scroll_multiplier);
            }
            if let Some(shape) = term.cursor_shape {
                self.terminal.cursor_shape = shape;
            }
            if let Some(enabled) = term.cursor_blink {
                self.terminal.cursor_blink = enabled;
            }
            if let Some(mode) = term.bell_mode {
                self.terminal.bell_mode = mode;
            }
            if let Some(action) = term.right_click_action {
                self.terminal.right_click_action = action;
            }
        }

        if let Some(theme) = file.theme {
            if let Some(scheme) = theme
                .color_scheme
                .as_deref()
                .map(str::trim)
                .filter(|s| !s.is_empty())
            {
                self.theme.color_scheme = scheme.to_string();
                // If a known preset, apply its colors as base
                if let Some(preset) = crate::terminal::theme::find_preset(scheme) {
                    self.theme.foreground = preset.fg;
                    self.theme.background = preset.bg;
                    self.theme.cursor = preset.cursor;
                    self.theme.ansi_colors = Some(preset.ansi);
                }
            }
            // Explicit colors override preset
            if let Some(foreground) = theme.foreground.as_deref().and_then(parse_hex_color) {
                self.theme.foreground = foreground;
            }
            if let Some(background) = theme.background.as_deref().and_then(parse_hex_color) {
                self.theme.background = background;
            }
            if let Some(cursor) = theme.cursor.as_deref().and_then(parse_hex_color) {
                self.theme.cursor = cursor;
            }
            if let Some(ansi) = theme.ansi_colors.as_ref() {
                let parsed: Vec<_> = ansi.iter().filter_map(|s| parse_hex_color(s)).collect();
                if parsed.len() == 16 {
                    let mut arr = [[0u8; 3]; 16];
                    for (i, c) in parsed.into_iter().enumerate() {
                        arr[i] = c;
                    }
                    self.theme.ansi_colors = Some(arr);
                }
            }
            if let Some(opacity) = theme.background_opacity {
                self.theme.background_opacity =
                    sanitize_opacity(opacity, self.theme.background_opacity);
            }
            if let Some(enabled) = theme.blur_enabled {
                self.theme.blur_enabled = enabled;
            }
            if let Some(radius) = theme.macos_blur_radius {
                self.theme.macos_blur_radius = radius.clamp(0, 100);
            }
        }

        if let Some(shortcuts) = file.shortcuts {
            if let Some(value) = shortcuts.new_tab.as_deref() {
                self.shortcuts.new_tab = sanitize_shortcut(value, &self.shortcuts.new_tab);
            }
            if let Some(value) = shortcuts.close_tab.as_deref() {
                self.shortcuts.close_tab = sanitize_shortcut(value, &self.shortcuts.close_tab);
            }
            if let Some(value) = shortcuts.open_settings.as_deref() {
                self.shortcuts.open_settings =
                    sanitize_shortcut(value, &self.shortcuts.open_settings);
            }
            if let Some(value) = shortcuts.next_tab.as_deref() {
                self.shortcuts.next_tab = sanitize_shortcut(value, &self.shortcuts.next_tab);
            }
            if let Some(value) = shortcuts.prev_tab.as_deref() {
                self.shortcuts.prev_tab = sanitize_shortcut(value, &self.shortcuts.prev_tab);
            }
            if let Some(value) = shortcuts.quit.as_deref() {
                self.shortcuts.quit = sanitize_shortcut(value, &self.shortcuts.quit);
            }
            if let Some(value) = shortcuts.font_size_increase.as_deref() {
                self.shortcuts.font_size_increase =
                    sanitize_shortcut(value, &self.shortcuts.font_size_increase);
            }
            if let Some(value) = shortcuts.font_size_decrease.as_deref() {
                self.shortcuts.font_size_decrease =
                    sanitize_shortcut(value, &self.shortcuts.font_size_decrease);
            }
            if let Some(value) = shortcuts.font_size_reset.as_deref() {
                self.shortcuts.font_size_reset =
                    sanitize_shortcut(value, &self.shortcuts.font_size_reset);
            }
            if let Some(value) = shortcuts.duplicate_tab.as_deref() {
                self.shortcuts.duplicate_tab =
                    sanitize_shortcut(value, &self.shortcuts.duplicate_tab);
            }
        }

        if let Some(profiles) = file.ssh_profiles {
            self.ssh_profiles = profiles
                .into_iter()
                .filter_map(|p| {
                    let host = p.host.as_deref().map(str::trim).unwrap_or("");
                    if host.is_empty() {
                        return None;
                    }
                    let identity_file = p
                        .identity_file
                        .as_deref()
                        .map(str::trim)
                        .filter(|s| !s.is_empty())
                        .map(String::from);
                    let auth_method = p.auth_method.unwrap_or_else(|| {
                        if identity_file.is_some() {
                            SshAuthMethod::KeyFile
                        } else {
                            SshAuthMethod::Password
                        }
                    });

                    Some(SshProfile {
                        name: p
                            .name
                            .as_deref()
                            .map(str::trim)
                            .filter(|s| !s.is_empty())
                            .unwrap_or(host)
                            .to_string(),
                        host: host.to_string(),
                        port: p.port.unwrap_or(22),
                        user: p.user.as_deref().map(str::trim).unwrap_or("").to_string(),
                        auth_method,
                        identity_file: if matches!(auth_method, SshAuthMethod::KeyFile) {
                            identity_file
                        } else {
                            None
                        },
                        password: None,
                        proxy_command: p
                            .proxy_command
                            .as_deref()
                            .map(str::trim)
                            .filter(|s| !s.is_empty())
                            .map(String::from),
                    })
                })
                .collect();
        }
    }
}

fn sanitize_positive(value: f32, fallback: f32) -> f32 {
    if value.is_finite() && value > 0.0 {
        value
    } else {
        fallback
    }
}

fn sanitize_opacity(value: f32, fallback: f32) -> f32 {
    if value.is_finite() && (0.0..=1.0).contains(&value) {
        value
    } else {
        fallback
    }
}

fn sanitize_shortcut(value: &str, fallback: &str) -> String {
    normalize_shortcut(value).unwrap_or_else(|| fallback.to_string())
}

fn sanitize_padding(value: f32) -> f32 {
    if value.is_finite() && value >= 0.0 {
        value.min(100.0)
    } else {
        0.0
    }
}

fn sanitize_language(value: &str) -> Option<String> {
    let trimmed = value.trim();
    if crate::i18n::is_known_locale(trimmed) {
        Some(trimmed.to_string())
    } else {
        None
    }
}

fn sanitize_terminal_font_selection(value: &str) -> Option<String> {
    let selection = value.trim();
    if selection.is_empty() {
        None
    } else {
        Some(selection.to_string())
    }
}

fn sanitize_scrollback(value: usize, fallback: usize) -> usize {
    if (100..=1_000_000).contains(&value) {
        value
    } else {
        fallback
    }
}

fn sanitize_scroll_multiplier(value: f32, fallback: f32) -> f32 {
    if value.is_finite() && (0.1..=10.0).contains(&value) {
        value
    } else {
        fallback
    }
}

fn sanitize_terminal_font_size(value: f32, fallback: f32) -> f32 {
    if value.is_finite() && (6.0..=72.0).contains(&value) {
        value
    } else {
        fallback
    }
}

fn normalize_shortcut(value: &str) -> Option<String> {
    let mut has_ctrl = false;
    let mut has_alt = false;
    let mut has_shift = false;
    let mut has_command = false;
    let mut key: Option<String> = None;

    for token in value.split('+') {
        let token = token.trim();
        if token.is_empty() {
            return None;
        }

        let normalized = token.to_ascii_lowercase();
        match normalized.as_str() {
            "ctrl" | "control" => has_ctrl = true,
            "alt" | "option" => has_alt = true,
            "shift" => has_shift = true,
            "cmd" | "command" | "meta" | "super" => has_command = true,
            _ => {
                if key.is_some() {
                    return None;
                }
                key = normalize_shortcut_key(token);
                key.as_ref()?;
            }
        }
    }

    let key = key?;
    let mut parts: Vec<String> = Vec::new();
    if has_command {
        parts.push("Command".to_string());
    }
    if has_ctrl {
        parts.push("Ctrl".to_string());
    }
    if has_alt {
        parts.push("Alt".to_string());
    }
    if has_shift {
        parts.push("Shift".to_string());
    }
    parts.push(key);

    Some(parts.join("+"))
}

fn normalize_shortcut_key(value: &str) -> Option<String> {
    let lower = value.trim().to_ascii_lowercase();
    let canonical = match lower.as_str() {
        "esc" | "escape" => "Escape",
        "enter" | "return" => "Enter",
        "tab" => "Tab",
        "space" | "spacebar" => "Space",
        "home" => "Home",
        "end" => "End",
        "delete" | "del" => "Delete",
        "backspace" => "Backspace",
        "insert" | "ins" => "Insert",
        "pageup" | "page-up" | "pgup" => "PageUp",
        "pagedown" | "page-down" | "pgdown" => "PageDown",
        "up" | "arrowup" => "ArrowUp",
        "down" | "arrowdown" => "ArrowDown",
        "left" | "arrowleft" => "ArrowLeft",
        "right" | "arrowright" => "ArrowRight",
        "comma" => "Comma",
        "period" | "dot" => "Period",
        "f1" => "F1",
        "f2" => "F2",
        "f3" => "F3",
        "f4" => "F4",
        "f5" => "F5",
        "f6" => "F6",
        "f7" => "F7",
        "f8" => "F8",
        "f9" => "F9",
        "f10" => "F10",
        "f11" => "F11",
        "f12" => "F12",
        _ => {
            if lower.chars().count() == 1 {
                let ch = lower.chars().next()?;
                if ch.is_ascii_alphanumeric() {
                    return Some(ch.to_ascii_uppercase().to_string());
                }
                if matches!(
                    ch,
                    ',' | '.' | '[' | ']' | '/' | ';' | '\'' | '-' | '=' | '`'
                ) {
                    return Some(ch.to_string());
                }
            }
            return None;
        }
    };

    Some(canonical.to_string())
}

pub(crate) fn parse_hex_color(value: &str) -> Option<[u8; 3]> {
    let value = value.trim();
    let value = value.strip_prefix('#').unwrap_or(value);
    let value = value.strip_prefix("0x").unwrap_or(value);
    if value.len() != 6 {
        return None;
    }
    let r = u8::from_str_radix(&value[0..2], 16).ok()?;
    let g = u8::from_str_radix(&value[2..4], 16).ok()?;
    let b = u8::from_str_radix(&value[4..6], 16).ok()?;
    Some([r, g, b])
}

impl From<&AppConfig> for FileConfig {
    fn from(config: &AppConfig) -> Self {
        Self {
            ui: Some(UiFileConfig {
                window_width: Some(config.ui.window_width),
                window_height: Some(config.ui.window_height),
                language: config.ui.language.clone(),
                animations_enabled: Some(config.ui.animations_enabled),
                tab_bar_position: Some(config.ui.tab_bar_position),
            }),
            terminal: Some(TerminalFileConfig {
                cell_width: None,
                cell_height: None,
                font_selection: config.terminal.font_selection.clone(),
                legacy_font_path: None,
                font_size: Some(config.terminal.font_size),
                padding_x: Some(config.terminal.padding_x),
                padding_y: Some(config.terminal.padding_y),
                scrollback_lines: Some(config.terminal.scrollback_lines),
                bracketed_paste: Some(config.terminal.bracketed_paste),
                multiline_paste_confirm: Some(config.terminal.multiline_paste_confirm),
                scroll_multiplier: Some(config.terminal.scroll_multiplier),
                cursor_shape: Some(config.terminal.cursor_shape),
                cursor_blink: Some(config.terminal.cursor_blink),
                bell_mode: Some(config.terminal.bell_mode),
                right_click_action: Some(config.terminal.right_click_action),
            }),
            theme: Some(ThemeFileConfig {
                color_scheme: if config.theme.color_scheme.is_empty() {
                    None
                } else {
                    Some(config.theme.color_scheme.clone())
                },
                foreground: Some(format!(
                    "#{:02x}{:02x}{:02x}",
                    config.theme.foreground[0],
                    config.theme.foreground[1],
                    config.theme.foreground[2]
                )),
                background: Some(format!(
                    "#{:02x}{:02x}{:02x}",
                    config.theme.background[0],
                    config.theme.background[1],
                    config.theme.background[2]
                )),
                cursor: Some(format!(
                    "#{:02x}{:02x}{:02x}",
                    config.theme.cursor[0], config.theme.cursor[1], config.theme.cursor[2]
                )),
                ansi_colors: config.theme.ansi_colors.map(|ansi| {
                    ansi.iter()
                        .map(|c| format!("#{:02x}{:02x}{:02x}", c[0], c[1], c[2]))
                        .collect()
                }),
                background_opacity: Some(config.theme.background_opacity),
                blur_enabled: Some(config.theme.blur_enabled),
                macos_blur_radius: Some(config.theme.macos_blur_radius),
            }),
            shortcuts: Some(ShortcutsFileConfig {
                new_tab: Some(config.shortcuts.new_tab.clone()),
                close_tab: Some(config.shortcuts.close_tab.clone()),
                open_settings: Some(config.shortcuts.open_settings.clone()),
                next_tab: Some(config.shortcuts.next_tab.clone()),
                prev_tab: Some(config.shortcuts.prev_tab.clone()),
                quit: Some(config.shortcuts.quit.clone()),
                font_size_increase: Some(config.shortcuts.font_size_increase.clone()),
                font_size_decrease: Some(config.shortcuts.font_size_decrease.clone()),
                font_size_reset: Some(config.shortcuts.font_size_reset.clone()),
                duplicate_tab: Some(config.shortcuts.duplicate_tab.clone()),
            }),
            ssh_profiles: if config.ssh_profiles.is_empty() {
                None
            } else {
                Some(
                    config
                        .ssh_profiles
                        .iter()
                        .filter(|p| !p.host.trim().is_empty())
                        .map(|p| SshProfileFileConfig {
                            name: Some(p.name.clone()),
                            host: Some(p.host.clone()),
                            port: Some(p.port),
                            user: if p.user.is_empty() {
                                None
                            } else {
                                Some(p.user.clone())
                            },
                            auth_method: Some(p.auth_method),
                            identity_file: if matches!(p.auth_method, SshAuthMethod::KeyFile) {
                                p.identity_file.clone()
                            } else {
                                None
                            },
                            proxy_command: p.proxy_command.clone(),
                        })
                        .collect(),
                )
            },
        }
    }
}

fn config_path() -> Option<PathBuf> {
    Some(dirs::config_dir()?.join("rabbitty").join("config.toml"))
}

fn ensure_config_file(path: &Path) -> std::io::Result<()> {
    if path.exists() {
        return Ok(());
    }
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent)?;
    }
    fs::write(path, default_config_toml().as_bytes())?;
    Ok(())
}

fn default_config_toml() -> String {
    format!(
        "[ui]\nwindow_width = {width}\nwindow_height = {height}\n\n[terminal]\nfont_selection = \"\"\nfont_size = {font_size:.1}\npadding_x = {padding_x:.1}\npadding_y = {padding_y:.1}\nbell_mode = \"sound\"\n\n[theme]\ncolor_scheme = \"Catppuccin Mocha\"\nforeground = \"#{fg:02x}{fg_g:02x}{fg_b:02x}\"\nbackground = \"#{bg:02x}{bg_g:02x}{bg_b:02x}\"\ncursor = \"#{cur:02x}{cur_g:02x}{cur_b:02x}\"\nbackground_opacity = {opacity:.2}\nblur_enabled = {blur_enabled}\nmacos_blur_radius = {macos_blur_radius}\n\n[shortcuts]\nnew_tab = \"{shortcut_new_tab}\"\nclose_tab = \"{shortcut_close_tab}\"\nopen_settings = \"{shortcut_open_settings}\"\nnext_tab = \"{shortcut_next_tab}\"\nprev_tab = \"{shortcut_prev_tab}\"\nquit = \"{shortcut_quit}\"\nfont_size_increase = \"{shortcut_font_size_increase}\"\nfont_size_decrease = \"{shortcut_font_size_decrease}\"\nfont_size_reset = \"{shortcut_font_size_reset}\"\nduplicate_tab = \"{shortcut_duplicate_tab}\"\n",
        width = DEFAULT_WINDOW_WIDTH as u32,
        height = DEFAULT_WINDOW_HEIGHT as u32,
        font_size = DEFAULT_TERMINAL_FONT_SIZE,
        padding_x = DEFAULT_TERMINAL_PADDING_X,
        padding_y = DEFAULT_TERMINAL_PADDING_Y,
        fg = DEFAULT_THEME_FOREGROUND[0],
        fg_g = DEFAULT_THEME_FOREGROUND[1],
        fg_b = DEFAULT_THEME_FOREGROUND[2],
        bg = DEFAULT_THEME_BACKGROUND[0],
        bg_g = DEFAULT_THEME_BACKGROUND[1],
        bg_b = DEFAULT_THEME_BACKGROUND[2],
        cur = DEFAULT_THEME_CURSOR[0],
        cur_g = DEFAULT_THEME_CURSOR[1],
        cur_b = DEFAULT_THEME_CURSOR[2],
        opacity = DEFAULT_THEME_BG_OPACITY,
        blur_enabled = DEFAULT_BLUR_ENABLED,
        macos_blur_radius = DEFAULT_MACOS_BLUR_RADIUS,
        shortcut_new_tab = DEFAULT_SHORTCUT_NEW_TAB,
        shortcut_close_tab = DEFAULT_SHORTCUT_CLOSE_TAB,
        shortcut_open_settings = DEFAULT_SHORTCUT_OPEN_SETTINGS,
        shortcut_next_tab = DEFAULT_SHORTCUT_NEXT_TAB,
        shortcut_prev_tab = DEFAULT_SHORTCUT_PREV_TAB,
        shortcut_quit = DEFAULT_SHORTCUT_QUIT,
        shortcut_font_size_increase = DEFAULT_SHORTCUT_FONT_SIZE_INCREASE,
        shortcut_font_size_decrease = DEFAULT_SHORTCUT_FONT_SIZE_DECREASE,
        shortcut_font_size_reset = DEFAULT_SHORTCUT_FONT_SIZE_RESET,
        shortcut_duplicate_tab = DEFAULT_SHORTCUT_DUPLICATE_TAB
    )
}

pub fn cell_metrics_for_font_size(font_size: f32) -> (f32, f32) {
    let font = FontArc::try_from_slice(DEJAVU_SANS_MONO).expect("font load failed");
    cell_metrics_for_font_arc(&font, font_size)
}

/// Calculate cell metrics using a specific font selection.
/// If `font_selection` is empty or the font can't be loaded, falls back to bundled font.
pub fn cell_metrics_for_selection(font_selection: Option<&str>, font_size: f32) -> (f32, f32) {
    let font = font_selection
        .map(str::trim)
        .filter(|s| !s.is_empty())
        .and_then(crate::terminal::font::load_system_font_by_family);

    match font {
        Some(ref f) => cell_metrics_for_font_arc(f, font_size),
        None => cell_metrics_for_font_size(font_size),
    }
}

fn cell_metrics_for_font_arc(font: &FontArc, font_size: f32) -> (f32, f32) {
    let scale = PxScale::from(font_size);
    let scaled = font.as_scaled(scale);
    let ascent = scaled.ascent();

    let mut min_y = 0.0;
    let mut max_y = 0.0;
    let mut has_bounds = false;
    for code in 32u8..=126u8 {
        let ch = code as char;
        let glyph_id = font.glyph_id(ch);
        let glyph = glyph_id.with_scale_and_position(scale, point(0.0, ascent));
        if let Some(outlined) = font.outline_glyph(glyph) {
            let bounds = outlined.px_bounds();
            if !has_bounds {
                min_y = bounds.min.y;
                max_y = bounds.max.y;
                has_bounds = true;
            } else {
                min_y = min_y.min(bounds.min.y);
                max_y = max_y.max(bounds.max.y);
            }
        }
    }
    let line_height = if has_bounds {
        (max_y - min_y).max(1.0)
    } else {
        scaled.height().max(1.0)
    };

    // For proportional fonts, use max advance across ASCII printable range
    // For monospaced fonts, all advances are the same
    let mut max_advance: f32 = 0.0;
    for code in 32u8..=126u8 {
        let candidate = scaled.h_advance(font.glyph_id(code as char));
        if candidate > max_advance {
            max_advance = candidate;
        }
    }
    if max_advance <= 0.0 {
        max_advance = (line_height * 0.6).max(1.0);
    }

    let cell_height = (font_size / FONT_SCALE_FACTOR).max(1.0);
    let cell_width = max_advance.max(1.0);
    (cell_width, cell_height)
}

fn default_cell_metrics() -> (f32, f32) {
    cell_metrics_for_font_size(DEFAULT_TERMINAL_FONT_SIZE)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parse_hex_color_accepts_valid_formats() {
        assert_eq!(parse_hex_color("#aBcD09"), Some([0xab, 0xcd, 0x09]));
        assert_eq!(parse_hex_color("0x112233"), Some([0x11, 0x22, 0x33]));
        assert_eq!(parse_hex_color("445566"), Some([0x44, 0x55, 0x66]));
    }

    #[test]
    fn parse_hex_color_rejects_invalid_values() {
        assert_eq!(parse_hex_color("#12345"), None);
        assert_eq!(parse_hex_color("#gg0011"), None);
        assert_eq!(parse_hex_color(""), None);
    }

    #[test]
    fn shortcut_normalization_handles_aliases_and_order() {
        assert_eq!(
            normalize_shortcut("control + shift + page-down"),
            Some("Ctrl+Shift+PageDown".to_string())
        );
        assert_eq!(normalize_shortcut("meta+t"), Some("Command+T".to_string()));
    }

    #[test]
    fn shortcut_normalization_rejects_invalid_tokens() {
        assert_eq!(normalize_shortcut("Ctrl+"), None);
        assert_eq!(normalize_shortcut("Ctrl+Tab+X"), None);
        assert_eq!(normalize_shortcut("Shift+UnknownKey"), None);
    }

    #[test]
    fn apply_updates_sanitizes_invalid_values() {
        let mut config = AppConfig::default();
        let original = config.clone();

        config.apply_updates(AppConfigUpdates {
            window_width: Some(-1.0),
            window_height: Some(f32::NAN),
            background_opacity: Some(1.5),
            macos_blur_radius: Some(200),
            shortcut_new_tab: Some("Ctrl+".to_string()),
            shortcut_close_tab: Some("Ctrl+W".to_string()),
            ..Default::default()
        });

        assert_eq!(config.ui.window_width, original.ui.window_width);
        assert_eq!(config.ui.window_height, original.ui.window_height);
        assert_eq!(
            config.theme.background_opacity,
            original.theme.background_opacity
        );
        assert_eq!(config.theme.macos_blur_radius, 100); // clamped from 200
        assert_eq!(config.shortcuts.new_tab, original.shortcuts.new_tab);
        assert_eq!(config.shortcuts.close_tab, "Ctrl+W");
    }

    #[test]
    fn apply_updates_terminal_font_selection_and_size_are_sanitized() {
        let mut config = AppConfig::default();
        let default_size = config.terminal.font_size;

        config.apply_updates(AppConfigUpdates {
            terminal_font_selection: Some(" Fira Code ".to_string()),
            terminal_font_size: Some(18.5),
            ..Default::default()
        });
        assert_eq!(
            config.terminal.font_selection,
            Some("Fira Code".to_string())
        );
        assert_eq!(config.terminal.font_size, 18.5);

        config.apply_updates(AppConfigUpdates {
            terminal_font_selection: Some("   ".to_string()),
            terminal_font_size: Some(4.0),
            ..Default::default()
        });
        assert_eq!(config.terminal.font_selection, None);
        assert_eq!(config.terminal.font_size, 18.5);

        config.apply_updates(AppConfigUpdates {
            terminal_font_size: Some(f32::NAN),
            ..Default::default()
        });
        assert_eq!(config.terminal.font_size, 18.5);

        config.apply_updates(AppConfigUpdates {
            terminal_font_size: Some(default_size),
            ..Default::default()
        });
        assert_eq!(config.terminal.font_size, default_size);
    }

    #[test]
    fn terminal_bell_defaults_to_sound() {
        let config = AppConfig::default();

        assert_eq!(config.terminal.bell_mode, BellMode::Sound);
    }

    #[test]
    fn generated_default_config_exposes_sound_bell_mode() {
        let toml_str = default_config_toml();
        let file = toml::from_str::<FileConfig>(&toml_str).expect("default config should parse");

        assert!(toml_str.contains("bell_mode = \"sound\""));
        assert_eq!(
            file.terminal.and_then(|terminal| terminal.bell_mode),
            Some(BellMode::Sound)
        );
    }

    #[test]
    fn ssh_profile_parsed_from_file_config() {
        let mut config = AppConfig::default();
        let file = toml::from_str::<FileConfig>(
            r#"
            [[ssh_profiles]]
            name = "My Server"
            host = "example.com"
            port = 2222
            user = "admin"
            identity_file = "~/.ssh/id_ed25519"

            [[ssh_profiles]]
            host = "bare.host"
            "#,
        )
        .expect("file config should parse");

        config.apply_file(file);
        assert_eq!(config.ssh_profiles.len(), 2);

        let p0 = &config.ssh_profiles[0];
        assert_eq!(p0.name, "My Server");
        assert_eq!(p0.host, "example.com");
        assert_eq!(p0.port, 2222);
        assert_eq!(p0.user, "admin");
        assert_eq!(p0.auth_method, SshAuthMethod::KeyFile);
        assert_eq!(p0.identity_file.as_deref(), Some("~/.ssh/id_ed25519"));
        assert!(p0.password.is_none()); // password is never in config file

        let p1 = &config.ssh_profiles[1];
        assert_eq!(p1.name, "bare.host"); // name defaults to host
        assert_eq!(p1.port, 22); // default port
        assert_eq!(p1.auth_method, SshAuthMethod::Password);
        assert!(p1.user.is_empty());
    }

    #[test]
    fn ssh_profile_auth_method_parsed_and_serialized() {
        let mut config = AppConfig::default();
        let file = toml::from_str::<FileConfig>(
            r#"
            [[ssh_profiles]]
            host = "key.host"
            auth_method = "key_file"
            identity_file = "~/.ssh/key"

            [[ssh_profiles]]
            host = "password.host"
            auth_method = "password"
            identity_file = "~/.ssh/ignored"
            "#,
        )
        .expect("file config should parse");

        config.apply_file(file);

        assert_eq!(config.ssh_profiles[0].auth_method, SshAuthMethod::KeyFile);
        assert_eq!(config.ssh_profiles[1].auth_method, SshAuthMethod::Password);

        let serialized = toml::to_string_pretty(&FileConfig::from(&config)).unwrap();
        assert!(serialized.contains("auth_method = \"key_file\""));
        assert!(serialized.contains("auth_method = \"password\""));
    }

    #[test]
    fn ssh_profile_proxy_command_parsed_and_serialized() {
        let mut config = AppConfig::default();
        let file = toml::from_str::<FileConfig>(
            r#"
            [[ssh_profiles]]
            host = "remote.example.com"
            proxy_command = "cloudflared access ssh --hostname %h"
            "#,
        )
        .expect("file config should parse");

        config.apply_file(file);

        let profile = &config.ssh_profiles[0];
        assert_eq!(
            profile.proxy_command.as_deref(),
            Some("cloudflared access ssh --hostname %h")
        );

        let serialized = toml::to_string_pretty(&FileConfig::from(&config)).unwrap();
        assert!(serialized.contains("proxy_command = \"cloudflared access ssh --hostname %h\""));
    }

    #[test]
    fn ssh_profile_skips_empty_host() {
        let mut config = AppConfig::default();
        let file = toml::from_str::<FileConfig>(
            r#"
            [[ssh_profiles]]
            name = "No Host"
            host = "  "
            "#,
        )
        .expect("file config should parse");

        config.apply_file(file);
        assert!(config.ssh_profiles.is_empty());
    }

    #[test]
    fn ssh_profile_serialization_excludes_password() {
        let config = AppConfig {
            ssh_profiles: vec![SshProfile {
                name: "test".into(),
                host: "host.com".into(),
                port: 22,
                user: "user".into(),
                auth_method: SshAuthMethod::Password,
                identity_file: None,
                password: Some("secret123".into()),
                proxy_command: None,
            }],
            ..Default::default()
        };

        let file = FileConfig::from(&config);
        let toml_str = toml::to_string_pretty(&file).unwrap();
        assert!(!toml_str.contains("secret123"));
        assert!(!toml_str.contains("password ="));
    }

    #[test]
    fn ssh_profile_serialization_skips_empty_host_profiles() {
        let config = AppConfig {
            ssh_profiles: vec![SshProfile {
                name: "".into(),
                host: "".into(),
                port: 22,
                user: "".into(),
                auth_method: SshAuthMethod::Password,
                identity_file: None,
                password: None,
                proxy_command: None,
            }],
            ..Default::default()
        };

        let file = FileConfig::from(&config);
        let toml_str = toml::to_string_pretty(&file).unwrap();

        assert!(!toml_str.contains("[[ssh_profiles]]"));
    }

    #[test]
    fn scrollback_sanitize_clamps_to_valid_range() {
        let mut config = AppConfig::default();
        assert_eq!(
            config.terminal.scrollback_lines,
            DEFAULT_TERMINAL_SCROLLBACK
        );

        // Value within range is accepted
        config.apply_updates(AppConfigUpdates {
            terminal_scrollback: Some(500),
            ..Default::default()
        });
        assert_eq!(config.terminal.scrollback_lines, 500);

        // Value below minimum keeps previous
        config.apply_updates(AppConfigUpdates {
            terminal_scrollback: Some(99),
            ..Default::default()
        });
        assert_eq!(config.terminal.scrollback_lines, 500);

        // Value above maximum keeps previous
        config.apply_updates(AppConfigUpdates {
            terminal_scrollback: Some(1_000_001),
            ..Default::default()
        });
        assert_eq!(config.terminal.scrollback_lines, 500);

        // Boundary values are accepted
        config.apply_updates(AppConfigUpdates {
            terminal_scrollback: Some(100),
            ..Default::default()
        });
        assert_eq!(config.terminal.scrollback_lines, 100);

        config.apply_updates(AppConfigUpdates {
            terminal_scrollback: Some(1_000_000),
            ..Default::default()
        });
        assert_eq!(config.terminal.scrollback_lines, 1_000_000);
    }

    #[test]
    fn file_config_terminal_font_selection_supports_legacy_alias() {
        let mut config = AppConfig::default();
        let file = toml::from_str::<FileConfig>(
            r#"
            [terminal]
            font_path = "  Legacy Font  "
            font_size = 15.0
            "#,
        )
        .expect("file config should parse");

        config.apply_file(file);
        assert_eq!(
            config.terminal.font_selection,
            Some("Legacy Font".to_string())
        );
        assert_eq!(config.terminal.font_size, 15.0);
    }

    #[test]
    fn file_config_terminal_font_selection_prefers_new_key() {
        let mut config = AppConfig::default();
        let file = toml::from_str::<FileConfig>(
            r#"
            [terminal]
            font_selection = " JetBrains Mono "
            font_path = " Legacy Font "
            "#,
        )
        .expect("file config should parse");

        config.apply_file(file);
        assert_eq!(
            config.terminal.font_selection,
            Some("JetBrains Mono".to_string())
        );
    }
}

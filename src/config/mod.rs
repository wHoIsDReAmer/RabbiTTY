mod defaults;
mod file;
mod metrics;
mod sanitize;
mod types;
mod updates;

pub use defaults::*;
pub use metrics::cell_metrics_for_selection;
pub use types::{
    BellMode, CursorShape, RightClickAction, SshAuthMethod, SshProfile, TabBarPosition,
};
pub use updates::AppConfigUpdates;

pub(crate) use sanitize::parse_hex_color;

use crate::gui::tab::Profile;
use file::{FileConfig, config_path, ensure_config_file};
use metrics::default_cell_metrics;
use sanitize::*;
use std::fs;

#[derive(Debug, Clone)]
pub struct AppConfig {
    pub ui: UiConfig,
    pub terminal: TerminalConfig,
    pub theme: ThemeConfig,
    pub shortcuts: ShortcutsConfig,
    pub profiles: Vec<Profile>,
}

impl AppConfig {
    /// The SSH connections among the configured profiles.
    pub fn ssh_profiles(&self) -> Vec<SshProfile> {
        self.profiles
            .iter()
            .filter_map(|p| p.ssh_profile().cloned())
            .collect()
    }

    /// Replace the SSH profiles, preserving local profiles and their order.
    pub fn set_ssh_profiles(&mut self, ssh: Vec<SshProfile>) {
        let mut locals: Vec<Profile> = self
            .profiles
            .iter()
            .filter(|p| p.ssh_profile().is_none())
            .cloned()
            .collect();
        locals.extend(ssh.into_iter().map(Profile::ssh));
        self.profiles = locals;
    }
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
    pub bold_is_bright: bool,
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
                bold_is_bright: DEFAULT_BOLD_IS_BRIGHT,
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
            profiles: vec![],
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
            if let Some(enabled) = term.bold_is_bright {
                self.terminal.bold_is_bright = enabled;
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

        // New unified `[[profiles]]` wins; otherwise migrate legacy
        // `[[ssh_profiles]]`. Once saved, only `profiles` is emitted.
        if let Some(profiles) = file.profiles {
            self.profiles = profiles
                .into_iter()
                .filter(|p| {
                    p.ssh_profile()
                        .is_none_or(|ssh| !ssh.host.trim().is_empty())
                })
                .collect();
        } else if let Some(legacy) = file.ssh_profiles {
            self.profiles = legacy
                .into_iter()
                .filter_map(parse_legacy_ssh_profile)
                .map(Profile::ssh)
                .collect();
        }
    }
}

fn parse_legacy_ssh_profile(p: file::SshProfileFileConfig) -> Option<SshProfile> {
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
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn terminal_bell_defaults_to_sound() {
        let config = AppConfig::default();

        assert_eq!(config.terminal.bell_mode, BellMode::Sound);
    }
}

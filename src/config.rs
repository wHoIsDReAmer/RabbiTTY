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
pub const DEFAULT_MACOS_BLUR_MATERIAL: &str = "sidebar";
pub const DEFAULT_MACOS_BLUR_ALPHA: f32 = 1.0;

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

const DEFAULT_FONT_PX: f32 = 14.0;
const DEJAVU_SANS_MONO: &[u8] = include_bytes!("../fonts/DejaVuSansMono.ttf");

#[derive(Debug, Clone)]
pub struct AppConfig {
    pub ui: UiConfig,
    pub terminal: TerminalConfig,
    pub theme: ThemeConfig,
    pub shortcuts: ShortcutsConfig,
}

#[derive(Debug, Clone)]
pub struct UiConfig {
    pub window_width: f32,
    pub window_height: f32,
}

#[derive(Debug, Clone)]
pub struct TerminalConfig {
    pub cell_width: f32,
    pub cell_height: f32,
}

#[derive(Debug, Clone)]
pub struct ThemeConfig {
    pub foreground: [u8; 3],
    pub background: [u8; 3],
    pub cursor: [u8; 3],
    pub background_opacity: f32,
    pub blur_enabled: bool,
    pub macos_blur_material: String,
    pub macos_blur_alpha: f32,
}

#[derive(Debug, Clone)]
pub struct ShortcutsConfig {
    pub new_tab: String,
    pub close_tab: String,
    pub open_settings: String,
    pub next_tab: String,
    pub prev_tab: String,
    pub quit: String,
}

#[derive(Debug, Deserialize, Serialize)]
struct FileConfig {
    ui: Option<UiFileConfig>,
    terminal: Option<TerminalFileConfig>,
    theme: Option<ThemeFileConfig>,
    shortcuts: Option<ShortcutsFileConfig>,
}

#[derive(Debug, Deserialize, Serialize)]
struct UiFileConfig {
    window_width: Option<f32>,
    window_height: Option<f32>,
}

#[derive(Debug, Deserialize, Serialize)]
struct TerminalFileConfig {
    cell_width: Option<f32>,
    cell_height: Option<f32>,
}

#[derive(Debug, Deserialize, Serialize)]
struct ThemeFileConfig {
    foreground: Option<String>,
    background: Option<String>,
    cursor: Option<String>,
    background_opacity: Option<f32>,
    blur_enabled: Option<bool>,
    macos_blur_material: Option<String>,
    macos_blur_alpha: Option<f32>,
}

#[derive(Debug, Deserialize, Serialize)]
struct ShortcutsFileConfig {
    new_tab: Option<String>,
    close_tab: Option<String>,
    open_settings: Option<String>,
    next_tab: Option<String>,
    prev_tab: Option<String>,
    quit: Option<String>,
}

#[derive(Debug, Default, Clone)]
pub struct AppConfigUpdates {
    pub window_width: Option<f32>,
    pub window_height: Option<f32>,
    pub cell_width: Option<f32>,
    pub cell_height: Option<f32>,
    pub foreground: Option<[u8; 3]>,
    pub background: Option<[u8; 3]>,
    pub cursor: Option<[u8; 3]>,
    pub background_opacity: Option<f32>,
    pub blur_enabled: Option<bool>,
    pub macos_blur_material: Option<String>,
    pub macos_blur_alpha: Option<f32>,
    pub shortcut_new_tab: Option<String>,
    pub shortcut_close_tab: Option<String>,
    pub shortcut_open_settings: Option<String>,
    pub shortcut_next_tab: Option<String>,
    pub shortcut_prev_tab: Option<String>,
    pub shortcut_quit: Option<String>,
}

impl Default for AppConfig {
    fn default() -> Self {
        let (cell_width, cell_height) = default_cell_metrics();
        Self {
            ui: UiConfig {
                window_width: DEFAULT_WINDOW_WIDTH,
                window_height: DEFAULT_WINDOW_HEIGHT,
            },
            terminal: TerminalConfig {
                cell_width,
                cell_height,
            },
            theme: ThemeConfig {
                foreground: DEFAULT_THEME_FOREGROUND,
                background: DEFAULT_THEME_BACKGROUND,
                cursor: DEFAULT_THEME_CURSOR,
                background_opacity: DEFAULT_THEME_BG_OPACITY,
                blur_enabled: DEFAULT_BLUR_ENABLED,
                macos_blur_material: DEFAULT_MACOS_BLUR_MATERIAL.to_string(),
                macos_blur_alpha: DEFAULT_MACOS_BLUR_ALPHA,
            },
            shortcuts: ShortcutsConfig {
                new_tab: DEFAULT_SHORTCUT_NEW_TAB.to_string(),
                close_tab: DEFAULT_SHORTCUT_CLOSE_TAB.to_string(),
                open_settings: DEFAULT_SHORTCUT_OPEN_SETTINGS.to_string(),
                next_tab: DEFAULT_SHORTCUT_NEXT_TAB.to_string(),
                prev_tab: DEFAULT_SHORTCUT_PREV_TAB.to_string(),
                quit: DEFAULT_SHORTCUT_QUIT.to_string(),
            },
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
        if let Some(width) = updates.cell_width {
            self.terminal.cell_width = sanitize_positive(width, self.terminal.cell_width);
        }
        if let Some(height) = updates.cell_height {
            self.terminal.cell_height = sanitize_positive(height, self.terminal.cell_height);
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
        if let Some(opacity) = updates.background_opacity {
            self.theme.background_opacity =
                sanitize_opacity(opacity, self.theme.background_opacity);
        }
        if let Some(enabled) = updates.blur_enabled {
            self.theme.blur_enabled = enabled;
        }
        if let Some(material) = updates.macos_blur_material {
            self.theme.macos_blur_material =
                sanitize_macos_material(&material, &self.theme.macos_blur_material);
        }
        if let Some(alpha) = updates.macos_blur_alpha {
            self.theme.macos_blur_alpha = sanitize_opacity(alpha, self.theme.macos_blur_alpha);
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
        }

        if let Some(term) = file.terminal {
            let mut cell_width = self.terminal.cell_width;
            let mut cell_height = self.terminal.cell_height;

            if let Some(width) = term.cell_width {
                cell_width = sanitize_positive(width, cell_width);
            }
            if let Some(height) = term.cell_height {
                cell_height = sanitize_positive(height, cell_height);
            }

            self.terminal.cell_width = cell_width;
            self.terminal.cell_height = cell_height;
        }

        if let Some(theme) = file.theme {
            if let Some(foreground) = theme.foreground.as_deref().and_then(parse_hex_color) {
                self.theme.foreground = foreground;
            }
            if let Some(background) = theme.background.as_deref().and_then(parse_hex_color) {
                self.theme.background = background;
            }
            if let Some(cursor) = theme.cursor.as_deref().and_then(parse_hex_color) {
                self.theme.cursor = cursor;
            }
            if let Some(opacity) = theme.background_opacity {
                self.theme.background_opacity =
                    sanitize_opacity(opacity, self.theme.background_opacity);
            }
            if let Some(enabled) = theme.blur_enabled {
                self.theme.blur_enabled = enabled;
            }
            if let Some(material) = theme.macos_blur_material.as_deref() {
                self.theme.macos_blur_material =
                    sanitize_macos_material(material, &self.theme.macos_blur_material);
            }
            if let Some(alpha) = theme.macos_blur_alpha {
                self.theme.macos_blur_alpha = sanitize_opacity(alpha, self.theme.macos_blur_alpha);
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

fn sanitize_macos_material(value: &str, fallback: &str) -> String {
    let normalized = value.trim().to_ascii_lowercase();
    match normalized.as_str() {
        "titlebar"
        | "selection"
        | "menu"
        | "popover"
        | "sidebar"
        | "headerview"
        | "sheet"
        | "windowbackground"
        | "hudwindow"
        | "fullscreenui"
        | "tooltip"
        | "contentbackground"
        | "underwindowbackground"
        | "underpagebackground" => normalized,
        _ => fallback.to_string(),
    }
}

fn sanitize_shortcut(value: &str, fallback: &str) -> String {
    normalize_shortcut(value).unwrap_or_else(|| fallback.to_string())
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
            }),
            terminal: Some(TerminalFileConfig {
                cell_width: Some(config.terminal.cell_width),
                cell_height: Some(config.terminal.cell_height),
            }),
            theme: Some(ThemeFileConfig {
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
                background_opacity: Some(config.theme.background_opacity),
                blur_enabled: Some(config.theme.blur_enabled),
                macos_blur_material: Some(config.theme.macos_blur_material.clone()),
                macos_blur_alpha: Some(config.theme.macos_blur_alpha),
            }),
            shortcuts: Some(ShortcutsFileConfig {
                new_tab: Some(config.shortcuts.new_tab.clone()),
                close_tab: Some(config.shortcuts.close_tab.clone()),
                open_settings: Some(config.shortcuts.open_settings.clone()),
                next_tab: Some(config.shortcuts.next_tab.clone()),
                prev_tab: Some(config.shortcuts.prev_tab.clone()),
                quit: Some(config.shortcuts.quit.clone()),
            }),
        }
    }
}

fn config_path() -> Option<PathBuf> {
    let home = dirs::home_dir()?;
    Some(home.join(".config").join("rabitty").join("config.toml"))
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
    let (cell_width, cell_height) = default_cell_metrics();
    format!(
        "[ui]\nwindow_width = {width}\nwindow_height = {height}\n\n[terminal]\ncell_width = {cell_width:.1}\ncell_height = {cell_height:.1}\n\n[theme]\nforeground = \"#{fg:02x}{fg_g:02x}{fg_b:02x}\"\nbackground = \"#{bg:02x}{bg_g:02x}{bg_b:02x}\"\ncursor = \"#{cur:02x}{cur_g:02x}{cur_b:02x}\"\nbackground_opacity = {opacity:.2}\nblur_enabled = {blur_enabled}\nmacos_blur_material = \"{macos_blur_material}\"\nmacos_blur_alpha = {macos_blur_alpha:.2}\n\n[shortcuts]\nnew_tab = \"{shortcut_new_tab}\"\nclose_tab = \"{shortcut_close_tab}\"\nopen_settings = \"{shortcut_open_settings}\"\nnext_tab = \"{shortcut_next_tab}\"\nprev_tab = \"{shortcut_prev_tab}\"\nquit = \"{shortcut_quit}\"\n",
        width = DEFAULT_WINDOW_WIDTH as u32,
        height = DEFAULT_WINDOW_HEIGHT as u32,
        cell_width = cell_width,
        cell_height = cell_height,
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
        macos_blur_material = DEFAULT_MACOS_BLUR_MATERIAL,
        macos_blur_alpha = DEFAULT_MACOS_BLUR_ALPHA,
        shortcut_new_tab = DEFAULT_SHORTCUT_NEW_TAB,
        shortcut_close_tab = DEFAULT_SHORTCUT_CLOSE_TAB,
        shortcut_open_settings = DEFAULT_SHORTCUT_OPEN_SETTINGS,
        shortcut_next_tab = DEFAULT_SHORTCUT_NEXT_TAB,
        shortcut_prev_tab = DEFAULT_SHORTCUT_PREV_TAB,
        shortcut_quit = DEFAULT_SHORTCUT_QUIT
    )
}

fn default_cell_metrics() -> (f32, f32) {
    let font = FontArc::try_from_slice(DEJAVU_SANS_MONO).expect("font load failed");
    let scale = PxScale::from(DEFAULT_FONT_PX);
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

    let mut advance: f32 = 0.0;
    for ch in ['M', 'W', '0', ' '].into_iter() {
        let candidate = scaled.h_advance(font.glyph_id(ch));
        if candidate > 0.0 {
            advance = candidate;
            break;
        }
    }
    if advance <= 0.0 {
        advance = (line_height * 0.6).max(1.0);
    }

    let cell_height = (DEFAULT_FONT_PX / FONT_SCALE_FACTOR).max(1.0);
    let cell_width = advance.max(1.0);
    (cell_width, cell_height)
}

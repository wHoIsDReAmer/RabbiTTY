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
const DEFAULT_FONT_PX: f32 = 14.0;
const DEJAVU_SANS_MONO: &[u8] = include_bytes!("../fonts/DejaVuSansMono.ttf");

#[derive(Debug, Clone)]
pub struct AppConfig {
    pub ui: UiConfig,
    pub terminal: TerminalConfig,
    pub theme: ThemeConfig,
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
}

#[derive(Debug, Deserialize, Serialize)]
struct FileConfig {
    ui: Option<UiFileConfig>,
    terminal: Option<TerminalFileConfig>,
    theme: Option<ThemeFileConfig>,
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
    }

    pub fn save(&self) -> std::io::Result<()> {
        let Some(path) = config_path() else {
            return Ok(());
        };

        if let Some(parent) = path.parent() {
            fs::create_dir_all(parent)?;
        }

        let file = FileConfig::from(self);
        let contents = toml::to_string_pretty(&file)
            .map_err(std::io::Error::other)?;
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
                    config.theme.cursor[0],
                    config.theme.cursor[1],
                    config.theme.cursor[2]
                )),
                background_opacity: Some(config.theme.background_opacity),
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
        "[ui]\nwindow_width = {width}\nwindow_height = {height}\n\n[terminal]\ncell_width = {cell_width:.1}\ncell_height = {cell_height:.1}\n\n[theme]\nforeground = \"#{fg:02x}{fg_g:02x}{fg_b:02x}\"\nbackground = \"#{bg:02x}{bg_g:02x}{bg_b:02x}\"\ncursor = \"#{cur:02x}{cur_g:02x}{cur_b:02x}\"\nbackground_opacity = {opacity:.2}\n",
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
        opacity = DEFAULT_THEME_BG_OPACITY
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

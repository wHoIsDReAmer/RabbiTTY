use ab_glyph::{Font, FontArc, PxScale, ScaleFont, point};
use serde::Deserialize;
use std::fs;
use std::path::{Path, PathBuf};

pub const DEFAULT_WINDOW_WIDTH: f32 = 600.0;
pub const DEFAULT_WINDOW_HEIGHT: f32 = 350.0;
pub const FONT_SCALE_FACTOR: f32 = 0.85;
const DEFAULT_FONT_PX: f32 = 14.0;
const DEJAVU_SANS_MONO: &[u8] = include_bytes!("../fonts/DejaVuSansMono.ttf");

#[derive(Debug, Clone)]
pub struct AppConfig {
    pub ui: UiConfig,
    pub terminal: TerminalConfig,
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

#[derive(Debug, Deserialize)]
struct FileConfig {
    ui: Option<UiFileConfig>,
    terminal: Option<TerminalFileConfig>,
}

#[derive(Debug, Deserialize)]
struct UiFileConfig {
    window_width: Option<f32>,
    window_height: Option<f32>,
}

#[derive(Debug, Deserialize)]
struct TerminalFileConfig {
    cell_width: Option<f32>,
    cell_height: Option<f32>,
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
    }
}

fn sanitize_positive(value: f32, fallback: f32) -> f32 {
    if value.is_finite() && value > 0.0 {
        value
    } else {
        fallback
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
        "[ui]\nwindow_width = {width}\nwindow_height = {height}\n\n[terminal]\ncell_width = {cell_width:.1}\ncell_height = {cell_height:.1}\n",
        width = DEFAULT_WINDOW_WIDTH as u32,
        height = DEFAULT_WINDOW_HEIGHT as u32,
        cell_width = cell_width,
        cell_height = cell_height
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

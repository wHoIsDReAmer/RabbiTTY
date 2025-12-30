use serde::Deserialize;
use std::fs;
use std::path::{Path, PathBuf};

pub const DEFAULT_WINDOW_WIDTH: f32 = 600.0;
pub const DEFAULT_WINDOW_HEIGHT: f32 = 350.0;
pub const DEFAULT_CELL_WIDTH: f32 = 9.0;
pub const DEFAULT_CELL_HEIGHT: f32 = 18.0;
pub const FONT_SCALE_FACTOR: f32 = 0.85;
const DEFAULT_CONFIG_TOML: &str = r#"[ui]
window_width = 600
window_height = 350

[terminal]
cell_width = 9.0
cell_height = 18.0
# font_px = 14.0
"#;

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
    font_px: Option<f32>,
}

impl Default for AppConfig {
    fn default() -> Self {
        Self {
            ui: UiConfig {
                window_width: DEFAULT_WINDOW_WIDTH,
                window_height: DEFAULT_WINDOW_HEIGHT,
            },
            terminal: TerminalConfig {
                cell_width: DEFAULT_CELL_WIDTH,
                cell_height: DEFAULT_CELL_HEIGHT,
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
                && let Ok(file) = toml::from_str::<FileConfig>(&contents) {
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

            if term.cell_height.is_none()
                && let Some(font_px) = term.font_px {
                    let derived_height =
                        sanitize_positive(font_px / FONT_SCALE_FACTOR, cell_height);
                    cell_height = derived_height;
                    if term.cell_width.is_none() {
                        let ratio = DEFAULT_CELL_WIDTH / DEFAULT_CELL_HEIGHT;
                        cell_width = (cell_height * ratio).max(1.0);
                    }
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
    fs::write(path, DEFAULT_CONFIG_TOML.as_bytes())?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn derives_cell_metrics_from_font_px() {
        let contents = r#"
[terminal]
font_px = 14.0
"#;

        let file: FileConfig = toml::from_str(contents).expect("parse config");
        let mut config = AppConfig::default();
        config.apply_file(file);

        let expected_height = 14.0 / FONT_SCALE_FACTOR;
        let expected_width = expected_height * (DEFAULT_CELL_WIDTH / DEFAULT_CELL_HEIGHT);

        assert!((config.terminal.cell_height - expected_height).abs() < 0.01);
        assert!((config.terminal.cell_width - expected_width).abs() < 0.01);
    }
}

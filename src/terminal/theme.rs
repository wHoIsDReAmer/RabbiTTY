use crate::config::AppConfig;
use alacritty_terminal::term::cell::Flags;
use alacritty_terminal::term::color::Colors;
use alacritty_terminal::vte::ansi::{Color as AnsiColor, NamedColor, Rgb};
use serde::Deserialize;
use std::sync::OnceLock;

#[derive(Debug, Clone)]
pub struct TerminalTheme {
    pub(super) foreground: Rgb,
    pub(super) background: Rgb,
    cursor: Rgb,
    ansi: [Rgb; 16],
}

/// A named color preset with foreground, background, cursor, and 16 ANSI colors.
#[derive(Debug, Clone)]
pub struct ColorPreset {
    pub name: String,
    pub fg: [u8; 3],
    pub bg: [u8; 3],
    pub cursor: [u8; 3],
    pub ansi: [[u8; 3]; 16],
}

/// TOML representation of a custom theme file.
#[derive(Debug, Deserialize)]
struct ThemeToml {
    name: String,
    foreground: String,
    background: String,
    cursor: String,
    ansi: AnsiToml,
}

#[derive(Debug, Deserialize)]
struct AnsiToml {
    black: String,
    red: String,
    green: String,
    yellow: String,
    blue: String,
    magenta: String,
    cyan: String,
    white: String,
    bright_black: String,
    bright_red: String,
    bright_green: String,
    bright_yellow: String,
    bright_blue: String,
    bright_magenta: String,
    bright_cyan: String,
    bright_white: String,
}

impl ThemeToml {
    fn to_preset(&self) -> Option<ColorPreset> {
        Some(ColorPreset {
            name: self.name.clone(),
            fg: parse_hex(&self.foreground)?,
            bg: parse_hex(&self.background)?,
            cursor: parse_hex(&self.cursor)?,
            ansi: [
                parse_hex(&self.ansi.black)?,
                parse_hex(&self.ansi.red)?,
                parse_hex(&self.ansi.green)?,
                parse_hex(&self.ansi.yellow)?,
                parse_hex(&self.ansi.blue)?,
                parse_hex(&self.ansi.magenta)?,
                parse_hex(&self.ansi.cyan)?,
                parse_hex(&self.ansi.white)?,
                parse_hex(&self.ansi.bright_black)?,
                parse_hex(&self.ansi.bright_red)?,
                parse_hex(&self.ansi.bright_green)?,
                parse_hex(&self.ansi.bright_yellow)?,
                parse_hex(&self.ansi.bright_blue)?,
                parse_hex(&self.ansi.bright_magenta)?,
                parse_hex(&self.ansi.bright_cyan)?,
                parse_hex(&self.ansi.bright_white)?,
            ],
        })
    }
}

fn parse_hex(s: &str) -> Option<[u8; 3]> {
    crate::config::parse_hex_color(s)
}

static ALL_PRESETS: OnceLock<Vec<ColorPreset>> = OnceLock::new();

/// Returns all color presets (built-in + user custom themes)
pub fn all_presets() -> &'static [ColorPreset] {
    ALL_PRESETS.get_or_init(|| {
        let mut presets = builtin_presets();
        let custom = load_custom_presets();
        for cp in custom {
            // Custom themes override built-in themes with the same name
            if let Some(existing) = presets
                .iter_mut()
                .find(|p| p.name.eq_ignore_ascii_case(&cp.name))
            {
                *existing = cp;
            } else {
                presets.push(cp);
            }
        }
        presets
    })
}

#[allow(dead_code)]
pub fn reload_presets() {
    let _ = all_presets();
}

pub fn find_preset(name: &str) -> Option<&'static ColorPreset> {
    all_presets()
        .iter()
        .find(|p| p.name.eq_ignore_ascii_case(name))
}

fn load_custom_presets() -> Vec<ColorPreset> {
    let Some(themes_dir) = dirs::config_dir().map(|d| d.join("rabbitty").join("themes")) else {
        return vec![];
    };
    let Ok(entries) = std::fs::read_dir(&themes_dir) else {
        return vec![];
    };
    let mut presets = Vec::new();
    for entry in entries.flatten() {
        let path = entry.path();
        if path.extension().is_some_and(|ext| ext == "toml") {
            match std::fs::read_to_string(&path) {
                Ok(content) => match toml::from_str::<ThemeToml>(&content) {
                    Ok(theme_toml) => {
                        if let Some(preset) = theme_toml.to_preset() {
                            presets.push(preset);
                        } else {
                            eprintln!(
                                "Warning: invalid color values in theme file: {}",
                                path.display()
                            );
                        }
                    }
                    Err(e) => {
                        eprintln!(
                            "Warning: failed to parse theme file {}: {}",
                            path.display(),
                            e
                        );
                    }
                },
                Err(e) => {
                    eprintln!(
                        "Warning: failed to read theme file {}: {}",
                        path.display(),
                        e
                    );
                }
            }
        }
    }
    presets
}

fn builtin_presets() -> Vec<ColorPreset> {
    vec![
        ColorPreset {
            name: "Catppuccin Mocha".into(),
            fg: [0xcd, 0xd6, 0xf4],
            bg: [0x1e, 0x1e, 0x2e],
            cursor: [0xf5, 0xe0, 0xdc],
            ansi: [
                [0x45, 0x47, 0x5a], // black
                [0xf3, 0x8b, 0xa8], // red
                [0xa6, 0xe3, 0xa1], // green
                [0xf9, 0xe2, 0xaf], // yellow
                [0x89, 0xb4, 0xfa], // blue
                [0xf5, 0xc2, 0xe7], // magenta
                [0x94, 0xe2, 0xd5], // cyan
                [0xba, 0xc2, 0xde], // white
                [0x58, 0x5b, 0x70], // bright black
                [0xf3, 0x8b, 0xa8], // bright red
                [0xa6, 0xe3, 0xa1], // bright green
                [0xf9, 0xe2, 0xaf], // bright yellow
                [0x89, 0xb4, 0xfa], // bright blue
                [0xf5, 0xc2, 0xe7], // bright magenta
                [0x94, 0xe2, 0xd5], // bright cyan
                [0xa6, 0xad, 0xc8], // bright white
            ],
        },
        ColorPreset {
            name: "Dracula".into(),
            fg: [0xf8, 0xf8, 0xf2],
            bg: [0x28, 0x2a, 0x36],
            cursor: [0xf8, 0xf8, 0xf2],
            ansi: [
                [0x21, 0x22, 0x2c], // black
                [0xff, 0x55, 0x55], // red
                [0x50, 0xfa, 0x7b], // green
                [0xf1, 0xfa, 0x8c], // yellow
                [0xbd, 0x93, 0xf9], // blue
                [0xff, 0x79, 0xc6], // magenta
                [0x8b, 0xe9, 0xfd], // cyan
                [0xf8, 0xf8, 0xf2], // white
                [0x62, 0x72, 0xa4], // bright black
                [0xff, 0x6e, 0x6e], // bright red
                [0x69, 0xff, 0x94], // bright green
                [0xff, 0xff, 0xa5], // bright yellow
                [0xd6, 0xac, 0xff], // bright blue
                [0xff, 0x92, 0xdf], // bright magenta
                [0xa4, 0xff, 0xff], // bright cyan
                [0xff, 0xff, 0xff], // bright white
            ],
        },
        ColorPreset {
            name: "Tokyo Night".into(),
            fg: [0xa9, 0xb1, 0xd6],
            bg: [0x1a, 0x1b, 0x26],
            cursor: [0xc0, 0xca, 0xf5],
            ansi: [
                [0x15, 0x16, 0x1e], // black
                [0xf7, 0x76, 0x8e], // red
                [0x9e, 0xce, 0x6a], // green
                [0xe0, 0xaf, 0x68], // yellow
                [0x7a, 0xa2, 0xf7], // blue
                [0xbb, 0x9a, 0xf7], // magenta
                [0x7d, 0xcf, 0xff], // cyan
                [0xa9, 0xb1, 0xd6], // white
                [0x41, 0x48, 0x68], // bright black
                [0xf7, 0x76, 0x8e], // bright red
                [0x9e, 0xce, 0x6a], // bright green
                [0xe0, 0xaf, 0x68], // bright yellow
                [0x7a, 0xa2, 0xf7], // bright blue
                [0xbb, 0x9a, 0xf7], // bright magenta
                [0x7d, 0xcf, 0xff], // bright cyan
                [0xc0, 0xca, 0xf5], // bright white
            ],
        },
        ColorPreset {
            name: "Nord".into(),
            fg: [0xd8, 0xde, 0xe9],
            bg: [0x2e, 0x34, 0x40],
            cursor: [0xd8, 0xde, 0xe9],
            ansi: [
                [0x3b, 0x42, 0x52], // black
                [0xbf, 0x61, 0x6a], // red
                [0xa3, 0xbe, 0x8c], // green
                [0xeb, 0xcb, 0x8b], // yellow
                [0x81, 0xa1, 0xc1], // blue
                [0xb4, 0x8e, 0xad], // magenta
                [0x88, 0xc0, 0xd0], // cyan
                [0xe5, 0xe9, 0xf0], // white
                [0x4c, 0x56, 0x6a], // bright black
                [0xbf, 0x61, 0x6a], // bright red
                [0xa3, 0xbe, 0x8c], // bright green
                [0xeb, 0xcb, 0x8b], // bright yellow
                [0x81, 0xa1, 0xc1], // bright blue
                [0xb4, 0x8e, 0xad], // bright magenta
                [0x8f, 0xbc, 0xbb], // bright cyan
                [0xec, 0xef, 0xf4], // bright white
            ],
        },
        ColorPreset {
            name: "One Dark".into(),
            fg: [0xab, 0xb2, 0xbf],
            bg: [0x28, 0x2c, 0x34],
            cursor: [0x52, 0x8b, 0xff],
            ansi: [
                [0x28, 0x2c, 0x34], // black
                [0xe0, 0x6c, 0x75], // red
                [0x98, 0xc3, 0x79], // green
                [0xe5, 0xc0, 0x7b], // yellow
                [0x61, 0xaf, 0xef], // blue
                [0xc6, 0x78, 0xdd], // magenta
                [0x56, 0xb6, 0xc2], // cyan
                [0xab, 0xb2, 0xbf], // white
                [0x54, 0x58, 0x62], // bright black
                [0xe0, 0x6c, 0x75], // bright red
                [0x98, 0xc3, 0x79], // bright green
                [0xe5, 0xc0, 0x7b], // bright yellow
                [0x61, 0xaf, 0xef], // bright blue
                [0xc6, 0x78, 0xdd], // bright magenta
                [0x56, 0xb6, 0xc2], // bright cyan
                [0xff, 0xff, 0xff], // bright white
            ],
        },
        ColorPreset {
            name: "Gruvbox Dark".into(),
            fg: [0xeb, 0xdb, 0xb2],
            bg: [0x28, 0x28, 0x28],
            cursor: [0xeb, 0xdb, 0xb2],
            ansi: [
                [0x28, 0x28, 0x28], // black
                [0xcc, 0x24, 0x1d], // red
                [0x98, 0x97, 0x1a], // green
                [0xd7, 0x99, 0x21], // yellow
                [0x45, 0x85, 0x88], // blue
                [0xb1, 0x62, 0x86], // magenta
                [0x68, 0x9d, 0x6a], // cyan
                [0xa8, 0x99, 0x84], // white
                [0x92, 0x83, 0x74], // bright black
                [0xfb, 0x49, 0x34], // bright red
                [0xb8, 0xbb, 0x26], // bright green
                [0xfa, 0xbd, 0x2f], // bright yellow
                [0x83, 0xa5, 0x98], // bright blue
                [0xd3, 0x86, 0x9b], // bright magenta
                [0x8e, 0xc0, 0x7c], // bright cyan
                [0xeb, 0xdb, 0xb2], // bright white
            ],
        },
        ColorPreset {
            name: "Desert Light".into(),
            fg: [0x3b, 0x32, 0x28],
            bg: [0xf5, 0xee, 0xd5],
            cursor: [0xc3, 0x5e, 0x2a],
            ansi: [
                [0x3b, 0x32, 0x28], // black
                [0xc3, 0x5e, 0x2a], // red
                [0x6a, 0x8a, 0x3a], // green
                [0xb5, 0x7e, 0x14], // yellow
                [0x4a, 0x7b, 0x9d], // blue
                [0x8b, 0x5c, 0x8a], // magenta
                [0x4d, 0x8e, 0x80], // cyan
                [0xd4, 0xc8, 0xa8], // white
                [0x7a, 0x6e, 0x5e], // bright black
                [0xd9, 0x72, 0x3e], // bright red
                [0x7f, 0xa3, 0x4a], // bright green
                [0xcc, 0x94, 0x28], // bright yellow
                [0x5a, 0x92, 0xb5], // bright blue
                [0xa3, 0x71, 0xa2], // bright magenta
                [0x5e, 0xa6, 0x96], // bright cyan
                [0xf5, 0xee, 0xd5], // bright white
            ],
        },
        ColorPreset {
            name: "Solarized Dark".into(),
            fg: [0x83, 0x94, 0x96],
            bg: [0x00, 0x2b, 0x36],
            cursor: [0x83, 0x94, 0x96],
            ansi: [
                [0x07, 0x36, 0x42], // black
                [0xdc, 0x32, 0x2f], // red
                [0x85, 0x99, 0x00], // green
                [0xb5, 0x89, 0x00], // yellow
                [0x26, 0x8b, 0xd2], // blue
                [0xd3, 0x36, 0x82], // magenta
                [0x2a, 0xa1, 0x98], // cyan
                [0xee, 0xe8, 0xd5], // white
                [0x00, 0x2b, 0x36], // bright black
                [0xcb, 0x4b, 0x16], // bright red
                [0x58, 0x6e, 0x75], // bright green
                [0x65, 0x7b, 0x83], // bright yellow
                [0x83, 0x94, 0x96], // bright blue
                [0x6c, 0x71, 0xc4], // bright magenta
                [0x93, 0xa1, 0xa1], // bright cyan
                [0xfd, 0xf6, 0xe3], // bright white
            ],
        },
    ]
}

impl Default for TerminalTheme {
    fn default() -> Self {
        let presets = all_presets();
        let preset = &presets[0]; // Catppuccin Mocha
        Self {
            foreground: rgb_from_triplet(preset.fg),
            background: rgb_from_triplet(preset.bg),
            cursor: rgb_from_triplet(preset.cursor),
            ansi: preset.ansi.map(rgb_from_triplet),
        }
    }
}

impl TerminalTheme {
    pub fn from_config(config: &AppConfig) -> Self {
        let base_ansi = if let Some(preset) = find_preset(&config.theme.color_scheme) {
            preset.ansi.map(rgb_from_triplet)
        } else if let Some(ref ansi) = config.theme.ansi_colors {
            ansi.map(rgb_from_triplet)
        } else {
            all_presets()[0].ansi.map(rgb_from_triplet)
        };

        Self {
            foreground: rgb_from_triplet(config.theme.foreground),
            background: rgb_from_triplet(config.theme.background),
            cursor: rgb_from_triplet(config.theme.cursor),
            ansi: base_ansi,
        }
    }

    pub(super) fn named_color(&self, named: NamedColor) -> Rgb {
        match named {
            NamedColor::Foreground | NamedColor::BrightForeground => self.foreground,
            NamedColor::DimForeground => dim_rgb(self.foreground),
            NamedColor::Background => self.background,
            NamedColor::Cursor => self.cursor,
            NamedColor::Black => self.ansi[0],
            NamedColor::Red => self.ansi[1],
            NamedColor::Green => self.ansi[2],
            NamedColor::Yellow => self.ansi[3],
            NamedColor::Blue => self.ansi[4],
            NamedColor::Magenta => self.ansi[5],
            NamedColor::Cyan => self.ansi[6],
            NamedColor::White => self.ansi[7],
            NamedColor::BrightBlack => self.ansi[8],
            NamedColor::BrightRed => self.ansi[9],
            NamedColor::BrightGreen => self.ansi[10],
            NamedColor::BrightYellow => self.ansi[11],
            NamedColor::BrightBlue => self.ansi[12],
            NamedColor::BrightMagenta => self.ansi[13],
            NamedColor::BrightCyan => self.ansi[14],
            NamedColor::BrightWhite => self.ansi[15],
            NamedColor::DimBlack => dim_rgb(self.ansi[0]),
            NamedColor::DimRed => dim_rgb(self.ansi[1]),
            NamedColor::DimGreen => dim_rgb(self.ansi[2]),
            NamedColor::DimYellow => dim_rgb(self.ansi[3]),
            NamedColor::DimBlue => dim_rgb(self.ansi[4]),
            NamedColor::DimMagenta => dim_rgb(self.ansi[5]),
            NamedColor::DimCyan => dim_rgb(self.ansi[6]),
            NamedColor::DimWhite => dim_rgb(self.ansi[7]),
        }
    }

    pub(super) fn indexed_color(&self, index: u8) -> Rgb {
        match index {
            0..=15 => self.ansi[index as usize],
            16..=231 => {
                let idx = index - 16;
                let r = idx / 36;
                let g = (idx / 6) % 6;
                let b = idx % 6;
                Rgb {
                    r: xterm_component(r),
                    g: xterm_component(g),
                    b: xterm_component(b),
                }
            }
            _ => {
                let level = 8 + (index - 232) * 10;
                Rgb {
                    r: level,
                    g: level,
                    b: level,
                }
            }
        }
    }
}

pub(super) fn resolve_rgb(
    color: AnsiColor,
    colors: &Colors,
    theme: &TerminalTheme,
    flags: Flags,
    apply_intensity: bool,
) -> Rgb {
    let is_dim = apply_intensity && flags.intersects(Flags::DIM | Flags::DIM_BOLD);
    let is_bold = apply_intensity && flags.intersects(Flags::BOLD | Flags::DIM_BOLD);

    match color {
        AnsiColor::Spec(rgb) => {
            if is_dim {
                dim_rgb(rgb)
            } else {
                rgb
            }
        }
        AnsiColor::Indexed(mut index) => {
            if is_bold && index < 8 {
                index = index.saturating_add(8);
            }
            let base = colors[index as usize].unwrap_or_else(|| theme.indexed_color(index));
            if is_dim { dim_rgb(base) } else { base }
        }
        AnsiColor::Named(mut named) => {
            if is_dim {
                named = named.to_dim();
            } else if is_bold {
                named = named.to_bright();
            }
            colors[named].unwrap_or_else(|| theme.named_color(named))
        }
    }
}

/// Minimum WCAG contrast ratio for terminal text readability.
const MIN_CONTRAST_RATIO: f32 = 3.0;

/// Ensure foreground has minimum contrast against background.
pub(super) fn enforce_min_contrast(fg: Rgb, bg: Rgb) -> Rgb {
    let fg_lum = relative_luminance(fg);
    let bg_lum = relative_luminance(bg);
    let ratio = contrast_ratio(fg_lum, bg_lum);

    if ratio >= MIN_CONTRAST_RATIO {
        return fg;
    }

    // Determine direction: lighten fg on dark bg, darken fg on light bg
    let lighten = bg_lum < 0.5;

    // Binary search for the right adjustment
    let mut lo: f32 = 0.0;
    let mut hi: f32 = 1.0;
    let mut best = fg;

    for _ in 0..8 {
        let mid = (lo + hi) / 2.0;
        let candidate = if lighten {
            mix_rgb(
                fg,
                Rgb {
                    r: 255,
                    g: 255,
                    b: 255,
                },
                mid,
            )
        } else {
            mix_rgb(fg, Rgb { r: 0, g: 0, b: 0 }, mid)
        };
        let cand_lum = relative_luminance(candidate);
        let cand_ratio = contrast_ratio(cand_lum, bg_lum);
        if cand_ratio >= MIN_CONTRAST_RATIO {
            best = candidate;
            hi = mid;
        } else {
            lo = mid;
        }
    }

    best
}

fn relative_luminance(rgb: Rgb) -> f32 {
    let r = srgb_component_to_linear(rgb.r);
    let g = srgb_component_to_linear(rgb.g);
    let b = srgb_component_to_linear(rgb.b);
    0.2126 * r + 0.7152 * g + 0.0722 * b
}

fn srgb_component_to_linear(value: u8) -> f32 {
    let v = f32::from(value) / 255.0;
    if v <= 0.04045 {
        v / 12.92
    } else {
        ((v + 0.055) / 1.055).powf(2.4)
    }
}

fn contrast_ratio(lum1: f32, lum2: f32) -> f32 {
    let (lighter, darker) = if lum1 > lum2 {
        (lum1, lum2)
    } else {
        (lum2, lum1)
    };
    (lighter + 0.05) / (darker + 0.05)
}

fn mix_rgb(a: Rgb, b: Rgb, t: f32) -> Rgb {
    Rgb {
        r: ((f32::from(a.r) * (1.0 - t) + f32::from(b.r) * t).round()).clamp(0.0, 255.0) as u8,
        g: ((f32::from(a.g) * (1.0 - t) + f32::from(b.g) * t).round()).clamp(0.0, 255.0) as u8,
        b: ((f32::from(a.b) * (1.0 - t) + f32::from(b.b) * t).round()).clamp(0.0, 255.0) as u8,
    }
}

pub(super) fn srgb_u8_to_linear(value: u8) -> f32 {
    let v = f32::from(value) / 255.0;
    if v <= 0.04045 {
        v / 12.92
    } else {
        ((v + 0.055) / 1.055).powf(2.4)
    }
}

pub(super) fn rgb_to_rgba(rgb: Rgb, alpha: f32) -> [f32; 4] {
    [
        srgb_u8_to_linear(rgb.r),
        srgb_u8_to_linear(rgb.g),
        srgb_u8_to_linear(rgb.b),
        alpha,
    ]
}

fn rgb_from_triplet(value: [u8; 3]) -> Rgb {
    Rgb {
        r: value[0],
        g: value[1],
        b: value[2],
    }
}

fn dim_rgb(rgb: Rgb) -> Rgb {
    let scale = 2.0 / 3.0;
    Rgb {
        r: (f32::from(rgb.r) * scale).round().clamp(0.0, 255.0) as u8,
        g: (f32::from(rgb.g) * scale).round().clamp(0.0, 255.0) as u8,
        b: (f32::from(rgb.b) * scale).round().clamp(0.0, 255.0) as u8,
    }
}

fn xterm_component(value: u8) -> u8 {
    match value {
        0 => 0,
        1 => 95,
        2 => 135,
        3 => 175,
        4 => 215,
        _ => 255,
    }
}

use crate::config::AppConfig;
use alacritty_terminal::term::cell::Flags;
use alacritty_terminal::term::color::Colors;
use alacritty_terminal::vte::ansi::{Color as AnsiColor, NamedColor, Rgb};

#[derive(Debug, Clone)]
pub struct TerminalTheme {
    pub(super) foreground: Rgb,
    pub(super) background: Rgb,
    cursor: Rgb,
    ansi: [Rgb; 16],
    pub(super) background_opacity: f32,
}

const DEFAULT_FOREGROUND: Rgb = Rgb {
    r: 0xcd,
    g: 0xd6,
    b: 0xf4,
};
const DEFAULT_BACKGROUND: Rgb = Rgb {
    r: 0x1e,
    g: 0x1e,
    b: 0x2e,
};
const DEFAULT_CURSOR: Rgb = Rgb {
    r: 0x89,
    g: 0xb4,
    b: 0xfa,
};
const DEFAULT_ANSI: [Rgb; 16] = [
    Rgb {
        r: 0x00,
        g: 0x00,
        b: 0x00,
    },
    Rgb {
        r: 0xcd,
        g: 0x00,
        b: 0x00,
    },
    Rgb {
        r: 0x00,
        g: 0xcd,
        b: 0x00,
    },
    Rgb {
        r: 0xcd,
        g: 0xcd,
        b: 0x00,
    },
    Rgb {
        r: 0x00,
        g: 0x00,
        b: 0xee,
    },
    Rgb {
        r: 0xcd,
        g: 0x00,
        b: 0xcd,
    },
    Rgb {
        r: 0x00,
        g: 0xcd,
        b: 0xcd,
    },
    Rgb {
        r: 0xe5,
        g: 0xe5,
        b: 0xe5,
    },
    Rgb {
        r: 0x7f,
        g: 0x7f,
        b: 0x7f,
    },
    Rgb {
        r: 0xff,
        g: 0x00,
        b: 0x00,
    },
    Rgb {
        r: 0x00,
        g: 0xff,
        b: 0x00,
    },
    Rgb {
        r: 0xff,
        g: 0xff,
        b: 0x00,
    },
    Rgb {
        r: 0x5c,
        g: 0x5c,
        b: 0xff,
    },
    Rgb {
        r: 0xff,
        g: 0x00,
        b: 0xff,
    },
    Rgb {
        r: 0x00,
        g: 0xff,
        b: 0xff,
    },
    Rgb {
        r: 0xff,
        g: 0xff,
        b: 0xff,
    },
];

impl Default for TerminalTheme {
    fn default() -> Self {
        Self {
            foreground: DEFAULT_FOREGROUND,
            background: DEFAULT_BACKGROUND,
            cursor: DEFAULT_CURSOR,
            ansi: DEFAULT_ANSI,
            background_opacity: 1.0,
        }
    }
}

impl TerminalTheme {
    pub fn from_config(config: &AppConfig) -> Self {
        let mut theme = Self::default();
        theme.foreground = rgb_from_triplet(config.theme.foreground);
        theme.background = rgb_from_triplet(config.theme.background);
        theme.cursor = rgb_from_triplet(config.theme.cursor);
        theme.background_opacity = config.theme.background_opacity;
        theme
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

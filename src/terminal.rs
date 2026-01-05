use crate::config::AppConfig;
use alacritty_terminal::event::{Event, EventListener, WindowSize};
use alacritty_terminal::grid::Dimensions;
use alacritty_terminal::term::cell::Flags;
use alacritty_terminal::term::color::Colors;
use alacritty_terminal::term::{Config as TermConfig, RenderableContent, Term, point_to_viewport};
use alacritty_terminal::vte::ansi::Processor;
use alacritty_terminal::vte::ansi::{Color as AnsiColor, CursorShape, NamedColor, Rgb};
use std::cell::{Cell, RefCell};
use std::io::Write;
use std::sync::{Arc, Mutex, OnceLock};

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct TerminalSize {
    pub columns: usize,
    pub lines: usize,
}

#[derive(Debug, Clone, Copy)]
pub struct CellVisual {
    pub ch: char,
    pub col: usize,
    pub row: usize,
    pub fg: [f32; 4],
    pub bg: [f32; 4],
    pub underline: bool,
}

#[derive(Debug, Clone)]
pub struct TerminalTheme {
    foreground: Rgb,
    background: Rgb,
    cursor: Rgb,
    ansi: [Rgb; 16],
    background_opacity: f32,
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

    fn named_color(&self, named: NamedColor) -> Rgb {
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

    fn indexed_color(&self, index: u8) -> Rgb {
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

impl TerminalSize {
    pub const fn new(columns: usize, lines: usize) -> Self {
        Self { columns, lines }
    }
}

impl Dimensions for TerminalSize {
    fn total_lines(&self) -> usize {
        self.lines
    }

    fn screen_lines(&self) -> usize {
        self.lines
    }

    fn columns(&self) -> usize {
        self.columns
    }
}

pub struct TerminalEngine {
    term: Term<PtyEventProxy>,
    processor: Processor,
    size: TerminalSize,
    theme: TerminalTheme,
    cells_cache: RefCell<Arc<Vec<CellVisual>>>,
    cache_dirty: Cell<bool>,
    cache_size: Cell<TerminalSize>,
}

impl TerminalEngine {
    pub fn new(
        size: TerminalSize,
        scrollback: usize,
        writer: Arc<Mutex<Box<dyn Write + Send>>>,
        theme: TerminalTheme,
    ) -> Self {
        let config = TermConfig {
            scrolling_history: scrollback,
            ..Default::default()
        };
        let term = Term::new(
            config,
            &size,
            PtyEventProxy {
                writer: Arc::clone(&writer),
                size,
            },
        );

        Self {
            term,
            processor: Processor::new(),
            size,
            theme,
            cells_cache: RefCell::new(Arc::new(Vec::new())),
            cache_dirty: Cell::new(true),
            cache_size: Cell::new(size),
        }
    }

    pub fn size(&self) -> TerminalSize {
        self.size
    }

    pub fn feed_bytes(&mut self, bytes: &[u8]) {
        self.processor.advance(&mut self.term, bytes);
        self.cache_dirty.set(true);
    }

    pub fn resize(&mut self, new_size: TerminalSize) {
        self.size = new_size;
        self.term.resize(new_size);
        self.cache_dirty.set(true);
    }

    pub fn render_cells(&self) -> Arc<Vec<CellVisual>> {
        if self.cache_dirty.get() || self.cache_size.get() != self.size {
            let mut cache = self.cells_cache.borrow_mut();
            if let Some(cells) = Arc::get_mut(&mut cache) {
                self.build_cells_into(cells);
            } else {
                let mut cells =
                    Vec::with_capacity(self.size.lines.saturating_mul(self.size.columns));
                self.build_cells_into(&mut cells);
                *cache = Arc::new(cells);
            }
            self.cache_dirty.set(false);
            self.cache_size.set(self.size);
        }
        self.cells_cache.borrow().clone()
    }

    pub fn set_theme(&mut self, theme: TerminalTheme) {
        self.theme = theme;
        self.cache_dirty.set(true);
    }

    fn build_cells_into(&self, cells: &mut Vec<CellVisual>) {
        cells.clear();

        let RenderableContent {
            display_iter,
            display_offset,
            cursor,
            colors,
            ..
        } = self.term.renderable_content();

        let default_fg = rgb_to_rgba(self.theme.foreground, 1.0);
        let default_bg = rgb_to_rgba(self.theme.background, self.theme.background_opacity);
        for row in 0..self.size.lines {
            for col in 0..self.size.columns {
                cells.push(CellVisual {
                    ch: ' ',
                    col,
                    row,
                    fg: default_fg,
                    bg: default_bg,
                    underline: false,
                });
            }
        }

        let idx = |row: usize, col: usize, cols: usize| row * cols + col;

        for indexed in display_iter {
            if let Some(point) = point_to_viewport(display_offset, indexed.point) {
                let col = point.column.0;
                let row = point.line;
                if row < self.size.lines && col < self.size.columns {
                    let slot = &mut cells[idx(row, col, self.size.columns)];
                    let mut fg_rgb = resolve_rgb(
                        indexed.cell.fg,
                        colors,
                        &self.theme,
                        indexed.cell.flags,
                        true,
                    );
                    let mut bg_rgb = resolve_rgb(
                        indexed.cell.bg,
                        colors,
                        &self.theme,
                        indexed.cell.flags,
                        false,
                    );

                    if indexed.cell.flags.contains(Flags::INVERSE) {
                        std::mem::swap(&mut fg_rgb, &mut bg_rgb);
                    }

                    let mut fg = rgb_to_rgba(fg_rgb, 1.0);
                    let bg = rgb_to_rgba(bg_rgb, self.theme.background_opacity);

                    if indexed.cell.flags.contains(Flags::HIDDEN) {
                        fg = bg;
                    }

                    slot.ch = indexed.cell.c;
                    slot.col = col;
                    slot.row = row;
                    slot.fg = fg;
                    slot.bg = bg;
                    slot.underline = indexed.cell.flags.intersects(Flags::ALL_UNDERLINES);
                }
            }
        }

        if cursor.shape != CursorShape::Hidden {
            let cursor_col = cursor.point.column.0;
            let cursor_line = cursor.point.line.0 as usize;

            if cursor_line < self.size.lines && cursor_col < self.size.columns {
                let slot = &mut cells[idx(cursor_line, cursor_col, self.size.columns)];
                let fg = slot.fg;
                let bg = slot.bg;

                if bg[3] > 0.0 {
                    slot.fg = bg;
                    slot.bg = fg;
                } else {
                    let luma = 0.2126 * fg[0] + 0.7152 * fg[1] + 0.0722 * fg[2];
                    let cursor_fg = if luma > 0.5 {
                        [0.0, 0.0, 0.0, 1.0]
                    } else {
                        [1.0, 1.0, 1.0, 1.0]
                    };
                    let mut cursor_bg = fg;
                    cursor_bg[3] = 1.0;
                    slot.fg = cursor_fg;
                    slot.bg = cursor_bg;
                }
            }
        }
    }
}

fn resolve_rgb(
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

static SRGB_TO_LINEAR: OnceLock<[f32; 256]> = OnceLock::new();

fn srgb_u8_to_linear(value: u8) -> f32 {
    let table = SRGB_TO_LINEAR.get_or_init(|| {
        let mut table = [0.0f32; 256];
        for (i, slot) in table.iter_mut().enumerate() {
            let v = i as f32 / 255.0;
            *slot = if v <= 0.04045 {
                v / 12.92
            } else {
                ((v + 0.055) / 1.055).powf(2.4)
            };
        }
        table
    });
    table[value as usize]
}

fn rgb_to_rgba(rgb: Rgb, alpha: f32) -> [f32; 4] {
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

#[derive(Clone)]
struct PtyEventProxy {
    writer: Arc<Mutex<Box<dyn Write + Send>>>,
    size: TerminalSize,
}

impl EventListener for PtyEventProxy {
    fn send_event(&self, event: Event) {
        match event {
            Event::PtyWrite(text) => {
                if let Ok(mut guard) = self.writer.lock() {
                    let _ = guard.write_all(text.as_bytes());
                    let _ = guard.flush();
                }
            }
            Event::TextAreaSizeRequest(formatter) => {
                let ws = WindowSize {
                    num_lines: self.size.lines as u16,
                    num_cols: self.size.columns as u16,
                    cell_width: 1,
                    cell_height: 1,
                };
                if let Ok(mut guard) = self.writer.lock() {
                    let text = formatter(ws);
                    let _ = guard.write_all(text.as_bytes());
                    let _ = guard.flush();
                }
            }
            _ => {}
        }
    }
}

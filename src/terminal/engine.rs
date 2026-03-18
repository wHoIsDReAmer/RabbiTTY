use super::theme::{resolve_rgb, rgb_to_rgba};
use super::{CellVisual, TerminalSize, TerminalTheme};
use alacritty_terminal::event::{Event, EventListener, WindowSize};
use alacritty_terminal::grid::Dimensions;
use alacritty_terminal::grid::Scroll;
use alacritty_terminal::term::cell::Flags;
use alacritty_terminal::term::{Config as TermConfig, RenderableContent, Term, point_to_viewport};
use alacritty_terminal::vte::ansi::{CursorShape, Processor};
use std::cell::{Cell, RefCell};
use std::io::Write;
use std::sync::{Arc, Mutex};

pub struct TerminalEngine {
    term: Term<PtyEventProxy>,
    processor: Processor,
    size: TerminalSize,
    theme: TerminalTheme,
    cells_cache: RefCell<Arc<Vec<CellVisual>>>,
    cache_dirty: Cell<bool>,
    cache_size: Cell<TerminalSize>,
    title: Arc<Mutex<Option<String>>>,
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
        let title: Arc<Mutex<Option<String>>> = Arc::new(Mutex::new(None));
        let term = Term::new(
            config,
            &size,
            PtyEventProxy {
                writer: Arc::clone(&writer),
                size,
                title: Arc::clone(&title),
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
            title,
        }
    }

    pub fn size(&self) -> TerminalSize {
        self.size
    }

    pub fn take_title(&self) -> Option<String> {
        self.title.lock().ok()?.take()
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

    pub fn scroll(&mut self, delta: i32) {
        self.term.scroll_display(Scroll::Delta(delta));
        self.cache_dirty.set(true);
    }

    /// Returns (display_offset, total_history_lines).
    /// display_offset == 0 means at the bottom (latest output).
    pub fn scroll_position(&self) -> (usize, usize) {
        let offset = self.term.grid().display_offset();
        let history = self.term.grid().history_size();
        (offset, history)
    }

    /// Scroll to a relative position (0.0 = top of history, 1.0 = bottom/latest).
    pub fn scroll_to_relative(&mut self, rel: f32) {
        let history = self.term.grid().history_size();
        if history == 0 {
            return;
        }
        let target_offset = ((1.0 - rel.clamp(0.0, 1.0)) * history as f32).round() as usize;
        let current = self.term.grid().display_offset();
        let delta = target_offset as i32 - current as i32;
        if delta != 0 {
            self.term.scroll_display(Scroll::Delta(delta));
            self.cache_dirty.set(true);
        }
    }

    pub fn set_theme(&mut self, theme: TerminalTheme) {
        self.theme = theme;
        self.cache_dirty.set(true);
    }

    fn build_cells_into(&self, cells: &mut Vec<CellVisual>) {
        let RenderableContent {
            display_iter,
            display_offset,
            cursor,
            colors,
            ..
        } = self.term.renderable_content();

        let default_fg = rgb_to_rgba(self.theme.foreground, 1.0);
        let default_bg = rgb_to_rgba(self.theme.background, self.theme.background_opacity);
        let total = self.size.lines * self.size.columns;
        let default_cell = CellVisual {
            ch: ' ',
            col: 0,
            row: 0,
            fg: default_fg,
            bg: default_bg,
            underline: false,
            wide: false,
        };

        cells.clear();
        cells.resize(total, default_cell);
        for row in 0..self.size.lines {
            let base = row * self.size.columns;
            for col in 0..self.size.columns {
                let slot = &mut cells[base + col];
                slot.col = col;
                slot.row = row;
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
                    slot.wide = indexed.cell.flags.contains(Flags::WIDE_CHAR);
                }
            }
        }

        if cursor.shape != CursorShape::Hidden && display_offset == 0 {
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

#[derive(Clone)]
struct PtyEventProxy {
    writer: Arc<Mutex<Box<dyn Write + Send>>>,
    size: TerminalSize,
    title: Arc<Mutex<Option<String>>>,
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
            Event::Title(new_title) => {
                if let Ok(mut guard) = self.title.lock() {
                    *guard = Some(new_title);
                }
            }
            _ => {}
        }
    }
}

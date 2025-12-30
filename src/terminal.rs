use alacritty_terminal::event::{Event, EventListener, WindowSize};
use alacritty_terminal::grid::Dimensions;
use alacritty_terminal::term::{Config as TermConfig, RenderableContent, Term, point_to_viewport};
use alacritty_terminal::vte::ansi::CursorShape;
use alacritty_terminal::vte::ansi::Processor;
use std::io::Write;
use std::sync::{Arc, Mutex};

#[derive(Debug, Clone, Copy)]
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
}

impl TerminalEngine {
    pub fn new(
        size: TerminalSize,
        scrollback: usize,
        writer: Arc<Mutex<Box<dyn Write + Send>>>,
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
        }
    }

    pub fn size(&self) -> TerminalSize {
        self.size
    }

    pub fn feed_bytes(&mut self, bytes: &[u8]) {
        self.processor.advance(&mut self.term, bytes);
    }

    pub fn resize(&mut self, new_size: TerminalSize) {
        self.size = new_size;
        self.term.resize(new_size);
    }

    pub fn render_cells(&self) -> Vec<CellVisual> {
        let RenderableContent {
            display_iter,
            display_offset,
            cursor,
            ..
        } = self.term.renderable_content();

        let mut cells = Vec::with_capacity(self.size.lines * self.size.columns);
        for row in 0..self.size.lines {
            for col in 0..self.size.columns {
                cells.push(CellVisual {
                    ch: ' ',
                    col,
                    row,
                    fg: [0.85, 0.88, 0.93, 1.0],
                    bg: [0.0, 0.0, 0.0, 0.0],
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
                    slot.ch = indexed.cell.c;
                    slot.col = col;
                    slot.row = row;
                    slot.fg = [0.85, 0.88, 0.93, 1.0];
                    slot.bg = [0.0, 0.0, 0.0, 0.0];
                    slot.underline = false;
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

        cells
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

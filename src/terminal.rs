use alacritty_terminal::event::{Event, EventListener, WindowSize};
use alacritty_terminal::grid::Dimensions;
use alacritty_terminal::term::{Config as TermConfig, RenderableContent, Term, point_to_viewport};
use alacritty_terminal::vte::ansi::Processor;
use std::io::Write;
use std::sync::{Arc, Mutex};

pub const DEFAULT_COLUMNS: usize = 120;
pub const DEFAULT_LINES: usize = 40;

#[derive(Debug, Clone, Copy)]
pub struct TerminalSize {
    pub columns: usize,
    pub lines: usize,
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

    pub fn render_lines(&self) -> Vec<String> {
        let RenderableContent {
            display_iter,
            display_offset,
            cursor,
            ..
        } = self.term.renderable_content();

        let mut lines = vec![String::with_capacity(self.size.columns); self.size.lines];

        for indexed in display_iter {
            if let Some(point) = point_to_viewport(display_offset, indexed.point) {
                let line = &mut lines[point.line];
                line.push(indexed.cell.c);
                if let Some(zerowidth) = indexed.cell.zerowidth() {
                    zerowidth.iter().for_each(|c| line.push(*c));
                }
            }
        }

        // Add block ascii to cursor position
        let cursor_col = cursor.point.column.0;
        let cursor_line = cursor.point.line.0 as usize;
        if cursor_line < lines.len() {
            let line = &mut lines[cursor_line];

            while line.chars().count() < cursor_col {
                line.push(' ');
            }

            let mut new_line = String::with_capacity(line.len() + 1);
            for (i, c) in line.chars().enumerate() {
                if i == cursor_col {
                    new_line.push('█'); // Cursor block
                } else {
                    new_line.push(c);
                }
            }

            if line.chars().count() <= cursor_col {
                new_line.push('█');
            }

            *line = new_line;
        }

        // Trim trailing spaces (but keep cursor)
        for (i, line) in lines.iter_mut().enumerate() {
            if i != cursor_line {
                while line.ends_with(' ') {
                    line.pop();
                }
            }
        }

        lines
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

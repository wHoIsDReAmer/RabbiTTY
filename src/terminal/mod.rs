mod engine;
pub mod font;
mod theme;

pub use engine::TerminalEngine;
pub use theme::TerminalTheme;

use alacritty_terminal::grid::Dimensions;

#[derive(Debug, Clone, Copy, PartialEq)]
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

#[derive(Debug, Clone, Copy)]
pub struct CellVisual {
    pub ch: char,
    pub col: usize,
    pub row: usize,
    pub fg: [f32; 4],
    pub bg: [f32; 4],
    pub underline: bool,
    pub wide: bool,
}

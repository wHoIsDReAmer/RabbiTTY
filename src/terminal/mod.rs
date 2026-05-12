mod engine;
pub mod font;
pub mod theme;

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

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct GridPos {
    pub row: usize,
    pub col: usize,
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Selection {
    pub start: GridPos,
    pub end: GridPos,
    /// `display_offset` at the moment the selection was anchored. Selection
    /// rows are stored in this frame so they follow content during scrolls.
    pub anchor_offset: usize,
}

impl Selection {
    pub fn ordered(&self) -> (GridPos, GridPos) {
        if self.start.row < self.end.row
            || (self.start.row == self.end.row && self.start.col <= self.end.col)
        {
            (self.start, self.end)
        } else {
            (self.end, self.start)
        }
    }

    /// Translate a current-viewport row back into the selection's anchor frame.
    fn anchor_row(&self, viewport_row: usize, current_offset: usize) -> Option<usize> {
        let delta = current_offset as isize - self.anchor_offset as isize;
        let anchored = viewport_row as isize - delta;
        if anchored < 0 {
            None
        } else {
            Some(anchored as usize)
        }
    }

    pub fn contains_at(&self, viewport_row: usize, col: usize, current_offset: usize) -> bool {
        let Some(row) = self.anchor_row(viewport_row, current_offset) else {
            return false;
        };
        let (start, end) = self.ordered();
        if row < start.row || row > end.row {
            return false;
        }
        if start.row == end.row {
            return col >= start.col && col <= end.col;
        }
        if row == start.row {
            return col >= start.col;
        }
        if row == end.row {
            return col <= end.col;
        }
        true
    }

    pub fn is_empty(&self) -> bool {
        self.start == self.end
    }
}

#[cfg(test)]
mod selection_tests {
    use super::{GridPos, Selection};

    fn sel(start_row: usize, end_row: usize, anchor: usize) -> Selection {
        Selection {
            start: GridPos {
                row: start_row,
                col: 0,
            },
            end: GridPos {
                row: end_row,
                col: 9,
            },
            anchor_offset: anchor,
        }
    }

    #[test]
    fn highlight_follows_content_when_scrolling_up() {
        // Anchored at offset 0, selection covers rows 5..10.
        let s = sel(5, 10, 0);
        // At offset 3 (scrolled up by 3) the content moved down by 3, so the
        // highlight should now cover viewport rows 8..13.
        assert!(!s.contains_at(7, 5, 3));
        assert!(s.contains_at(8, 5, 3));
        assert!(s.contains_at(13, 5, 3));
        assert!(!s.contains_at(14, 5, 3));
    }

    #[test]
    fn highlight_follows_content_when_scrolling_down() {
        // Anchored at offset 5, selection covers rows 5..10.
        let s = sel(5, 10, 5);
        // Scrolling down to offset 2 (delta = -3) shifts highlight up by 3.
        assert!(!s.contains_at(1, 5, 2));
        assert!(s.contains_at(2, 5, 2));
        assert!(s.contains_at(7, 5, 2));
        assert!(!s.contains_at(8, 5, 2));
    }

    #[test]
    fn highlight_drops_off_when_anchor_above_viewport() {
        // delta=10, anchor row stored as 0. Viewport row 0 maps to -10 → false.
        let s = sel(0, 2, 0);
        assert!(!s.contains_at(0, 5, 10));
        assert!(s.contains_at(10, 5, 10));
        assert!(s.contains_at(12, 5, 10));
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

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
    /// Anchor-frame row of the selection's start. Signed so the selection can
    /// extend into scrollback (negative rows) once it crosses the top of the
    /// viewport that was active when the drag started.
    pub start_row: i64,
    pub start_col: usize,
    pub end_row: i64,
    pub end_col: usize,
    /// `display_offset` at the moment the selection was anchored.
    pub anchor_offset: usize,
}

impl Selection {
    pub fn ordered(&self) -> ((i64, usize), (i64, usize)) {
        if self.start_row < self.end_row
            || (self.start_row == self.end_row && self.start_col <= self.end_col)
        {
            (
                (self.start_row, self.start_col),
                (self.end_row, self.end_col),
            )
        } else {
            (
                (self.end_row, self.end_col),
                (self.start_row, self.start_col),
            )
        }
    }

    /// Map a current-viewport row back into the selection's anchor frame.
    fn anchor_row(&self, viewport_row: usize, current_offset: usize) -> i64 {
        let delta = current_offset as i64 - self.anchor_offset as i64;
        viewport_row as i64 - delta
    }

    pub fn contains_at(&self, viewport_row: usize, col: usize, current_offset: usize) -> bool {
        let row = self.anchor_row(viewport_row, current_offset);
        let ((start_row, start_col), (end_row, end_col)) = self.ordered();
        if row < start_row || row > end_row {
            return false;
        }
        if start_row == end_row {
            return col >= start_col && col <= end_col;
        }
        if row == start_row {
            return col >= start_col;
        }
        if row == end_row {
            return col <= end_col;
        }
        true
    }

    pub fn is_empty(&self) -> bool {
        self.start_row == self.end_row && self.start_col == self.end_col
    }
}

#[cfg(test)]
mod selection_tests {
    use super::Selection;

    fn sel(start_row: i64, end_row: i64, anchor: usize) -> Selection {
        Selection {
            start_row,
            start_col: 0,
            end_row,
            end_col: 9,
            anchor_offset: anchor,
        }
    }

    #[test]
    fn highlight_follows_content_when_scrolling_up() {
        let s = sel(5, 10, 0);
        assert!(!s.contains_at(7, 5, 3));
        assert!(s.contains_at(8, 5, 3));
        assert!(s.contains_at(13, 5, 3));
        assert!(!s.contains_at(14, 5, 3));
    }

    #[test]
    fn highlight_follows_content_when_scrolling_down() {
        let s = sel(5, 10, 5);
        assert!(!s.contains_at(1, 5, 2));
        assert!(s.contains_at(2, 5, 2));
        assert!(s.contains_at(7, 5, 2));
        assert!(!s.contains_at(8, 5, 2));
    }

    #[test]
    fn highlight_extends_into_scrollback_for_negative_anchor_rows() {
        // Drag started at (5, 9) and ended at (-3, 0) — natural diagonal
        // selection sweeping up-left across the original viewport top.
        let s = Selection {
            start_row: 5,
            start_col: 9,
            end_row: -3,
            end_col: 0,
            anchor_offset: 0,
        };
        // At offset 0 the row -3 sits above the viewport so it isn't visible,
        // but rows 0..5 are part of the selection.
        assert!(s.contains_at(0, 5, 0));
        assert!(s.contains_at(5, 5, 0));
        assert!(!s.contains_at(6, 5, 0));
        // Scrolled up by 3: anchor row -3 now sits at viewport row 0 and is
        // the ordered-start of the selection (full row from col 0).
        assert!(s.contains_at(0, 5, 3));
        assert!(s.contains_at(8, 5, 3));
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

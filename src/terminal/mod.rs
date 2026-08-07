mod engine;
pub mod font;
pub mod theme;
pub mod url;

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

/// A row/column pair in a selection's anchor frame. Rows are signed so the
/// selection can extend into scrollback past the top of the viewport that was
/// active when the drag started.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct SelectionPoint {
    pub row: i64,
    pub col: usize,
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Selection {
    pub start: SelectionPoint,
    pub end: SelectionPoint,
    /// `display_offset` at the moment the selection was anchored.
    pub anchor_offset: usize,
}

impl Selection {
    /// Signed delta from the anchor frame to the supplied display offset.
    /// Positive when the viewport has scrolled toward older content since the
    /// anchor was captured.
    pub fn delta(&self, current_offset: usize) -> i64 {
        current_offset as i64 - self.anchor_offset as i64
    }

    /// Returns the selection's two endpoints in reading order.
    pub fn ordered(&self) -> (SelectionPoint, SelectionPoint) {
        let earlier_first = self.start.row < self.end.row
            || (self.start.row == self.end.row && self.start.col <= self.end.col);
        if earlier_first {
            (self.start, self.end)
        } else {
            (self.end, self.start)
        }
    }

    /// Smallest selection covering both, used to extend a word or line drag
    /// past the unit it was anchored on. Both must share an anchor frame.
    pub fn union(&self, other: &Selection) -> Selection {
        let (a_start, a_end) = self.ordered();
        let (b_start, b_end) = other.ordered();
        Selection {
            start: if before(a_start, b_start) {
                a_start
            } else {
                b_start
            },
            end: if before(a_end, b_end) { b_end } else { a_end },
            anchor_offset: self.anchor_offset,
        }
    }

    pub fn contains_at(&self, viewport_row: usize, col: usize, current_offset: usize) -> bool {
        let row = viewport_row as i64 - self.delta(current_offset);
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
}

fn before(a: SelectionPoint, b: SelectionPoint) -> bool {
    a.row < b.row || (a.row == b.row && a.col < b.col)
}

#[cfg(test)]
mod selection_tests {
    use super::{Selection, SelectionPoint};

    fn sel(start: (i64, usize), end: (i64, usize), anchor: usize) -> Selection {
        Selection {
            start: SelectionPoint {
                row: start.0,
                col: start.1,
            },
            end: SelectionPoint {
                row: end.0,
                col: end.1,
            },
            anchor_offset: anchor,
        }
    }

    #[test]
    fn a_union_spans_from_the_earlier_start_to_the_later_end() {
        let anchor = sel((5, 4), (5, 8), 0);
        let later = sel((7, 2), (7, 6), 0);
        let u = anchor.union(&later);
        assert_eq!((u.start.row, u.start.col), (5, 4));
        assert_eq!((u.end.row, u.end.col), (7, 6));
    }

    #[test]
    fn a_union_keeps_the_anchor_whole_when_dragging_backwards() {
        let anchor = sel((5, 4), (5, 8), 0);
        let earlier = sel((3, 1), (3, 3), 0);
        let u = anchor.union(&earlier);
        assert_eq!((u.start.row, u.start.col), (3, 1));
        assert_eq!((u.end.row, u.end.col), (5, 8));
    }

    #[test]
    fn a_union_with_a_unit_inside_the_anchor_changes_nothing() {
        let anchor = sel((5, 4), (5, 8), 0);
        let inside = sel((5, 6), (5, 6), 0);
        let u = anchor.union(&inside);
        assert_eq!((u.start.row, u.start.col), (5, 4));
        assert_eq!((u.end.row, u.end.col), (5, 8));
    }

    #[test]
    fn a_selection_that_starts_and_ends_on_one_cell_covers_that_cell_only() {
        let s = sel((5, 7), (5, 7), 0);
        assert!(s.contains_at(5, 7, 0));
        assert!(!s.contains_at(5, 6, 0));
        assert!(!s.contains_at(5, 8, 0));
        assert!(!s.contains_at(4, 7, 0));
        assert!(!s.contains_at(6, 7, 0));
    }

    #[test]
    fn highlight_follows_content_when_scrolling_up() {
        let s = sel((5, 0), (10, 9), 0);
        assert!(!s.contains_at(7, 5, 3));
        assert!(s.contains_at(8, 5, 3));
        assert!(s.contains_at(13, 5, 3));
        assert!(!s.contains_at(14, 5, 3));
    }

    #[test]
    fn highlight_follows_content_when_scrolling_down() {
        let s = sel((5, 0), (10, 9), 5);
        assert!(!s.contains_at(1, 5, 2));
        assert!(s.contains_at(2, 5, 2));
        assert!(s.contains_at(7, 5, 2));
        assert!(!s.contains_at(8, 5, 2));
    }

    #[test]
    fn highlight_extends_into_scrollback_for_negative_anchor_rows() {
        // Drag started at (5, 9) and ended at (-3, 0) — natural diagonal
        // selection sweeping up-left across the original viewport top.
        let s = sel((5, 9), (-3, 0), 0);
        assert!(s.contains_at(0, 5, 0));
        assert!(s.contains_at(5, 5, 0));
        assert!(!s.contains_at(6, 5, 0));
        // Scrolled up by 3: anchor row -3 now sits at viewport row 0.
        assert!(s.contains_at(0, 5, 3));
        assert!(s.contains_at(8, 5, 3));
    }
}

#[derive(Debug, Clone)]
pub struct CellVisual {
    pub ch: char,
    pub col: usize,
    pub row: usize,
    pub fg: [f32; 4],
    pub bg: [f32; 4],
    pub underline: bool,
    pub wide: bool,
    pub hyperlink: Option<std::sync::Arc<str>>,
}

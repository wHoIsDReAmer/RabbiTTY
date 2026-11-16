use super::{Reply, TerminalEngine};
use crate::terminal::graphics::{IMAGE_CELL, ImageSlice, Placed};
use alacritty_terminal::grid::Dimensions;
use alacritty_terminal::index::{Column, Line};
use alacritty_terminal::term::TermMode;
use alacritty_terminal::term::cell::{Cell as GridCell, Flags};
use alacritty_terminal::vte::ansi::{Color, Handler, Rgb};
use std::collections::HashSet;

pub(super) struct ImageRun {
    key: u32,
    image_row: u16,
    first_image_col: u16,
    len: u16,
    row: usize,
    col: usize,
}

impl ImageRun {
    pub(super) fn at(cell: &GridCell, row: usize, col: usize) -> Option<Self> {
        if cell.c != IMAGE_CELL {
            return None;
        }
        let (Color::Spec(key), Color::Spec(position)) = (cell.fg, cell.bg) else {
            return None;
        };
        let (image_row, first_image_col) = color_position(position);
        Some(Self {
            key: color_key(key),
            image_row,
            first_image_col,
            len: 1,
            row,
            col,
        })
    }

    pub(super) fn absorbs(&mut self, next: &ImageRun) -> bool {
        let follows = self.key == next.key
            && self.image_row == next.image_row
            && self.row == next.row
            && self.col + usize::from(self.len) == next.col
            && self.first_image_col + self.len == next.first_image_col;
        if follows {
            self.len += 1;
        }
        follows
    }
}

impl TerminalEngine {
    pub(super) fn graphics_command(&mut self) {
        if self.processor.sync_bytes_count() > 0 {
            self.processor.stop_sync(&mut self.term);
        }
        let alt_screen = self.term.mode().contains(TermMode::ALT_SCREEN);
        let effects = self
            .graphics
            .command(self.scanner.payload(), self.cell_pixels, alt_screen);
        if effects.clear_visible {
            self.clear_visible_images();
        }
        if let Some(placed) = effects.place {
            self.place_image(placed);
        }
        if let Some(reply) = effects.reply
            && let Ok(mut guard) = self.osc.lock()
        {
            guard.replies.push(Reply::Text(reply));
        }
        if effects.clear_visible || self.graphics.over_budget() {
            self.collect_images();
        }
    }

    pub(super) fn release_lost_images(&mut self) {
        let alt_screen = self.term.mode().contains(TermMode::ALT_SCREEN);
        let history = self.term.grid().history_size();
        let lost = (self.was_alt_screen && !alt_screen) || history < self.seen_history;
        self.was_alt_screen = alt_screen;
        self.seen_history = history;
        if lost && !self.graphics.is_idle() {
            self.collect_images();
        }
    }

    pub(super) fn image_slice(&self, run: &ImageRun) -> Option<ImageSlice> {
        let (image, size, uv) =
            self.graphics
                .slice(run.key, run.image_row, run.first_image_col, run.len)?;
        Some(ImageSlice {
            image,
            row: run.row,
            col: run.col,
            size,
            uv,
        })
    }

    fn place_image(&mut self, placed: Placed) {
        let columns = self.term.grid().columns();
        let screen_lines = self.term.grid().screen_lines() as i32;
        let start = self.term.grid().cursor.point;
        let visible = usize::from(placed.cols).min(columns - start.column.0);
        for image_row in 0..placed.rows {
            let line = if placed.hold_cursor {
                start.line + i32::from(image_row)
            } else {
                if image_row > 0 {
                    self.term.linefeed();
                }
                self.term.grid().cursor.point.line
            };
            if line.0 >= screen_lines {
                break;
            }
            self.write_image_row(line, start.column.0, visible, placed.key, image_row);
        }
        if placed.hold_cursor {
            return;
        }
        let after = start.column.0 + usize::from(placed.cols);
        let cursor = &mut self.term.grid_mut().cursor;
        cursor.input_needs_wrap = false;
        if after < columns {
            cursor.point.column = Column(after);
        } else {
            cursor.point.column = Column(0);
            self.term.linefeed();
        }
    }

    fn write_image_row(&mut self, line: Line, first: usize, len: usize, key: u32, image_row: u16) {
        let end = first + len;
        let row = &mut self.term.grid_mut()[line];
        if first > 0 && row[Column(first)].flags.contains(Flags::WIDE_CHAR_SPACER) {
            row[Column(first - 1)] = GridCell::default();
        }
        if end < row.len() && row[Column(end)].flags.contains(Flags::WIDE_CHAR_SPACER) {
            row[Column(end)] = GridCell::default();
        }
        for offset in 0..len {
            row[Column(first + offset)] = GridCell {
                c: IMAGE_CELL,
                fg: Color::Spec(key_color(key)),
                bg: Color::Spec(position_color(image_row, offset as u16)),
                ..GridCell::default()
            };
        }
    }

    pub(super) fn clip_images(&mut self, columns: usize) {
        if self.graphics.is_idle() {
            return;
        }
        let grid = self.term.grid();
        let lines = grid.topmost_line().0..=grid.bottommost_line().0;
        self.erase_images(lines, columns);
    }

    fn clear_visible_images(&mut self) {
        let last = self.term.grid().screen_lines() as i32 - 1;
        self.erase_images(0..=last, 0);
    }

    fn erase_images(&mut self, lines: std::ops::RangeInclusive<i32>, from_column: usize) {
        let grid = self.term.grid_mut();
        for line in lines {
            let row = &mut grid[Line(line)];
            for column in from_column..row.len() {
                if row[Column(column)].c == IMAGE_CELL {
                    row[Column(column)] = GridCell::default();
                }
            }
        }
    }

    fn collect_images(&mut self) {
        let grid = self.term.grid();
        let mut live = HashSet::new();
        for line in grid.topmost_line().0..=grid.bottommost_line().0 {
            let row = &grid[Line(line)];
            for column in 0..row.len() {
                let cell = &row[Column(column)];
                if cell.c == IMAGE_CELL
                    && let Color::Spec(key) = cell.fg
                {
                    live.insert(color_key(key));
                }
            }
        }
        let alt_screen = self.term.mode().contains(TermMode::ALT_SCREEN);
        self.graphics.collect(alt_screen, &live);
    }
}

fn key_color(key: u32) -> Rgb {
    Rgb {
        r: (key >> 16) as u8,
        g: (key >> 8) as u8,
        b: key as u8,
    }
}

fn color_key(color: Rgb) -> u32 {
    u32::from(color.r) << 16 | u32::from(color.g) << 8 | u32::from(color.b)
}

fn position_color(image_row: u16, image_col: u16) -> Rgb {
    Rgb {
        r: (image_row >> 4) as u8,
        g: ((image_row & 0xf) << 4 | image_col >> 8) as u8,
        b: image_col as u8,
    }
}

fn color_position(color: Rgb) -> (u16, u16) {
    (
        u16::from(color.r) << 4 | u16::from(color.g) >> 4,
        (u16::from(color.g) & 0xf) << 8 | u16::from(color.b),
    )
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::terminal::graphics::tests::{payload, png};
    use crate::terminal::{Selection, SelectionPoint, TerminalSize, TerminalTheme};
    use std::io::Write;
    use std::sync::{Arc, Mutex};

    #[derive(Clone, Default)]
    struct Replies(Arc<Mutex<Vec<u8>>>);

    impl Write for Replies {
        fn write(&mut self, buf: &[u8]) -> std::io::Result<usize> {
            self.0.lock().unwrap().extend_from_slice(buf);
            Ok(buf.len())
        }
        fn flush(&mut self) -> std::io::Result<()> {
            Ok(())
        }
    }

    fn engine(columns: usize, lines: usize) -> (TerminalEngine, Replies) {
        let replies = Replies::default();
        let mut engine = TerminalEngine::new(
            TerminalSize::new(columns, lines),
            50,
            Arc::new(Mutex::new(Box::new(replies.clone()))),
            TerminalTheme::default(),
        );
        engine.set_cell_pixels((10, 20));
        (engine, replies)
    }

    fn command(control: &str, data: &[u8]) -> Vec<u8> {
        [b"\x1b_G".as_slice(), &payload(control, data), b"\x1b\\"].concat()
    }

    fn picture(columns: u32, rows: u32) -> Vec<u8> {
        command(
            "a=T,f=100",
            &png(columns * 10, rows * 20, [200, 100, 50, 255]),
        )
    }

    fn drawn(engine: &TerminalEngine) -> Vec<(usize, usize, f32)> {
        engine
            .render_images()
            .iter()
            .map(|slice| (slice.row, slice.col, slice.size[0]))
            .collect()
    }

    fn text_at(engine: &TerminalEngine, row: usize, col: usize) -> char {
        engine.render_cells()[row * engine.size().columns + col].ch
    }

    #[test]
    fn an_image_fills_cells_from_the_cursor_and_leaves_the_cursor_after_it() {
        let (mut engine, _) = engine(20, 6);

        engine.feed_bytes(b"ab");
        engine.feed_bytes(&picture(3, 2));
        engine.feed_bytes(b"X");

        assert_eq!(drawn(&engine), vec![(0, 2, 3.0), (1, 2, 3.0)]);
        assert_eq!(
            text_at(&engine, 1, 5),
            'X',
            "text continues on the last row"
        );
        assert_eq!(text_at(&engine, 0, 3), ' ', "an image cell shows no glyph");
        assert_eq!(engine.cursor_position(), (6, 1));
    }

    #[test]
    fn an_image_that_reaches_the_right_edge_is_cut_there_and_the_cursor_wraps() {
        let (mut engine, _) = engine(8, 6);

        engine.feed_bytes(b"12345");
        engine.feed_bytes(&picture(6, 1));

        assert_eq!(drawn(&engine), vec![(0, 5, 3.0)]);
        assert_eq!(engine.cursor_position(), (0, 1));
    }

    #[test]
    fn an_image_taller_than_the_space_below_scrolls_and_stays_whole() {
        let (mut engine, _) = engine(20, 3);

        engine.feed_bytes(b"one\r\ntwo\r\n");
        engine.feed_bytes(&picture(2, 3));

        assert_eq!(drawn(&engine), vec![(0, 0, 2.0), (1, 0, 2.0), (2, 0, 2.0)]);
        assert_eq!(engine.recent_lines(3, 2), vec!["one", "two"]);
    }

    #[test]
    fn holding_the_cursor_draws_without_moving_it_or_scrolling() {
        let (mut engine, _) = engine(20, 3);
        let held = command("a=T,f=100,C=1", &png(20, 60, [1, 2, 3, 255]));

        engine.feed_bytes(b"one\r\ntwo");
        engine.feed_bytes(&held);

        assert_eq!(engine.cursor_position(), (3, 1));
        assert_eq!(drawn(&engine), vec![(1, 3, 2.0), (2, 3, 2.0)]);
        assert_eq!(
            engine.scroll_position().1,
            0,
            "nothing scrolled into history"
        );
    }

    #[test]
    fn images_scroll_with_the_text_and_are_freed_when_everything_is_cleared() {
        let (mut engine, _) = engine(20, 4);

        engine.feed_bytes(&picture(2, 2));
        engine.feed_bytes(b"\r\n1\r\n2\r\n3\r\n4");
        assert_eq!(drawn(&engine), vec![], "scrolled off the top");

        engine.scroll(1);
        assert_eq!(drawn(&engine), vec![(0, 0, 2.0)]);
        engine.scroll(1);
        assert_eq!(drawn(&engine), vec![(0, 0, 2.0), (1, 0, 2.0)]);

        engine.scroll_to_bottom();
        engine.feed_bytes(b"\x1b[H\x1b[2J\x1b[3J");
        assert_eq!(drawn(&engine), vec![]);
        assert!(
            engine.graphics.is_idle(),
            "cleared images still hold memory"
        );
    }

    #[test]
    fn a_row_of_an_image_is_one_piece_until_text_is_written_over_part_of_it() {
        let (mut engine, _) = engine(20, 3);

        engine.feed_bytes(&picture(4, 1));
        engine.feed_bytes(b"\x1b[1;2Hz");

        let pieces: Vec<(usize, f32, f32)> = engine
            .render_images()
            .iter()
            .map(|slice| (slice.col, slice.uv[0], slice.uv[2]))
            .collect();
        assert_eq!(pieces, vec![(0, 0.0, 0.25), (2, 0.5, 1.0)]);
        assert_eq!(text_at(&engine, 0, 1), 'z');
    }

    #[test]
    fn text_read_back_from_the_screen_skips_images() {
        let (mut engine, _) = engine(20, 3);

        engine.feed_bytes(b"a");
        engine.feed_bytes(&picture(2, 1));
        engine.feed_bytes(b"b\r\n");

        assert_eq!(engine.recent_lines(2, 1), vec!["ab"]);
        let everything = Selection {
            start: SelectionPoint { row: 0, col: 0 },
            end: SelectionPoint { row: 0, col: 19 },
            anchor_offset: 0,
        };
        assert_eq!(engine.selection_text(&everything).as_deref(), Some("ab"));
    }

    #[test]
    fn a_support_query_is_answered_before_the_device_attributes_asked_after_it() {
        let (mut engine, replies) = engine(20, 3);
        let query = command("a=q,f=24,s=1,v=1,i=31", &[1, 2, 3]);

        engine.feed_bytes(&[query.as_slice(), b"\x1b[c"].concat());

        assert_eq!(
            String::from_utf8_lossy(&replies.0.lock().unwrap()),
            "\x1b_Gi=31;OK\x1b\\\x1b[?62;22c",
            "tools read the answers in order and expect more than a VT102 id"
        );
    }

    #[test]
    fn an_image_inside_a_synchronized_update_lands_where_the_update_put_the_cursor() {
        let (mut engine, _) = engine(20, 6);

        engine.feed_bytes(b"\x1b[?2026h\x1b[3;5H");
        engine.feed_bytes(&picture(2, 1));
        engine.feed_bytes(b"\x1b[?2026l");

        assert_eq!(drawn(&engine), vec![(2, 4, 2.0)]);
    }

    #[test]
    fn an_image_over_half_of_a_wide_character_removes_the_whole_character() {
        let (mut engine, _) = engine(20, 3);

        engine.feed_bytes("한글".as_bytes());
        engine.feed_bytes(b"\x1b[1;2H");
        engine.feed_bytes(&picture(1, 1));

        assert_eq!(
            text_at(&engine, 0, 0),
            ' ',
            "half a glyph would overlap the image"
        );
        assert_eq!(text_at(&engine, 0, 2), '글');
    }

    #[test]
    fn deleting_all_images_removes_them_from_the_screen_and_frees_them() {
        let (mut engine, _) = engine(20, 4);

        engine.feed_bytes(&picture(2, 2));
        engine.feed_bytes(&command("a=d", &[]));

        assert_eq!(drawn(&engine), vec![]);
        assert!(engine.graphics.is_idle());
    }

    #[test]
    fn leaving_a_fullscreen_program_frees_the_images_it_showed() {
        let (mut engine, _) = engine(20, 4);

        engine.feed_bytes(b"\x1b[?1049h");
        engine.feed_bytes(&picture(2, 2));
        assert!(!engine.graphics.is_idle());

        engine.feed_bytes(b"\x1b[?1049l");
        assert!(engine.graphics.is_idle());
    }

    #[test]
    fn images_that_were_drawn_over_are_forgotten_once_enough_pile_up() {
        let (mut engine, _) = engine(20, 4);
        let frame = [b"\x1b[H".as_slice(), &picture(1, 1)].concat();

        for _ in 0..600 {
            engine.feed_bytes(&frame);
        }

        assert!(engine.graphics.placements() < 300);
        assert_eq!(drawn(&engine), vec![(0, 0, 1.0)]);
    }

    #[test]
    fn narrowing_the_terminal_cuts_an_image_at_the_new_edge_instead_of_wrapping_it() {
        let (mut engine, _) = engine(20, 4);

        engine.feed_bytes(&picture(6, 2));
        engine.feed_bytes(b"\r\n");
        engine.resize(TerminalSize::new(4, 4));

        assert_eq!(drawn(&engine), vec![(0, 0, 4.0), (1, 0, 4.0)]);
    }
}

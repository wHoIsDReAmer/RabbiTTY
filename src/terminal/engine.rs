use super::theme::{enforce_min_contrast, resolve_rgb, rgb_to_rgba};
use super::{CellVisual, TerminalSize, TerminalTheme};
use alacritty_terminal::event::{Event, EventListener, WindowSize};
use alacritty_terminal::grid::Dimensions;
use alacritty_terminal::grid::Scroll;
use alacritty_terminal::term::cell::Flags;
use alacritty_terminal::term::color::Colors;
use alacritty_terminal::term::{
    Config as TermConfig, Osc52, RenderableContent, Term, TermMode, point_to_viewport,
};
use alacritty_terminal::vte::ansi::{CursorShape, NamedColor, Processor, Rgb};
use std::cell::{Cell, RefCell};
use std::io::Write;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex};

pub struct TerminalEngine {
    term: Term<PtyEventProxy>,
    processor: Processor,
    size: TerminalSize,
    theme: TerminalTheme,
    cells_cache: RefCell<Arc<Vec<CellVisual>>>,
    cache_dirty: Cell<bool>,
    cache_size: Cell<TerminalSize>,
    /// Bumped once per real cache rebuild. The renderer keys damage detection
    /// on this instead of the buffer address, because a rebuild reuses the
    /// same allocation in place and so leaves the pointer unchanged.
    cells_generation: Cell<u64>,
    title: Arc<Mutex<Option<TitleChange>>>,
    bell_pending: Arc<AtomicBool>,
    osc: Arc<Mutex<OscPending>>,
    writer: Arc<Mutex<Box<dyn Write + Send>>>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum TitleChange {
    Set(String),
    Reset,
}

pub type ClipboardFormatter = Arc<dyn Fn(&str) -> String + Send + Sync>;
type ColorFormatter = Arc<dyn Fn(Rgb) -> String + Send + Sync>;

/// OSC work the pty thread cannot finish on its own.
#[derive(Default)]
pub(super) struct OscPending {
    clipboard_write: Option<String>,
    clipboard_read: Option<ClipboardFormatter>,
    colors: Vec<(usize, ColorFormatter)>,
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
            // Refused later against our own config, which can change freely.
            osc52: Osc52::CopyPaste,
            ..Default::default()
        };
        let title: Arc<Mutex<Option<TitleChange>>> = Arc::new(Mutex::new(None));
        let bell_pending: Arc<AtomicBool> = Arc::new(AtomicBool::new(false));
        let osc: Arc<Mutex<OscPending>> = Arc::new(Mutex::new(OscPending::default()));
        let term = Term::new(
            config,
            &size,
            PtyEventProxy {
                writer: Arc::clone(&writer),
                size,
                title: Arc::clone(&title),
                bell_pending: Arc::clone(&bell_pending),
                osc: Arc::clone(&osc),
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
            cells_generation: Cell::new(0),
            title,
            bell_pending,
            osc,
            writer,
        }
    }

    pub fn size(&self) -> TerminalSize {
        self.size
    }

    pub fn take_title(&self) -> Option<TitleChange> {
        self.title.lock().ok()?.take()
    }

    /// Returns true once if a bell rang since the last call.
    pub fn take_bell(&self) -> bool {
        self.bell_pending.swap(false, Ordering::Relaxed)
    }

    pub fn feed_bytes(&mut self, bytes: &[u8]) {
        self.processor.advance(&mut self.term, bytes);
        self.answer_color_requests();
        self.cache_dirty.set(true);
    }

    /// Answered from the live palette, so an OSC 4 override wins over the theme.
    fn answer_color_requests(&mut self) {
        let pending = match self.osc.lock() {
            Ok(mut guard) if !guard.colors.is_empty() => std::mem::take(&mut guard.colors),
            _ => return,
        };
        let colors = self.term.colors();
        let replies: Vec<String> = pending
            .into_iter()
            .map(|(index, format)| format(self.color_at(colors, index)))
            .collect();
        if let Ok(mut guard) = self.writer.lock() {
            for reply in replies {
                let _ = guard.write_all(reply.as_bytes());
            }
            let _ = guard.flush();
        }
    }

    fn color_at(&self, colors: &Colors, index: usize) -> Rgb {
        if let Some(rgb) = colors[index] {
            return rgb;
        }
        match index {
            0..=255 => self.theme.indexed_color(index as u8),
            257 => self.theme.named_color(NamedColor::Background),
            258 => self.theme.named_color(NamedColor::Cursor),
            _ => self.theme.named_color(NamedColor::Foreground),
        }
    }

    pub fn take_clipboard_write(&self) -> Option<String> {
        self.osc.lock().ok()?.clipboard_write.take()
    }

    pub fn take_clipboard_read(&self) -> Option<ClipboardFormatter> {
        self.osc.lock().ok()?.clipboard_read.take()
    }

    pub fn reply_clipboard(&self, format: &ClipboardFormatter, text: &str) {
        if let Ok(mut guard) = self.writer.lock() {
            let _ = guard.write_all(format(text).as_bytes());
            let _ = guard.flush();
        }
    }

    pub fn resize(&mut self, new_size: TerminalSize) {
        self.size = new_size;
        self.term.resize(new_size);
        self.cache_dirty.set(true);
    }

    pub fn render_cells(&self) -> Arc<Vec<CellVisual>> {
        self.render_cells_versioned().0
    }

    pub fn render_cells_versioned(&self) -> (Arc<Vec<CellVisual>>, u64) {
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
            self.cells_generation.set(self.cells_generation.get() + 1);
            self.cache_dirty.set(false);
            self.cache_size.set(self.size);
        }
        (
            self.cells_cache.borrow().clone(),
            self.cells_generation.get(),
        )
    }

    pub fn scroll(&mut self, delta: i32) {
        self.term.scroll_display(Scroll::Delta(delta));
        self.cache_dirty.set(true);
    }

    pub fn scroll_to_bottom(&mut self) {
        if self.term.grid().display_offset() == 0 {
            return;
        }
        self.term.scroll_display(Scroll::Bottom);
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

    /// Returns true when the running program has enabled mouse reporting.
    pub fn mouse_mode(&self) -> bool {
        self.term.mode().intersects(TermMode::MOUSE_MODE)
    }

    /// Returns true when the program requests SGR-encoded mouse events.
    pub fn sgr_mouse(&self) -> bool {
        self.term.mode().contains(TermMode::SGR_MOUSE)
    }

    /// Returns true when the program has enabled application cursor keys
    /// (`\e[?1h`). ncurses programs set this and then expect `ESC O A` rather
    /// than `ESC [ A`, so sending the wrong form makes arrows unreadable.
    pub fn app_cursor(&self) -> bool {
        self.term.mode().contains(TermMode::APP_CURSOR)
    }

    /// Returns true when the terminal is in the alternate screen buffer.
    pub fn alt_screen(&self) -> bool {
        self.term.mode().contains(TermMode::ALT_SCREEN)
    }

    /// Returns true when the running program has enabled bracketed paste
    /// (`\e[?2004h`).
    pub fn bracketed_paste(&self) -> bool {
        self.term.mode().contains(TermMode::BRACKETED_PASTE)
    }

    /// Current text cursor as `(col, row)` in viewport coordinates.
    pub fn cursor_position(&self) -> (usize, usize) {
        let point = self.term.grid().cursor.point;
        (point.column.0, point.line.0.max(0) as usize)
    }

    /// The renderable cursor cell as `(col, row)`, or `None` when the cursor is
    /// hidden or the viewport is scrolled away from the latest output.
    pub fn cursor_cell(&self) -> Option<(usize, usize)> {
        let RenderableContent {
            display_offset,
            cursor,
            ..
        } = self.term.renderable_content();

        if cursor.shape == CursorShape::Hidden || display_offset != 0 {
            return None;
        }

        let col = cursor.point.column.0;
        let row = cursor.point.line.0 as usize;
        if row < self.size.lines && col < self.size.columns {
            Some((col, row))
        } else {
            None
        }
    }

    /// The theme cursor color as linear RGBA (opaque).
    pub fn cursor_color(&self) -> [f32; 4] {
        rgb_to_rgba(self.theme.cursor_rgb(), 1.0)
    }

    fn build_cells_into(&self, cells: &mut Vec<CellVisual>) {
        let RenderableContent {
            display_iter,
            display_offset,
            colors,
            ..
        } = self.term.renderable_content();

        let default_fg = rgb_to_rgba(self.theme.foreground, 1.0);

        // Default cells are transparent so the panel background shows through.
        let default_bg = [0.0, 0.0, 0.0, 0.0];
        let total = self.size.lines * self.size.columns;
        let default_cell = CellVisual {
            ch: ' ',
            col: 0,
            row: 0,
            fg: default_fg,
            bg: default_bg,
            underline: false,
            hyperlink: None,
            wide: false,
        };

        cells.clear();
        cells.resize(total, default_cell);

        let idx = |row: usize, col: usize, cols: usize| row * cols + col;

        let mut contrast_memo: Vec<((Rgb, Rgb), Rgb)> = Vec::with_capacity(32);

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

                    // Contrast enforcement runs powf per channel; a screen reuses
                    // only a handful of (fg, bg) pairs, so memoize within the frame.
                    let key = (fg_rgb, bg_rgb);
                    fg_rgb = if let Some((_, v)) = contrast_memo.iter().find(|(k, _)| *k == key) {
                        *v
                    } else {
                        let v = enforce_min_contrast(fg_rgb, bg_rgb, self.theme.minimum_contrast());
                        if contrast_memo.len() < 64 {
                            contrast_memo.push((key, v));
                        }
                        v
                    };

                    let mut fg = rgb_to_rgba(fg_rgb, 1.0);
                    // When the cell background matches the theme background, leave
                    // it transparent so the panel background shows through exactly
                    // (avoids double-alpha compositing vs. other panes like Settings).
                    // Non-default backgrounds (selections, highlights) stay opaque.
                    let bg = if bg_rgb == self.theme.background {
                        [0.0, 0.0, 0.0, 0.0]
                    } else {
                        rgb_to_rgba(bg_rgb, 1.0)
                    };

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
                    slot.hyperlink = indexed
                        .cell
                        .hyperlink()
                        .map(|link| std::sync::Arc::from(link.uri()));
                }
            }
        }
    }
}

#[derive(Clone)]
struct PtyEventProxy {
    writer: Arc<Mutex<Box<dyn Write + Send>>>,
    size: TerminalSize,
    title: Arc<Mutex<Option<TitleChange>>>,
    bell_pending: Arc<AtomicBool>,
    osc: Arc<Mutex<OscPending>>,
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
                    *guard = Some(TitleChange::Set(new_title));
                }
            }
            Event::ResetTitle => {
                if let Ok(mut guard) = self.title.lock() {
                    *guard = Some(TitleChange::Reset);
                }
            }
            Event::Bell => {
                self.bell_pending.store(true, Ordering::Relaxed);
            }
            // Last write wins.
            Event::ClipboardStore(_, text) => {
                if let Ok(mut guard) = self.osc.lock() {
                    guard.clipboard_write = Some(text);
                }
            }
            Event::ClipboardLoad(_, formatter) => {
                if let Ok(mut guard) = self.osc.lock() {
                    guard.clipboard_read = Some(formatter);
                }
            }
            Event::ColorRequest(index, formatter) => {
                if let Ok(mut guard) = self.osc.lock() {
                    guard.colors.push((index, formatter));
                }
            }
            _ => {}
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn test_engine() -> TerminalEngine {
        TerminalEngine::new(
            TerminalSize::new(8, 3),
            100,
            Arc::new(Mutex::new(Box::new(std::io::sink()))),
            TerminalTheme::default(),
        )
    }

    /// Shares the engine's reply buffer so a test can read what it answered.
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

    fn engine_with_replies() -> (TerminalEngine, Replies) {
        let replies = Replies::default();
        let engine = TerminalEngine::new(
            TerminalSize::new(8, 3),
            100,
            Arc::new(Mutex::new(Box::new(replies.clone()))),
            TerminalTheme::default(),
        );
        (engine, replies)
    }

    fn answered(replies: &Replies) -> String {
        String::from_utf8_lossy(&replies.0.lock().unwrap()).into_owned()
    }

    #[test]
    fn a_colour_query_is_answered_from_the_theme() {
        let (mut engine, replies) = engine_with_replies();
        engine.feed_bytes(b"\x1b]4;1;?\x07");
        let reply = answered(&replies);
        assert!(reply.contains("4;1;"), "no OSC 4 reply: {reply:?}");
        assert!(
            reply.contains("rgb:"),
            "reply is not an rgb spec: {reply:?}"
        );
    }

    #[test]
    fn a_colour_query_reports_what_osc_4_set_rather_than_the_theme() {
        let (mut engine, replies) = engine_with_replies();
        engine.feed_bytes(b"\x1b]4;1;rgb:ffff/0000/0000\x07");
        engine.feed_bytes(b"\x1b]4;1;?\x07");
        let reply = answered(&replies);
        assert!(
            reply.contains("ffff/0000/0000"),
            "override not reported: {reply:?}"
        );
    }

    #[test]
    fn the_foreground_and_background_can_be_queried_too() {
        let (mut engine, replies) = engine_with_replies();
        engine.feed_bytes(b"\x1b]10;?\x07");
        engine.feed_bytes(b"\x1b]11;?\x07");
        let reply = answered(&replies);
        assert!(reply.contains("10;"), "no foreground reply: {reply:?}");
        assert!(reply.contains("11;"), "no background reply: {reply:?}");
    }

    #[test]
    fn osc_52_hands_the_decoded_text_to_the_host() {
        let mut engine = test_engine();
        // "hi" base64 encoded.
        engine.feed_bytes(b"\x1b]52;c;aGk=\x07");
        assert_eq!(engine.take_clipboard_write().as_deref(), Some("hi"));
        assert_eq!(engine.take_clipboard_write(), None, "not drained");
    }

    #[test]
    fn an_osc_52_query_asks_the_host_and_the_reply_reaches_the_program() {
        let (mut engine, replies) = engine_with_replies();
        engine.feed_bytes(b"\x1b]52;c;?\x07");
        let format = engine.take_clipboard_read().expect("no read request");
        engine.reply_clipboard(&format, "hi");
        let reply = answered(&replies);
        assert!(reply.contains("52;"), "no OSC 52 reply: {reply:?}");
        assert!(reply.contains("aGk="), "text not encoded back: {reply:?}");
    }

    #[test]
    fn a_title_reset_is_reported_separately_from_a_title_change() {
        let mut engine = test_engine();
        // Stack the absent title, set one, then pop back to it.
        engine.feed_bytes(b"\x1b[22t");
        engine.feed_bytes(b"\x1b]2;hello\x07");
        assert_eq!(engine.take_title(), Some(TitleChange::Set("hello".into())));
        engine.feed_bytes(b"\x1b[23t");
        assert_eq!(engine.take_title(), Some(TitleChange::Reset));
    }

    #[test]
    fn scroll_to_bottom_returns_viewport_to_latest_output() {
        let mut engine = test_engine();

        engine.feed_bytes(b"one\r\ntwo\r\nthree\r\nfour\r\nfive\r\n");
        engine.scroll(2);

        assert!(engine.scroll_position().0 > 0);

        engine.scroll_to_bottom();

        assert_eq!(engine.scroll_position().0, 0);
    }

    #[test]
    fn repainting_the_same_cells_in_place_still_advances_the_generation() {
        let mut engine = test_engine();

        engine.feed_bytes(b"\x1b[Haaaa");
        let (cells, first_gen) = engine.render_cells_versioned();
        let first_ptr = Arc::as_ptr(&cells);
        let before: String = cells.iter().map(|cell| cell.ch).collect();
        // Releasing the clone is what the render loop does between frames, and
        // it is what lets the rebuild happen in place.
        drop(cells);

        engine.feed_bytes(b"\x1b[Hbbbb");
        let (cells, second_gen) = engine.render_cells_versioned();
        let after: String = cells.iter().map(|cell| cell.ch).collect();

        assert_ne!(before, after, "repaint did not reach the cell buffer");
        assert_eq!(
            Arc::as_ptr(&cells),
            first_ptr,
            "in-place rebuild expected; the pointer is not a damage signal"
        );
        assert!(
            second_gen > first_gen,
            "generation did not advance: {first_gen} -> {second_gen}"
        );
    }
}

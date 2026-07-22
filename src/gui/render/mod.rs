use crate::gui::pane::PaneNode;
use crate::terminal::{CellVisual, GridPos, Selection, SelectionPoint, TerminalSize};
use iced::advanced::mouse::{Click, click};
use iced::mouse;
use iced::wgpu;
use iced::widget::shader::Program as ShaderProgram;
use iced::widget::shader::{Action, Pipeline, Primitive, Shader, Viewport};
use iced::{Event, Length, Point, Rectangle};
use std::sync::Arc;

pub const SCROLLBAR_WIDTH: f32 = 8.0;

mod bg;
mod composite;
mod text;
use bg::BackgroundPipeline;
use composite::CompositePipeline;
use text::TextPipelineData;

const SELECTION_BG: [f32; 4] = [0.25, 0.38, 0.60, 1.0];

/// Iced shader wrapper for terminal rendering.
#[derive(Debug, Clone)]
pub struct PaneView {
    pub id: u64,
    pub scroll_history: usize,
    pub cells: Arc<Vec<CellVisual>>,
    pub grid_size: TerminalSize,
    pub selection: Option<Selection>,
    pub display_offset: usize,
    pub cursor: Option<[u32; 2]>,
    pub cursor_visible: bool,
    pub cursor_color: [f32; 4],
    pub mouse_mode: bool,
}

pub struct TerminalProgram {
    pub panes: Vec<PaneView>,
    pub scrollbar_color: [f32; 4],
    pub focused: u64,
    pub focus_color: [f32; 4],
    pub divider_color: [f32; 4],
    pub cell_size: [f32; 2],
    pub layout: PaneNode,
    pub terminal_font_selection: Option<String>,
    pub terminal_font_size: f32,
    pub padding: [f32; 2],
    pub clear_color: [f32; 4],
    pub cursor_shape: crate::config::CursorShape,
    pub background_opacity: f32,
}

impl PaneView {
    fn inner(rect: Rectangle, padding: [f32; 2]) -> Rectangle {
        Rectangle {
            x: rect.x + padding[0],
            y: rect.y + padding[1],
            width: (rect.width - padding[0] * 2.0).max(1.0),
            height: (rect.height - padding[1] * 2.0).max(1.0),
        }
    }

    fn pixel_to_grid(
        &self,
        pos: Point,
        rect: Rectangle,
        padding: [f32; 2],
        cell: [f32; 2],
    ) -> GridPos {
        let inner = Self::inner(rect, padding);
        let cell_w = cell[0].max(1.0);
        let cell_h = cell[1].max(1.0);
        let x = (pos.x - inner.x).max(0.0);
        let y = (pos.y - inner.y).max(0.0);
        GridPos {
            row: ((y / cell_h) as usize).min(self.grid_size.lines.saturating_sub(1)),
            col: ((x / cell_w) as usize).min(self.grid_size.columns.saturating_sub(1)),
        }
    }

    fn row_chars(&self, row: usize) -> Vec<char> {
        let cols = self.grid_size.columns;
        if cols == 0 || row >= self.grid_size.lines {
            return Vec::new();
        }
        (0..cols).map(|col| self.cell_char(row, col)).collect()
    }

    fn link_at(&self, grid: GridPos) -> Option<String> {
        if let Some(uri) = self
            .cells
            .get(grid.row * self.grid_size.columns.max(1) + grid.col)
            .and_then(|cell| cell.hyperlink.clone())
        {
            return crate::terminal::url::is_openable(&uri).then(|| uri.to_string());
        }
        crate::terminal::url::url_at(&self.row_chars(grid.row), grid.col).map(|span| span.url)
    }

    fn link_span_at(&self, grid: GridPos) -> Option<(usize, usize)> {
        if let Some(cell) = self
            .cells
            .get(grid.row * self.grid_size.columns.max(1) + grid.col)
            && let Some(uri) = cell.hyperlink.as_deref()
            && crate::terminal::url::is_openable(uri)
        {
            return Some(self.hyperlink_run(grid, uri));
        }
        crate::terminal::url::url_at(&self.row_chars(grid.row), grid.col)
            .map(|span| (span.start, span.end))
    }

    fn hyperlink_run(&self, grid: GridPos, uri: &str) -> (usize, usize) {
        let cols = self.grid_size.columns.max(1);
        let same = |col: usize| {
            self.cells
                .get(grid.row * cols + col)
                .and_then(|c| c.hyperlink.as_deref())
                == Some(uri)
        };
        let mut start = grid.col;
        while start > 0 && same(start - 1) {
            start -= 1;
        }
        let mut end = grid.col;
        while end + 1 < self.grid_size.columns && same(end + 1) {
            end += 1;
        }
        (start, end)
    }

    fn cell_char(&self, row: usize, col: usize) -> char {
        let cols = self.grid_size.columns;
        if cols == 0 || col >= cols || row >= self.grid_size.lines {
            return ' ';
        }
        self.cells
            .get(row * cols + col)
            .map(|c| c.ch)
            .unwrap_or(' ')
    }

    fn word_selection(&self, grid: GridPos) -> Option<Selection> {
        let cols = self.grid_size.columns;
        if cols == 0 || !is_word_char(self.cell_char(grid.row, grid.col)) {
            return None;
        }
        let mut start = grid.col;
        while start > 0 && is_word_char(self.cell_char(grid.row, start - 1)) {
            start -= 1;
        }
        let mut end = grid.col;
        while end + 1 < cols && is_word_char(self.cell_char(grid.row, end + 1)) {
            end += 1;
        }
        Some(Selection {
            start: SelectionPoint {
                row: grid.row as i64,
                col: start,
            },
            end: SelectionPoint {
                row: grid.row as i64,
                col: end,
            },
            anchor_offset: self.display_offset,
        })
    }

    fn line_selection(&self, grid: GridPos) -> Selection {
        let cols = self.grid_size.columns;
        let mut end = 0usize;
        for col in 0..cols {
            if self.cell_char(grid.row, col) != ' ' {
                end = col;
            }
        }
        Selection {
            start: SelectionPoint {
                row: grid.row as i64,
                col: 0,
            },
            end: SelectionPoint {
                row: grid.row as i64,
                col: end,
            },
            anchor_offset: self.display_offset,
        }
    }
}

impl TerminalProgram {
    pub fn widget(self) -> Shader<crate::gui::app::Message, Self> {
        Shader::new(self).width(Length::Fill).height(Length::Fill)
    }

    fn regions(&self, bounds: Rectangle) -> Vec<(u64, Rectangle)> {
        self.layout.regions(Rectangle {
            x: 0.0,
            y: 0.0,
            width: bounds.width,
            height: bounds.height,
        })
    }

    fn pane(&self, id: u64) -> Option<&PaneView> {
        self.panes.iter().find(|p| p.id == id)
    }

    fn scrollbar_at(&self, pos: Point, bounds: Rectangle) -> Option<(u64, Rectangle)> {
        let regions = self.regions(bounds);
        regions.into_iter().find_map(|(id, rect)| {
            let pane = self.pane(id)?;
            if pane.scroll_history == 0 {
                return None;
            }
            let bar = Rectangle {
                x: rect.x + rect.width - SCROLLBAR_WIDTH,
                y: rect.y,
                width: SCROLLBAR_WIDTH,
                height: rect.height,
            };
            bar.contains(pos).then_some((id, rect))
        })
    }

    fn scroll_rel_at(&self, id: u64, pos: Point, bounds: Rectangle) -> Option<f32> {
        let pane = self.pane(id)?;
        let rect = self
            .regions(bounds)
            .into_iter()
            .find(|(rid, _)| *rid == id)?
            .1;
        let total = (pane.scroll_history + pane.grid_size.lines).max(1) as f32;
        let thumb = (rect.height * pane.grid_size.lines as f32 / total).max(16.0);
        Some(scrollbar_rel(pos.y, rect.y, rect.height, thumb))
    }

    fn pane_under(&self, pos: Point, bounds: Rectangle) -> Option<(&PaneView, Rectangle)> {
        let regions = self.regions(bounds);
        let id = crate::gui::pane::pane_at(&regions, pos)?;
        let rect = regions.iter().find(|(rid, _)| *rid == id)?.1;
        self.pane(id).map(|pane| (pane, rect))
    }
}

#[derive(Debug, Default)]
pub struct TerminalShaderState {
    dragging: bool,
    drag_start: Option<GridPos>,
    drag_anchor_offset: usize,
    /// Last left-button click, used to detect double/triple clicks.
    last_click: Option<Click>,
    drag_pane: Option<u64>,
    scrollbar_drag: Option<u64>,
    last_bounds: Rectangle,
    modifiers: iced::keyboard::Modifiers,
}

/// Word delimiter check (alacritty-style). A "word" is a run of non-whitespace
/// characters that are not common semantic-escape delimiters, so paths and URLs
/// select as a single unit.
fn is_word_char(c: char) -> bool {
    if c == '\0' || c.is_whitespace() {
        return false;
    }
    !matches!(
        c,
        ',' | '│' | '`' | '"' | '\'' | '(' | ')' | '[' | ']' | '{' | '}' | '<' | '>'
    )
}

type Message = crate::gui::app::Message;

/// Translate an absolute window-space cursor position into bounds-local
/// coordinates clamped to the bounds rectangle. Used while dragging so a
/// selection still updates after the cursor leaves the terminal area.
fn clamp_to_bounds(absolute: Point, bounds: Rectangle) -> Point {
    Point::new(
        (absolute.x - bounds.x).clamp(0.0, bounds.width.max(0.0)),
        (absolute.y - bounds.y).clamp(0.0, bounds.height.max(0.0)),
    )
}

fn scrollbar_rel(pos_y: f32, rect_y: f32, rect_height: f32, thumb: f32) -> f32 {
    let travel = (rect_height - thumb).max(1.0);
    ((pos_y - rect_y - thumb / 2.0) / travel).clamp(0.0, 1.0)
}

fn link_modifier(modifiers: iced::keyboard::Modifiers) -> bool {
    #[cfg(target_os = "macos")]
    {
        modifiers.logo()
    }
    #[cfg(not(target_os = "macos"))]
    {
        modifiers.control()
    }
}

impl ShaderProgram<Message> for TerminalProgram {
    type State = TerminalShaderState;
    type Primitive = TerminalPrimitive;

    fn update(
        &self,
        state: &mut Self::State,
        event: &Event,
        bounds: Rectangle,
        cursor: mouse::Cursor,
    ) -> Option<Action<Message>> {
        let padding = self.padding;

        if bounds.width != state.last_bounds.width || bounds.height != state.last_bounds.height {
            state.last_bounds = bounds;
            return Some(Action::publish(Message::TerminalAreaResized(
                iced::Size::new(bounds.width, bounds.height),
            )));
        }

        match event {
            Event::Keyboard(iced::keyboard::Event::ModifiersChanged(modifiers)) => {
                state.modifiers = *modifiers;
            }
            Event::Mouse(mouse::Event::ButtonPressed(mouse::Button::Left)) => {
                let pos = cursor.position_in(bounds)?;

                if let Some((id, _)) = self.scrollbar_at(pos, bounds) {
                    state.scrollbar_drag = Some(id);
                    if let Some(rel) = self.scroll_rel_at(id, pos, bounds) {
                        return Some(
                            Action::publish(Message::PaneScrollTo { pane: id, rel }).and_capture(),
                        );
                    }
                    return None;
                }

                let (pane, rect) = self.pane_under(pos, bounds)?;
                let grid_pos = pane.pixel_to_grid(pos, rect, padding, self.cell_size);

                if link_modifier(state.modifiers)
                    && let Some(url) = pane.link_at(grid_pos)
                {
                    return Some(Action::publish(Message::OpenUrl(url)).and_capture());
                }
                if pane.mouse_mode {
                    state.dragging = true;
                    state.drag_pane = Some(pane.id);
                    return Some(
                        Action::publish(Message::TerminalMousePress {
                            pane: pane.id,
                            col: grid_pos.col,
                            row: grid_pos.row,
                        })
                        .and_capture(),
                    );
                }

                let click = Click::new(pos, mouse::Button::Left, state.last_click);
                state.last_click = Some(click);
                match click.kind() {
                    click::Kind::Double => {
                        state.dragging = false;
                        state.drag_start = None;
                        let sel = pane.word_selection(grid_pos);
                        return Some(
                            Action::publish(Message::SelectionChanged {
                                pane: pane.id,
                                selection: sel,
                            })
                            .and_capture(),
                        );
                    }
                    click::Kind::Triple => {
                        state.dragging = false;
                        state.drag_start = None;
                        let sel = pane.line_selection(grid_pos);
                        return Some(
                            Action::publish(Message::SelectionChanged {
                                pane: pane.id,
                                selection: Some(sel),
                            })
                            .and_capture(),
                        );
                    }
                    click::Kind::Single => {
                        state.dragging = true;
                        state.drag_pane = Some(pane.id);
                        state.drag_start = Some(grid_pos);
                        state.drag_anchor_offset = pane.display_offset;
                        return Some(
                            Action::publish(Message::SelectionChanged {
                                pane: pane.id,
                                selection: None,
                            })
                            .and_capture(),
                        );
                    }
                }
            }
            Event::Mouse(mouse::Event::ButtonPressed(mouse::Button::Right)) => {
                if let Some(pos) = cursor.position_in(bounds)
                    && let Some((pane, _)) = self.pane_under(pos, bounds)
                {
                    return Some(
                        Action::publish(Message::TerminalRightClick(pane.id)).and_capture(),
                    );
                }
            }
            Event::Mouse(mouse::Event::CursorMoved { .. }) => {
                // Use the absolute cursor position while dragging so the selection
                // still extends after the cursor leaves the terminal bounds.
                let pos_in = cursor.position_in(bounds);
                let pos_dragging = state.dragging.then(|| {
                    pos_in.unwrap_or_else(|| {
                        cursor
                            .position()
                            .map(|p| clamp_to_bounds(p, bounds))
                            .unwrap_or(Point::ORIGIN)
                    })
                });
                if let Some(id) = state.scrollbar_drag {
                    let pos = cursor
                        .position_in(bounds)
                        .or_else(|| cursor.position().map(|p| clamp_to_bounds(p, bounds)))?;
                    let rel = self.scroll_rel_at(id, pos, bounds)?;
                    return Some(
                        Action::publish(Message::PaneScrollTo { pane: id, rel }).and_capture(),
                    );
                }

                let pos = pos_dragging?;
                let regions = self.regions(bounds);
                let (pane, rect) = state
                    .drag_pane
                    .and_then(|id| {
                        regions
                            .iter()
                            .find(|(rid, _)| *rid == id)
                            .and_then(|(_, rect)| self.pane(id).map(|pane| (pane, *rect)))
                    })
                    .or_else(|| self.pane_under(pos, bounds))?;
                let grid_pos = pane.pixel_to_grid(pos, rect, padding, self.cell_size);

                if pane.mouse_mode {
                    return Some(
                        Action::publish(Message::TerminalMouseDrag {
                            col: grid_pos.col,
                            row: grid_pos.row,
                        })
                        .and_capture(),
                    );
                }
                if let Some(drag_start) = state.drag_start {
                    let raw_y = cursor.position().map(|p| p.y);
                    let out_up = raw_y.is_some_and(|y| y < bounds.y + rect.y);
                    let out_down = raw_y.is_some_and(|y| y > bounds.y + rect.y + rect.height);
                    if out_up || out_down {
                        return Some(
                            Action::publish(Message::TerminalSelectionAutoscroll {
                                up: out_up,
                                col: grid_pos.col,
                            })
                            .and_capture(),
                        );
                    }
                    // Translate the current viewport row back into the anchor frame
                    // so the selection follows content when the user scrolls.
                    let delta = pane.display_offset as i64 - state.drag_anchor_offset as i64;
                    let start = SelectionPoint {
                        row: drag_start.row as i64,
                        col: drag_start.col,
                    };
                    let end = SelectionPoint {
                        row: grid_pos.row as i64 - delta,
                        col: grid_pos.col,
                    };
                    if start != end {
                        let sel = Selection {
                            start,
                            end,
                            anchor_offset: state.drag_anchor_offset,
                        };
                        return Some(
                            Action::publish(Message::SelectionChanged {
                                pane: pane.id,
                                selection: Some(sel),
                            })
                            .and_capture(),
                        );
                    }
                }
            }
            Event::Mouse(mouse::Event::ButtonReleased(mouse::Button::Left)) => {
                if let Some(pos) = cursor.position_in(bounds)
                    && let Some((pane, rect)) = self.pane_under(pos, bounds)
                    && pane.mouse_mode
                {
                    let grid_pos = pane.pixel_to_grid(pos, rect, padding, self.cell_size);
                    state.dragging = false;
                    state.drag_pane = None;
                    return Some(
                        Action::publish(Message::TerminalMouseRelease {
                            col: grid_pos.col,
                            row: grid_pos.row,
                        })
                        .and_capture(),
                    );
                }
                if state.scrollbar_drag.take().is_some() {
                    return None;
                }
                if state.dragging {
                    state.dragging = false;
                    state.drag_pane = None;
                    return Some(
                        Action::publish(Message::TerminalSelectionAutoscrollStop).and_capture(),
                    );
                }
            }
            _ => {}
        }
        None
    }

    fn draw(
        &self,
        state: &Self::State,
        cursor: mouse::Cursor,
        bounds: Rectangle,
    ) -> Self::Primitive {
        let hovered = link_modifier(state.modifiers)
            .then(|| cursor.position_in(bounds))
            .flatten()
            .and_then(|pos| {
                self.pane_under(pos, bounds)
                    .map(|(pane, rect)| (pane, rect, pos))
            });

        let panes = self
            .regions(bounds)
            .into_iter()
            .filter_map(|(id, rect)| {
                let pane = self.pane(id)?;
                let inner = PaneView::inner(rect, self.padding);
                let link_row =
                    hovered
                        .as_ref()
                        .filter(|(p, _, _)| p.id == id)
                        .and_then(|(p, r, pos)| {
                            let grid = p.pixel_to_grid(*pos, *r, self.padding, self.cell_size);
                            p.link_span_at(grid)
                                .map(|(start, end)| (grid.row, start, end))
                        });
                let scrollbar = (pane.scroll_history > 0).then(|| {
                    let total = (pane.scroll_history + pane.grid_size.lines).max(1) as f32;
                    let height = pane.grid_size.lines as f32 / total;
                    let top = (pane.scroll_history - pane.display_offset.min(pane.scroll_history))
                        as f32
                        / total;
                    [top, height]
                });
                Some(PanePrimitive {
                    cells: Arc::clone(&pane.cells),
                    origin: [inner.x, inner.y],
                    rect: [rect.x, rect.y, rect.width, rect.height],
                    scrollbar,
                    focused: pane.id == self.focused,
                    selection: pane.selection,
                    display_offset: pane.display_offset,
                    cursor: pane.cursor.filter(|_| pane.cursor_visible),
                    cursor_color: pane.cursor_color,
                    link_row,
                })
            })
            .collect();

        TerminalPrimitive {
            panes,
            scrollbar_color: self.scrollbar_color,
            focus_color: self.focus_color,
            divider_color: self.divider_color,
            cell_size: self.cell_size,
            viewport: [bounds.width, bounds.height],
            clear_color: self.clear_color,
            terminal_font_selection: self.terminal_font_selection.clone(),
            terminal_font_size: self.terminal_font_size,
            cursor_shape: self.cursor_shape,
            background_opacity: self.background_opacity,
        }
    }

    fn mouse_interaction(
        &self,
        state: &Self::State,
        bounds: Rectangle,
        cursor: mouse::Cursor,
    ) -> mouse::Interaction {
        let Some(pos) = cursor.position_in(bounds) else {
            return mouse::Interaction::default();
        };
        if link_modifier(state.modifiers)
            && let Some((pane, rect)) = self.pane_under(pos, bounds)
            && pane
                .link_at(pane.pixel_to_grid(pos, rect, self.padding, self.cell_size))
                .is_some()
        {
            return mouse::Interaction::Pointer;
        }
        mouse::Interaction::Text
    }
}

#[derive(Debug)]
pub struct TerminalPipeline {
    bg: BackgroundPipeline,
    text: TextPipelineData,
    composite: CompositePipeline,
    last_panes: Vec<PaneSignature>,
    last_viewport: [f32; 2],
    last_cell_size: [f32; 2],
    last_font_size: f32,
    last_cursor_shape: crate::config::CursorShape,
    last_background_opacity: f32,
}

impl Pipeline for TerminalPipeline {
    fn new(device: &wgpu::Device, _queue: &wgpu::Queue, format: wgpu::TextureFormat) -> Self {
        Self {
            bg: BackgroundPipeline::new(device, format),
            text: TextPipelineData::new(device, format),
            composite: CompositePipeline::new(device, format),
            last_panes: Vec::new(),
            last_viewport: [0.0; 2],
            last_cell_size: [0.0; 2],
            last_font_size: 0.0,
            last_cursor_shape: crate::config::CursorShape::Block,
            last_background_opacity: 1.0,
        }
    }
}

#[derive(Debug, Clone, PartialEq)]
struct PaneSignature {
    cells_ptr: usize,
    cells_len: usize,
    origin: [f32; 2],
    rect: [f32; 4],
    scrollbar: Option<[u32; 2]>,
    focused: bool,
    selection: Option<Selection>,
    display_offset: usize,
    cursor: Option<[u32; 2]>,
    cursor_color: [f32; 4],
    link_row: Option<(usize, usize, usize)>,
}

#[derive(Debug)]
pub struct PanePrimitive {
    cells: Arc<Vec<CellVisual>>,
    origin: [f32; 2],
    rect: [f32; 4],
    scrollbar: Option<[f32; 2]>,
    focused: bool,
    selection: Option<Selection>,
    display_offset: usize,
    cursor: Option<[u32; 2]>,
    cursor_color: [f32; 4],
    link_row: Option<(usize, usize, usize)>,
}

impl PanePrimitive {
    fn signature(&self, scale: f32) -> PaneSignature {
        PaneSignature {
            cells_ptr: Arc::as_ptr(&self.cells) as usize,
            cells_len: self.cells.len(),
            origin: [self.origin[0] * scale, self.origin[1] * scale],
            rect: [
                self.rect[0] * scale,
                self.rect[1] * scale,
                self.rect[2] * scale,
                self.rect[3] * scale,
            ],
            focused: self.focused,
            scrollbar: self
                .scrollbar
                .map(|[top, height]| [(top * 4096.0) as u32, (height * 4096.0) as u32]),
            selection: self.selection,
            display_offset: self.display_offset,
            cursor: self.cursor,
            cursor_color: self.cursor_color,
            link_row: self.link_row,
        }
    }
}

#[derive(Debug)]
pub struct TerminalPrimitive {
    panes: Vec<PanePrimitive>,
    scrollbar_color: [f32; 4],
    focus_color: [f32; 4],
    divider_color: [f32; 4],
    cell_size: [f32; 2],
    viewport: [f32; 2],
    clear_color: [f32; 4],
    terminal_font_selection: Option<String>,
    terminal_font_size: f32,
    cursor_shape: crate::config::CursorShape,
    background_opacity: f32,
}

impl Primitive for TerminalPrimitive {
    type Pipeline = TerminalPipeline;

    fn prepare(
        &self,
        pipeline: &mut Self::Pipeline,
        device: &wgpu::Device,
        queue: &wgpu::Queue,
        _bounds: &Rectangle,
        viewport: &Viewport,
    ) {
        let scale = viewport.scale_factor().max(1.0);
        let view = [self.viewport[0] * scale, self.viewport[1] * scale];
        let font_size = self.terminal_font_size * scale;
        let offscreen_size = [
            view[0].ceil().max(1.0) as u32,
            view[1].ceil().max(1.0) as u32,
        ];

        pipeline.composite.ensure_offscreen(device, offscreen_size);

        let cell_size = [self.cell_size[0] * scale, self.cell_size[1] * scale];
        let signatures: Vec<PaneSignature> =
            self.panes.iter().map(|p| p.signature(scale)).collect();
        let unchanged = signatures == pipeline.last_panes
            && view == pipeline.last_viewport
            && cell_size == pipeline.last_cell_size
            && (font_size - pipeline.last_font_size).abs() < 0.01
            && self.cursor_shape == pipeline.last_cursor_shape
            && self.background_opacity == pipeline.last_background_opacity;

        if unchanged {
            return;
        }

        pipeline.last_panes = signatures;
        pipeline.last_viewport = view;
        pipeline.last_cell_size = cell_size;
        pipeline.last_font_size = font_size;
        pipeline.last_cursor_shape = self.cursor_shape;
        pipeline.last_background_opacity = self.background_opacity;

        pipeline
            .text
            .apply_terminal_font_selection(device, self.terminal_font_selection.as_deref());
        pipeline.text.set_requested_font_size(font_size);
        pipeline.text.update_uniforms(queue, view, [0.0, 0.0]);
        pipeline
            .bg
            .update_uniforms(queue, cell_size, view, [0.0, 0.0]);

        pipeline.bg.begin();
        pipeline.text.begin();

        for pane in &self.panes {
            let origin = [pane.origin[0] * scale, pane.origin[1] * scale];
            let cells = pane.cells.as_slice();
            pipeline.bg.push_pane(
                cells,
                pane.selection.as_ref(),
                pane.display_offset,
                pane.cursor,
                self.cursor_shape,
                pane.cursor_color,
                self.background_opacity,
                pane.link_row,
                origin,
            );
            pipeline.text.push_pane(
                device,
                queue,
                cells,
                cell_size,
                pane.selection.as_ref(),
                pane.display_offset,
                pane.cursor
                    .filter(|_| self.cursor_shape == crate::config::CursorShape::Block),
                pane.cursor_color,
                origin,
            );
        }

        for pane in &self.panes {
            let Some([top, height]) = pane.scrollbar else {
                continue;
            };
            let [x, y, w, h] = pane.rect.map(|v| v * scale);
            let bar_w = SCROLLBAR_WIDTH * scale;
            let bar_x = x + w - bar_w;
            pipeline.bg.push_px_rect(
                [bar_x, y],
                [bar_w, h],
                cell_size,
                [
                    self.divider_color[0],
                    self.divider_color[1],
                    self.divider_color[2],
                    self.divider_color[3] * 0.6,
                ],
            );
            let thumb_h = (h * height).max(16.0 * scale).min(h);
            let thumb_y = y + (h - thumb_h) * (top / (1.0 - height).max(0.0001)).clamp(0.0, 1.0);
            pipeline.bg.push_px_rect(
                [bar_x, thumb_y],
                [bar_w, thumb_h],
                cell_size,
                self.scrollbar_color,
            );
        }

        if self.panes.len() > 1 {
            for pane in &self.panes {
                let [x, y, w, h] = pane.rect.map(|v| v * scale);
                let color = if pane.focused {
                    self.focus_color
                } else {
                    self.divider_color
                };
                let t = scale;
                for (origin, size) in [
                    ([x, y], [w, t]),
                    ([x, y + h - t], [w, t]),
                    ([x, y], [t, h]),
                    ([x + w - t, y], [t, h]),
                ] {
                    pipeline.bg.push_px_rect(origin, size, cell_size, color);
                }
            }
        }

        pipeline.bg.upload(device, queue);
        pipeline.text.upload(device, queue);
    }

    fn render(
        &self,
        pipeline: &Self::Pipeline,
        encoder: &mut wgpu::CommandEncoder,
        target: &wgpu::TextureView,
        clip_bounds: &Rectangle<u32>,
    ) {
        let bg_pipeline = &pipeline.bg;
        let text_pipeline = &pipeline.text;
        let composite = &pipeline.composite;
        let offscreen_view = composite.offscreen_view();
        let offscreen_size = composite.offscreen_size();
        let clear_color = wgpu::Color {
            r: self.clear_color[0] as f64,
            g: self.clear_color[1] as f64,
            b: self.clear_color[2] as f64,
            a: self.clear_color[3] as f64,
        };

        {
            let mut offscreen_pass = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
                label: Some("terminal.offscreen_pass"),
                color_attachments: &[Some(wgpu::RenderPassColorAttachment {
                    view: offscreen_view,
                    depth_slice: None,
                    resolve_target: None,
                    ops: wgpu::Operations {
                        load: wgpu::LoadOp::Clear(clear_color),
                        store: wgpu::StoreOp::Store,
                    },
                })],
                depth_stencil_attachment: None,
                occlusion_query_set: None,
                timestamp_writes: None,
            });

            offscreen_pass.set_viewport(
                0.0,
                0.0,
                offscreen_size[0] as f32,
                offscreen_size[1] as f32,
                0.0,
                1.0,
            );
            offscreen_pass.set_scissor_rect(0, 0, offscreen_size[0], offscreen_size[1]);

            offscreen_pass.set_pipeline(bg_pipeline.pipeline());
            offscreen_pass.set_bind_group(0, bg_pipeline.uniform_bind_group(), &[]);
            offscreen_pass.set_vertex_buffer(0, bg_pipeline.quad_buffer().slice(..));
            offscreen_pass.set_vertex_buffer(1, bg_pipeline.instance_buffer().slice(..));

            let instance_count = bg_pipeline.instance_count().max(1) as u32;
            offscreen_pass.draw(0..6, 0..instance_count);

            if text_pipeline.instance_len() > 0 {
                offscreen_pass.set_pipeline(text_pipeline.pipeline());
                offscreen_pass.set_bind_group(0, text_pipeline.empty_bind_group(), &[]);
                offscreen_pass.set_bind_group(1, text_pipeline.uniform_bind_group(), &[]);
                offscreen_pass.set_vertex_buffer(0, bg_pipeline.quad_buffer().slice(..));
                offscreen_pass.set_vertex_buffer(1, text_pipeline.instance_buffer().slice(..));
                offscreen_pass.draw(0..6, 0..text_pipeline.instance_len() as u32);
            }
        }

        let mut composite_pass = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
            label: Some("terminal.composite_pass"),
            color_attachments: &[Some(wgpu::RenderPassColorAttachment {
                view: target,
                depth_slice: None,
                resolve_target: None,
                ops: wgpu::Operations {
                    load: wgpu::LoadOp::Load,
                    store: wgpu::StoreOp::Store,
                },
            })],
            depth_stencil_attachment: None,
            occlusion_query_set: None,
            timestamp_writes: None,
        });

        composite_pass.set_viewport(
            clip_bounds.x as f32,
            clip_bounds.y as f32,
            clip_bounds.width as f32,
            clip_bounds.height as f32,
            0.0,
            1.0,
        );
        composite_pass.set_scissor_rect(
            clip_bounds.x,
            clip_bounds.y,
            clip_bounds.width,
            clip_bounds.height,
        );
        composite_pass.set_pipeline(composite.pipeline());
        composite_pass.set_bind_group(0, composite.bind_group(), &[]);
        composite_pass.set_vertex_buffer(0, composite.quad_buffer().slice(..));
        composite_pass.draw(0..6, 0..1);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn dragging_the_scrollbar_to_the_top_scrolls_back_through_history() {
        // `Pane::scroll_to_relative` reads 0.0 as the oldest line and 1.0 as
        // the newest, so the top of the track must map to 0.0.
        let top = scrollbar_rel(0.0, 0.0, 600.0, 60.0);
        let bottom = scrollbar_rel(600.0, 0.0, 600.0, 60.0);

        assert_eq!(top, 0.0);
        assert_eq!(bottom, 1.0);
        assert!(top < bottom, "scrollbar axis is inverted");
    }

    #[test]
    fn the_scrollbar_midpoint_lands_mid_history() {
        let mid = scrollbar_rel(300.0, 0.0, 600.0, 60.0);
        assert!((mid - 0.5).abs() < 0.01, "midpoint mapped to {mid}");
    }

    #[test]
    fn scrollbar_rel_is_offset_by_the_pane_position() {
        let inside_second_pane = scrollbar_rel(400.0, 400.0, 600.0, 60.0);
        assert_eq!(inside_second_pane, 0.0);
    }
}

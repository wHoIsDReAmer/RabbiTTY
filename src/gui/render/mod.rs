use crate::terminal::{CellVisual, GridPos, Selection, TerminalSize};
use iced::mouse;
use iced::wgpu;
use iced::widget::shader::Program as ShaderProgram;
use iced::widget::shader::{Action, Pipeline, Primitive, Shader, Viewport};
use iced::{Event, Length, Point, Rectangle};
use std::sync::Arc;

mod bg;
mod composite;
mod text;
use bg::BackgroundPipeline;
use composite::CompositePipeline;
use text::TextPipelineData;

const SELECTION_BG: [f32; 4] = [0.25, 0.38, 0.60, 1.0];

/// Iced shader wrapper for terminal rendering.
#[derive(Debug, Clone)]
pub struct TerminalProgram {
    pub cells: Arc<Vec<CellVisual>>,
    pub grid_size: TerminalSize,
    pub terminal_font_selection: Option<String>,
    pub terminal_font_size: f32,
    pub padding: [f32; 2],
    pub clear_color: [f32; 4],
    pub selection: Option<Selection>,
    pub mouse_mode: bool,
    pub display_offset: usize,
}

impl TerminalProgram {
    pub fn widget(self) -> Shader<crate::gui::app::Message, Self> {
        Shader::new(self).width(Length::Fill).height(Length::Fill)
    }

    fn pixel_to_grid(&self, pos: Point, bounds: Rectangle) -> GridPos {
        let inner_w = (bounds.width - self.padding[0] * 2.0).max(1.0);
        let inner_h = (bounds.height - self.padding[1] * 2.0).max(1.0);
        let cell_w = inner_w / self.grid_size.columns.max(1) as f32;
        let cell_h = inner_h / self.grid_size.lines.max(1) as f32;
        let x = (pos.x - self.padding[0]).max(0.0);
        let y = (pos.y - self.padding[1]).max(0.0);
        let col = (x / cell_w) as usize;
        let row = (y / cell_h) as usize;
        GridPos {
            row: row.min(self.grid_size.lines.saturating_sub(1)),
            col: col.min(self.grid_size.columns.saturating_sub(1)),
        }
    }
}

#[derive(Default)]
pub struct TerminalShaderState {
    dragging: bool,
    drag_start: Option<GridPos>,
    drag_anchor_offset: usize,
}

type Message = crate::gui::app::Message;

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
        match event {
            Event::Mouse(mouse::Event::ButtonPressed(mouse::Button::Left)) => {
                if let Some(pos) = cursor.position_in(bounds) {
                    let grid_pos = self.pixel_to_grid(pos, bounds);
                    if self.mouse_mode {
                        state.dragging = true;
                        return Some(
                            Action::publish(Message::TerminalMousePress {
                                col: grid_pos.col,
                                row: grid_pos.row,
                            })
                            .and_capture(),
                        );
                    }
                    state.dragging = true;
                    state.drag_start = Some(grid_pos);
                    state.drag_anchor_offset = self.display_offset;
                    return Some(Action::publish(Message::SelectionChanged(None)).and_capture());
                }
            }
            Event::Mouse(mouse::Event::CursorMoved { .. }) => {
                if let Some(pos) = cursor.position_in(bounds) {
                    if self.mouse_mode && state.dragging {
                        let grid_pos = self.pixel_to_grid(pos, bounds);
                        return Some(
                            Action::publish(Message::TerminalMouseDrag {
                                col: grid_pos.col,
                                row: grid_pos.row,
                            })
                            .and_capture(),
                        );
                    }
                    if state.dragging
                        && let Some(start) = state.drag_start
                    {
                        let viewport_end = self.pixel_to_grid(pos, bounds);
                        // Translate the current viewport row back into the anchor frame
                        // so the selection follows content when the user scrolls.
                        let delta = self.display_offset as i64 - state.drag_anchor_offset as i64;
                        let anchored_row = viewport_end.row as i64 - delta;
                        let start_row = start.row as i64;
                        if start_row != anchored_row || start.col != viewport_end.col {
                            let sel = Selection {
                                start_row,
                                start_col: start.col,
                                end_row: anchored_row,
                                end_col: viewport_end.col,
                                anchor_offset: state.drag_anchor_offset,
                            };
                            return Some(
                                Action::publish(Message::SelectionChanged(Some(sel))).and_capture(),
                            );
                        }
                    }
                }
            }
            Event::Mouse(mouse::Event::ButtonReleased(mouse::Button::Left)) => {
                if self.mouse_mode
                    && let Some(pos) = cursor.position_in(bounds)
                {
                    let grid_pos = self.pixel_to_grid(pos, bounds);
                    state.dragging = false;
                    return Some(
                        Action::publish(Message::TerminalMouseRelease {
                            col: grid_pos.col,
                            row: grid_pos.row,
                        })
                        .and_capture(),
                    );
                }
                if state.dragging {
                    state.dragging = false;
                    return Some(Action::capture());
                }
            }
            _ => {}
        }
        None
    }

    fn draw(
        &self,
        _state: &Self::State,
        _cursor: mouse::Cursor,
        bounds: Rectangle,
    ) -> Self::Primitive {
        let pad_x = self.padding[0];
        let pad_y = self.padding[1];
        let columns = self.grid_size.columns.max(1) as f32;
        let lines = self.grid_size.lines.max(1) as f32;
        let inner_w = (bounds.width - pad_x * 2.0).max(1.0);
        let inner_h = (bounds.height - pad_y * 2.0).max(1.0);
        let cell_size = [inner_w / columns, inner_h / lines];
        TerminalPrimitive {
            cells: Arc::clone(&self.cells),
            cell_size,
            viewport: [bounds.width, bounds.height],
            offset: [pad_x, pad_y],
            clear_color: self.clear_color,
            terminal_font_selection: self.terminal_font_selection.clone(),
            terminal_font_size: self.terminal_font_size,
            selection: self.selection,
            display_offset: self.display_offset,
        }
    }

    fn mouse_interaction(
        &self,
        _state: &Self::State,
        bounds: Rectangle,
        cursor: mouse::Cursor,
    ) -> mouse::Interaction {
        if cursor.is_over(bounds) {
            mouse::Interaction::Text
        } else {
            mouse::Interaction::default()
        }
    }
}

#[derive(Debug)]
pub struct TerminalPipeline {
    bg: BackgroundPipeline,
    text: TextPipelineData,
    composite: CompositePipeline,
    last_cells_ptr: usize,
    last_cells_len: usize,
    last_cell_size: [f32; 2],
    last_viewport: [f32; 2],
    last_offset: [f32; 2],
    last_font_size: f32,
    last_selection: Option<Selection>,
    last_display_offset: usize,
}

impl Pipeline for TerminalPipeline {
    fn new(device: &wgpu::Device, _queue: &wgpu::Queue, format: wgpu::TextureFormat) -> Self {
        Self {
            bg: BackgroundPipeline::new(device, format),
            text: TextPipelineData::new(device, format),
            composite: CompositePipeline::new(device, format),
            last_cells_ptr: 0,
            last_cells_len: 0,
            last_cell_size: [0.0; 2],
            last_viewport: [0.0; 2],
            last_offset: [0.0; 2],
            last_font_size: 0.0,
            last_selection: None,
            last_display_offset: 0,
        }
    }
}

#[derive(Debug)]
pub struct TerminalPrimitive {
    cells: Arc<Vec<CellVisual>>,
    cell_size: [f32; 2],
    viewport: [f32; 2],
    offset: [f32; 2],
    clear_color: [f32; 4],
    terminal_font_selection: Option<String>,
    terminal_font_size: f32,
    selection: Option<Selection>,
    display_offset: usize,
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
        let cell_size = [self.cell_size[0] * scale, self.cell_size[1] * scale];
        let viewport = [self.viewport[0] * scale, self.viewport[1] * scale];
        let offset = [self.offset[0] * scale, self.offset[1] * scale];
        let font_size = self.terminal_font_size * scale;
        let offscreen_size = [
            viewport[0].ceil().max(1.0) as u32,
            viewport[1].ceil().max(1.0) as u32,
        ];

        pipeline.composite.ensure_offscreen(device, offscreen_size);

        let cells_ptr = Arc::as_ptr(&self.cells) as usize;
        let cells_len = self.cells.len();
        let unchanged = cells_ptr == pipeline.last_cells_ptr
            && cells_len == pipeline.last_cells_len
            && cell_size == pipeline.last_cell_size
            && viewport == pipeline.last_viewport
            && offset == pipeline.last_offset
            && (font_size - pipeline.last_font_size).abs() < 0.01
            && self.selection == pipeline.last_selection
            && self.display_offset == pipeline.last_display_offset;

        if unchanged {
            return;
        }

        pipeline.last_cells_ptr = cells_ptr;
        pipeline.last_cells_len = cells_len;
        pipeline.last_cell_size = cell_size;
        pipeline.last_viewport = viewport;
        pipeline.last_offset = offset;
        pipeline.last_font_size = font_size;
        pipeline.last_selection = self.selection;
        pipeline.last_display_offset = self.display_offset;

        let cells = self.cells.as_slice();

        pipeline
            .bg
            .update_uniforms(queue, cell_size, viewport, offset);
        pipeline.bg.prepare_instances(
            device,
            queue,
            cells,
            self.selection.as_ref(),
            self.display_offset,
        );

        pipeline
            .text
            .apply_terminal_font_selection(device, self.terminal_font_selection.as_deref());
        pipeline.text.set_requested_font_size(font_size);
        pipeline.text.update_uniforms(queue, viewport, offset);
        pipeline.text.prepare_instances(
            device,
            queue,
            cells,
            cell_size,
            self.selection.as_ref(),
            self.display_offset,
        );
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

            let instance_count = self.cells.len().max(1) as u32;
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

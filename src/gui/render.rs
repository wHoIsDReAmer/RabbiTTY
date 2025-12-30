use crate::terminal::{CellVisual, TerminalSize};
use iced::mouse;
use iced::wgpu;
use iced::widget::shader::Program as ShaderProgram;
use iced::widget::shader::{Pipeline, Primitive, Shader, Viewport};
use iced::{Length, Rectangle};

mod bg;
mod text;
use bg::BackgroundPipeline;
use text::TextPipelineData;

/// Iced shader wrapper for terminal rendering.
#[derive(Debug, Clone)]
pub struct TerminalProgram {
    pub cells: Vec<CellVisual>,
    pub grid_size: TerminalSize,
}

impl TerminalProgram {
    pub fn widget(self) -> Shader<crate::gui::app::Message, Self> {
        Shader::new(self).width(Length::Fill).height(Length::Fill)
    }
}

impl ShaderProgram<crate::gui::app::Message> for TerminalProgram {
    type State = ();
    type Primitive = TerminalPrimitive;

    fn draw(
        &self,
        _state: &Self::State,
        _cursor: mouse::Cursor,
        bounds: Rectangle,
    ) -> Self::Primitive {
        let columns = self.grid_size.columns.max(1) as f32;
        let lines = self.grid_size.lines.max(1) as f32;
        let cell_size = [bounds.width / columns, bounds.height / lines];

        TerminalPrimitive {
            cells: self.cells.clone(),
            cell_size,
            viewport: [bounds.width, bounds.height],
            offset: [0.0, 0.0],
            // offset: [bounds.x, bounds.y],
        }
    }
}

#[derive(Debug)]
pub struct TerminalPipeline {
    bg: BackgroundPipeline,
    text: TextPipelineData,
}

impl Pipeline for TerminalPipeline {
    fn new(device: &wgpu::Device, _queue: &wgpu::Queue, format: wgpu::TextureFormat) -> Self {
        Self {
            bg: BackgroundPipeline::new(device, format),
            text: TextPipelineData::new(device, format),
        }
    }
}

#[derive(Debug)]
pub struct TerminalPrimitive {
    cells: Vec<CellVisual>,
    cell_size: [f32; 2],
    viewport: [f32; 2],
    offset: [f32; 2],
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

        {
            pipeline
                .bg
                .update_uniforms(queue, cell_size, viewport, offset);
            pipeline.bg.prepare_instances(device, queue, &self.cells);
        }

        {
            pipeline.text.update_uniforms(queue, viewport, offset);
            pipeline
                .text
                .prepare_instances(device, queue, &self.cells, cell_size);
        }
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

        let mut pass = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
            label: Some("terminal.render_pass"),
            color_attachments: &[Some(wgpu::RenderPassColorAttachment {
                view: target,
                depth_slice: None,
                resolve_target: None,
                ops: wgpu::Operations {
                    // Load existing attachment to avoid wiping other UI layers
                    load: wgpu::LoadOp::Load,
                    store: wgpu::StoreOp::Store,
                },
            })],
            depth_stencil_attachment: None,
            occlusion_query_set: None,
            timestamp_writes: None,
        });

        pass.set_viewport(
            clip_bounds.x as f32,
            clip_bounds.y as f32,
            clip_bounds.width as f32,
            clip_bounds.height as f32,
            0.0,
            1.0,
        );
        pass.set_scissor_rect(
            clip_bounds.x,
            clip_bounds.y,
            clip_bounds.width,
            clip_bounds.height,
        );

        pass.set_pipeline(bg_pipeline.pipeline());
        pass.set_bind_group(0, bg_pipeline.uniform_bind_group(), &[]);
        pass.set_vertex_buffer(0, bg_pipeline.quad_buffer().slice(..));
        pass.set_vertex_buffer(1, bg_pipeline.instance_buffer().slice(..));

        let instance_count = self.cells.len().max(1) as u32;
        pass.draw(0..6, 0..instance_count);

        if text_pipeline.instance_len() > 0 {
            pass.set_pipeline(text_pipeline.pipeline());
            pass.set_bind_group(0, text_pipeline.empty_bind_group(), &[]);
            pass.set_bind_group(1, text_pipeline.uniform_bind_group(), &[]);
            pass.set_vertex_buffer(0, bg_pipeline.quad_buffer().slice(..));
            pass.set_vertex_buffer(1, text_pipeline.instance_buffer().slice(..));
            pass.draw(0..6, 0..text_pipeline.instance_len() as u32);
        }
    }
}

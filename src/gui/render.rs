use crate::terminal::{CellVisual, TerminalSize};
use iced::mouse;
use iced::widget::shader::Program as ShaderProgram;
use iced::widget::shader::Viewport;
use iced::widget::shader::wgpu;
use iced::widget::shader::{Primitive, Shader, Storage};
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
            offset: [bounds.x, bounds.y],
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
    fn prepare(
        &self,
        device: &wgpu::Device,
        queue: &wgpu::Queue,
        format: wgpu::TextureFormat,
        storage: &mut Storage,
        _bounds: &Rectangle,
        viewport: &Viewport,
    ) {
        let scale = (viewport.scale_factor() as f32).max(1.0);
        let cell_size = [self.cell_size[0] * scale, self.cell_size[1] * scale];
        let viewport = [self.viewport[0] * scale, self.viewport[1] * scale];
        let offset = [self.offset[0] * scale, self.offset[1] * scale];

        // Recreate pipelines if needed
        let needs_bg = storage
            .get::<BackgroundPipeline>()
            .map(|p| p.format() != format)
            .unwrap_or(true);

        if needs_bg {
            let pipeline = BackgroundPipeline::new(device, format);
            storage.store(pipeline);
        }

        let needs_text = storage
            .get::<TextPipelineData>()
            .map(|p| p.format() != format)
            .unwrap_or(true);

        if needs_text {
            let text_pipeline = TextPipelineData::new(device, format);
            storage.store(text_pipeline);
        }

        {
            let pipeline = storage
                .get_mut::<BackgroundPipeline>()
                .expect("pipeline just stored or existed");

            pipeline.update_uniforms(queue, cell_size, viewport, offset);
            pipeline.prepare_instances(device, queue, &self.cells);
        }

        {
            let text_pipeline = storage
                .get_mut::<TextPipelineData>()
                .expect("text pipeline just stored or existed");

            text_pipeline.update_uniforms(queue, viewport, offset);
            text_pipeline.prepare_instances(device, queue, &self.cells, cell_size);
        }
    }

    fn render(
        &self,
        encoder: &mut wgpu::CommandEncoder,
        storage: &Storage,
        target: &wgpu::TextureView,
        clip_bounds: &Rectangle<u32>,
    ) {
        let pipeline = storage
            .get::<BackgroundPipeline>()
            .expect("pipeline prepared before render");
        let text_pipeline = storage
            .get::<TextPipelineData>()
            .expect("text pipeline prepared before render");

        let mut pass = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
            label: Some("terminal.render_pass"),
            color_attachments: &[Some(wgpu::RenderPassColorAttachment {
                view: target,
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

        pass.set_pipeline(pipeline.pipeline());
        pass.set_bind_group(0, pipeline.uniform_bind_group(), &[]);
        pass.set_vertex_buffer(0, pipeline.quad_buffer().slice(..));
        pass.set_vertex_buffer(1, pipeline.instance_buffer().slice(..));

        let instance_count = self.cells.len().max(1) as u32;
        pass.draw(0..6, 0..instance_count);

        if text_pipeline.instance_len() > 0 {
            pass.set_pipeline(text_pipeline.pipeline());
            pass.set_bind_group(0, text_pipeline.empty_bind_group(), &[]);
            pass.set_bind_group(1, text_pipeline.uniform_bind_group(), &[]);
            pass.set_vertex_buffer(0, pipeline.quad_buffer().slice(..));
            pass.set_vertex_buffer(1, text_pipeline.instance_buffer().slice(..));
            pass.draw(0..6, 0..text_pipeline.instance_len() as u32);
        }
    }
}

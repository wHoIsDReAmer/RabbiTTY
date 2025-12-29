use crate::terminal::{CellVisual, TerminalSize};
use bytemuck::{Pod, Zeroable};
use iced::mouse;
use iced::widget::shader::Program as ShaderProgram;
use iced::widget::shader::Viewport;
use iced::widget::shader::wgpu::{self, util::DeviceExt};
use iced::widget::shader::{Primitive, Shader, Storage};
use iced::{Length, Rectangle};

mod text;
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

#[derive(Debug, Copy, Clone, Pod, Zeroable)]
#[repr(C)]
struct Uniforms {
    cell_size: [f32; 2],
    viewport: [f32; 2],
    offset: [f32; 2],
}

#[derive(Debug, Copy, Clone, Pod, Zeroable)]
#[repr(C)]
struct InstanceRaw {
    pos: [u32; 2],
    color: [f32; 4],
}

#[derive(Debug)]
struct PipelineData {
    pipeline: wgpu::RenderPipeline,
    bind_group_layout: wgpu::BindGroupLayout,
    uniform_buffer: wgpu::Buffer,
    uniform_bind_group: wgpu::BindGroup,
    quad_buffer: wgpu::Buffer,
    instance_buffer: wgpu::Buffer,
    instance_capacity: usize,
    format: wgpu::TextureFormat,
}

impl Primitive for TerminalPrimitive {
    fn prepare(
        &self,
        device: &wgpu::Device,
        queue: &wgpu::Queue,
        format: wgpu::TextureFormat,
        storage: &mut Storage,
        _bounds: &Rectangle,
        _viewport: &Viewport,
    ) {
        // Recreate pipelines if needed
        let needs_bg = storage
            .get::<PipelineData>()
            .map(|p| p.format != format)
            .unwrap_or(true);

        if needs_bg {
            let pipeline = build_pipeline(device, format);
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
                .get_mut::<PipelineData>()
                .expect("pipeline just stored or existed");

            // Update uniforms
            let uniforms = Uniforms {
                cell_size: self.cell_size,
                viewport: self.viewport,
                offset: self.offset,
            };
            queue.write_buffer(&pipeline.uniform_buffer, 0, bytemuck::bytes_of(&uniforms));

            // Prepare instances
            let instances: Vec<InstanceRaw> = self
                .cells
                .iter()
                .map(|cell| InstanceRaw {
                    pos: [cell.col as u32, cell.row as u32],
                    color: cell.bg,
                })
                .collect();

            let required = instances.len().max(1);

            if required > pipeline.instance_capacity {
                let new_cap = (required.next_power_of_two()).max(64);
                pipeline.instance_buffer = device.create_buffer(&wgpu::BufferDescriptor {
                    label: Some("terminal.instances"),
                    size: (new_cap * std::mem::size_of::<InstanceRaw>()) as wgpu::BufferAddress,
                    usage: wgpu::BufferUsages::VERTEX | wgpu::BufferUsages::COPY_DST,
                    mapped_at_creation: false,
                });
                pipeline.instance_capacity = new_cap;
            }

            if !instances.is_empty() {
                queue.write_buffer(
                    &pipeline.instance_buffer,
                    0,
                    bytemuck::cast_slice(&instances),
                );
            } else {
                // ensure at least one dummy to avoid zero-draw
                queue.write_buffer(
                    &pipeline.instance_buffer,
                    0,
                    bytemuck::cast_slice(&[InstanceRaw {
                        pos: [0, 0],
                        color: [0.1, 0.1, 0.1, 1.0],
                    }]),
                );
            }
        }

        {
            let text_pipeline = storage
                .get_mut::<TextPipelineData>()
                .expect("text pipeline just stored or existed");

            text_pipeline.update_uniforms(queue, self.viewport, self.offset);
            text_pipeline.prepare_instances(device, queue, &self.cells, self.cell_size);
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
            .get::<PipelineData>()
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

        pass.set_pipeline(&pipeline.pipeline);
        pass.set_bind_group(0, &pipeline.uniform_bind_group, &[]);
        pass.set_vertex_buffer(0, pipeline.quad_buffer.slice(..));
        pass.set_vertex_buffer(1, pipeline.instance_buffer.slice(..));

        let instance_count = self.cells.len().max(1) as u32;
        pass.draw(0..6, 0..instance_count);

        if text_pipeline.instance_len() > 0 {
            pass.set_pipeline(text_pipeline.pipeline());
            pass.set_bind_group(0, text_pipeline.empty_bind_group(), &[]);
            pass.set_bind_group(1, text_pipeline.uniform_bind_group(), &[]);
            pass.set_vertex_buffer(0, pipeline.quad_buffer.slice(..));
            pass.set_vertex_buffer(1, text_pipeline.instance_buffer().slice(..));
            pass.draw(0..6, 0..text_pipeline.instance_len() as u32);
        }
    }
}

fn build_pipeline(device: &wgpu::Device, format: wgpu::TextureFormat) -> PipelineData {
    let shader_src = include_str!("terminal.wgsl");
    let module = device.create_shader_module(wgpu::ShaderModuleDescriptor {
        label: Some("terminal.wgsl"),
        source: wgpu::ShaderSource::Wgsl(shader_src.into()),
    });

    let bind_group_layout = device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
        label: Some("terminal.uniform.layout"),
        entries: &[wgpu::BindGroupLayoutEntry {
            binding: 0,
            visibility: wgpu::ShaderStages::VERTEX,
            ty: wgpu::BindingType::Buffer {
                ty: wgpu::BufferBindingType::Uniform,
                has_dynamic_offset: false,
                min_binding_size: wgpu::BufferSize::new(std::mem::size_of::<Uniforms>() as u64),
            },
            count: None,
        }],
    });

    let uniform_buffer = device.create_buffer(&wgpu::BufferDescriptor {
        label: Some("terminal.uniform.buffer"),
        size: std::mem::size_of::<Uniforms>() as u64,
        usage: wgpu::BufferUsages::UNIFORM | wgpu::BufferUsages::COPY_DST,
        mapped_at_creation: false,
    });

    let uniform_bind_group = device.create_bind_group(&wgpu::BindGroupDescriptor {
        label: Some("terminal.uniform.bind_group"),
        layout: &bind_group_layout,
        entries: &[wgpu::BindGroupEntry {
            binding: 0,
            resource: uniform_buffer.as_entire_binding(),
        }],
    });

    let quad: [[f32; 2]; 6] = [
        [0.0, 0.0],
        [1.0, 0.0],
        [1.0, 1.0],
        [0.0, 0.0],
        [1.0, 1.0],
        [0.0, 1.0],
    ];
    let quad_buffer = device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
        label: Some("terminal.quad"),
        contents: bytemuck::cast_slice(&quad),
        usage: wgpu::BufferUsages::VERTEX,
    });

    let instance_buffer = device.create_buffer(&wgpu::BufferDescriptor {
        label: Some("terminal.instances"),
        size: (64 * std::mem::size_of::<InstanceRaw>()) as u64,
        usage: wgpu::BufferUsages::VERTEX | wgpu::BufferUsages::COPY_DST,
        mapped_at_creation: false,
    });

    let pipeline_layout = device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
        label: Some("terminal.pipeline.layout"),
        bind_group_layouts: &[&bind_group_layout],
        push_constant_ranges: &[],
    });

    let pipeline = device.create_render_pipeline(&wgpu::RenderPipelineDescriptor {
        label: Some("terminal.pipeline"),
        layout: Some(&pipeline_layout),
        vertex: wgpu::VertexState {
            module: &module,
            entry_point: "vs_main",
            buffers: &[
                wgpu::VertexBufferLayout {
                    array_stride: std::mem::size_of::<[f32; 2]>() as u64,
                    step_mode: wgpu::VertexStepMode::Vertex,
                    attributes: &wgpu::vertex_attr_array![0 => Float32x2],
                },
                wgpu::VertexBufferLayout {
                    array_stride: std::mem::size_of::<InstanceRaw>() as u64,
                    step_mode: wgpu::VertexStepMode::Instance,
                    attributes: &wgpu::vertex_attr_array![
                        1 => Uint32x2,
                        2 => Float32x4
                    ],
                },
            ],
        },
        fragment: Some(wgpu::FragmentState {
            module: &module,
            entry_point: "fs_main",
            targets: &[Some(wgpu::ColorTargetState {
                format,
                blend: Some(wgpu::BlendState::ALPHA_BLENDING),
                write_mask: wgpu::ColorWrites::ALL,
            })],
        }),
        primitive: wgpu::PrimitiveState {
            topology: wgpu::PrimitiveTopology::TriangleList,
            strip_index_format: None,
            front_face: wgpu::FrontFace::Ccw,
            cull_mode: None,
            unclipped_depth: false,
            polygon_mode: wgpu::PolygonMode::Fill,
            conservative: false,
        },
        depth_stencil: None,
        multisample: wgpu::MultisampleState::default(),
        multiview: None,
    });

    PipelineData {
        pipeline,
        bind_group_layout,
        uniform_buffer,
        uniform_bind_group,
        quad_buffer,
        instance_buffer,
        instance_capacity: 64,
        format,
    }
}

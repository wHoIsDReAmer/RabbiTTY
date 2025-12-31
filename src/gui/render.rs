use crate::terminal::{CellVisual, TerminalSize};
use bytemuck::{Pod, Zeroable};
use iced::mouse;
use iced::wgpu;
use iced::wgpu::util::DeviceExt;
use iced::widget::shader::Program as ShaderProgram;
use iced::widget::shader::{Pipeline, Primitive, Shader, Viewport};
use iced::{Length, Rectangle};
use std::sync::Arc;

mod bg;
mod text;
use bg::BackgroundPipeline;
use text::TextPipelineData;

/// Iced shader wrapper for terminal rendering.
#[derive(Debug, Clone)]
pub struct TerminalProgram {
    pub cells: Arc<Vec<CellVisual>>,
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
        let clear_color = self
            .cells
            .first()
            .map(|cell| cell.bg)
            .unwrap_or([0.0, 0.0, 0.0, 0.0]);

        TerminalPrimitive {
            cells: Arc::clone(&self.cells),
            cell_size,
            viewport: [bounds.width, bounds.height],
            offset: [0.0, 0.0],
            clear_color,
            // offset: [bounds.x, bounds.y],
        }
    }
}

#[derive(Debug)]
pub struct TerminalPipeline {
    bg: BackgroundPipeline,
    text: TextPipelineData,
    composite: CompositePipeline,
}

impl Pipeline for TerminalPipeline {
    fn new(device: &wgpu::Device, _queue: &wgpu::Queue, format: wgpu::TextureFormat) -> Self {
        Self {
            bg: BackgroundPipeline::new(device, format),
            text: TextPipelineData::new(device, format),
            composite: CompositePipeline::new(device, format),
        }
    }
}

#[derive(Debug, Copy, Clone, Pod, Zeroable)]
#[repr(C)]
struct CompositeVertex {
    pos: [f32; 2],
    uv: [f32; 2],
}

#[derive(Debug)]
struct OffscreenTarget {
    texture: wgpu::Texture,
    view: wgpu::TextureView,
    size: [u32; 2],
}

#[derive(Debug)]
struct CompositePipeline {
    pipeline: wgpu::RenderPipeline,
    bind_group_layout: wgpu::BindGroupLayout,
    sampler: wgpu::Sampler,
    quad_buffer: wgpu::Buffer,
    bind_group: Option<wgpu::BindGroup>,
    offscreen: Option<OffscreenTarget>,
    format: wgpu::TextureFormat,
}

impl CompositePipeline {
    fn new(device: &wgpu::Device, format: wgpu::TextureFormat) -> Self {
        let shader_src = include_str!("terminal.wgsl");
        let module = device.create_shader_module(wgpu::ShaderModuleDescriptor {
            label: Some("terminal.composite.wgsl"),
            source: wgpu::ShaderSource::Wgsl(shader_src.into()),
        });

        let bind_group_layout = device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
            label: Some("terminal.composite.bind_group_layout"),
            entries: &[
                wgpu::BindGroupLayoutEntry {
                    binding: 0,
                    visibility: wgpu::ShaderStages::FRAGMENT,
                    ty: wgpu::BindingType::Sampler(wgpu::SamplerBindingType::Filtering),
                    count: None,
                },
                wgpu::BindGroupLayoutEntry {
                    binding: 1,
                    visibility: wgpu::ShaderStages::FRAGMENT,
                    ty: wgpu::BindingType::Texture {
                        sample_type: wgpu::TextureSampleType::Float { filterable: true },
                        view_dimension: wgpu::TextureViewDimension::D2,
                        multisampled: false,
                    },
                    count: None,
                },
            ],
        });

        let sampler = device.create_sampler(&wgpu::SamplerDescriptor {
            label: Some("terminal.composite.sampler"),
            address_mode_u: wgpu::AddressMode::ClampToEdge,
            address_mode_v: wgpu::AddressMode::ClampToEdge,
            address_mode_w: wgpu::AddressMode::ClampToEdge,
            mag_filter: wgpu::FilterMode::Nearest,
            min_filter: wgpu::FilterMode::Nearest,
            mipmap_filter: wgpu::FilterMode::Nearest,
            ..Default::default()
        });

        let quad: [CompositeVertex; 6] = [
            CompositeVertex {
                pos: [-1.0, 1.0],
                uv: [0.0, 0.0],
            },
            CompositeVertex {
                pos: [1.0, 1.0],
                uv: [1.0, 0.0],
            },
            CompositeVertex {
                pos: [1.0, -1.0],
                uv: [1.0, 1.0],
            },
            CompositeVertex {
                pos: [-1.0, 1.0],
                uv: [0.0, 0.0],
            },
            CompositeVertex {
                pos: [1.0, -1.0],
                uv: [1.0, 1.0],
            },
            CompositeVertex {
                pos: [-1.0, -1.0],
                uv: [0.0, 1.0],
            },
        ];
        let quad_buffer = device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
            label: Some("terminal.composite.quad"),
            contents: bytemuck::cast_slice(&quad),
            usage: wgpu::BufferUsages::VERTEX,
        });

        let pipeline_layout = device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
            label: Some("terminal.composite.pipeline_layout"),
            bind_group_layouts: &[&bind_group_layout],
            push_constant_ranges: &[],
        });

        let pipeline = device.create_render_pipeline(&wgpu::RenderPipelineDescriptor {
            label: Some("terminal.composite.pipeline"),
            layout: Some(&pipeline_layout),
            vertex: wgpu::VertexState {
                module: &module,
                entry_point: Some("composite_vs_main"),
                compilation_options: wgpu::PipelineCompilationOptions::default(),
                buffers: &[wgpu::VertexBufferLayout {
                    array_stride: std::mem::size_of::<CompositeVertex>() as u64,
                    step_mode: wgpu::VertexStepMode::Vertex,
                    attributes: &wgpu::vertex_attr_array![0 => Float32x2, 1 => Float32x2],
                }],
            },
            fragment: Some(wgpu::FragmentState {
                module: &module,
                entry_point: Some("composite_fs_main"),
                compilation_options: wgpu::PipelineCompilationOptions::default(),
                targets: &[Some(wgpu::ColorTargetState {
                    format,
                    blend: None,
                    write_mask: wgpu::ColorWrites::ALL,
                })],
            }),
            primitive: wgpu::PrimitiveState::default(),
            depth_stencil: None,
            multisample: wgpu::MultisampleState::default(),
            multiview: None,
            cache: None,
        });

        Self {
            pipeline,
            bind_group_layout,
            sampler,
            quad_buffer,
            bind_group: None,
            offscreen: None,
            format,
        }
    }

    fn ensure_offscreen(&mut self, device: &wgpu::Device, size: [u32; 2]) {
        let size = [size[0].max(1), size[1].max(1)];
        let needs_resize = self
            .offscreen
            .as_ref()
            .map(|target| target.size != size)
            .unwrap_or(true);

        if !needs_resize {
            return;
        }

        let texture = device.create_texture(&wgpu::TextureDescriptor {
            label: Some("terminal.offscreen"),
            size: wgpu::Extent3d {
                width: size[0],
                height: size[1],
                depth_or_array_layers: 1,
            },
            mip_level_count: 1,
            sample_count: 1,
            dimension: wgpu::TextureDimension::D2,
            format: self.format,
            usage: wgpu::TextureUsages::RENDER_ATTACHMENT | wgpu::TextureUsages::TEXTURE_BINDING,
            view_formats: &[],
        });
        let view = texture.create_view(&wgpu::TextureViewDescriptor::default());
        let bind_group = device.create_bind_group(&wgpu::BindGroupDescriptor {
            label: Some("terminal.composite.bind_group"),
            layout: &self.bind_group_layout,
            entries: &[
                wgpu::BindGroupEntry {
                    binding: 0,
                    resource: wgpu::BindingResource::Sampler(&self.sampler),
                },
                wgpu::BindGroupEntry {
                    binding: 1,
                    resource: wgpu::BindingResource::TextureView(&view),
                },
            ],
        });

        self.offscreen = Some(OffscreenTarget {
            texture,
            view,
            size,
        });
        self.bind_group = Some(bind_group);
    }

    fn offscreen_view(&self) -> &wgpu::TextureView {
        &self
            .offscreen
            .as_ref()
            .expect("offscreen texture not initialized")
            .view
    }

    fn offscreen_size(&self) -> [u32; 2] {
        self.offscreen
            .as_ref()
            .expect("offscreen texture not initialized")
            .size
    }

    fn bind_group(&self) -> &wgpu::BindGroup {
        self.bind_group
            .as_ref()
            .expect("composite bind group not initialized")
    }

    fn pipeline(&self) -> &wgpu::RenderPipeline {
        &self.pipeline
    }

    fn quad_buffer(&self) -> &wgpu::Buffer {
        &self.quad_buffer
    }
}

#[derive(Debug)]
pub struct TerminalPrimitive {
    cells: Arc<Vec<CellVisual>>,
    cell_size: [f32; 2],
    viewport: [f32; 2],
    offset: [f32; 2],
    clear_color: [f32; 4],
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
        let offscreen_size = [
            viewport[0].ceil().max(1.0) as u32,
            viewport[1].ceil().max(1.0) as u32,
        ];

        pipeline.composite.ensure_offscreen(device, offscreen_size);

        {
            pipeline
                .bg
                .update_uniforms(queue, cell_size, viewport, offset);
            pipeline
                .bg
                .prepare_instances(device, queue, self.cells.as_slice());
        }

        {
            pipeline.text.update_uniforms(queue, viewport, offset);
            pipeline
                .text
                .prepare_instances(device, queue, self.cells.as_slice(), cell_size);
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
                    // Preserve other UI layers outside the terminal area
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

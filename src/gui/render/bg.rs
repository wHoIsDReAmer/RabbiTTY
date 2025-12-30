use crate::terminal::CellVisual;
use bytemuck::{Pod, Zeroable};
use iced::wgpu::{self, util::DeviceExt};

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
pub(super) struct BackgroundPipeline {
    pipeline: wgpu::RenderPipeline,
    uniform_buffer: wgpu::Buffer,
    uniform_bind_group: wgpu::BindGroup,
    quad_buffer: wgpu::Buffer,
    instance_buffer: wgpu::Buffer,
    instance_capacity: usize,
}

impl BackgroundPipeline {
    pub(super) fn new(device: &wgpu::Device, format: wgpu::TextureFormat) -> Self {
        let shader_src = include_str!("../terminal.wgsl");
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
                entry_point: Some("vs_main"),
                compilation_options: wgpu::PipelineCompilationOptions::default(),
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
                entry_point: Some("fs_main"),
                compilation_options: wgpu::PipelineCompilationOptions::default(),
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
            cache: None,
        });

        Self {
            pipeline,
            uniform_buffer,
            uniform_bind_group,
            quad_buffer,
            instance_buffer,
            instance_capacity: 64,
        }
    }

    pub(super) fn update_uniforms(
        &self,
        queue: &wgpu::Queue,
        cell_size: [f32; 2],
        viewport: [f32; 2],
        offset: [f32; 2],
    ) {
        let uniforms = Uniforms {
            cell_size,
            viewport,
            offset,
        };
        queue.write_buffer(&self.uniform_buffer, 0, bytemuck::bytes_of(&uniforms));
    }

    pub(super) fn prepare_instances(
        &mut self,
        device: &wgpu::Device,
        queue: &wgpu::Queue,
        cells: &[CellVisual],
    ) {
        let instances: Vec<InstanceRaw> = cells
            .iter()
            .map(|cell| InstanceRaw {
                pos: [cell.col as u32, cell.row as u32],
                color: cell.bg,
            })
            .collect();

        let required = instances.len().max(1);

        if required > self.instance_capacity {
            let new_cap = (required.next_power_of_two()).max(64);
            self.instance_buffer = device.create_buffer(&wgpu::BufferDescriptor {
                label: Some("terminal.instances"),
                size: (new_cap * std::mem::size_of::<InstanceRaw>()) as wgpu::BufferAddress,
                usage: wgpu::BufferUsages::VERTEX | wgpu::BufferUsages::COPY_DST,
                mapped_at_creation: false,
            });
            self.instance_capacity = new_cap;
        }

        if !instances.is_empty() {
            queue.write_buffer(&self.instance_buffer, 0, bytemuck::cast_slice(&instances));
        } else {
            queue.write_buffer(
                &self.instance_buffer,
                0,
                bytemuck::cast_slice(&[InstanceRaw {
                    pos: [0, 0],
                    color: [0.0, 0.0, 0.0, 0.0],
                }]),
            );
        }
    }

    pub(super) fn pipeline(&self) -> &wgpu::RenderPipeline {
        &self.pipeline
    }

    pub(super) fn uniform_bind_group(&self) -> &wgpu::BindGroup {
        &self.uniform_bind_group
    }

    pub(super) fn quad_buffer(&self) -> &wgpu::Buffer {
        &self.quad_buffer
    }

    pub(super) fn instance_buffer(&self) -> &wgpu::Buffer {
        &self.instance_buffer
    }
}

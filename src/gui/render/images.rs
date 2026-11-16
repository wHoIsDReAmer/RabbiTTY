use crate::terminal::ImageSlice;
use crate::terminal::graphics::Image;
use bytemuck::{Pod, Zeroable};
use iced::wgpu;
use std::collections::HashMap;
use std::ops::Range;

#[derive(Debug, Copy, Clone, Pod, Zeroable)]
#[repr(C)]
struct Uniforms {
    viewport: [f32; 2],
    unused: [f32; 2],
}

#[derive(Debug, Copy, Clone, Pod, Zeroable)]
#[repr(C)]
struct InstanceRaw {
    pos: [f32; 2],
    size: [f32; 2],
    uv_min: [f32; 2],
    uv_max: [f32; 2],
}

#[derive(Debug)]
struct Texture {
    bind_group: wgpu::BindGroup,
    used: bool,
}

#[derive(Debug)]
struct Batch {
    uid: u64,
    instances: Range<u32>,
}

#[derive(Debug)]
pub(super) struct ImagePipeline {
    pipeline: wgpu::RenderPipeline,
    layout: wgpu::BindGroupLayout,
    uniform_buffer: wgpu::Buffer,
    sampler: wgpu::Sampler,
    instance_buffer: wgpu::Buffer,
    instance_capacity: usize,
    queued: Vec<(u64, InstanceRaw)>,
    instances: Vec<InstanceRaw>,
    batches: Vec<Batch>,
    textures: HashMap<u64, Texture>,
    max_dimension: u32,
}

impl ImagePipeline {
    pub(super) fn new(device: &wgpu::Device, format: wgpu::TextureFormat) -> Self {
        let module = device.create_shader_module(wgpu::ShaderModuleDescriptor {
            label: Some("image.wgsl"),
            source: wgpu::ShaderSource::Wgsl(include_str!("shaders/image.wgsl").into()),
        });

        let layout = device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
            label: Some("terminal.image.layout"),
            entries: &[
                wgpu::BindGroupLayoutEntry {
                    binding: 0,
                    visibility: wgpu::ShaderStages::VERTEX,
                    ty: wgpu::BindingType::Buffer {
                        ty: wgpu::BufferBindingType::Uniform,
                        has_dynamic_offset: false,
                        min_binding_size: wgpu::BufferSize::new(
                            std::mem::size_of::<Uniforms>() as u64
                        ),
                    },
                    count: None,
                },
                wgpu::BindGroupLayoutEntry {
                    binding: 1,
                    visibility: wgpu::ShaderStages::FRAGMENT,
                    ty: wgpu::BindingType::Sampler(wgpu::SamplerBindingType::Filtering),
                    count: None,
                },
                wgpu::BindGroupLayoutEntry {
                    binding: 2,
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

        let uniform_buffer = device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("terminal.image.uniforms"),
            size: std::mem::size_of::<Uniforms>() as u64,
            usage: wgpu::BufferUsages::UNIFORM | wgpu::BufferUsages::COPY_DST,
            mapped_at_creation: false,
        });

        let sampler = device.create_sampler(&wgpu::SamplerDescriptor {
            label: Some("terminal.image.sampler"),
            mag_filter: wgpu::FilterMode::Linear,
            min_filter: wgpu::FilterMode::Linear,
            ..Default::default()
        });

        let instance_capacity = 16;
        let instance_buffer = Self::instance_buffer(device, instance_capacity);

        let pipeline_layout = device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
            label: Some("terminal.image.pipeline.layout"),
            bind_group_layouts: &[&layout],
            push_constant_ranges: &[],
        });

        let pipeline = device.create_render_pipeline(&wgpu::RenderPipelineDescriptor {
            label: Some("terminal.image.pipeline"),
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
                            1 => Float32x2,
                            2 => Float32x2,
                            3 => Float32x2,
                            4 => Float32x2
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
                    blend: None,
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
            layout,
            uniform_buffer,
            sampler,
            instance_buffer,
            instance_capacity,
            queued: Vec::new(),
            instances: Vec::new(),
            batches: Vec::new(),
            textures: HashMap::new(),
            max_dimension: device.limits().max_texture_dimension_2d,
        }
    }

    fn instance_buffer(device: &wgpu::Device, capacity: usize) -> wgpu::Buffer {
        device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("terminal.image.instances"),
            size: (capacity * std::mem::size_of::<InstanceRaw>()) as wgpu::BufferAddress,
            usage: wgpu::BufferUsages::VERTEX | wgpu::BufferUsages::COPY_DST,
            mapped_at_creation: false,
        })
    }

    pub(super) fn begin(&mut self) {
        self.queued.clear();
        for texture in self.textures.values_mut() {
            texture.used = false;
        }
    }

    pub(super) fn push_pane(
        &mut self,
        device: &wgpu::Device,
        queue: &wgpu::Queue,
        slices: &[ImageSlice],
        cell_size: [f32; 2],
        origin: [f32; 2],
    ) {
        for slice in slices {
            if !self.ensure_texture(device, queue, &slice.image) {
                continue;
            }
            self.queued.push((
                slice.image.uid,
                InstanceRaw {
                    pos: [
                        origin[0] + slice.col as f32 * cell_size[0],
                        origin[1] + slice.row as f32 * cell_size[1],
                    ],
                    size: [slice.size[0] * cell_size[0], slice.size[1] * cell_size[1]],
                    uv_min: [slice.uv[0], slice.uv[1]],
                    uv_max: [slice.uv[2], slice.uv[3]],
                },
            ));
        }
    }

    fn ensure_texture(
        &mut self,
        device: &wgpu::Device,
        queue: &wgpu::Queue,
        image: &Image,
    ) -> bool {
        if let Some(texture) = self.textures.get_mut(&image.uid) {
            texture.used = true;
            return true;
        }
        let fits = |length: u32| (1..=self.max_dimension).contains(&length);
        if !fits(image.width) || !fits(image.height) {
            return false;
        }
        let size = wgpu::Extent3d {
            width: image.width,
            height: image.height,
            depth_or_array_layers: 1,
        };
        let texture = device.create_texture(&wgpu::TextureDescriptor {
            label: Some("terminal.image"),
            size,
            mip_level_count: 1,
            sample_count: 1,
            dimension: wgpu::TextureDimension::D2,
            format: wgpu::TextureFormat::Rgba8UnormSrgb,
            usage: wgpu::TextureUsages::TEXTURE_BINDING | wgpu::TextureUsages::COPY_DST,
            view_formats: &[],
        });
        queue.write_texture(
            texture.as_image_copy(),
            &image.rgba,
            wgpu::TexelCopyBufferLayout {
                offset: 0,
                bytes_per_row: Some(4 * image.width),
                rows_per_image: Some(image.height),
            },
            size,
        );
        let view = texture.create_view(&wgpu::TextureViewDescriptor::default());
        let bind_group = device.create_bind_group(&wgpu::BindGroupDescriptor {
            label: Some("terminal.image.bind_group"),
            layout: &self.layout,
            entries: &[
                wgpu::BindGroupEntry {
                    binding: 0,
                    resource: self.uniform_buffer.as_entire_binding(),
                },
                wgpu::BindGroupEntry {
                    binding: 1,
                    resource: wgpu::BindingResource::Sampler(&self.sampler),
                },
                wgpu::BindGroupEntry {
                    binding: 2,
                    resource: wgpu::BindingResource::TextureView(&view),
                },
            ],
        });
        self.textures.insert(
            image.uid,
            Texture {
                bind_group,
                used: true,
            },
        );
        true
    }

    pub(super) fn upload(
        &mut self,
        device: &wgpu::Device,
        queue: &wgpu::Queue,
        viewport: [f32; 2],
    ) {
        self.textures.retain(|_, texture| texture.used);
        self.batches.clear();
        if self.queued.is_empty() {
            return;
        }
        self.queued.sort_by_key(|(uid, _)| *uid);
        for (index, (uid, _)) in self.queued.iter().enumerate() {
            let index = index as u32;
            match self.batches.last_mut().filter(|batch| batch.uid == *uid) {
                Some(batch) => batch.instances.end = index + 1,
                None => self.batches.push(Batch {
                    uid: *uid,
                    instances: index..index + 1,
                }),
            }
        }
        if self.queued.len() > self.instance_capacity {
            self.instance_capacity = self.queued.len().next_power_of_two();
            self.instance_buffer = Self::instance_buffer(device, self.instance_capacity);
        }
        self.instances.clear();
        self.instances
            .extend(self.queued.iter().map(|(_, raw)| *raw));
        queue.write_buffer(
            &self.instance_buffer,
            0,
            bytemuck::cast_slice(&self.instances),
        );
        queue.write_buffer(
            &self.uniform_buffer,
            0,
            bytemuck::bytes_of(&Uniforms {
                viewport,
                unused: [0.0; 2],
            }),
        );
    }

    pub(super) fn draw(&self, pass: &mut wgpu::RenderPass<'_>, quad: &wgpu::Buffer) {
        if self.batches.is_empty() {
            return;
        }
        pass.set_pipeline(&self.pipeline);
        pass.set_vertex_buffer(0, quad.slice(..));
        pass.set_vertex_buffer(1, self.instance_buffer.slice(..));
        for batch in &self.batches {
            if let Some(texture) = self.textures.get(&batch.uid) {
                pass.set_bind_group(0, &texture.bind_group, &[]);
                pass.draw(0..6, batch.instances.clone());
            }
        }
    }
}

use bytemuck::{Pod, Zeroable};
use iced::wgpu;
use iced::wgpu::util::DeviceExt;

#[derive(Debug, Copy, Clone, Pod, Zeroable)]
#[repr(C)]
struct CompositeVertex {
    pos: [f32; 2],
    uv: [f32; 2],
}

#[derive(Debug)]
pub(super) struct OffscreenTarget {
    _texture: wgpu::Texture,
    pub(super) view: wgpu::TextureView,
    pub(super) size: [u32; 2],
}

#[derive(Debug)]
pub(super) struct CompositePipeline {
    pipeline: wgpu::RenderPipeline,
    bind_group_layout: wgpu::BindGroupLayout,
    sampler: wgpu::Sampler,
    quad_buffer: wgpu::Buffer,
    bind_group: Option<wgpu::BindGroup>,
    offscreen: Option<OffscreenTarget>,
    format: wgpu::TextureFormat,
}

impl CompositePipeline {
    pub(super) fn new(device: &wgpu::Device, format: wgpu::TextureFormat) -> Self {
        let shader_src = include_str!("../terminal.wgsl");
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
                    blend: Some(wgpu::BlendState::ALPHA_BLENDING),
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

    pub(super) fn ensure_offscreen(&mut self, device: &wgpu::Device, size: [u32; 2]) {
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
            _texture: texture,
            view,
            size,
        });
        self.bind_group = Some(bind_group);
    }

    pub(super) fn offscreen_view(&self) -> &wgpu::TextureView {
        &self
            .offscreen
            .as_ref()
            .expect("offscreen texture not initialized")
            .view
    }

    pub(super) fn offscreen_size(&self) -> [u32; 2] {
        self.offscreen
            .as_ref()
            .expect("offscreen texture not initialized")
            .size
    }

    pub(super) fn bind_group(&self) -> &wgpu::BindGroup {
        self.bind_group
            .as_ref()
            .expect("composite bind group not initialized")
    }

    pub(super) fn pipeline(&self) -> &wgpu::RenderPipeline {
        &self.pipeline
    }

    pub(super) fn quad_buffer(&self) -> &wgpu::Buffer {
        &self.quad_buffer
    }
}

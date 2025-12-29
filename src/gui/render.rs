use crate::terminal::{CellVisual, TerminalSize};
use ab_glyph::{Font, FontArc, PxScale, ScaleFont, point};
use bytemuck::{Pod, Zeroable};
use iced::mouse;
use iced::widget::shader::Program as ShaderProgram;
use iced::widget::shader::Viewport;
use iced::widget::shader::wgpu::{self, util::DeviceExt};
use iced::widget::shader::{Primitive, Shader, Storage};
use iced::{Length, Rectangle};
use std::collections::HashMap;

const DEJAVU_SANS: &[u8] = include_bytes!("../../fonts/DejaVuSans.ttf");
const FONT_SCALE_FACTOR: f32 = 0.85;
const ATLAS_INITIAL_SIZE: u32 = 2048;
const ATLAS_MAX_SIZE: u32 = 4096;
const ATLAS_PADDING: u32 = 1;
const COPY_BYTES_PER_ROW_ALIGNMENT: u32 = 256;

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

#[derive(Debug, Copy, Clone, Pod, Zeroable)]
#[repr(C)]
struct TextUniforms {
    viewport: [f32; 2],
    offset: [f32; 2],
}

#[derive(Debug, Copy, Clone, Pod, Zeroable)]
#[repr(C)]
struct GlyphInstance {
    pos: [f32; 2],
    size: [f32; 2],
    uv_min: [f32; 2],
    uv_max: [f32; 2],
    color: [f32; 4],
}

#[derive(Debug, Copy, Clone)]
struct GlyphInfo {
    uv_min: [f32; 2],
    uv_max: [f32; 2],
    size: [f32; 2],
    bearing: [f32; 2],
    advance: f32,
}

#[derive(Debug)]
struct AtlasPacker {
    size: u32,
    cursor_x: u32,
    cursor_y: u32,
    row_height: u32,
}

impl AtlasPacker {
    fn new(size: u32) -> Self {
        Self {
            size,
            cursor_x: 0,
            cursor_y: 0,
            row_height: 0,
        }
    }

    fn reset(&mut self, size: u32) {
        self.size = size;
        self.cursor_x = 0;
        self.cursor_y = 0;
        self.row_height = 0;
    }

    fn allocate(&mut self, width: u32, height: u32) -> Option<(u32, u32)> {
        if width > self.size || height > self.size {
            return None;
        }

        if self.cursor_x + width > self.size {
            self.cursor_x = 0;
            self.cursor_y = self.cursor_y.saturating_add(self.row_height);
            self.row_height = 0;
        }

        if self.cursor_y + height > self.size {
            return None;
        }

        let pos = (self.cursor_x, self.cursor_y);
        self.cursor_x = self.cursor_x.saturating_add(width);
        self.row_height = self.row_height.max(height);
        Some(pos)
    }
}

#[derive(Debug)]
struct GlyphAtlas {
    texture: wgpu::Texture,
    view: wgpu::TextureView,
    size: u32,
    packer: AtlasPacker,
}

impl GlyphAtlas {
    fn new(device: &wgpu::Device, size: u32) -> Self {
        let texture = device.create_texture(&wgpu::TextureDescriptor {
            label: Some("terminal.glyph_atlas"),
            size: wgpu::Extent3d {
                width: size,
                height: size,
                depth_or_array_layers: 1,
            },
            mip_level_count: 1,
            sample_count: 1,
            dimension: wgpu::TextureDimension::D2,
            format: wgpu::TextureFormat::R8Unorm,
            usage: wgpu::TextureUsages::TEXTURE_BINDING | wgpu::TextureUsages::COPY_DST,
            view_formats: &[],
        });
        let view = texture.create_view(&wgpu::TextureViewDescriptor::default());

        Self {
            texture,
            view,
            size,
            packer: AtlasPacker::new(size),
        }
    }
}

#[derive(Debug)]
struct TextPipelineData {
    pipeline: wgpu::RenderPipeline,
    bind_group_layout: wgpu::BindGroupLayout,
    uniform_buffer: wgpu::Buffer,
    uniform_bind_group: wgpu::BindGroup,
    empty_bind_group: wgpu::BindGroup,
    sampler: wgpu::Sampler,
    atlas: GlyphAtlas,
    font: FontArc,
    scale: PxScale,
    font_px: f32,
    ascent: f32,
    descent: f32,
    line_height: f32,
    glyphs: HashMap<char, GlyphInfo>,
    instance_buffer: wgpu::Buffer,
    instance_capacity: usize,
    instance_len: usize,
    format: wgpu::TextureFormat,
}

fn align_to(value: u32, alignment: u32) -> u32 {
    if alignment == 0 {
        return value;
    }
    value.div_ceil(alignment) * alignment
}

impl TextPipelineData {
    fn new(device: &wgpu::Device, format: wgpu::TextureFormat) -> Self {
        let shader_src = include_str!("terminal.wgsl");
        let module = device.create_shader_module(wgpu::ShaderModuleDescriptor {
            label: Some("terminal.text.wgsl"),
            source: wgpu::ShaderSource::Wgsl(shader_src.into()),
        });

        let empty_bind_group_layout =
            device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
                label: Some("terminal.text.empty_bind_group_layout"),
                entries: &[],
            });
        let empty_bind_group = device.create_bind_group(&wgpu::BindGroupDescriptor {
            label: Some("terminal.text.empty_bind_group"),
            layout: &empty_bind_group_layout,
            entries: &[],
        });

        let bind_group_layout = device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
            label: Some("terminal.text.bind_group_layout"),
            entries: &[
                wgpu::BindGroupLayoutEntry {
                    binding: 0,
                    visibility: wgpu::ShaderStages::VERTEX,
                    ty: wgpu::BindingType::Buffer {
                        ty: wgpu::BufferBindingType::Uniform,
                        has_dynamic_offset: false,
                        min_binding_size: wgpu::BufferSize::new(
                            std::mem::size_of::<TextUniforms>() as u64,
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
                        multisampled: false,
                        view_dimension: wgpu::TextureViewDimension::D2,
                        sample_type: wgpu::TextureSampleType::Float { filterable: true },
                    },
                    count: None,
                },
            ],
        });

        let uniform_buffer = device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("terminal.text.uniform_buffer"),
            size: std::mem::size_of::<TextUniforms>() as u64,
            usage: wgpu::BufferUsages::UNIFORM | wgpu::BufferUsages::COPY_DST,
            mapped_at_creation: false,
        });

        let sampler = device.create_sampler(&wgpu::SamplerDescriptor {
            label: Some("terminal.text.sampler"),
            address_mode_u: wgpu::AddressMode::ClampToEdge,
            address_mode_v: wgpu::AddressMode::ClampToEdge,
            address_mode_w: wgpu::AddressMode::ClampToEdge,
            mag_filter: wgpu::FilterMode::Linear,
            min_filter: wgpu::FilterMode::Linear,
            mipmap_filter: wgpu::FilterMode::Nearest,
            ..Default::default()
        });

        let atlas = GlyphAtlas::new(device, ATLAS_INITIAL_SIZE);
        let uniform_bind_group = device.create_bind_group(&wgpu::BindGroupDescriptor {
            label: Some("terminal.text.bind_group"),
            layout: &bind_group_layout,
            entries: &[
                wgpu::BindGroupEntry {
                    binding: 0,
                    resource: uniform_buffer.as_entire_binding(),
                },
                wgpu::BindGroupEntry {
                    binding: 1,
                    resource: wgpu::BindingResource::Sampler(&sampler),
                },
                wgpu::BindGroupEntry {
                    binding: 2,
                    resource: wgpu::BindingResource::TextureView(&atlas.view),
                },
            ],
        });

        let pipeline_layout = device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
            label: Some("terminal.text.pipeline_layout"),
            bind_group_layouts: &[&empty_bind_group_layout, &bind_group_layout],
            push_constant_ranges: &[],
        });

        let pipeline = device.create_render_pipeline(&wgpu::RenderPipelineDescriptor {
            label: Some("terminal.text.pipeline"),
            layout: Some(&pipeline_layout),
            vertex: wgpu::VertexState {
                module: &module,
                entry_point: "text_vs_main",
                buffers: &[
                    wgpu::VertexBufferLayout {
                        array_stride: std::mem::size_of::<[f32; 2]>() as u64,
                        step_mode: wgpu::VertexStepMode::Vertex,
                        attributes: &wgpu::vertex_attr_array![0 => Float32x2],
                    },
                    wgpu::VertexBufferLayout {
                        array_stride: std::mem::size_of::<GlyphInstance>() as u64,
                        step_mode: wgpu::VertexStepMode::Instance,
                        attributes: &wgpu::vertex_attr_array![
                            1 => Float32x2,
                            2 => Float32x2,
                            3 => Float32x2,
                            4 => Float32x2,
                            5 => Float32x4
                        ],
                    },
                ],
            },
            fragment: Some(wgpu::FragmentState {
                module: &module,
                entry_point: "text_fs_main",
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

        let font = FontArc::try_from_slice(DEJAVU_SANS).expect("font load failed");
        let scale = PxScale::from(1.0);

        let instance_buffer = device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("terminal.text.instances"),
            size: (64 * std::mem::size_of::<GlyphInstance>()) as u64,
            usage: wgpu::BufferUsages::VERTEX | wgpu::BufferUsages::COPY_DST,
            mapped_at_creation: false,
        });

        Self {
            pipeline,
            bind_group_layout,
            uniform_buffer,
            uniform_bind_group,
            empty_bind_group,
            sampler,
            atlas,
            font,
            scale,
            font_px: 0.0,
            ascent: 0.0,
            descent: 0.0,
            line_height: 0.0,
            glyphs: HashMap::new(),
            instance_buffer,
            instance_capacity: 64,
            instance_len: 0,
            format,
        }
    }

    fn update_uniforms(&self, queue: &wgpu::Queue, viewport: [f32; 2], offset: [f32; 2]) {
        let uniforms = TextUniforms { viewport, offset };
        queue.write_buffer(&self.uniform_buffer, 0, bytemuck::bytes_of(&uniforms));
    }

    fn ensure_font_size(&mut self, font_px: f32) {
        if (self.font_px - font_px).abs() < 0.1 {
            return;
        }

        self.font_px = font_px;
        self.scale = PxScale::from(font_px);
        let scaled = self.font.as_scaled(self.scale);
        self.ascent = scaled.ascent();
        self.descent = scaled.descent();
        self.line_height = self.ascent - self.descent;
        self.glyphs.clear();
        self.atlas.packer.reset(self.atlas.size);
    }

    fn rebuild_atlas(&mut self, device: &wgpu::Device, size: u32) {
        let atlas = GlyphAtlas::new(device, size);
        self.atlas = atlas;
        self.glyphs.clear();
        self.atlas.packer.reset(size);
        self.uniform_bind_group = device.create_bind_group(&wgpu::BindGroupDescriptor {
            label: Some("terminal.text.bind_group"),
            layout: &self.bind_group_layout,
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
                    resource: wgpu::BindingResource::TextureView(&self.atlas.view),
                },
            ],
        });
    }

    fn allocate_in_atlas(
        &mut self,
        device: &wgpu::Device,
        width: u32,
        height: u32,
    ) -> Option<(u32, u32)> {
        let padded_width = width.saturating_add(ATLAS_PADDING * 2);
        let padded_height = height.saturating_add(ATLAS_PADDING * 2);

        if let Some(pos) = self.atlas.packer.allocate(padded_width, padded_height) {
            return Some(pos);
        }

        if self.atlas.size < ATLAS_MAX_SIZE {
            let new_size = (self.atlas.size * 2).min(ATLAS_MAX_SIZE);
            self.rebuild_atlas(device, new_size);
            return self.atlas.packer.allocate(padded_width, padded_height);
        }

        None
    }

    fn get_or_insert_glyph(
        &mut self,
        ch: char,
        device: &wgpu::Device,
        queue: &wgpu::Queue,
    ) -> Option<GlyphInfo> {
        if let Some(info) = self.glyphs.get(&ch) {
            return Some(*info);
        }

        let glyph_id = self.font.glyph_id(ch);
        let scaled = self.font.as_scaled(self.scale);
        let glyph = glyph_id.with_scale_and_position(self.scale, point(0.0, self.ascent));
        let advance = scaled.h_advance(glyph_id);

        let outlined = match self.font.outline_glyph(glyph) {
            Some(outlined) => outlined,
            None => {
                let info = GlyphInfo {
                    uv_min: [0.0, 0.0],
                    uv_max: [0.0, 0.0],
                    size: [0.0, 0.0],
                    bearing: [0.0, 0.0],
                    advance,
                };
                self.glyphs.insert(ch, info);
                return Some(info);
            }
        };

        let bounds = outlined.px_bounds();
        let width = (bounds.max.x - bounds.min.x).ceil().max(0.0) as u32;
        let height = (bounds.max.y - bounds.min.y).ceil().max(0.0) as u32;

        if width == 0 || height == 0 {
            let info = GlyphInfo {
                uv_min: [0.0, 0.0],
                uv_max: [0.0, 0.0],
                size: [0.0, 0.0],
                bearing: [0.0, 0.0],
                advance,
            };
            self.glyphs.insert(ch, info);
            return Some(info);
        }

        let pos = self.allocate_in_atlas(device, width, height)?;
        let origin_x = pos.0 + ATLAS_PADDING;
        let origin_y = pos.1 + ATLAS_PADDING;

        let mut pixels = vec![0u8; (width * height) as usize];
        outlined.draw(|x, y, v| {
            let idx = (y * width + x) as usize;
            if let Some(slot) = pixels.get_mut(idx) {
                *slot = (v * 255.0) as u8;
            }
        });

        let bytes_per_row = width;
        let padded_bytes_per_row = align_to(bytes_per_row, COPY_BYTES_PER_ROW_ALIGNMENT);
        let mut padded = vec![0u8; (padded_bytes_per_row * height) as usize];

        for row in 0..height {
            let src_start = (row * width) as usize;
            let dst_start = (row * padded_bytes_per_row) as usize;
            padded[dst_start..dst_start + width as usize]
                .copy_from_slice(&pixels[src_start..src_start + width as usize]);
        }

        queue.write_texture(
            wgpu::ImageCopyTexture {
                texture: &self.atlas.texture,
                mip_level: 0,
                origin: wgpu::Origin3d {
                    x: origin_x,
                    y: origin_y,
                    z: 0,
                },
                aspect: wgpu::TextureAspect::All,
            },
            &padded,
            wgpu::ImageDataLayout {
                offset: 0,
                bytes_per_row: Some(padded_bytes_per_row),
                rows_per_image: Some(height),
            },
            wgpu::Extent3d {
                width,
                height,
                depth_or_array_layers: 1,
            },
        );

        let atlas_size = self.atlas.size as f32;
        let uv_min = [origin_x as f32 / atlas_size, origin_y as f32 / atlas_size];
        let uv_max = [
            (origin_x + width) as f32 / atlas_size,
            (origin_y + height) as f32 / atlas_size,
        ];

        let info = GlyphInfo {
            uv_min,
            uv_max,
            size: [width as f32, height as f32],
            bearing: [bounds.min.x, bounds.min.y],
            advance,
        };

        self.glyphs.insert(ch, info);
        Some(info)
    }
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
            .map(|p| p.format != format)
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

            let font_px = (self.cell_size[1] * FONT_SCALE_FACTOR).max(1.0);
            text_pipeline.ensure_font_size(font_px);
            let baseline_offset = ((self.cell_size[1] - text_pipeline.line_height).max(0.0) * 0.5)
                + text_pipeline.ascent;
            let cell_width = self.cell_size[0];
            let cell_height = self.cell_size[1];

            let mut glyph_instances = Vec::with_capacity(self.cells.len());
            for cell in &self.cells {
                if cell.ch == ' ' {
                    continue;
                }

                let Some(info) = text_pipeline.get_or_insert_glyph(cell.ch, device, queue) else {
                    continue;
                };

                if info.size[0] == 0.0 || info.size[1] == 0.0 {
                    continue;
                }

                let cell_x = cell.col as f32 * cell_width;
                let cell_y = cell.row as f32 * cell_height;
                let offset_x = (cell_width - info.advance).max(0.0) * 0.5;
                let origin_x = cell_x + offset_x;
                let origin_y = cell_y + baseline_offset - text_pipeline.ascent;
                let pos = [origin_x + info.bearing[0], origin_y + info.bearing[1]];

                glyph_instances.push(GlyphInstance {
                    pos,
                    size: info.size,
                    uv_min: info.uv_min,
                    uv_max: info.uv_max,
                    color: cell.fg,
                });
            }

            text_pipeline.instance_len = glyph_instances.len();

            if text_pipeline.instance_len > text_pipeline.instance_capacity {
                let new_cap = text_pipeline.instance_len.next_power_of_two().max(64);
                text_pipeline.instance_buffer = device.create_buffer(&wgpu::BufferDescriptor {
                    label: Some("terminal.text.instances"),
                    size: (new_cap * std::mem::size_of::<GlyphInstance>()) as wgpu::BufferAddress,
                    usage: wgpu::BufferUsages::VERTEX | wgpu::BufferUsages::COPY_DST,
                    mapped_at_creation: false,
                });
                text_pipeline.instance_capacity = new_cap;
            }

            if !glyph_instances.is_empty() {
                queue.write_buffer(
                    &text_pipeline.instance_buffer,
                    0,
                    bytemuck::cast_slice(&glyph_instances),
                );
            }
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

        if text_pipeline.instance_len > 0 {
            pass.set_pipeline(&text_pipeline.pipeline);
            pass.set_bind_group(0, &text_pipeline.empty_bind_group, &[]);
            pass.set_bind_group(1, &text_pipeline.uniform_bind_group, &[]);
            pass.set_vertex_buffer(0, pipeline.quad_buffer.slice(..));
            pass.set_vertex_buffer(1, text_pipeline.instance_buffer.slice(..));
            pass.draw(0..6, 0..text_pipeline.instance_len as u32);
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

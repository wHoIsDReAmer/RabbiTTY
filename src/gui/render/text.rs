use crate::config::DEFAULT_TERMINAL_FONT_SIZE;
use crate::terminal::CellVisual;
use crate::terminal_font::load_system_font_by_family;
use ab_glyph::{Font, FontArc, PxScale, ScaleFont, point};
use bytemuck::{Pod, Zeroable};
use iced::wgpu;
use std::collections::HashMap;
use std::fs;

const DEJAVU_SANS_MONO: &[u8] = include_bytes!("../../../fonts/DejaVuSansMono.ttf");
const ATLAS_INITIAL_SIZE: u32 = 2048;
const ATLAS_MAX_SIZE: u32 = 4096;
const ATLAS_PADDING: u32 = 1;
const COPY_BYTES_PER_ROW_ALIGNMENT: u32 = 256;

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
    bg_color: [f32; 4],
}

#[derive(Debug, Copy, Clone)]
struct GlyphInfo {
    uv_min: [f32; 2],
    uv_max: [f32; 2],
    size: [f32; 2],
    bearing: [f32; 2],
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
            format: wgpu::TextureFormat::Rgba8Unorm,
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
pub(super) struct TextPipelineData {
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
    requested_font_size: f32,
    ascent: f32,
    descent: f32,
    line_height: f32,
    line_min_y: f32,
    cell_advance: f32,
    glyphs: HashMap<char, GlyphInfo>,
    raster_buf: Vec<u8>,
    filter_buf: Vec<u8>,
    glyph_instances: Vec<GlyphInstance>,
    instance_buffer: wgpu::Buffer,
    instance_capacity: usize,
    instance_len: usize,
    requested_font_selection: Option<String>,
}

fn align_to(value: u32, alignment: u32) -> u32 {
    if alignment == 0 {
        return value;
    }
    value.div_ceil(alignment) * alignment
}

impl TextPipelineData {
    pub(super) fn new(device: &wgpu::Device, format: wgpu::TextureFormat) -> Self {
        let shader_src = include_str!("../terminal.wgsl");
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
            mag_filter: wgpu::FilterMode::Nearest,
            min_filter: wgpu::FilterMode::Nearest,
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

        // Single-pass subpixel: shader computes blending using known bg color
        let pipeline = device.create_render_pipeline(&wgpu::RenderPipelineDescriptor {
            label: Some("terminal.text.pipeline"),
            layout: Some(&pipeline_layout),
            vertex: wgpu::VertexState {
                module: &module,
                entry_point: Some("text_vs_main"),
                compilation_options: wgpu::PipelineCompilationOptions::default(),
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
                            5 => Float32x4,
                            6 => Float32x4
                        ],
                    },
                ],
            },
            fragment: Some(wgpu::FragmentState {
                module: &module,
                entry_point: Some("text_fs_subpixel"),
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

        let font = FontArc::try_from_slice(DEJAVU_SANS_MONO).expect("font load failed");
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
            requested_font_size: DEFAULT_TERMINAL_FONT_SIZE,
            ascent: 0.0,
            descent: 0.0,
            line_height: 0.0,
            line_min_y: 0.0,
            cell_advance: 0.0,
            glyphs: HashMap::new(),
            raster_buf: Vec::new(),
            filter_buf: Vec::new(),
            glyph_instances: Vec::new(),
            instance_buffer,
            instance_capacity: 64,
            instance_len: 0,
            requested_font_selection: None,
        }
    }

    pub(super) fn apply_terminal_font_selection(
        &mut self,
        device: &wgpu::Device,
        font_selection: Option<&str>,
    ) {
        let requested_font_selection = font_selection
            .map(str::trim)
            .filter(|path| !path.is_empty())
            .map(ToOwned::to_owned);
        if requested_font_selection == self.requested_font_selection {
            return;
        }
        self.requested_font_selection = requested_font_selection.clone();

        let next_font = requested_font_selection
            .as_deref()
            .and_then(load_font_from_selection)
            .unwrap_or_else(default_terminal_font);
        self.set_font(device, next_font);
    }

    pub(super) fn set_requested_font_size(&mut self, font_size: f32) {
        if font_size.is_finite() && font_size > 0.0 {
            self.requested_font_size = font_size;
        }
    }

    pub(super) fn update_uniforms(
        &self,
        queue: &wgpu::Queue,
        viewport: [f32; 2],
        offset: [f32; 2],
    ) {
        let uniforms = TextUniforms { viewport, offset };
        queue.write_buffer(&self.uniform_buffer, 0, bytemuck::bytes_of(&uniforms));
    }

    pub(super) fn prepare_instances(
        &mut self,
        device: &wgpu::Device,
        queue: &wgpu::Queue,
        cells: &[CellVisual],
        cell_size: [f32; 2],
    ) {
        let cell_width = cell_size[0];
        let cell_height = cell_size[1];

        let mut font_px = self.requested_font_size.max(1.0);
        self.ensure_font_size(font_px);
        if cell_width > 0.0 && self.cell_advance > cell_width {
            let scale = cell_width / self.cell_advance;
            font_px = (font_px * scale).max(1.0);
            self.ensure_font_size(font_px);
        }
        if cell_height > 0.0 && self.line_height > cell_height {
            let scale = cell_height / self.line_height;
            font_px = (font_px * scale).max(1.0);
            self.ensure_font_size(font_px);
        }

        let top_margin = (cell_height - self.line_height).max(0.0) * 0.5;

        self.glyph_instances.clear();
        let needed = cells.len().saturating_sub(self.glyph_instances.capacity());
        if needed > 0 {
            self.glyph_instances.reserve(needed);
        }
        for cell in cells {
            if cell.ch == ' ' {
                continue;
            }

            let Some(info) = self.get_or_insert_glyph(cell.ch, device, queue) else {
                continue;
            };

            if info.size[0] == 0.0 || info.size[1] == 0.0 {
                continue;
            }

            let span = if cell.wide { 2.0 } else { 1.0 };
            let cell_x = cell.col as f32 * cell_width;
            let cell_y = cell.row as f32 * cell_height;
            let wide_offset_x = (cell_width * span - self.cell_advance * span).max(0.0) * 0.5;
            let origin_x = cell_x + wide_offset_x;
            let origin_y = cell_y + top_margin - self.line_min_y;
            let pos = [origin_x + info.bearing[0], origin_y + info.bearing[1]];

            self.glyph_instances.push(GlyphInstance {
                pos,
                size: info.size,
                uv_min: info.uv_min,
                uv_max: info.uv_max,
                color: cell.fg,
                bg_color: cell.bg,
            });
        }

        self.instance_len = self.glyph_instances.len();

        if self.instance_len > self.instance_capacity {
            let new_cap = self.instance_len.next_power_of_two().max(64);
            self.instance_buffer = device.create_buffer(&wgpu::BufferDescriptor {
                label: Some("terminal.text.instances"),
                size: (new_cap * std::mem::size_of::<GlyphInstance>()) as wgpu::BufferAddress,
                usage: wgpu::BufferUsages::VERTEX | wgpu::BufferUsages::COPY_DST,
                mapped_at_creation: false,
            });
            self.instance_capacity = new_cap;
        }

        if !self.glyph_instances.is_empty() {
            queue.write_buffer(
                &self.instance_buffer,
                0,
                bytemuck::cast_slice(&self.glyph_instances),
            );
        }
    }

    pub(super) fn pipeline(&self) -> &wgpu::RenderPipeline {
        &self.pipeline
    }

    pub(super) fn uniform_bind_group(&self) -> &wgpu::BindGroup {
        &self.uniform_bind_group
    }

    pub(super) fn empty_bind_group(&self) -> &wgpu::BindGroup {
        &self.empty_bind_group
    }

    pub(super) fn instance_buffer(&self) -> &wgpu::Buffer {
        &self.instance_buffer
    }

    pub(super) fn instance_len(&self) -> usize {
        self.instance_len
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
        let mut min_y = 0.0;
        let mut max_y = 0.0;
        let mut has_bounds = false;
        for code in 32u8..=126u8 {
            let ch = code as char;
            let glyph_id = self.font.glyph_id(ch);
            let glyph = glyph_id.with_scale_and_position(self.scale, point(0.0, self.ascent));
            if let Some(outlined) = self.font.outline_glyph(glyph) {
                let bounds = outlined.px_bounds();
                if !has_bounds {
                    min_y = bounds.min.y;
                    max_y = bounds.max.y;
                    has_bounds = true;
                } else {
                    min_y = min_y.min(bounds.min.y);
                    max_y = max_y.max(bounds.max.y);
                }
            }
        }
        if has_bounds {
            self.line_min_y = min_y;
            self.line_height = (max_y - min_y).max(1.0);
        } else {
            self.line_min_y = 0.0;
            self.line_height = scaled.height();
        }
        let mut advance = 0.0;
        for ch in ['M', 'W', '0', ' '].into_iter() {
            let candidate = scaled.h_advance(self.font.glyph_id(ch));
            if candidate > 0.0 {
                advance = candidate;
                break;
            }
        }
        if advance <= 0.0 {
            advance = (self.line_height * 0.6).max(1.0);
        }
        self.cell_advance = advance;
        self.glyphs.clear();
        self.atlas.packer.reset(self.atlas.size);
    }

    fn set_font(&mut self, device: &wgpu::Device, font: FontArc) {
        self.font = font;
        self.font_px = 0.0;
        self.scale = PxScale::from(1.0);
        self.ascent = 0.0;
        self.descent = 0.0;
        self.line_height = 0.0;
        self.line_min_y = 0.0;
        self.cell_advance = 0.0;
        self.glyphs.clear();
        self.rebuild_atlas(device, self.atlas.size);
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

        // Use 3x horizontal scale for subpixel rendering
        let subpixel_scale = PxScale {
            x: self.scale.x * 3.0,
            y: self.scale.y,
        };
        let glyph = glyph_id.with_scale_and_position(subpixel_scale, point(0.0, self.ascent));

        let outlined = match self.font.outline_glyph(glyph) {
            Some(outlined) => outlined,
            None => {
                let info = GlyphInfo {
                    uv_min: [0.0, 0.0],
                    uv_max: [0.0, 0.0],
                    size: [0.0, 0.0],
                    bearing: [0.0, 0.0],
                };
                self.glyphs.insert(ch, info);
                return Some(info);
            }
        };

        let bounds = outlined.px_bounds();
        let raster_width = (bounds.max.x - bounds.min.x).ceil().max(0.0) as u32;
        let raster_height = (bounds.max.y - bounds.min.y).ceil().max(0.0) as u32;
        let display_width = raster_width.div_ceil(3);

        if display_width == 0 || raster_height == 0 {
            let info = GlyphInfo {
                uv_min: [0.0, 0.0],
                uv_max: [0.0, 0.0],
                size: [0.0, 0.0],
                bearing: [0.0, 0.0],
            };
            self.glyphs.insert(ch, info);
            return Some(info);
        }

        let pos = self.allocate_in_atlas(device, display_width, raster_height)?;
        let origin_x = pos.0 + ATLAS_PADDING;
        let origin_y = pos.1 + ATLAS_PADDING;

        // Rasterize at 3x horizontal resolution (reuse buffers)
        let raster_len = (raster_width * raster_height) as usize;
        self.raster_buf.clear();
        self.raster_buf.resize(raster_len, 0);
        outlined.draw(|x, y, v| {
            let idx = (y * raster_width + x) as usize;
            if let Some(slot) = self.raster_buf.get_mut(idx) {
                *slot = (v * 255.0) as u8;
            }
        });

        // Apply FreeType-style 5-tap LCD filter to reduce color fringing
        const LCD_W: [u32; 5] = [16, 64, 112, 64, 16]; // sum = 272
        self.filter_buf.clear();
        self.filter_buf.resize(raster_len, 0);
        let rw = raster_width as i32;
        for row in 0..raster_height {
            let base = (row * raster_width) as i32;
            for x in 0..rw {
                let mut acc: u32 = 0;
                for (k, &w) in LCD_W.iter().enumerate() {
                    let sx = x + k as i32 - 2;
                    if sx >= 0 && sx < rw {
                        acc += self.raster_buf[(base + sx) as usize] as u32 * w;
                    }
                }
                self.filter_buf[(base + x) as usize] = (acc / 272).min(255) as u8;
            }
        }

        // Pack every 3 filtered samples into RGBA (R=left, G=center, B=right subpixel)
        let rgba_row_bytes = display_width * 4;
        let padded_bytes_per_row = align_to(rgba_row_bytes, COPY_BYTES_PER_ROW_ALIGNMENT);
        let mut padded = vec![0u8; (padded_bytes_per_row * raster_height) as usize];

        for row in 0..raster_height {
            for col in 0..display_width {
                let rx = col * 3;
                let r = self
                    .filter_buf
                    .get((row * raster_width + rx) as usize)
                    .copied()
                    .unwrap_or(0);
                let g = self
                    .filter_buf
                    .get((row * raster_width + rx + 1) as usize)
                    .copied()
                    .unwrap_or(0);
                let b = self
                    .filter_buf
                    .get((row * raster_width + rx + 2) as usize)
                    .copied()
                    .unwrap_or(0);
                let idx = (row * padded_bytes_per_row + col * 4) as usize;
                padded[idx] = r;
                padded[idx + 1] = g;
                padded[idx + 2] = b;
                padded[idx + 3] = r.max(g).max(b);
            }
        }

        queue.write_texture(
            wgpu::TexelCopyTextureInfo {
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
            wgpu::TexelCopyBufferLayout {
                offset: 0,
                bytes_per_row: Some(padded_bytes_per_row),
                rows_per_image: Some(raster_height),
            },
            wgpu::Extent3d {
                width: display_width,
                height: raster_height,
                depth_or_array_layers: 1,
            },
        );

        let atlas_size = self.atlas.size as f32;
        let uv_min = [origin_x as f32 / atlas_size, origin_y as f32 / atlas_size];
        let uv_max = [
            (origin_x + display_width) as f32 / atlas_size,
            (origin_y + raster_height) as f32 / atlas_size,
        ];

        let info = GlyphInfo {
            uv_min,
            uv_max,
            size: [display_width as f32, raster_height as f32],
            bearing: [bounds.min.x / 3.0, bounds.min.y],
        };

        self.glyphs.insert(ch, info);
        Some(info)
    }
}

fn default_terminal_font() -> FontArc {
    FontArc::try_from_slice(DEJAVU_SANS_MONO).expect("font load failed")
}

fn load_font_from_path(path: &str) -> Option<FontArc> {
    let bytes = fs::read(path).ok()?;
    FontArc::try_from_vec(bytes).ok()
}

fn load_font_from_selection(selection: &str) -> Option<FontArc> {
    load_system_font_by_family(selection).or_else(|| load_font_from_path(selection))
}

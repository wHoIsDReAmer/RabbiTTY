use iced::wgpu;

pub(super) const ATLAS_INITIAL_SIZE: u32 = 2048;
pub(super) const ATLAS_MAX_SIZE: u32 = 4096;
pub(super) const ATLAS_PADDING: u32 = 1;

#[derive(Debug)]
pub(super) struct AtlasPacker {
    pub(super) size: u32,
    cursor_x: u32,
    cursor_y: u32,
    row_height: u32,
}

impl AtlasPacker {
    pub(super) fn new(size: u32) -> Self {
        Self {
            size,
            cursor_x: 0,
            cursor_y: 0,
            row_height: 0,
        }
    }

    pub(super) fn reset(&mut self, size: u32) {
        self.size = size;
        self.cursor_x = 0;
        self.cursor_y = 0;
        self.row_height = 0;
    }

    pub(super) fn allocate(&mut self, width: u32, height: u32) -> Option<(u32, u32)> {
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
pub(super) struct GlyphAtlas {
    pub(super) texture: wgpu::Texture,
    pub(super) view: wgpu::TextureView,
    pub(super) size: u32,
    pub(super) packer: AtlasPacker,
}

impl GlyphAtlas {
    pub(super) fn new(device: &wgpu::Device, size: u32) -> Self {
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

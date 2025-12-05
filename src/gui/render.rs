use wgpu::Backends;

#[derive(Debug, Clone)]
pub struct RenderConfig {
    pub backend: Backends,
}

impl Default for RenderConfig {
    fn default() -> Self {
        Self {
            backend: Backends::PRIMARY,
        }
    }
}

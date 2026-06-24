use crate::assets::Asset;

pub struct WindowSize {
    pub width: u32,
    pub height: u32,
}

impl WindowSize {
    pub fn new(width: u32, height: u32) -> Self {
        Self { width, height }
    }
}

pub struct FerrumConfig {
    pub size: WindowSize,
    pub asset: Asset,
    pub surface_config: Option<wgpu::SurfaceConfiguration>,
}

impl Default for FerrumConfig {
    fn default() -> Self {
        Self {
            size: WindowSize::new(500, 500),
            asset: Asset::default(),
            surface_config: None,
        }
    }
}

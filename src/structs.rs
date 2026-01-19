use {std::sync::Arc, winit::window::Window};

pub struct State {
    pub surface: wgpu::Surface<'static>,
    pub device: wgpu::Device,
    pub queue: wgpu::Queue,
    pub config: wgpu::SurfaceConfiguration,
    pub is_surface_configuration: bool,
    pub window: Arc<Window>,
}

pub struct App {
    pub state: Option<State>,
    #[cfg(target_arch = "wasm32")]
    pub proxy: Option<EventLoopProxy<State>>,
}

use {crate::texture, std::sync::Arc, wgpu::BindGroup, winit::window::Window};

pub struct State {
    pub surface: wgpu::Surface<'static>,
    pub device: wgpu::Device,
    pub queue: wgpu::Queue,
    pub config: wgpu::SurfaceConfiguration,
    pub is_surface_configuration: bool,
    pub render_pipeline: wgpu::RenderPipeline,
    pub vertex_buffer: wgpu::Buffer,
    pub num_vertex: u32,
    pub index_buffer: wgpu::Buffer,
    pub num_index: u32,
    pub diffuse_bind_group: BindGroup,
    pub diffuse_texture: texture::Texture,
    pub window: Arc<Window>,
}

pub struct App {
    pub state: Option<State>,
    #[cfg(target_arch = "wasm32")]
    pub proxy: Option<EventLoopProxy<State>>,
}

// #[repr(C)]
// #[derive(Debug, Clone, Copy, bytemuck::Pod, bytemuck::Zeroable)]
// pub struct Vertex {
//     pub position: [f32; 3],
//     pub color: [f32; 3],
// }

#[repr(C)]
#[derive(Debug, Clone, Copy, bytemuck::Pod, bytemuck::Zeroable)]
pub struct Vertex {
    pub position: [f32; 3],
    pub text_cords: [f32; 2],
}

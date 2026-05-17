#[cfg(target_arch = "wasm32")]
use winit::event_loop::EventLoopProxy;

use crate::material::Material;

#[repr(C)]
#[derive(Debug, Clone, Copy, bytemuck::Pod, bytemuck::Zeroable)]
pub struct Vertex {
    pub position: [f32; 3],
    pub text_cords: [f32; 2],
}

pub struct Mesh {
    pub name: String,
    pub vertex_buffer: wgpu::Buffer,
    pub index_buffer: wgpu::Buffer,
    pub material: usize,
    pub indices: u32,
}

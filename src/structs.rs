#[cfg(target_arch = "wasm32")]
use winit::event_loop::EventLoopProxy;

use {crate::texture, std::sync::Arc, winit::window::Window};

pub struct State {
    pub surface: wgpu::Surface<'static>,
    pub device: wgpu::Device,
    pub queue: wgpu::Queue,
    pub config: wgpu::SurfaceConfiguration,
    pub is_surface_configuration: bool,
    pub render_pipeline: wgpu::RenderPipeline,
    pub camera: Camera,
    pub camera_uniform: CameraUniform,
    pub camera_buffer: wgpu::Buffer,
    pub camera_bind_group: wgpu::BindGroup,
    pub camera_controller: CameraController,
    pub obj_model: crate::structs::Model,
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

pub struct Camera {
    pub eye: cgmath::Point3<f32>,
    pub target: cgmath::Point3<f32>,
    pub up: cgmath::Vector3<f32>,
    pub aspect: f32,
    pub fovy: f32,
    pub znear: f32,
    pub zfar: f32,
}

#[repr(C)]
#[derive(Debug, Copy, Clone, bytemuck::Pod, bytemuck::Zeroable)]
pub struct CameraUniform {
    pub view_proj: [[f32; 4]; 4],
}

pub struct CameraController {
    pub speed: f32,
    pub is_forward_pressed: bool,
    pub is_backward_pressed: bool,
    pub if_left_pressed: bool,
    pub if_right_pressed: bool,
}

#[repr(C)]
#[derive(Debug, Clone, Copy, bytemuck::Zeroable, bytemuck::Pod)]
pub struct ModelVertex {
    pub position: [f32; 3],
    pub text_cords: [f32; 2],
    pub normal: [f32; 3],
}

pub struct Mesh {
    pub name: String,
    pub vertex_buffer: wgpu::Buffer,
    pub index_buffer: wgpu::Buffer,
    pub material: usize,
    pub indices: u32,
}

pub struct Material {
    pub name: String,
    pub diffuse_texture: texture::Texture,
    pub bind_group: wgpu::BindGroup,
}

pub struct Model {
    pub meshes: Vec<Mesh>,
    pub materials: Vec<Material>,
}

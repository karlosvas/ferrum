use wgpu::BindGroup;

use crate::renderer;

pub struct Material {
    pub name: String,
    pub diffuse_texture: renderer::Texture,
    pub normal_texture: renderer::Texture,
    pub bind_group: wgpu::BindGroup,
}

impl Material {
    pub fn new(
        device: wgpu::Device,
        name: &str,
        diffuse_texture: renderer::Texture,
        normal_texture: renderer::Texture,
        layout: &wgpu::BindGroupLayout,
    ) -> Self {
        let bind_group: BindGroup = device.create_bind_group(&wgpu::BindGroupDescriptor {
            layout,
            label: Some(name),
            entries: &[
                // Texture
                wgpu::BindGroupEntry {
                    binding: 0,
                    resource: wgpu::BindingResource::TextureView(&diffuse_texture.view),
                },
                wgpu::BindGroupEntry {
                    binding: 1,
                    resource: wgpu::BindingResource::Sampler(&diffuse_texture.sampler),
                },
                // Normal
                wgpu::BindGroupEntry {
                    binding: 2,
                    resource: wgpu::BindingResource::TextureView(&diffuse_texture.view),
                },
                wgpu::BindGroupEntry {
                    binding: 3,
                    resource: wgpu::BindingResource::Sampler(&diffuse_texture.sampler),
                },
            ],
        });

        Self {
            name: String::from(name),
            diffuse_texture,
            normal_texture,
            bind_group,
        }
    }
}

#[cfg(target_arch = "wasm32")]
use winit::event_loop::EventLoopProxy;

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

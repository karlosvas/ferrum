// #[repr(C)]
// #[derive(Debug, Clone, Copy, bytemuck::Pod, bytemuck::Zeroable)]
// pub struct Vertex {
//     pub position: [f32; 3],
//     pub color: [f32; 3],
// }

// pub const VERTICES: &[Vertex] = &[
//     Vertex {
//         position: [0.0, 0.5, 0.0],
//         text_cords: [0.5, 0.0],
//     },
//     Vertex {
//         position: [-0.5, -0.5, 0.0],
//         text_cords: [0.0, 1.0],
//     },
//     Vertex {
//         position: [0.5, -0.5, 0.0],
//         text_cords: [1.0, 1.0],
//     },
// ];
// const INDICES: &[u16] = &[0, 1, 2];

// use wgpu::{BindGroup, BindGroupLayout, Device, Queue, wgc::device};

// use crate::texture;

// impl Vertex {
//     const ATRIBS: [wgpu::VertexAttribute; 2] =
//         wgpu::vertex_attr_array![0 => Float32x3, 1 => Float32x2];

//     pub fn desc() -> wgpu::VertexBufferLayout<'static> {
//         use std::mem;

//         wgpu::VertexBufferLayout {
//             array_stride: mem::size_of::<Self>() as wgpu::BufferAddress,
//             step_mode: wgpu::VertexStepMode::Vertex,
//             attributes: &Self::ATRIBS,
//         }
//     }

//     // pub fn build_diffuse(
//     //     device: &Device,
//     //     queue: &Queue,
//     //     texture_bind_group_layout: &BindGroupLayout,
//     // ) -> BindGroup {
//     //     let diffuse_bytes: &[u8] = include_bytes!("planta.png");
//     //     let diffuse_texture: texture::Texture =
//     //         texture::Texture::from_bytes(&device, &queue, diffuse_bytes, "planta.png").unwrap();

//     //     device.create_bind_group(&wgpu::BindGroupDescriptor {
//     //         label: Some("diffuse_bind_group"),
//     //         layout: &texture_bind_group_layout,
//     //         entries: &[
//     //             wgpu::BindGroupEntry {
//     //                 binding: 0,
//     //                 resource: wgpu::BindingResource::TextureView(&diffuse_texture.view),
//     //             },
//     //             wgpu::BindGroupEntry {
//     //                 binding: 1,
//     //                 resource: wgpu::BindingResource::Sampler(&diffuse_texture.sampler),
//     //             },
//     //         ],
//     //     })
//     // let vertex_buffer: Buffer = device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
//     //     label: Some("vertex_buffer"),
//     //     contents: bytemuck::cast_slice(VERTICES),
//     //     usage: wgpu::BufferUsages::VERTEX,
//     // });

//     // let num_vertex: u32 = VERTICES.len() as u32;

//     // let index_buffer: Buffer = device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
//     //     label: Some("index_buffer"),
//     //     contents: bytemuck::cast_slice(INDICES),
//     //     usage: wgpu::BufferUsages::INDEX,
//     // });

//     // let num_index: u32 = INDICES.len() as u32;
//     // }
// }

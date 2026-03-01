// Forma 1 vertices
// Vertices de prueba de un pentágono
// const VERTICES: &[Vertex] = &[
//     Vertex {
//         position: [0.0, 0.5, 0.0],
//         color: [1.0, 0.0, 0.0],
//     },
//     Vertex {
//         position: [-0.5, -0.5, 0.0],
//         color: [0.0, 1.0, 0.0],
//     },
//     Vertex {
//         position: [0.5, -0.5, 0.0],
//         color: [0.0, 0.0, 1.0],
//     },
// ];

// Forma 2 vertices
// use crate::structs::Vertex;

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
// }

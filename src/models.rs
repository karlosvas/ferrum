// Forma 3 (Modelo)
use crate::structs::ModelVertex;
use crate::{models, structs};

pub trait Vertex {
    fn desc() -> wgpu::VertexBufferLayout<'static>;
}

impl Vertex for ModelVertex {
    fn desc() -> wgpu::VertexBufferLayout<'static> {
        use std::mem;
        wgpu::VertexBufferLayout {
            array_stride: mem::size_of::<ModelVertex>() as wgpu::BufferAddress,
            step_mode: wgpu::VertexStepMode::Vertex,
            attributes: &[
                wgpu::VertexAttribute {
                    offset: 0,
                    shader_location: 0,
                    format: wgpu::VertexFormat::Float32x3,
                },
                wgpu::VertexAttribute {
                    offset: mem::size_of::<[f32; 3]>() as wgpu::BufferAddress,
                    shader_location: 1,
                    format: wgpu::VertexFormat::Float32x2,
                },
                wgpu::VertexAttribute {
                    offset: mem::size_of::<[f32; 5]>() as wgpu::BufferAddress,
                    shader_location: 2,
                    format: wgpu::VertexFormat::Float32x3,
                },
            ],
        }
    }
}

pub trait DrawModel<'a> {
    fn draw_mesh(
        &mut self,
        mesh: &'a structs::Mesh,
        material: &'a structs::Material,
        camera_bind_group: &'a wgpu::BindGroup,
    );
    fn draw_mesh_instanced(
        &mut self,
        mesh: &'a structs::Mesh,
        material: &'a structs::Material,
        camera_bind_group: &'a wgpu::BindGroup,
    );
    fn draw_model(&mut self, model: &'a structs::Model, camera_bind_group: &'a wgpu::BindGroup);
}

impl<'a, 'b> DrawModel<'b> for wgpu::RenderPass<'a>
where
    'b: 'a,
{
    fn draw_mesh(
        &mut self,
        mesh: &'b structs::Mesh,
        material: &'b structs::Material,
        camera_bind_group: &'b wgpu::BindGroup,
    ) {
        self.draw_mesh_instanced(mesh, material, camera_bind_group);
    }

    fn draw_mesh_instanced(
        &mut self,
        mesh: &'b structs::Mesh,
        material: &'b structs::Material,
        camera_bind_group: &'b wgpu::BindGroup,
    ) {
        self.set_vertex_buffer(0, mesh.vertex_buffer.slice(..));
        self.set_index_buffer(mesh.index_buffer.slice(..), wgpu::IndexFormat::Uint32);
        self.set_bind_group(0, &material.bind_group, &[]);
        self.set_bind_group(1, camera_bind_group, &[]);
        self.draw_indexed(0..mesh.indices, 0, 0..1);
    }

    fn draw_model(&mut self, models: &'b structs::Model, camera_bind_group: &'b wgpu::BindGroup) {
        for mesh in &models.meshes {
            let material: &structs::Material = &models.materials[mesh.material];
            self.draw_mesh(mesh, material, camera_bind_group);
        }
    }
}

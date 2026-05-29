use cgmath::{Matrix3, Matrix4, One, Quaternion, Vector3, Zero};

use crate::{material, structs};

pub trait Vertex {
    fn desc() -> wgpu::VertexBufferLayout<'static>;
}

#[repr(C)]
#[derive(Debug, Clone, Copy, bytemuck::Zeroable, bytemuck::Pod)]
pub struct ModelVertex {
    pub position: [f32; 3],
    pub text_cords: [f32; 2],
    pub normal: [f32; 3],
    pub color: [f32; 3],
    pub tangent: [f32; 3],
    pub bitangent: [f32; 3],
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
                wgpu::VertexAttribute {
                    offset: mem::size_of::<[f32; 8]>() as wgpu::BufferAddress,
                    shader_location: 3,
                    format: wgpu::VertexFormat::Float32x3,
                },
                wgpu::VertexAttribute {
                    offset: mem::size_of::<[f32; 8]>() as wgpu::BufferAddress,
                    shader_location: 4,
                    format: wgpu::VertexFormat::Float32x3,
                },
                wgpu::VertexAttribute {
                    offset: mem::size_of::<[f32; 11]>() as wgpu::BufferAddress,
                    shader_location: 5,
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
        material: &'a material::Material,
        instances: std::ops::Range<u32>,
        instance_buffer: &'a wgpu::Buffer,
        camera_bind_group: &'a wgpu::BindGroup,
        light_bind_group: &'a wgpu::BindGroup,
        shadow_bind_group: &'a wgpu::BindGroup,
    );

    fn draw_model(
        &mut self,
        model: &'a Model,
        camera_bind_group: &'a wgpu::BindGroup,
        light_bind_group: &'a wgpu::BindGroup,
        shadow_bind_group: &'a wgpu::BindGroup,
    );
}

impl<'a, 'b> DrawModel<'b> for wgpu::RenderPass<'a>
where
    'b: 'a,
{
    fn draw_mesh(
        &mut self,
        mesh: &'b structs::Mesh,
        material: &'b material::Material,
        instances: std::ops::Range<u32>,
        instance_buffer: &'a wgpu::Buffer,
        camera_bind_group: &'b wgpu::BindGroup,
        light_bind_group: &'a wgpu::BindGroup,
        shadow_bind_group: &'a wgpu::BindGroup,
    ) {
        self.set_vertex_buffer(0, mesh.vertex_buffer.slice(..));
        self.set_vertex_buffer(1, instance_buffer.slice(..));
        self.set_index_buffer(mesh.index_buffer.slice(..), wgpu::IndexFormat::Uint32);
        self.set_bind_group(0, &material.bind_group, &[]);
        self.set_bind_group(1, camera_bind_group, &[]);
        self.set_bind_group(2, light_bind_group, &[]);
        self.set_bind_group(3, shadow_bind_group, &[]);
        self.draw_indexed(0..mesh.indices, 0, instances);
    }

    fn draw_model(
        &mut self,
        model: &'b Model,
        camera_bind_group: &'b wgpu::BindGroup,
        light_bind_group: &'a wgpu::BindGroup,
        shadow_bind_group: &'a wgpu::BindGroup,
    ) {
        for mesh in &model.meshes {
            let material: &material::Material = &model.materials[mesh.material];
            self.draw_mesh(
                mesh,
                material,
                0..model.instances.len() as u32,
                &model.instance_buffer,
                camera_bind_group,
                light_bind_group,
                shadow_bind_group,
            );
        }
    }
}

#[repr(C)]
#[derive(Clone, Copy, Debug, bytemuck::Pod, bytemuck::Zeroable)]
#[allow(dead_code)]
pub struct InstanceRaw {
    pub model: [[f32; 4]; 4],
    pub normals: [[f32; 3]; 3],
}

impl InstanceRaw {
    pub fn desc() -> wgpu::VertexBufferLayout<'static> {
        use std::mem;

        wgpu::VertexBufferLayout {
            array_stride: mem::size_of::<InstanceRaw>() as wgpu::BufferAddress,
            step_mode: wgpu::VertexStepMode::Instance,
            attributes: &[
                wgpu::VertexAttribute {
                    format: wgpu::VertexFormat::Float32x4,
                    offset: 0,
                    shader_location: 6,
                },
                wgpu::VertexAttribute {
                    format: wgpu::VertexFormat::Float32x4,
                    offset: mem::size_of::<[f32; 4]>() as wgpu::BufferAddress,
                    shader_location: 7,
                },
                wgpu::VertexAttribute {
                    format: wgpu::VertexFormat::Float32x4,
                    offset: mem::size_of::<[f32; 8]>() as wgpu::BufferAddress,
                    shader_location: 8,
                },
                wgpu::VertexAttribute {
                    format: wgpu::VertexFormat::Float32x4,
                    offset: mem::size_of::<[f32; 12]>() as wgpu::BufferAddress,
                    shader_location: 9,
                },
                wgpu::VertexAttribute {
                    format: wgpu::VertexFormat::Float32x3,
                    offset: mem::size_of::<[f32; 16]>() as wgpu::BufferAddress,
                    shader_location: 10,
                },
                wgpu::VertexAttribute {
                    format: wgpu::VertexFormat::Float32x3,
                    offset: mem::size_of::<[f32; 19]>() as wgpu::BufferAddress,
                    shader_location: 11,
                },
                wgpu::VertexAttribute {
                    format: wgpu::VertexFormat::Float32x3,
                    offset: mem::size_of::<[f32; 22]>() as wgpu::BufferAddress,
                    shader_location: 12,
                },
            ],
        }
    }
}

pub struct Instance {
    pub position: cgmath::Vector3<f32>,
    pub rotation: cgmath::Quaternion<f32>,
    pub scale: cgmath::Vector3<f32>,
}

impl Instance {
    pub fn new(
        position: cgmath::Vector3<f32>,
        rotation: cgmath::Quaternion<f32>,
        scale: cgmath::Vector3<f32>,
    ) -> Self {
        Self {
            position,
            rotation,
            scale,
        }
    }

    pub fn to_raw(&self) -> InstanceRaw {
        let translation: Matrix4<f32> = cgmath::Matrix4::from_translation(self.position);
        let rotation: Matrix4<f32> = cgmath::Matrix4::from(self.rotation);
        let scale: Matrix4<f32> =
            cgmath::Matrix4::from_nonuniform_scale(self.scale.x, self.scale.y, self.scale.z);

        let normals: Matrix3<f32> = cgmath::Matrix3::from(self.rotation);

        InstanceRaw {
            model: (translation * rotation * scale).into(),
            normals: normals.into(),
        }
    }
}

impl Default for Instance {
    fn default() -> Self {
        Self {
            position: Vector3::zero(),
            rotation: Quaternion::one(),
            scale: Vector3::new(1.0, 1.0, 1.0),
        }
    }
}

pub trait DrawLight<'a> {
    fn draw_light_model(
        &mut self,
        model: &'a Model,
        camera_bind_group: &'a wgpu::BindGroup,
        light_bind_group: &'a wgpu::BindGroup,
    );
}

impl<'a, 'b> DrawLight<'b> for wgpu::RenderPass<'a>
where
    'b: 'a,
{
    fn draw_light_model(
        &mut self,
        model: &'a Model,
        camera_bind_group: &'b wgpu::BindGroup,
        light_bind_group: &'b wgpu::BindGroup,
    ) {
        for mesh in &model.meshes {
            let material: &material::Material = &model.materials[mesh.material];
            self.set_vertex_buffer(0, mesh.vertex_buffer.slice(..));
            self.set_index_buffer(mesh.index_buffer.slice(..), wgpu::IndexFormat::Uint32);
            self.set_bind_group(0, camera_bind_group, &[]);
            self.set_bind_group(1, light_bind_group, &[]);
            self.set_bind_group(2, &material.bind_group, &[]);
            self.draw_indexed(0..mesh.indices, 0, 0..1);
        }
    }
}

pub trait DrawShadow<'a> {
    fn draw_shadow_model(&mut self, model: &'a Model, light_bind_group: &'a wgpu::BindGroup);
}

impl<'a, 'b> DrawShadow<'b> for wgpu::RenderPass<'a>
where
    'b: 'a,
{
    fn draw_shadow_model(&mut self, model: &'a Model, light_bind_group: &'b wgpu::BindGroup) {
        for mesh in &model.meshes {
            self.set_vertex_buffer(0, mesh.vertex_buffer.slice(..));
            self.set_vertex_buffer(1, model.instance_buffer.slice(..));
            self.set_index_buffer(mesh.index_buffer.slice(..), wgpu::IndexFormat::Uint32);
            self.set_bind_group(0, light_bind_group, &[]);
            self.draw_indexed(0..mesh.indices, 0, 0..model.instances.len() as u32);
        }
    }
}

pub struct Model {
    pub meshes: Vec<structs::Mesh>,
    pub materials: Vec<material::Material>,
    pub instances: Vec<Instance>,
    pub instance_buffer: wgpu::Buffer,
}

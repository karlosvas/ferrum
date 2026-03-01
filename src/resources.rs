use std::{
    io::{BufReader, Cursor},
    path::PathBuf,
};

use crate::{
    structs::{self, Model, ModelVertex},
    texture::{self, Texture},
};
use anyhow::Ok;
use wgpu::{Buffer, util::DeviceExt};

pub async fn load_string(file_name: &str) -> anyhow::Result<String> {
    let text: String = {
        let path: PathBuf = std::path::Path::new(env!("OUT_DIR"))
            .join("res")
            .join(file_name);

        std::fs::read_to_string(path)?
    };

    Ok(text)
}

pub async fn load_binary(file_name: &str) -> anyhow::Result<Vec<u8>> {
    let data = {
        let path: PathBuf = std::path::Path::new(env!("OUT_DIR"))
            .join("res")
            .join(file_name);

        std::fs::read(path)?
    };

    Ok(data)
}

pub async fn load_texture(
    file_name: &str,
    device: &wgpu::Device,
    queue: &wgpu::Queue,
) -> anyhow::Result<texture::Texture> {
    let data: Vec<u8> = load_binary(file_name).await?;
    texture::Texture::from_bytes(device, queue, &data, file_name)
}

pub async fn load_model(
    file_name: &str,
    device: &wgpu::Device,
    queue: &wgpu::Queue,
    layout: &wgpu::BindGroupLayout,
) -> anyhow::Result<structs::Model> {
    let obj_text: String = load_string(file_name).await?;
    let obj_cursor: Cursor<String> = Cursor::new(obj_text);
    let mut obj_reder: BufReader<_> = BufReader::new(obj_cursor);

    let (models, obj_materials): (
        Vec<tobj::Model>,
        Result<Vec<tobj::Material>, tobj::LoadError>,
    ) = tobj::load_obj_buf_async(
        &mut obj_reder,
        &tobj::LoadOptions {
            triangulate: true,
            single_index: true,
            ..Default::default()
        },
        |mtl_path| async move {
            println!("Buscando mtl: {}", mtl_path);
            let mat_text: String = load_string(&mtl_path).await.unwrap();
            tobj::load_mtl_buf(&mut BufReader::new(Cursor::new(mat_text)))
        },
    )
    .await?;

    let mut materials: Vec<structs::Material> = Vec::new();
    for m in obj_materials? {
        let diffuse_texture: Texture = match load_texture(&m.diffuse_texture, device, queue).await {
            anyhow::Result::Ok(t) => t,
            Err((e)) => {
                println!("WARN: No se ha podido caragr la textura {}, {}", m.name, e);
                panic!()
            }
        };
        let bind_group = device.create_bind_group(&wgpu::BindGroupDescriptor {
            layout,
            entries: &[
                wgpu::BindGroupEntry {
                    binding: 0,
                    resource: wgpu::BindingResource::TextureView(&diffuse_texture.view),
                },
                wgpu::BindGroupEntry {
                    binding: 1,
                    resource: wgpu::BindingResource::Sampler(&diffuse_texture.sampler),
                },
            ],
            label: None,
        });

        materials.push(structs::Material {
            name: m.name,
            diffuse_texture,
            bind_group,
        });
    }

    let meshes: Vec<structs::Mesh> = models
        .into_iter()
        .map(|m| {
            let vertices = (0..m.mesh.positions.len() / 3)
                .map(|i| {
                    if m.mesh.normals.is_empty() {
                        ModelVertex {
                            position: [
                                m.mesh.positions[i * 3],
                                m.mesh.positions[i * 3 + 1],
                                m.mesh.positions[i * 3 + 2],
                            ],
                            text_cords: [
                                m.mesh.texcoords[i * 2],
                                1.0 - m.mesh.texcoords[i * 2 + 1],
                            ],
                            normal: [0.0, 0.0, 0.0],
                        }
                    } else {
                        ModelVertex {
                            position: [
                                m.mesh.positions[i * 3],
                                m.mesh.positions[i * 3 + 1],
                                m.mesh.positions[i * 3 + 2],
                            ],
                            text_cords: [
                                m.mesh.texcoords[i * 2],
                                1.0 - m.mesh.texcoords[i * 2 + 1],
                            ],
                            normal: [
                                m.mesh.positions[i * 3],
                                m.mesh.positions[i * 3 + 1],
                                m.mesh.positions[i * 3 + 2],
                            ],
                        }
                    }
                })
                .collect::<Vec<_>>();

            // DEBUG
            println!("Num vertices: {}", vertices.len());
            println!("Num indices: {}", m.mesh.indices.len());
            println!("Max index: {}", m.mesh.indices.iter().max().unwrap());
            if let Some(v) = vertices.first() {
                println!("Primer vertice: {:?}", v.position);
            }
            if let Some(v) = vertices.last() {
                println!("Ultimo vertice: {:?}", v.position);
            }

            let vertex_buffer: Buffer =
                device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
                    label: Some(&format!("{:?} Vertex Buffer", file_name)),
                    contents: bytemuck::cast_slice(&vertices),
                    usage: wgpu::BufferUsages::VERTEX,
                });

            let index_buffer: Buffer =
                device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
                    label: Some(&format!("{:?} Index Buffer", file_name)),
                    contents: bytemuck::cast_slice(&m.mesh.indices),
                    usage: wgpu::BufferUsages::INDEX,
                });

            structs::Mesh {
                name: file_name.to_string(),
                vertex_buffer,
                index_buffer,
                material: m.mesh.material_id.unwrap_or(0),
                indices: m.mesh.indices.len() as u32,
            }
        })
        .collect::<Vec<_>>();

    println!("Num meshes: {}", meshes.len());
    println!("Num materials: {}", materials.len());
    for mesh in &meshes {
        println!("Mesh: {} material_id: {}", mesh.name, mesh.material);
    }
    Ok(Model { meshes, materials })
}

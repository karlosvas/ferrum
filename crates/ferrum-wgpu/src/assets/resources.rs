use crate::{
    assets::{self, TypeModel},
    renderer,
};
use {
    cgmath::{Vector2, Vector3},
    std::io::{BufReader, Cursor},
    wgpu::{BindGroup, Buffer, util::DeviceExt},
};

#[cfg(not(target_arch = "wasm32"))]
use std::path::{Path, PathBuf};

#[derive(Clone)]
pub struct Asset {
    pub out_dir: String,
}

impl Default for Asset {
    fn default() -> Self {
        Self {
            out_dir: "res".to_string(),
        }
    }
}

impl Asset {
    pub fn new(out_dir: String) -> Self {
        Self { out_dir }
    }
}

pub trait AssetSource: Send + Sync {
    #[cfg(target_arch = "wasm32")]
    fn format_url(&self, file_name: &str) -> reqwest::Url;
    #[cfg(not(target_arch = "wasm32"))]
    fn resolve_resource_path(&self, file_name: &str) -> anyhow::Result<PathBuf>;
    fn load_string(&self, file_name: &str) -> impl Future<Output = anyhow::Result<String>> + Send;
    fn load_binary(&self, file_name: &str) -> impl Future<Output = anyhow::Result<Vec<u8>>> + Send;
}

impl AssetSource for Asset {
    #[cfg(target_arch = "wasm32")]
    fn format_url(&self, file_name: &str) -> reqwest::Url {
        let base = reqwest::Url::parse(self.out_dir).unwrap();
        base.join(file_name).unwrap()
    }

    #[cfg(not(target_arch = "wasm32"))]
    fn resolve_resource_path(&self, file_name: &str) -> anyhow::Result<PathBuf> {
        let out_dir_path = Path::new(self.out_dir.as_str()).join(file_name);
        if out_dir_path.is_file() {
            return Ok(out_dir_path);
        }

        if let Ok(exe) = std::env::current_exe()
            && let Some(exe_dir) = exe.parent()
        {
            let exe_path = exe_dir.join("res").join(file_name);
            if exe_path.is_file() {
                return Ok(exe_path);
            }
        }

        anyhow::bail!("Recurso no encontrado: '{file_name}'")
    }

    async fn load_string(&self, file_name: &str) -> anyhow::Result<String> {
        log::debug!("Open: {:?}", file_name);

        #[cfg(target_arch = "wasm32")]
        let text = {
            let url = self.format_url(file_name);
            log::debug!("[load_string] GET {}", url);
            let resp = reqwest::get(url.clone()).await.map_err(|e| {
                log::error!("[load_string] fetch failed for '{}': {}", url, e);
                e
            })?;
            if !resp.status().is_success() {
                log::error!("[load_string] HTTP {} for '{}'", resp.status(), url);
                anyhow::bail!("HTTP {} fetching '{}'", resp.status(), url);
            }
            resp.text().await?
        };

        #[cfg(not(target_arch = "wasm32"))]
        let text: String = {
            let path: PathBuf = self.resolve_resource_path(file_name).map_err(|e| {
                log::error!("[load_string] resource not found '{}': {}", file_name, e);
                e
            })?;

            log::debug!("[load_string] resolved '{}' -> {:?}", file_name, path);
            std::fs::read_to_string(path)?
        };

        Ok(text)
    }

    async fn load_binary(&self, file_name: &str) -> anyhow::Result<Vec<u8>> {
        #[cfg(target_arch = "wasm32")]
        let data = {
            let url = self.format_url(file_name);
            log::debug!("[load_binary] GET {}", url);
            let resp = reqwest::get(url.clone()).await.map_err(|e| {
                log::error!("[load_binary] fetch failed for '{}': {}", url, e);
                e
            })?;
            if !resp.status().is_success() {
                log::error!("[load_binary] HTTP {} for '{}'", resp.status(), url);
                anyhow::bail!("HTTP {} fetching '{}'", resp.status(), url);
            }
            resp.bytes().await?.to_vec()
        };

        #[cfg(not(target_arch = "wasm32"))]
        let data = {
            let path: PathBuf = self.resolve_resource_path(file_name).map_err(|e| {
                log::error!("[load_binary] resource not found '{}': {}", file_name, e);
                e
            })?;

            std::fs::read(path)?
        };

        Ok(data)
    }
}

pub fn load_texture(
    data: Vec<u8>,
    file_name: &str,
    device: &wgpu::Device,
    queue: &wgpu::Queue,
    is_normal_map: bool,
) -> anyhow::Result<renderer::Texture> {
    renderer::Texture::from_bytes(device, queue, &data, file_name, is_normal_map)
}

pub async fn load_model(
    asset: &impl AssetSource,
    file_name: &str,
    device: &wgpu::Device,
    queue: &wgpu::Queue,
    layout: &wgpu::BindGroupLayout,
    instances: Vec<assets::Instance>,
    type_model: TypeModel,
) -> anyhow::Result<assets::Model> {
    let model_binary: Vec<u8> = asset.load_binary(file_name).await?;
    let mut obj_reader = BufReader::new(Cursor::new(model_binary));

    let parent_path: String = std::path::Path::new(file_name)
        .parent()
        .and_then(|p| p.to_str())
        .unwrap_or("")
        .to_string();

    let (models, obj_materials): (
        Vec<tobj::Model>,
        Result<Vec<tobj::Material>, tobj::LoadError>,
    ) = tobj::load_obj_buf_async(
        &mut obj_reader,
        &tobj::LoadOptions {
            triangulate: true,
            single_index: true,
            ..Default::default()
        },
        |mtl_path| {
            // diffuse_texture
            let parent_path: String = parent_path.clone();
            async move {
                let full_mtl_path: String = if parent_path.is_empty() {
                    mtl_path.clone()
                } else {
                    std::path::Path::new(&parent_path)
                        .join(&mtl_path)
                        .to_str()
                        .unwrap()
                        .to_string()
                };
                log::debug!("Buscando mtl: {}", full_mtl_path);
                let mat_text: String = asset.load_string(&full_mtl_path).await.unwrap();
                tobj::load_mtl_buf(&mut BufReader::new(Cursor::new(mat_text)))
            }
        },
    )
    .await?;

    let mut materials: Vec<renderer::Material> = Vec::new();
    for m in obj_materials? {
        let full_diffuse_path: String = if parent_path.is_empty() {
            m.diffuse_texture.clone()
        } else {
            std::path::Path::new(&parent_path)
                .join(&m.diffuse_texture)
                .to_str()
                .unwrap()
                .to_string()
        };

        let diffuse_binary = asset.load_binary(&full_diffuse_path).await?;
        let diffuse_texture: renderer::Texture =
            load_texture(diffuse_binary, &full_diffuse_path, device, queue, false)?;

        let normal_texture: renderer::Texture = if m.normal_texture.is_empty() {
            renderer::Texture::default_normal(device, queue)
        } else {
            let full_normal_path: String = if parent_path.is_empty() {
                m.normal_texture.clone()
            } else {
                std::path::Path::new(&parent_path)
                    .join(&m.normal_texture)
                    .to_str()
                    .unwrap()
                    .to_string()
            };
            let texture_binary: Vec<u8> = asset.load_binary(&full_normal_path).await?;
            load_texture(texture_binary, &full_normal_path, device, queue, true)?
        };

        let bind_group: BindGroup = device.create_bind_group(&wgpu::BindGroupDescriptor {
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
                wgpu::BindGroupEntry {
                    binding: 2,
                    resource: wgpu::BindingResource::TextureView(&normal_texture.view),
                },
                wgpu::BindGroupEntry {
                    binding: 3,
                    resource: wgpu::BindingResource::Sampler(&normal_texture.sampler),
                },
            ],
            label: None,
        });

        materials.push(renderer::Material {
            name: m.name,
            diffuse_texture,
            normal_texture,
            bind_group,
        });
    }

    let meshes: Vec<renderer::Mesh> = models
        .into_iter()
        .map(|m| {
            let has_colors: bool = !m.mesh.vertex_color.is_empty();
            let has_normals: bool = !m.mesh.normals.is_empty();
            let has_texcoords: bool = !m.mesh.texcoords.is_empty();
            let has_normal_indices: bool = !m.mesh.normal_indices.is_empty();

            let mut vertices: Vec<assets::ModelVertex> = (0..m.mesh.positions.len() / 3)
                .map(|i| {
                    let pi = i * 3;

                    let position = [
                        m.mesh.positions[pi],
                        m.mesh.positions[pi + 1],
                        m.mesh.positions[pi + 2],
                    ];

                    let text_cords = if has_texcoords {
                        [m.mesh.texcoords[i * 2], 1.0 - m.mesh.texcoords[i * 2 + 1]]
                    } else {
                        [0.0, 0.0]
                    };

                    let normal = if has_normals {
                        let ni = if has_normal_indices {
                            m.mesh.normal_indices[i] as usize * 3
                        } else {
                            pi
                        };
                        [
                            m.mesh.normals[ni],
                            m.mesh.normals[ni + 1],
                            m.mesh.normals[ni + 2],
                        ]
                    } else {
                        [0.0, 0.0, 0.0]
                    };

                    let color = if has_colors {
                        [
                            m.mesh.vertex_color[pi],
                            m.mesh.vertex_color[pi + 1],
                            m.mesh.vertex_color[pi + 2],
                        ]
                    } else {
                        [1.0, 1.0, 1.0]
                    };

                    let tangent: [f32; 3] = [0.0, 0.0, 0.0];

                    let bitangent: [f32; 3] = [0.0, 0.0, 0.0];

                    assets::ModelVertex {
                        position,
                        text_cords,
                        normal,
                        color,
                        tangent,
                        bitangent,
                    }
                })
                .collect::<Vec<_>>();

            let indices: &Vec<u32> = &m.mesh.indices;
            let mut trangles_included = vec![0; vertices.len()];

            for c in indices.chunks(3) {
                let v0: assets::ModelVertex = vertices[c[0] as usize];
                let v1: assets::ModelVertex = vertices[c[1] as usize];
                let v2: assets::ModelVertex = vertices[c[2] as usize];

                let pos0: Vector3<f32> = v0.position.into();
                let pos1: Vector3<f32> = v1.position.into();
                let pos2: Vector3<f32> = v2.position.into();

                let uv0: Vector2<f32> = v0.text_cords.into();
                let uv1: Vector2<f32> = v1.text_cords.into();
                let uv2: Vector2<f32> = v2.text_cords.into();

                // Calculate the edges of the triangle
                let delta_pos1: Vector3<f32> = pos1 - pos0;
                let delta_pos2: Vector3<f32> = pos2 - pos0;

                // This will give us a direction to calculate the
                // tangent and bitangent
                let delta_uv1: Vector2<f32> = uv1 - uv0;
                let delta_uv2: Vector2<f32> = uv2 - uv0;

                // Solving the following system of equations will
                // give us the tangent and bitangent.
                //     delta_pos1 = delta_uv1.x * T + delta_u.y * B
                //     delta_pos2 = delta_uv2.x * T + delta_uv2.y * B

                let r: f32 = 1.0 / (delta_uv1.x * delta_uv2.y - delta_uv1.y * delta_uv2.x);
                let tanget: Vector3<f32> =
                    (delta_pos1 * delta_uv2.y - delta_pos2 * delta_uv1.y) * r;
                let bitanget: Vector3<f32> =
                    (delta_pos2 * delta_uv1.x - delta_pos1 * delta_uv2.x) * -r;

                // Tangent
                vertices[c[0] as usize].tangent =
                    (tanget + Vector3::from(vertices[c[0] as usize].tangent)).into();
                vertices[c[1] as usize].tangent =
                    (tanget + Vector3::from(vertices[c[1] as usize].tangent)).into();
                vertices[c[2] as usize].tangent =
                    (tanget + Vector3::from(vertices[c[2] as usize].tangent)).into();

                // Bitangent
                vertices[c[0] as usize].bitangent =
                    (bitanget + Vector3::from(vertices[c[0] as usize].bitangent)).into();
                vertices[c[1] as usize].bitangent =
                    (bitanget + Vector3::from(vertices[c[1] as usize].bitangent)).into();
                vertices[c[2] as usize].bitangent =
                    (bitanget + Vector3::from(vertices[c[2] as usize].bitangent)).into();

                // Used to average the tangents/bitangents
                trangles_included[c[0] as usize] += 1;
                trangles_included[c[1] as usize] += 1;
                trangles_included[c[2] as usize] += 1;
            }

            // Average the tangents/bitangents
            for (i, n) in trangles_included.into_iter().enumerate() {
                let denom: f32 = 1.0 / n as f32;
                let v: &mut assets::ModelVertex = &mut vertices[i];
                v.tangent = (Vector3::from(v.tangent) * denom).into();
                v.bitangent = (Vector3::from(v.bitangent) * denom).into();
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

            renderer::Mesh {
                name: file_name.to_string(),
                vertex_buffer,
                index_buffer,
                material: m.mesh.material_id.unwrap_or(0),
                indices: m.mesh.indices.len() as u32,
            }
        })
        .collect::<Vec<_>>();

    let instances_raws: Vec<assets::InstanceRaw> = instances.iter().map(|i| i.to_raw()).collect();

    let label: String = format!("{file_name}_instance_buffer");
    let instance_buffer: Buffer = device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
        label: Some(&label),
        contents: bytemuck::cast_slice(&instances_raws),
        usage: wgpu::BufferUsages::VERTEX | wgpu::BufferUsages::COPY_DST,
    });

    Ok(assets::Model {
        meshes,
        materials,
        instances,
        instance_buffer,
        type_model,
    })
}

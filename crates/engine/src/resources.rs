use std::{
    io::{BufReader, Cursor},
    path::PathBuf,
};

use crate::{
    material,
    models::ModelVertex,
    structs::{self, Model},
    texture::{self, CubeTexture, Texture},
};
use anyhow::Ok;
use cgmath::{Vector2, Vector3};
use wgpu::{
    BindGroup, Buffer, CommandEncoder, ComputePass, ComputePipeline, PipelineLayout, ShaderModule,
    TextureFormat, TextureView, util::DeviceExt,
};

#[cfg(target_arch = "wasm32")]
fn format_url(file_name: &str) -> reqwest::Url {
    let base = reqwest::Url::parse("https://pub-72c0c8dd15e249a0a448095fb52cc05c.r2.dev/").unwrap();
    base.join(file_name).unwrap()
}

pub async fn load_string(file_name: &str) -> anyhow::Result<String> {
    log::debug!("Intentando abrir: {:?}", file_name);

    #[cfg(target_arch = "wasm32")]
    let text = {
        let url = format_url(file_name);
        reqwest::get(url).await?.text().await?
    };

    #[cfg(not(target_arch = "wasm32"))]
    let text: String = {
        let path: PathBuf = std::path::Path::new(env!("OUT_DIR"))
            .join("res")
            .join(file_name);

        log::debug!("Ruta absoluta resuelta: {:?}", path);
        std::fs::read_to_string(path)?
    };

    Ok(text)
}

pub async fn load_binary(file_name: &str) -> anyhow::Result<Vec<u8>> {
    #[cfg(target_arch = "wasm32")]
    let data = {
        let url = format_url(file_name);
        reqwest::get(url).await?.bytes().await?.to_vec()
    };

    #[cfg(not(target_arch = "wasm32"))]
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

    let parent_path: String = std::path::Path::new(file_name)
        .parent()
        .and_then(|p| p.to_str())
        .unwrap_or("")
        .to_string();

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
        |mtl_path| {
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
                let mat_text: String = load_string(&full_mtl_path).await.unwrap();
                tobj::load_mtl_buf(&mut BufReader::new(Cursor::new(mat_text)))
            }
        },
    )
    .await?;

    let mut materials: Vec<material::Material> = Vec::new();
    for m in obj_materials? {
        let full_mtl_path: String = if parent_path.is_empty() {
            m.diffuse_texture.clone()
        } else {
            std::path::Path::new(&parent_path)
                .join(&m.diffuse_texture)
                .to_str()
                .unwrap()
                .to_string()
        };

        let full_normal_path: String = if m.normal_texture.is_empty() {
            full_mtl_path.clone()
        } else if parent_path.is_empty() {
            m.normal_texture.clone()
        } else {
            std::path::Path::new(&parent_path)
                .join(&m.normal_texture)
                .to_str()
                .unwrap()
                .to_string()
        };

        let diffuse_texture: Texture = load_texture(&full_mtl_path, device, queue).await?;
        let normal_texture: Texture = load_texture(&full_normal_path, device, queue).await?;

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

        materials.push(material::Material {
            name: m.name,
            diffuse_texture,
            normal_texture,
            bind_group,
        });
    }

    let meshes: Vec<structs::Mesh> = models
        .into_iter()
        .map(|m| {
            let has_colors = !m.mesh.vertex_color.is_empty();
            let has_normals = !m.mesh.normals.is_empty();
            let has_texcoords = !m.mesh.texcoords.is_empty();
            let has_normal_indices = !m.mesh.normal_indices.is_empty();

            let mut vertices = (0..m.mesh.positions.len() / 3)
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

                    let tangent = [0.0, 0.0, 0.0];

                    let bitangent = [0.0, 0.0, 0.0];

                    ModelVertex {
                        position,
                        text_cords,
                        normal,
                        color,
                        tangent,
                        bitangent,
                    }
                })
                .collect::<Vec<_>>();

            log::debug!("Num vertices: {}", vertices.len());
            log::debug!("Num indices: {}", m.mesh.indices.len());
            log::debug!("Max index: {}", m.mesh.indices.iter().max().unwrap());

            let indices = &m.mesh.indices;
            let mut trangles_included = vec![0; vertices.len()];

            for c in indices.chunks(3) {
                let v0: ModelVertex = vertices[c[0] as usize];
                let v1: ModelVertex = vertices[c[1] as usize];
                let v2: ModelVertex = vertices[c[2] as usize];

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
                let v: &mut ModelVertex = &mut vertices[i];
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

            structs::Mesh {
                name: file_name.to_string(),
                vertex_buffer,
                index_buffer,
                material: m.mesh.material_id.unwrap_or(0),
                indices: m.mesh.indices.len() as u32,
            }
        })
        .collect::<Vec<_>>();

    log::debug!("Num meshes: {}", meshes.len());
    log::debug!("Num materials: {}", materials.len());
    for mesh in &meshes {
        log::debug!("Mesh: {} material_id: {}", mesh.name, mesh.material);
    }
    Ok(Model { meshes, materials })
}

pub struct HdrLoader {
    texture_format: wgpu::TextureFormat,
    equirect_layout: wgpu::BindGroupLayout,
    equirect_to_cubemap: wgpu::ComputePipeline,
}

impl HdrLoader {
    pub fn new(device: &wgpu::Device) -> Self {
        let module: ShaderModule =
            device.create_shader_module(wgpu::include_wgsl!("shaders/equirectangular.wgsl"));
        let texture_format: TextureFormat = wgpu::TextureFormat::Rgba32Float;
        let equirect_layout = device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
            label: Some("HdrLoader::equirect_layout"),
            entries: &[
                wgpu::BindGroupLayoutEntry {
                    binding: 0,
                    visibility: wgpu::ShaderStages::COMPUTE,
                    ty: wgpu::BindingType::Texture {
                        sample_type: wgpu::TextureSampleType::Float { filterable: false },
                        view_dimension: wgpu::TextureViewDimension::D2,
                        multisampled: false,
                    },
                    count: None,
                },
                wgpu::BindGroupLayoutEntry {
                    binding: 1,
                    visibility: wgpu::ShaderStages::COMPUTE,
                    ty: wgpu::BindingType::StorageTexture {
                        access: wgpu::StorageTextureAccess::WriteOnly,
                        format: texture_format,
                        view_dimension: wgpu::TextureViewDimension::D2Array,
                    },
                    count: None,
                },
            ],
        });

        let pipeline_layout: PipelineLayout =
            device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
                label: Some("Cubemap pipeline_layout"),
                bind_group_layouts: &[&equirect_layout],
                immediate_size: 0,
            });

        let equirect_to_cubemap: ComputePipeline =
            device.create_compute_pipeline(&wgpu::ComputePipelineDescriptor {
                label: Some("equirect_to_cubemap"),
                layout: Some(&pipeline_layout),
                module: &module,
                entry_point: Some("cmompute_equirect_to_cubemap"),
                compilation_options: Default::default(),
                cache: None,
            });

        Self {
            equirect_to_cubemap,
            texture_format,
            equirect_layout,
        }
    }

    pub fn from_equirectangular_bytes(
        &self,
        device: &wgpu::Device,
        queue: &wgpu::Queue,
        data: &[u8],
        dst_size: u32,
        label: Option<&str>,
    ) -> anyhow::Result<texture::CubeTexture> {
        let hdr_decoder = HdrDecoder::new(Cursor::new(data))?;
        let meta = hdr_decoder.metadata();

        #[cfg(not(target_arch = "wasm32"))]
        let pixels: Vec<[f64; 4]> = {
            let mut pixels: Vec<[f64; 4]> =
                vec![[0.0, 0.0, 0.0, 0.0]; meta.width as usize * meta.height as usize];
            hdr_decoder.read_image_transform(
                |pix| {
                    let rgb = pix.to_hdr();
                    [rgb.0[0], rgb.0[1], rgb.0[2], 1.0f32]
                },
                &mut pixels[..],
            )?;
            pixels
        };
        #[cfg(target_arch = "wasm32")]
        let pixels = hdr_decoder
            .read_image_native()?
            .into_iter()
            .map(|pix| {
                let rgb = pix.to_hdr();
                [rgb.0[0], rgb.0[1], rgb.0[2], 1.0f32]
            })
            .collect::<Vec<_>>();

        let src = texture::Texture::create_2d_texture(
            device,
            meta.width,
            meta.height,
            self.texture_format,
            wgpu::TextureUsages::TEXTURE_BINDING | wgpu::TextureUsages::COPY_DST,
            wgpu::FilterMode::Linear,
            None,
        );

        queue.write_texture(
            wgpu::TexelCopyTextureInfo {
                texture: &src.texture,
                mip_level: 0,
                origin: wgpu::Origin3d::ZERO,
                aspect: wgpu::TextureAspect::All,
            },
            &bytemuck::cast_slice(&pixels),
            wgpu::TexelCopyBufferLayout {
                offset: 0,
                bytes_per_row: Some(src.size.width * std::mem::size_of::<[f32; 4]>() as u32),
                rows_per_image: Some(src.size.height),
            },
            src.size,
        );

        let dst: CubeTexture = texture::CubeTexture::create_2d(
            device,
            dst_size,
            dst_size,
            self.texture_format,
            1,
            wgpu::TextureUsages::STORAGE_BINDING | wgpu::TextureUsages::TEXTURE_BINDING,
            wgpu::FilterMode::Nearest,
            label,
        );

        let dst_view: TextureView = dst.texture().create_view(&wgpu::TextureViewDescriptor {
            label,
            dimension: Some(wgpu::TextureViewDimension::D2Array),
            ..Default::default()
        });

        let bind_group: BindGroup = device.create_bind_group(&wgpu::BindGroupDescriptor {
            label,
            layout: &self.equirect_layout,
            entries: &[
                wgpu::BindGroupEntry {
                    binding: 0,
                    resource: wgpu::BindingResource::TextureView(&src.view),
                },
                wgpu::BindGroupEntry {
                    binding: 1,
                    resource: wgpu::BindingResource::TextureView(&dst_view),
                },
            ],
        });

        let mut encoder: CommandEncoder = device.create_command_encoder(&Default::default());
        let mut pass: ComputePass = encoder.begin_compute_pass(&wgpu::ComputePassDescriptor {
            label,
            timestamp_writes,
        });

        let num_workgroups: u32 = (dst_size + 15) / 16;
        pass.set_pipeline(&self.equirect_to_cubemap);
        pass.set_bind_group(0, &bind_group, &[]);
        pass.dispatch_workgroups(num_workgroups, num_workgroups, 6);

        drop(pass);

        queue.submit([encoder.finish()]);

        Ok(dst)
    }
}

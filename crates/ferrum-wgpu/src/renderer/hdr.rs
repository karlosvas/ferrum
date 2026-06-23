use crate::renderer::{self, Texture, texture};
use image::{DynamicImage, ImageDecoder, codecs::hdr::HdrDecoder, codecs::openexr::OpenExrDecoder};
use wgpu::{
    BindGroup, BindGroupLayout, CommandEncoder, ComputePass, ComputePipeline, Operations,
    PipelineLayout, RenderPass, RenderPipeline, ShaderModule, ShaderModuleDescriptor,
    TextureFormat,
};

/// Skybox: HDR pipeline (tonemapping), environment cubemap and the render
/// pipeline that paints the sky where no geometry was drawn.
pub struct SkyRig {
    pub texture: texture::CubeTexture,
    pub bind_group: BindGroup,
    pub pipeline: RenderPipeline,
}

impl SkyRig {
    pub async fn new(
        device: &wgpu::Device,
        queue: &wgpu::Queue,
        _config: &wgpu::SurfaceConfiguration,
        camera_layout: &BindGroupLayout,
        format: wgpu::TextureFormat,
        bytes: &[u8],
        sky_format: SkyFormat,
    ) -> anyhow::Result<Self> {
        let hdr_loader: HdrLoader = HdrLoader::new(device);

        let sky_texture: texture::CubeTexture = hdr_loader.load_equirectangular_bytes(
            device,
            queue,
            &bytes,
            sky_format,
            None,
            Some("sky_texture"),
        )?;

        let environment_layout: BindGroupLayout =
            device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
                label: Some("environment_layout"),
                entries: &[
                    wgpu::BindGroupLayoutEntry {
                        binding: 0,
                        visibility: wgpu::ShaderStages::FRAGMENT,
                        ty: wgpu::BindingType::Texture {
                            sample_type: wgpu::TextureSampleType::Float { filterable: true },
                            view_dimension: wgpu::TextureViewDimension::Cube,
                            multisampled: false,
                        },
                        count: None,
                    },
                    wgpu::BindGroupLayoutEntry {
                        binding: 1,
                        visibility: wgpu::ShaderStages::FRAGMENT,
                        ty: wgpu::BindingType::Sampler(wgpu::SamplerBindingType::Filtering),
                        count: None,
                    },
                ],
            });

        let bind_group: BindGroup = device.create_bind_group(&wgpu::BindGroupDescriptor {
            label: Some("environment_bind_group"),
            layout: &environment_layout,
            entries: &[
                wgpu::BindGroupEntry {
                    binding: 0,
                    resource: wgpu::BindingResource::TextureView(sky_texture.view()),
                },
                wgpu::BindGroupEntry {
                    binding: 1,
                    resource: wgpu::BindingResource::Sampler(sky_texture.sampler()),
                },
            ],
        });

        let layout: PipelineLayout =
            device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
                label: Some("Sky Pipeline Layout"),
                bind_group_layouts: &[Some(camera_layout), Some(&environment_layout)],
                immediate_size: 0,
            });

        // LessEqual with the fullscreen triangle at z=1.0: the sky only wins on
        // pixels the geometry left untouched.
        let pipeline: RenderPipeline = renderer::create_render_pipeline(
            device,
            &layout,
            format,
            Some(Texture::DEPTH_FORMAT),
            &[],
            wgpu::PrimitiveTopology::TriangleList,
            wgpu::include_wgsl!("../shaders/sky.wgsl"),
            wgpu::CompareFunction::LessEqual,
        );

        Ok(Self {
            texture: sky_texture,
            bind_group,
            pipeline,
        })
    }
}

/// Equirectangular sky file format.
#[derive(Debug, Clone, Copy)]
pub enum SkyFormat {
    Hdr,
    Exr,
}

pub struct HdrPipeline {
    pipeline: wgpu::RenderPipeline,
    bind_group: wgpu::BindGroup,
    texture: renderer::Texture,
    width: u32,
    heigth: u32,
    format: wgpu::TextureFormat,
    layout: wgpu::BindGroupLayout,
}

impl HdrPipeline {
    pub fn new(device: &wgpu::Device, config: &wgpu::SurfaceConfiguration) -> Self {
        let width: u32 = config.width;
        let heigth: u32 = config.height;

        let format: TextureFormat = wgpu::TextureFormat::Rgba16Float;

        let texture: texture::Texture = texture::Texture::create_2d_texture(
            device,
            width,
            heigth,
            format,
            wgpu::TextureUsages::TEXTURE_BINDING | wgpu::TextureUsages::RENDER_ATTACHMENT,
            wgpu::FilterMode::Nearest,
            Some("Hdr::texture"),
        );

        let layout: BindGroupLayout =
            device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
                label: Some("Hdr::layout"),
                entries: &[
                    wgpu::BindGroupLayoutEntry {
                        binding: 0,
                        visibility: wgpu::ShaderStages::FRAGMENT,
                        ty: wgpu::BindingType::Texture {
                            sample_type: wgpu::TextureSampleType::Float { filterable: true },
                            view_dimension: wgpu::TextureViewDimension::D2,
                            multisampled: false,
                        },
                        count: None,
                    },
                    wgpu::BindGroupLayoutEntry {
                        binding: 1,
                        visibility: wgpu::ShaderStages::FRAGMENT,
                        ty: wgpu::BindingType::Sampler(wgpu::SamplerBindingType::Filtering),
                        count: None,
                    },
                ],
            });

        let bind_group: BindGroup = device.create_bind_group(&wgpu::BindGroupDescriptor {
            label: Some("Hdr::bind_group"),
            layout: &layout,
            entries: &[
                wgpu::BindGroupEntry {
                    binding: 0,
                    resource: wgpu::BindingResource::TextureView(&texture.view),
                },
                wgpu::BindGroupEntry {
                    binding: 1,
                    resource: wgpu::BindingResource::Sampler(&texture.sampler),
                },
            ],
        });

        let shader: ShaderModuleDescriptor = wgpu::include_wgsl!("../shaders/hdr.wgsl");
        let pipeline_layout: PipelineLayout =
            device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
                label: Some("Hdr::pipeline_layout"),
                bind_group_layouts: &[Some(&layout)],
                immediate_size: 0,
            });

        let pipeline: RenderPipeline = renderer::create_render_pipeline(
            device,
            &pipeline_layout,
            config.format.add_srgb_suffix(),
            None,
            &[],
            wgpu::PrimitiveTopology::TriangleList,
            shader,
            wgpu::CompareFunction::LessEqual,
        );

        Self {
            pipeline,
            bind_group,
            layout,
            texture,
            width,
            heigth,
            format,
        }
    }

    pub fn resize(&mut self, device: &wgpu::Device, width: u32, height: u32) {
        self.texture = texture::Texture::create_2d_texture(
            device,
            width,
            height,
            wgpu::TextureFormat::Rgba16Float,
            wgpu::TextureUsages::TEXTURE_BINDING | wgpu::TextureUsages::RENDER_ATTACHMENT,
            wgpu::FilterMode::Nearest,
            Some("Hdr::texture"),
        );

        self.bind_group = device.create_bind_group(&wgpu::BindGroupDescriptor {
            label: Some("Hrd::bind_group"),
            layout: &self.layout,
            entries: &[
                wgpu::BindGroupEntry {
                    binding: 0,
                    resource: wgpu::BindingResource::TextureView(&self.texture.view),
                },
                wgpu::BindGroupEntry {
                    binding: 1,
                    resource: wgpu::BindingResource::Sampler(&self.texture.sampler),
                },
            ],
        });

        self.width = width;
        self.heigth = height;
    }

    pub fn view(&self) -> &wgpu::TextureView {
        &self.texture.view
    }

    pub fn format(&self) -> wgpu::TextureFormat {
        self.format
    }

    pub fn process(&self, encoder: &mut wgpu::CommandEncoder, ouput: &wgpu::TextureView) {
        let mut pass: RenderPass = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
            label: Some("Hdr::process"),
            color_attachments: &[Some(wgpu::RenderPassColorAttachment {
                view: ouput,
                depth_slice: None,
                resolve_target: None,
                ops: Operations {
                    load: wgpu::LoadOp::Load,
                    store: wgpu::StoreOp::Store,
                },
            })],
            depth_stencil_attachment: None,
            timestamp_writes: None,
            occlusion_query_set: None,
            multiview_mask: None,
        });

        pass.set_pipeline(&self.pipeline);
        pass.set_bind_group(0, &self.bind_group, &[]);
        pass.draw(0..3, 0..1);
    }
}

pub struct HdrLoader {
    source_format: wgpu::TextureFormat,
    cube_format: wgpu::TextureFormat,
    equirect_layout: wgpu::BindGroupLayout,
    equirect_to_cubemap: wgpu::ComputePipeline,
}

impl HdrLoader {
    pub fn new(device: &wgpu::Device) -> Self {
        let module: ShaderModule =
            device.create_shader_module(wgpu::include_wgsl!("../shaders/equirectangular.wgsl"));
        // Source equirectangular: 32-bit float because that's what .hdr / .exr give us.
        // Cubemap destination: 16-bit float, filterable on every device without features.
        let source_format: TextureFormat = wgpu::TextureFormat::Rgba32Float;
        let cube_format: TextureFormat = wgpu::TextureFormat::Rgba16Float;
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
                        format: cube_format,
                        view_dimension: wgpu::TextureViewDimension::D2Array,
                    },
                    count: None,
                },
            ],
        });

        let pipeline_layout: PipelineLayout =
            device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
                label: Some("Cubemap pipeline_layout"),
                bind_group_layouts: &[Some(&equirect_layout)],
                immediate_size: 0,
            });

        let equirect_to_cubemap: ComputePipeline =
            device.create_compute_pipeline(&wgpu::ComputePipelineDescriptor {
                label: Some("equirect_to_cubemap"),
                layout: Some(&pipeline_layout),
                module: &module,
                entry_point: Some("compute_equirect_to_cubemap"),
                compilation_options: Default::default(),
                cache: None,
            });

        Self {
            equirect_to_cubemap,
            source_format,
            cube_format,
            equirect_layout,
        }
    }

    pub fn load_equirectangular_bytes(
        &self,
        device: &wgpu::Device,
        queue: &wgpu::Queue,
        data: &[u8],
        format: SkyFormat,
        dst_size: Option<u32>,
        label: Option<&str>,
    ) -> anyhow::Result<texture::CubeTexture> {
        let (pixels, width, height): (Vec<[f32; 4]>, u32, u32) = match format {
            SkyFormat::Hdr => Self::decode_radiance_hdr(data)?,
            SkyFormat::Exr => Self::decode_openexr(data)?,
        };

        let dst_size: u32 = dst_size.unwrap_or_else(|| Self::cube_face_size_for_source(width));

        let src: Texture = texture::Texture::create_2d_texture(
            device,
            width,
            height,
            self.source_format,
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
            bytemuck::cast_slice(&pixels),
            wgpu::TexelCopyBufferLayout {
                offset: 0,
                bytes_per_row: Some(
                    src.texture.size().width * std::mem::size_of::<[f32; 4]>() as u32,
                ),
                rows_per_image: Some(src.texture.size().height),
            },
            src.texture.size(),
        );

        let dst: texture::CubeTexture = texture::CubeTexture::create_2d(
            device,
            dst_size,
            dst_size,
            self.cube_format,
            1,
            wgpu::TextureUsages::STORAGE_BINDING | wgpu::TextureUsages::TEXTURE_BINDING,
            wgpu::FilterMode::Linear,
            label,
        );

        let dst_view: wgpu::TextureView = dst.texture().create_view(&wgpu::TextureViewDescriptor {
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
            timestamp_writes: None,
        });

        let num_workgroups: u32 = dst_size.div_ceil(16);
        pass.set_pipeline(&self.equirect_to_cubemap);
        pass.set_bind_group(0, &bind_group, &[]);
        pass.dispatch_workgroups(num_workgroups, num_workgroups, 6);

        drop(pass);

        queue.submit([encoder.finish()]);

        Ok(dst)
    }

    fn decode_radiance_hdr(data: &[u8]) -> anyhow::Result<(Vec<[f32; 4]>, u32, u32)> {
        let hdr_decoder: HdrDecoder<std::io::Cursor<&[u8]>> =
            HdrDecoder::new(std::io::Cursor::new(data))?;
        let meta = hdr_decoder.metadata();
        let (width, height) = (meta.width, meta.height);

        #[cfg(not(target_arch = "wasm32"))]
        let pixels: Vec<[f32; 4]> = {
            let mut pixels: Vec<[f32; 4]> = vec![[0.0; 4]; width as usize * height as usize];
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
        let pixels: Vec<[f32; 4]> = hdr_decoder
            .read_image_native()?
            .into_iter()
            .map(|pix| {
                let rgb = pix.to_hdr();
                [rgb.0[0], rgb.0[1], rgb.0[2], 1.0f32]
            })
            .collect();

        Ok((pixels, width, height))
    }

    fn decode_openexr(data: &[u8]) -> anyhow::Result<(Vec<[f32; 4]>, u32, u32)> {
        let decoder = OpenExrDecoder::new(std::io::Cursor::new(data))?;
        let (width, height) = decoder.dimensions();
        let dynamic = DynamicImage::from_decoder(decoder)?;
        let rgba = dynamic.into_rgba32f();
        let pixels: Vec<[f32; 4]> = rgba
            .as_raw()
            .chunks_exact(4)
            .map(|c| [c[0], c[1], c[2], c[3]])
            .collect();
        Ok((pixels, width, height))
    }

    fn cube_face_size_for_source(source_width: u32) -> u32 {
        let target: u32 = source_width.max(64) / 6;
        1u32 << (31 - target.leading_zeros())
    }
}

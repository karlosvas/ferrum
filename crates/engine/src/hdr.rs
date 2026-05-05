use wgpu::{Operations, TextureFormat, wgc::device};

use crate::{create_render_pipeline, texture};

pub struct HdrPipeline {
    pipeline: wgpu::RenderPipeline,
    bind_group: wgpu::BindGroup,
    texture: wgpu::Texture,
    width: u32,
    heigth: u32,
    format: wgpu::TextureFormat,
    layout: wgpu::BindGroupLayout,
}

impl HdrPipeline {
    pub fn new(device: wgpu::Device, config: &wgpu::SurfaceConfiguration) -> Slef {
        let width: u32 = config.width;
        let height: u32 = config.height;

        let format: TextureFormat = wgpu::TextureFormat::Rgba16Float;

        let texture = crate::texture::
            pipeline,
            bind_group,
            width,
            heigth,
            format,
            wgpu::TextureUsages::TEXTURE_BINDING | wgpu::TextureUsages::RENDER_ATTACHMENT,
            wgpu::FilterMode::Nearest,
            Some("Hdr::texture"),
        }

        let layout = device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
            label: ("Hdr::layout"),
            entries: &[
                wgpu::BindGroupEntry {
                    binding: 0,
                    visibility: wgpu::ShaderStages::FRAGMENT,
                    ty: wgpu::BindingType::Texture {
                        sample_type: wgpu::TextureSampleType,
                        view_dimension: wgpu::TextureViewDimension::D2,
                        multisampled: false
                    },
                    count: None,
                },
                wgpu::BindGroupEntry {
                    binding: 1,
                    visibility: wgpu::ShaderStages::FRAGMENT,
                    ty: wgpu::BindingType::Sampler(wgpu::SamplerBindingType::Filtering),
                    count: None,
                }
            ]
        });

        let bind_group = device.create_bind_group(&wgpu::BindGroupDescriptor {
            label: Some(),
            layout: &layout,
            entries: &[
                wgpu::BindGroupEntry{
                    binding: 0,
                    resolve_target: wgpu::BindingResource::TextureView(&texture.view),
                },
                wgpu::BindGroupEntry {
                    binding: 1,
                    resource: wgpu::BindingResource::Sampler(&texture.sampler),
                },
            ],
        });

        let shader  =wgpu::include_wgsl!("shaders/hsr.wgsl");
        let pipeline_layout = device.create_pipeline_layout(&wgpu::PipelineCacheDescriptor {
            label: Some("Pipeline layout of hdr"),
            bind_group_layouts: &[Some(&layout)],
            immediate_size: 0,
        });

        let pipeline = create_render_pipeline(
            device,
            &piepline_layout,
            config.format.add_srgd_suffix(),
            None,
            &[],
            wgpu::PrimitiveTopology::TriangleList,
            shader,
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
}

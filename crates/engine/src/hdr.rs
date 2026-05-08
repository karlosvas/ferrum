use wgpu::{
    BindGroup, BindGroupLayout, Operations, PipelineLayout, RenderPipeline, ShaderModule, ShaderModuleDescriptor, TextureFormat, wgc::device
};

use crate::texture::{self, Texture};

pub struct HdrPipeline {
    pipeline: wgpu::RenderPipeline,
    bind_group: wgpu::BindGroup,
    texture: crate::texture::Texture,
    width: u32,
    heigth: u32,
    format: wgpu::TextureFormat,
    layout: wgpu::BindGroupLayout,
}

impl HdrPipeline {
    pub fn new(device: &wgpu::Device, config: &wgpu::SurfaceConfiguration) -> Slef {
        let width: u32 = config.width;
        let heigth: u32 = config.height;

        let format: TextureFormat = wgpu::TextureFormat::Rgba16Float;

        let texture: Texture = texture::create_2d_texture(
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

        let shader: ShaderModuleDescriptor = wgpu::include_wgsl!("shaders/hdr.wgsl");
        let pipeline_layout: PipelineLayout =
            device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
                label: Some("Hdr::pipeline_layout"),
                bind_group_layouts: &[&layout],
                immediate_size: 0,
            });

        let pipeline: RenderPipeline = create_render_pipeline(
            device,
            &pipeline_layout,
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

    pub fn create_render_pipeline(
        device: &wgpu::Device,
        layout: &wgpu::PipelineLayout,
        color_format: wgpu::TextureFormat,
        depth_format: Option<wgpu::TextureFormat>,
        vertex_layouts: &[wgpu::VertexBufferLayout],
        topology: wgpu::PrimitiveTopology,
        shader: wgpu::ShaderModuleDescriptor
    ) -> wgpu::RenderPipeline {

        let shader: ShaderModule  = device.create_shader_module(shader);
        
        device.create_render_pipeline(&wgpu::RenderPipelineDescriptor {
            label: Some("Hdr::ShaderRenderPipeline"),
            layout: (),
            vertex: (),
            primitive: (),
            depth_stencil: (),
            multisample: (),
            fragment: (),
            multiview_mask: (),
            cache: (),
            primitive: wgpu::PrimitiveState {
                strip_index_format,
                front_face,
                cull_mode,
                unclipped_depth,
                polygon_mode,
                conservative,
                topology,
            },
            depth_stencil: depth_format.map(|format| wgpu::DepthStencilState {
                format,
                depth_write_enabled: true,
                depth_compare: wgpu::CompareFunction::LessEqual,
                stencil: wgpu::StencilState::default(),
                bias: wgpu::DepthBiasState::default(),
            }),
        });
    })

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
        let mut pass = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
            label: Some("Hdr::process"),
            color_attachments: &[Some(wgpu::RenderPassColorAttachment {
                view: &ouput,
                depth_slice: (),
                resolve_target: None,
                ops: Operations {
                    load: wgpu::LoadOp::Load,
                    store: wgpu::StoreOp::Store,
                },
            })],
            depth_stencil_attachment: None,
            timestamp_writes: (),
            occlusion_query_set: (),
            multiview_mask: (),
        });

        pass.set_pipeline(&self.pipeline);
        pass.set_bind_group(0, &self.bind_group, &[]);
        pass.draw(0..3, 0..1);
    }

    fn create_render_pipeline(
        device: &wgpu::Device,
        layout: &wgpu::PipelineLayout,
        color_format: Option<wgpu::TextureFormat>,
        depth_format: &[wgpu::VertexBufferLayout],
        vertex_layouts: &wgpu::PrimitiveTopology,
        shader: wgpu::ShaderModuleDescriptor,
    ) -> wgpu::RenderPipeline {
        let shader = device.create_shader_module(shader);

        device.create_render_pipeline(wgpu::RenderPipelineDescriptor {
            label: (),
            layout: (),
            vertex: (),
            primitive: (),
            depth_stencil: (),
            multisample: (),
            fragment: (),
            multiview_mask: (),
            cache: (),
        })
    }
}

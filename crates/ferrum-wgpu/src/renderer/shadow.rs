use {
    crate::{
        assets::{InstanceRaw, ModelVertex, Vertex},
        renderer,
        scene::LightUniform,
    },
    wgpu::{BindGroup, BindGroupLayout, Device, PipelineLayout, RenderPipeline},
};

/// Shadow map (depth-only pass from the light's point of view) and the bind
/// group that exposes it to the main pass.
pub struct ShadowRig {
    pub texture: renderer::Texture,
    pub bind_group: BindGroup,
    pub layout: BindGroupLayout,
    pub pipeline: RenderPipeline,
}

impl ShadowRig {
    pub fn new(device: &Device, light_layout: &BindGroupLayout) -> Self {
        let texture: renderer::Texture =
            renderer::Texture::create_shadow_map(device, 2048, "shadow_texture");

        let layout: BindGroupLayout =
            device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
                label: Some("shadow_bind_group_layout"),
                entries: &[
                    wgpu::BindGroupLayoutEntry {
                        binding: 0,
                        visibility: wgpu::ShaderStages::FRAGMENT,
                        ty: wgpu::BindingType::Texture {
                            sample_type: wgpu::TextureSampleType::Depth,
                            view_dimension: wgpu::TextureViewDimension::D2,
                            multisampled: false,
                        },
                        count: None,
                    },
                    wgpu::BindGroupLayoutEntry {
                        binding: 1,
                        visibility: wgpu::ShaderStages::FRAGMENT,
                        ty: wgpu::BindingType::Sampler(wgpu::SamplerBindingType::Comparison),
                        count: None,
                    },
                ],
            });

        let bind_group: BindGroup = device.create_bind_group(&wgpu::BindGroupDescriptor {
            label: Some("shadow_bind_group"),
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

        let pipeline_layout: PipelineLayout =
            device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
                label: Some("shadow_pipeline_layout"),
                bind_group_layouts: &[Some(light_layout)],
                ..Default::default()
            });

        let pipeline: RenderPipeline = LightUniform::create_render_pipeline(
            device,
            &pipeline_layout,
            None,
            Some(renderer::Texture::DEPTH_FORMAT),
            &[ModelVertex::desc(), InstanceRaw::desc()],
            wgpu::ShaderModuleDescriptor {
                label: Some("shadow_normal_shader"),
                source: wgpu::ShaderSource::Wgsl(include_str!("../shaders/shadow.wgsl").into()),
            },
            None, // no culling: geometry blocks light from both sides
            wgpu::DepthBiasState {
                constant: 2,
                slope_scale: 4.0, // compensates for grazing-angle precision loss
                clamp: 0.0,
            },
        );

        Self {
            texture,
            bind_group,
            layout,
            pipeline,
        }
    }
}

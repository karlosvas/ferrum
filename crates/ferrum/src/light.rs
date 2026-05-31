use wgpu::{RenderPipeline, ShaderModule};

#[repr(C)]
#[derive(Debug, Clone, Copy, bytemuck::Pod, bytemuck::Zeroable)]
pub struct LightUniform {
    pub position: [f32; 3],
    pub _padding: u32,           // aligns color to offset 16 (vec3 WGSL alignment)
    pub color: [f32; 3],
    pub _padding2: u32,          // aligns light_view_proj to offset 32
    pub light_view_proj: [[f32; 4]; 4],
}

impl LightUniform {
    fn new(
        position: [f32; 3],
        color: [f32; 3],
        light_view_proj: [[f32; 4]; 4],
        _padding: u32,
        _padding2: u32,
    ) -> Self {
        Self {
            position,
            color,
            light_view_proj,
            _padding,
            _padding2,
        }
    }

    pub fn create_render_pipeline(
        device: &wgpu::Device,
        layout: &wgpu::PipelineLayout,
        color_format: Option<wgpu::TextureFormat>,
        depth_format: Option<wgpu::TextureFormat>,
        vertex_layaut: &[wgpu::VertexBufferLayout],
        shader: wgpu::ShaderModuleDescriptor,
        cull_mode: Option<wgpu::Face>,
        depth_bias: wgpu::DepthBiasState,
    ) -> RenderPipeline {
        let shader: ShaderModule = device.create_shader_module(shader);

        let color_targets: Option<Vec<Option<wgpu::ColorTargetState>>> =
            color_format.map(|format| {
                vec![Some(wgpu::ColorTargetState {
                    format,
                    blend: Some(wgpu::BlendState {
                        color: wgpu::BlendComponent::REPLACE,
                        alpha: wgpu::BlendComponent::REPLACE,
                    }),
                    write_mask: wgpu::ColorWrites::ALL,
                })]
            });

        device.create_render_pipeline(&wgpu::RenderPipelineDescriptor {
            label: Some("light_render_pipeline"),
            layout: Some(layout),
            vertex: wgpu::VertexState {
                module: &shader,
                entry_point: Some("vs_main"),
                buffers: vertex_layaut,
                compilation_options: Default::default(),
            },
            primitive: wgpu::PrimitiveState {
                topology: wgpu::PrimitiveTopology::TriangleList,
                strip_index_format: None,
                front_face: wgpu::FrontFace::Ccw,
                cull_mode,
                unclipped_depth: false,
                polygon_mode: wgpu::PolygonMode::Fill,
                conservative: false,
            },
            depth_stencil: depth_format.map(|format| wgpu::DepthStencilState {
                format,
                depth_write_enabled: true,
                depth_compare: wgpu::CompareFunction::Less,
                stencil: wgpu::StencilState::default(),
                bias: depth_bias,
            }),
            multisample: wgpu::MultisampleState {
                count: 1,
                mask: !0,
                alpha_to_coverage_enabled: false,
            },
            fragment: color_targets.as_deref().map(|targets| wgpu::FragmentState {
                module: &shader,
                entry_point: Some("fs_main"),
                targets,
                compilation_options: Default::default(),
            }),
            multiview_mask: None,
            cache: None,
        })
    }
}

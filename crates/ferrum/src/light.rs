use crate::{State, math::TransformDelta};
use cgmath::{Matrix4, Point3, Vector3, ortho};
use wgpu::{RenderPipeline, ShaderModule};

pub struct Light;

impl Light {
    pub fn move_flare_object_light(
        &self,
        state: &mut State,
        light_id: &usize,
        transform: TransformDelta,
        lux: f32,
    ) {
        if let Some(crate::Bead::Molten(l)) = state.light_models.get_mut(light_id) {
            if let Some(instance) = l.instances.get_mut(0) {
                instance.position += transform.translation;
            }

            let raws: Vec<crate::models::InstanceRaw> =
                l.instances.iter().map(|i| i.to_raw()).collect();
            state
                .queue
                .write_buffer(&l.instance_buffer, 0, bytemuck::cast_slice(&raws));

            self.flare_light(state, transform, lux);

            state.queue.write_buffer(
                &state.light_buffer,
                0,
                bytemuck::cast_slice(&[state.light_uniform]),
            );
        } else {
            log::error!("ID {:?} invalid", light_id);
        }
    }

    pub fn flare_light(&self, state: &mut State, transform: TransformDelta, lux: f32) {
        const MAX_LUX: f32 = 1000.0;
        const MIN_INTENSITY: f32 = 0.05;
        let intensity: f32 = (lux / MAX_LUX).clamp(MIN_INTENSITY, 1.0);
        state.light_uniform.color = [intensity, intensity, intensity];

        let old_position: cgmath::Vector3<f32> = state.light_uniform.position.into();
        let new_pos: Vector3<f32> = old_position + transform.translation;
        state.light_uniform.position = [new_pos.x, new_pos.y, new_pos.z];

        let light_pos: cgmath::Point3<f32> = state.light_uniform.position.into();
        let up: Vector3<f32> = if state.light_uniform.position[0].abs() < 0.01
            && state.light_uniform.position[2].abs() < 0.01
        {
            Vector3::unit_z()
        } else {
            Vector3::unit_y()
        };
        let light_view: Matrix4<f32> =
            Matrix4::look_at_rh(light_pos, Point3::new(0.0, 0.0, 0.0), up);
        let light_proj: Matrix4<f32> = ortho(-20.0, 20.0, -20.0, 20.0, 0.1, 100.0);

        let light_view_proj: Matrix4<f32> = light_proj * light_view;
        state.light_uniform.light_view_proj = cgmath::Matrix4::into(light_view_proj);
    }
}

#[repr(C)]
#[derive(Debug, Clone, Copy, bytemuck::Pod, bytemuck::Zeroable)]
pub struct LightUniform {
    pub position: [f32; 3],
    pub _padding: u32, // aligns color to offset 16 (vec3 WGSL alignment)
    pub color: [f32; 3],
    pub _padding2: u32, // aligns light_view_proj to offset 32
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

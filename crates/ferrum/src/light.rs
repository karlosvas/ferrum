use crate::{State, math::TransformDelta};
use cgmath::{Matrix4, Point3, Vector3, VectorSpace, ortho};
use wgpu::{RenderPipeline, ShaderModule};

pub struct Light;

/// Convierte un valor de lux en una intensidad de luz HDR en el rango
/// `[0, MAX_INTENSITY]`.
///
/// Usa una curva perceptual (raíz cuadrada) en lugar de una rampa lineal: el ojo
/// humano percibe la luz de forma logarítmica, así que un mapeo lineal deja todo
/// oscuro hasta lux muy altos. Con la raíz, niveles moderados (p. ej. 250 lux ->
/// ~0.5 del rango) ya se ven claramente brillantes.
///
/// El resultado puede superar 1.0 a propósito: el pipeline aplica tonemapping
/// ACES (ver `hdr.wgsl`), así que valores HDR > 1.0 representan una fuente
/// potente. Venus hace de sol, por lo que `MAX_INTENSITY` se eleva muy por
/// encima de 1.0 para que ilumine de verdad toda la escena.
///
/// Por debajo de `MIN_LUX` se considera oscuridad total y la intensidad es 0, para
/// que la luz no siga brillando cuando no le llega nada (el sensor ronda 0 lux a
/// oscuras, con algo de ruido). Se interpola suavemente hasta `MIN_LUX` para
/// evitar parpadeos cerca del umbral.
fn lux_to_intensity(lux: f32) -> f32 {
    const MAX_LUX: f32 = 1000.0;
    const MIN_LUX: f32 = 8.0;
    const MAX_INTENSITY: f32 = 11.0;
    // El sensor (tsl2591 < 0.1.5) devuelve NaN/Inf en oscuridad total (0/0 en su
    // fórmula). Cualquier comparación con NaN es false, así que sin este guard el
    // NaN saltaba el umbral, llegaba al shader y se pintaba como blanco brillante.
    if !lux.is_finite() || lux <= MIN_LUX {
        return 0.0;
    }
    let normalized: f32 = (lux / MAX_LUX).clamp(0.0, 1.0);
    // Atenúa la entrada/salida cerca del umbral para que no haga "pop".
    let gate: f32 = ((lux - MIN_LUX) / MIN_LUX).clamp(0.0, 1.0);
    normalized.sqrt() * MAX_INTENSITY * gate
}

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

    /// Establece la posición ABSOLUTA de un objeto de luz (Opción B), en lugar de
    /// desplazarlo por deltas como `move_flare_object_light`.
    ///
    /// Pensado para fuentes de posición externas (p. ej. la cámara de la RPi), que ya
    /// proporcionan coordenadas de mundo y no incrementos.
    pub fn set_object_light_position(
        &self,
        state: &mut State,
        light_id: &usize,
        position: Vector3<f32>,
        lux: f32,
    ) {
        // Interpolamos hacia la posición objetivo en vez de saltar a ella: como
        // este método se llama cada frame con el mismo objetivo, la luz converge
        // suavemente desde su posición actual.
        //
        // Suavizado exponencial con dt real => independiente del framerate.
        // TIME_CONSTANT es, aproximadamente, los segundos que tarda en recorrer
        // ~63% de la distancia al objetivo. Mayor = más lento y fluido.
        const TIME_CONSTANT: f32 = 0.8;
        // Constante de tiempo del brillo, independiente de la de la posición para
        // poder afinar cada una por separado. Mayor = el encendido/apagado y los
        // cambios de luz son más graduales (también suaviza el ruido del sensor).
        const INTENSITY_TIME_CONSTANT: f32 = 0.6;
        let now: web_time::Instant = web_time::Instant::now();
        let dt: f32 = (now - state.light_last_update).as_secs_f32();
        state.light_last_update = now;
        let factor: f32 = 1.0 - (-dt / TIME_CONSTANT).exp();
        let intensity_factor: f32 = 1.0 - (-dt / INTENSITY_TIME_CONSTANT).exp();

        let current: Vector3<f32> = state.light_uniform.position.into();
        let position: Vector3<f32> = current.lerp(position, factor);

        // Interpolación del brillo desde el valor actual hacia el objetivo (lux),
        // igual que la posición: evita saltos y parpadeos.
        let target_intensity: f32 = lux_to_intensity(lux);
        let current_intensity: f32 = state.light_uniform.color[0];
        let intensity: f32 =
            current_intensity + (target_intensity - current_intensity) * intensity_factor;

        if let Some(crate::Bead::Molten(l)) = state.light_models.get_mut(light_id) {
            if let Some(instance) = l.instances.get_mut(0) {
                instance.position = position;
            }

            let raws: Vec<crate::models::InstanceRaw> =
                l.instances.iter().map(|i| i.to_raw()).collect();
            state
                .queue
                .write_buffer(&l.instance_buffer, 0, bytemuck::cast_slice(&raws));

            self.set_light_position(state, position, intensity);

            state.queue.write_buffer(
                &state.light_buffer,
                0,
                bytemuck::cast_slice(&[state.light_uniform]),
            );
        } else {
            log::error!("ID {:?} invalid", light_id);
        }
    }

    /// Actualiza el uniform de la luz con una posición ABSOLUTA y una `intensity`
    /// ya calculada/suavizada y recalcula `light_view_proj`. No recibe lux: el
    /// suavizado del brillo lo hace `set_object_light_position`.
    pub fn set_light_position(&self, state: &mut State, position: Vector3<f32>, intensity: f32) {
        state.light_uniform.color = [intensity, intensity, intensity];

        state.light_uniform.position = [position.x, position.y, position.z];

        let light_pos: Point3<f32> = Point3::new(position.x, position.y, position.z);
        let up: Vector3<f32> = if position.x.abs() < 0.01 && position.z.abs() < 0.01 {
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

    pub fn flare_light(&self, state: &mut State, transform: TransformDelta, lux: f32) {
        let intensity: f32 = lux_to_intensity(lux);
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

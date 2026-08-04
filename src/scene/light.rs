use crate::{
    State,
    assets::{InstanceRaw, ModelVertex, Vertex},
    math::TransformDelta,
    renderer::{self, uniform_layout},
};
use cgmath::{Matrix4, Point3, Vector3, VectorSpace, ortho};
use wgpu::{
    BindGroup, BindGroupLayout, Buffer, Device, PipelineLayout, Queue, RenderPipeline,
    ShaderModule, util::DeviceExt,
};

pub struct Light;

/// Light uniform plus its GPU resources and the emissive-object pipeline.
pub struct LightRig {
    pub uniform: LightUniform,
    pub buffer: Buffer,
    pub bind_group: BindGroup,
    pub layout: BindGroupLayout,
    pub pipeline: RenderPipeline,
    pub last_update: web_time::Instant,
}

impl LightRig {
    pub fn new(
        device: &Device,
        camera_layout: &BindGroupLayout,
        texture_layout: &BindGroupLayout,
        color_format: wgpu::TextureFormat,
    ) -> Self {
        let uniform: LightUniform = LightUniform::new(
            [15.0, 0.0, 0.0],
            [7.0, 6.95, 6.85],
            [
                [1.0, 0.0, 0.0, 0.0],
                [0.0, 1.0, 0.0, 0.0],
                [0.0, 0.0, 1.0, 0.0],
                [0.0, 0.0, 0.0, 1.0],
            ],
        );

        let buffer: Buffer = device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
            label: Some("light_buffer"),
            contents: bytemuck::cast_slice(&[uniform]),
            usage: wgpu::BufferUsages::UNIFORM | wgpu::BufferUsages::COPY_DST,
        });

        let layout: BindGroupLayout = uniform_layout(
            device,
            "light_bind_group_layout",
            wgpu::ShaderStages::VERTEX_FRAGMENT,
        );

        let bind_group: BindGroup = device.create_bind_group(&wgpu::BindGroupDescriptor {
            label: Some("light_bind_group"),
            layout: &layout,
            entries: &[wgpu::BindGroupEntry {
                binding: 0,
                resource: buffer.as_entire_binding(),
            }],
        });

        let pipeline_layout: PipelineLayout =
            device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
                label: Some("light_pipeline_layout"),
                bind_group_layouts: &[Some(camera_layout), Some(&layout), Some(texture_layout)],
                ..Default::default()
            });

        let pipeline: RenderPipeline = LightUniform::create_render_pipeline(
            device,
            &pipeline_layout,
            Some(color_format),
            Some(renderer::Texture::DEPTH_FORMAT),
            &[ModelVertex::desc()],
            wgpu::ShaderModuleDescriptor {
                label: Some("normal_shader"),
                source: wgpu::ShaderSource::Wgsl(include_str!("../shaders/light.wgsl").into()),
            },
            Some(wgpu::Face::Back),
            wgpu::DepthBiasState::default(),
        );

        Self {
            uniform,
            buffer,
            bind_group,
            layout,
            pipeline,
            last_update: web_time::Instant::now(),
        }
    }

    /// Recomputes the shadow view-projection from the current position and
    /// uploads the uniform to the GPU. Called once per frame.
    pub fn update(&mut self, queue: &Queue) {
        self.uniform.recompute_view_proj();
        queue.write_buffer(&self.buffer, 0, bytemuck::cast_slice(&[self.uniform]));
    }
}

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
        if let Some(l) = state.models.light_model_mut(light_id) {
            if let Some(instance) = l.instances.get_mut(0) {
                instance.position += transform.translation;
            }

            let raws: Vec<InstanceRaw> = l.instances.iter().map(|i| i.to_raw()).collect();
            state
                .queue
                .write_buffer(&l.instance_buffer, 0, bytemuck::cast_slice(&raws));

            self.flare_light(state, transform, lux);

            state.queue.write_buffer(
                &state.light.buffer,
                0,
                bytemuck::cast_slice(&[state.light.uniform]),
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
        let dt: f32 = (now - state.light.last_update).as_secs_f32();
        state.light.last_update = now;
        let factor: f32 = 1.0 - (-dt / TIME_CONSTANT).exp();
        let intensity_factor: f32 = 1.0 - (-dt / INTENSITY_TIME_CONSTANT).exp();

        let current: Vector3<f32> = state.light.uniform.position.into();
        let position: Vector3<f32> = current.lerp(position, factor);

        // Interpolación del brillo desde el valor actual hacia el objetivo (lux),
        // igual que la posición: evita saltos y parpadeos.
        let target_intensity: f32 = lux_to_intensity(lux);
        let current_intensity: f32 = state.light.uniform.color[0];
        let intensity: f32 =
            current_intensity + (target_intensity - current_intensity) * intensity_factor;

        if let Some(l) = state.models.light_model_mut(light_id) {
            if let Some(instance) = l.instances.get_mut(0) {
                instance.position = position;
            }

            let raws: Vec<InstanceRaw> = l.instances.iter().map(|i| i.to_raw()).collect();
            state
                .queue
                .write_buffer(&l.instance_buffer, 0, bytemuck::cast_slice(&raws));

            self.set_light_position(state, position, intensity);

            state.queue.write_buffer(
                &state.light.buffer,
                0,
                bytemuck::cast_slice(&[state.light.uniform]),
            );
        } else {
            log::error!("ID {:?} invalid", light_id);
        }
    }

    /// Actualiza el uniform de la luz con una posición ABSOLUTA y una `intensity`
    /// ya calculada/suavizada y recalcula `light_view_proj`. No recibe lux: el
    /// suavizado del brillo lo hace `set_object_light_position`.
    pub fn set_light_position(&self, state: &mut State, position: Vector3<f32>, intensity: f32) {
        state.light.uniform.color = [intensity, intensity, intensity];
        state.light.uniform.position = [position.x, position.y, position.z];
        state.light.uniform.recompute_view_proj();
    }

    pub fn flare_light(&self, state: &mut State, transform: TransformDelta, lux: f32) {
        let intensity: f32 = lux_to_intensity(lux);
        state.light.uniform.color = [intensity, intensity, intensity];

        let old_position: cgmath::Vector3<f32> = state.light.uniform.position.into();
        let new_pos: Vector3<f32> = old_position + transform.translation;
        state.light.uniform.position = [new_pos.x, new_pos.y, new_pos.z];
        state.light.uniform.recompute_view_proj();
    }
}

#[repr(C)]
#[derive(Debug, Clone, Copy, bytemuck::Pod, bytemuck::Zeroable)]
pub struct LightUniform {
    pub position: [f32; 3],
    // aligns color to offset 16 (vec3 WGSL alignment)
    pub _padding: u32,
    pub color: [f32; 3],
    // aligns light_view_proj to offset 32
    pub _padding2: u32,
    pub light_view_proj: [[f32; 4]; 4],
}

impl LightUniform {
    pub fn new(position: [f32; 3], color: [f32; 3], light_view_proj: [[f32; 4]; 4]) -> Self {
        Self {
            position,
            color,
            light_view_proj,
            _padding: 0,
            _padding2: 0,
        }
    }

    /// Recomputes `light_view_proj` from the current `position` (ortho shadow
    /// frustum looking at the origin).
    pub fn recompute_view_proj(&mut self) {
        let light_pos: Point3<f32> = self.position.into();

        // Avoid degenerate look_at when the light is nearly aligned with the Y axis.
        let up: Vector3<f32> = if self.position[0].abs() < 0.01 && self.position[2].abs() < 0.01 {
            Vector3::unit_z()
        } else {
            Vector3::unit_y()
        };
        let light_view: Matrix4<f32> =
            Matrix4::look_at_rh(light_pos, Point3::new(0.0, 0.0, 0.0), up);
        let light_proj: Matrix4<f32> = ortho(-20.0, 20.0, -20.0, 20.0, 0.1, 100.0);

        self.light_view_proj = (light_proj * light_view).into();
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
                depth_write_enabled: Some(true),
                depth_compare: Some(wgpu::CompareFunction::Less),
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

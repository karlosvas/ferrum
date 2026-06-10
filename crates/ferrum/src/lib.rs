pub mod assets;
pub mod config;
mod error;
pub mod math;
mod renderer;
mod scene;

use crate::{
    assets::{DrawShadow, InstanceRaw, Model, ModelVertex, Vertex},
    config::WindowSize,
    renderer::CubeTexture,
    renderer::HdrPipeline,
    scene::Light,
    scene::WindUniform,
};
pub use {
    assets::{Instance, TypeModel},
    cgmath::{Deg, Matrix4, Point3, Quaternion, Rotation3, Vector3, ortho},
    error::SurfaceError,
    std::{
        collections::HashMap,
        marker::PhantomData,
        sync::{
            Arc,
            atomic::{AtomicUsize, Ordering},
            mpsc::{self, Sender},
        },
    },
    wgpu::{
        Adapter, BindGroup, BindGroupLayout, Buffer, CommandEncoder, Device, PipelineLayout, Queue,
        RenderPass, RenderPipeline, ShaderModule, ShaderModuleDescriptor, Surface,
        SurfaceCapabilities, SurfaceTexture, TextureFormat, TextureView, util::DeviceExt,
        wgt::SurfaceConfiguration,
    },
    winit::{dpi::PhysicalSize, event_loop::ActiveEventLoop, keyboard::KeyCode},
};

pub struct Ingot<T> {
    pub id: usize,
    _marker: PhantomData<T>,
}

enum Bead<T> {
    Burning,
    Molten(T),
    #[allow(dead_code)]
    Ash,
}

pub struct State {
    pub window_surface: wgpu::Surface<'static>,
    pub device: Arc<wgpu::Device>,
    pub queue: Arc<wgpu::Queue>,
    pub config: wgpu::SurfaceConfiguration,
    pub is_surface_configuration: bool,
    pub render_pipeline: wgpu::RenderPipeline,
    pub camera: renderer::Camera,
    pub camera_uniform: renderer::CameraUniform,
    pub camera_buffer: wgpu::Buffer,
    pub camera_bind_group: wgpu::BindGroup,
    pub camera_controller: renderer::CameraController,
    // Models
    static_models: HashMap<usize, Bead<Model>>,
    light_models: HashMap<usize, Bead<Model>>,
    actual_ingot: AtomicUsize,
    model_sender: mpsc::Sender<(usize, Model)>,
    model_receiver: mpsc::Receiver<(usize, Model)>,
    pub texture_bind_group_layout: Arc<wgpu::BindGroupLayout>,
    pub last_render_time: web_time::Instant,
    pub depth_texture: renderer::Texture,
    // Light
    pub light_uniform: scene::LightUniform,
    pub light_buffer: Buffer,
    pub light_bind_group: wgpu::BindGroup,
    pub light_render_pipeline: wgpu::RenderPipeline,
    pub light_last_update: web_time::Instant,

    // Wind
    pub wind_uniform: WindUniform,
    pub wind_buffer: Buffer,
    pub wind_bind_group: wgpu::BindGroup,
    /// Instant of last frame to wind accumulate wind, because strog soplido agite and doblarlas las hojas more faster
    /// Instante del último frame para acumular la fase del viento. `time` no es
    /// tiempo real: avanza más rápido cuanto mayor es la intensidad, para que un
    /// soplido fuerte agite las hojas más deprisa además de doblarlas más.
    pub wind_start: web_time::Instant,
    // Shadow
    pub shadow_texture: renderer::Texture,
    pub shadow_bind_group: wgpu::BindGroup,
    pub shadow_render_pipeline: wgpu::RenderPipeline,
    // HDR
    pub hdr: renderer::HdrPipeline,
    pub environment_bind_group: wgpu::BindGroup,
    pub sky_pipeline: wgpu::RenderPipeline,
}

impl State {
    pub async fn new(
        target: impl raw_window_handle::HasWindowHandle
        + raw_window_handle::HasDisplayHandle
        + wgpu::WasmNotSendSync
        + 'static,
        window_size: WindowSize,
    ) -> anyhow::Result<Self> {
        let mut instance_desc: wgpu::InstanceDescriptor =
            wgpu::InstanceDescriptor::new_without_display_handle();
        #[cfg(target_arch = "wasm32")]
        {
            instance_desc.backends = wgpu::Backends::GL | wgpu::Backends::BROWSER_WEBGPU;
        }
        #[cfg(all(not(target_arch = "wasm32"), not(feature = "rpi")))]
        {
            instance_desc.backends = wgpu::Backends::PRIMARY;
        }
        #[cfg(all(not(target_arch = "wasm32"), feature = "rpi"))]
        {
            instance_desc.backends = wgpu::Backends::GL;
        }
        let backend_instance: wgpu::Instance = wgpu::Instance::new(instance_desc);

        // Surface to be drawn
        let window_surface: Surface = backend_instance.create_surface(target)?;

        // Representation of the system's physical GPU
        let adapter: Adapter = backend_instance
            .request_adapter(&wgpu::RequestAdapterOptions {
                power_preference: wgpu::PowerPreference::default(),
                force_fallback_adapter: false,
                compatible_surface: Some(&window_surface),
            })
            .await?;

        // Logic interface for creating resources and a command queue that is sent to the GPU
        let (device, queue) = adapter
            .request_device(&wgpu::DeviceDescriptor {
                label: None,
                // The engine uses no optional features. all_webgpu_mask() would demand
                // every WebGPU feature as required,
                required_features: wgpu::Features::empty(),
                // The engine requires WebGPU (compute shader for the HDR cubemap) and never
                // runs on WebGL2, so use the adapter's real limits on every target.
                // downlevel_webgl2_defaults() would cap compute limits at 0 and break the
                // equirect→cubemap compute pass.
                required_limits: adapter.limits(),
                experimental_features: wgpu::ExperimentalFeatures::disabled(),
                memory_hints: Default::default(),
                trace: wgpu::Trace::Off,
            })
            .await?;
        let device: Arc<Device> = Arc::new(device);
        let queue: Arc<Queue> = Arc::new(queue);

        // A dynamic query of the capabilities that varies according to the adapter you have
        let surface_caps: SurfaceCapabilities = window_surface.get_capabilities(&adapter);

        // Define how pixels are stored in memory
        let surface_format: TextureFormat = surface_caps
            .formats
            .iter()
            .find(|f| f.is_srgb())
            .copied()
            .unwrap_or(surface_caps.formats[0]);

        // Describe the surface configuration, which includes the format, size, and present mode
        let config: SurfaceConfiguration<Vec<TextureFormat>> = wgpu::SurfaceConfiguration {
            usage: wgpu::TextureUsages::RENDER_ATTACHMENT,
            format: surface_format,
            width: window_size.width,
            height: window_size.height,
            present_mode: surface_caps.present_modes[0],
            desired_maximum_frame_latency: 2,
            alpha_mode: surface_caps.alpha_modes[0],
            view_formats: vec![surface_format.add_srgb_suffix()],
        };

        let texture_bind_group_layout: Arc<BindGroupLayout> = Arc::new(
            device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
                label: Some("texture_bind_group_layout"),
                entries: &[
                    // Texture
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
                    // Normal Map
                    wgpu::BindGroupLayoutEntry {
                        binding: 2,
                        visibility: wgpu::ShaderStages::FRAGMENT,
                        ty: wgpu::BindingType::Texture {
                            multisampled: false,
                            sample_type: wgpu::TextureSampleType::Float { filterable: true },
                            view_dimension: wgpu::TextureViewDimension::D2,
                        },
                        count: None,
                    },
                    wgpu::BindGroupLayoutEntry {
                        binding: 3,
                        visibility: wgpu::ShaderStages::FRAGMENT,
                        ty: wgpu::BindingType::Sampler(wgpu::SamplerBindingType::Filtering),
                        count: None,
                    },
                ],
            }),
        );

        let shader: ShaderModule =
            device.create_shader_module(wgpu::include_wgsl!("shaders/shaders.wgsl"));

        // Camera
        let camera: renderer::Camera = renderer::Camera {
            eye: (0.0, 4.0, 10.0).into(),
            target: (0.0, 3.0, 0.0).into(),
            up: cgmath::Vector3::unit_y(),
            aspect: config.width as f32 / config.height as f32,
            fovy: 45.0,
            znear: 0.1,
            zfar: 100.0,
        };

        let camera_bind_group_layout: BindGroupLayout =
            device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
                label: Some("camera_bind_group_layout"),
                entries: &[wgpu::BindGroupLayoutEntry {
                    binding: 0,
                    visibility: wgpu::ShaderStages::VERTEX_FRAGMENT,
                    ty: wgpu::BindingType::Buffer {
                        ty: wgpu::BufferBindingType::Uniform,
                        has_dynamic_offset: false,
                        min_binding_size: None,
                    },
                    count: None,
                }],
            });

        let (camera_bind_group, camera_buffer, camera_controller, camera_uniform) =
            renderer::Camera::build_camera_setup(&camera, &device, &camera_bind_group_layout);

        // Deth texture
        let depth_texture: renderer::Texture =
            renderer::Texture::create_depth_texture(&device, &config, "depth_texture");

        // Sky
        let hdr: HdrPipeline = renderer::HdrPipeline::new(&device, &config);

        let hdr_loader: renderer::HdrLoader = renderer::HdrLoader::new(&device);

        // Web caps max_texture_dimension_2d at 8192 and wasm32 has a 4 GiB address
        // space, so the 16K equirectangular (16384px, ~2 GiB decoded) cannot be
        // loaded in the browser. Use a 4K version on web and keep 16K on native.
        #[cfg(target_arch = "wasm32")]
        let sky_file: &str = "exr/NightSkyHDRI014_4K_HDR.exr";
        #[cfg(not(target_arch = "wasm32"))]
        let sky_file: &str = "exr/NightSkyHDRI014_16K_HDR.exr";

        let sky_bytes: Vec<u8> = assets::load_binary(sky_file).await?;

        let sky_texture: CubeTexture = hdr_loader.from_equirectangular_bytes(
            &device,
            &queue,
            &sky_bytes,
            if sky_file.ends_with(".exr") {
                renderer::SkyFormat::Exr
            } else {
                renderer::SkyFormat::Hdr
            },
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

        let environment_bind_group: BindGroup =
            device.create_bind_group(&wgpu::BindGroupDescriptor {
                label: Some("environment_bind_group"),
                layout: &environment_layout,
                entries: &[
                    wgpu::BindGroupEntry {
                        binding: 0,
                        resource: wgpu::BindingResource::TextureView(&sky_texture.view()),
                    },
                    wgpu::BindGroupEntry {
                        binding: 1,
                        resource: wgpu::BindingResource::Sampler(sky_texture.sampler()),
                    },
                ],
            });

        let sky_pipeline: RenderPipeline = {
            let layout: PipelineLayout =
                device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
                    label: Some("Sky Pipeline Layout"),
                    bind_group_layouts: &[
                        Some(&camera_bind_group_layout),
                        Some(&environment_layout),
                    ],
                    immediate_size: 0,
                });
            let shader: ShaderModuleDescriptor = wgpu::include_wgsl!("shaders/sky.wgsl");
            renderer::create_render_pipeline(
                &device,
                &layout,
                hdr.format(),
                Some(renderer::Texture::DEPTH_FORMAT),
                &[],
                wgpu::PrimitiveTopology::TriangleList,
                shader,
            )
        };

        // Light
        let light_uniform: scene::LightUniform = scene::LightUniform::new(
            [15.0, 0.0, 0.0],
            [7.0, 6.95, 6.85],
            [
                [1.0, 0.0, 0.0, 0.0],
                [0.0, 1.0, 0.0, 0.0],
                [0.0, 0.0, 1.0, 0.0],
                [0.0, 0.0, 0.0, 1.0],
            ],
        );

        let light_buffer: Buffer = device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
            label: Some("light_buffer"),
            contents: bytemuck::cast_slice(&[light_uniform]),
            usage: wgpu::BufferUsages::UNIFORM | wgpu::BufferUsages::COPY_DST,
        });

        let light_bind_group_layout: BindGroupLayout =
            device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
                label: Some("light_bind_group_layout"),
                entries: &[wgpu::BindGroupLayoutEntry {
                    binding: 0,
                    visibility: wgpu::ShaderStages::VERTEX_FRAGMENT,
                    ty: wgpu::BindingType::Buffer {
                        ty: wgpu::BufferBindingType::Uniform,
                        has_dynamic_offset: false,
                        min_binding_size: None,
                    },
                    count: None,
                }],
            });

        let light_bind_group: BindGroup = device.create_bind_group(&wgpu::BindGroupDescriptor {
            label: Some("light_bind_group"),
            layout: &light_bind_group_layout,
            entries: &[wgpu::BindGroupEntry {
                binding: 0,
                resource: light_buffer.as_entire_binding(),
            }],
        });

        let light_pipeline_layout: PipelineLayout =
            device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
                label: Some("light_pipeline_layout"),
                bind_group_layouts: &[
                    Some(&camera_bind_group_layout),
                    Some(&light_bind_group_layout),
                    Some(&texture_bind_group_layout),
                ],
                ..Default::default()
            });

        let light_render_pipeline: RenderPipeline = {
            let normal_shader: ShaderModuleDescriptor = wgpu::ShaderModuleDescriptor {
                label: Some("normal_shader"),
                source: wgpu::ShaderSource::Wgsl(include_str!("shaders/light.wgsl").into()),
            };

            scene::LightUniform::create_render_pipeline(
                &device,
                &light_pipeline_layout,
                Some(hdr.format()),
                Some(renderer::Texture::DEPTH_FORMAT),
                &[ModelVertex::desc()],
                normal_shader,
                Some(wgpu::Face::Back),
                wgpu::DepthBiasState::default(),
            )
        };

        // Shadow
        let shadow_texture: renderer::Texture =
            renderer::Texture::create_shadow_map(&device, 2048, "shadow_texture");

        let shadow_bind_group_layout: BindGroupLayout =
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

        let shadow_bind_group: BindGroup = device.create_bind_group(&wgpu::BindGroupDescriptor {
            label: Some("shadow_bind_group"),
            layout: &shadow_bind_group_layout,
            entries: &[
                wgpu::BindGroupEntry {
                    binding: 0,
                    resource: wgpu::BindingResource::TextureView(&shadow_texture.view),
                },
                wgpu::BindGroupEntry {
                    binding: 1,
                    resource: wgpu::BindingResource::Sampler(&shadow_texture.sampler),
                },
            ],
        });

        let shadow_pipeline_layout: PipelineLayout =
            device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
                label: Some("light_pipeline_layout"),
                bind_group_layouts: &[Some(&light_bind_group_layout)],
                ..Default::default()
            });

        let shadow_render_pipeline: RenderPipeline = {
            let normal_shader: ShaderModuleDescriptor = wgpu::ShaderModuleDescriptor {
                label: Some("shadow_normal_shader"),
                source: wgpu::ShaderSource::Wgsl(include_str!("shaders/shadow.wgsl").into()),
            };

            scene::LightUniform::create_render_pipeline(
                &device,
                &shadow_pipeline_layout,
                None,
                Some(renderer::Texture::DEPTH_FORMAT),
                &[ModelVertex::desc(), InstanceRaw::desc()],
                normal_shader,
                None, // no culling: geometry blocks light from both sides
                wgpu::DepthBiasState {
                    constant: 2,
                    slope_scale: 4.0, // compensates for grazing-angle precision loss
                    clamp: 0.0,
                },
            )
        };

        // Wind
        let wind_uniform: WindUniform = WindUniform {
            intensity: 0.8,
            ..Default::default()
        };

        let wind_buffer: Buffer = device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
            label: Some("wind_buffer"),
            contents: bytemuck::cast_slice(&[wind_uniform]),
            usage: wgpu::BufferUsages::UNIFORM | wgpu::BufferUsages::COPY_DST,
        });

        let wind_bind_group_layout: BindGroupLayout =
            device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
                label: Some("wind_bind_group_layout"),
                entries: &[wgpu::BindGroupLayoutEntry {
                    binding: 0,
                    visibility: wgpu::ShaderStages::VERTEX,
                    ty: wgpu::BindingType::Buffer {
                        ty: wgpu::BufferBindingType::Uniform,
                        has_dynamic_offset: false,
                        min_binding_size: None,
                    },
                    count: None,
                }],
            });

        let wind_bind_group: BindGroup = device.create_bind_group(&wgpu::BindGroupDescriptor {
            label: Some("wind_bind_group"),
            layout: &wind_bind_group_layout,
            entries: &[wgpu::BindGroupEntry {
                binding: 0,
                resource: wind_buffer.as_entire_binding(),
            }],
        });

        // Render Pipeline
        let pipeline_render_layout: PipelineLayout =
            device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
                bind_group_layouts: &[
                    Some(&texture_bind_group_layout),
                    Some(&camera_bind_group_layout),
                    Some(&light_bind_group_layout),
                    Some(&shadow_bind_group_layout),
                    Some(&wind_bind_group_layout),
                ],
                label: Some("render_pipeline_layout"),
                ..Default::default()
            });

        let render_pipeline: RenderPipeline =
            device.create_render_pipeline(&wgpu::RenderPipelineDescriptor {
                label: Some("render_pipeline"),
                layout: Some(&pipeline_render_layout),
                vertex: wgpu::VertexState {
                    module: &shader,
                    entry_point: Some("vs_main"),
                    compilation_options: wgpu::PipelineCompilationOptions::default(),
                    buffers: &[ModelVertex::desc(), InstanceRaw::desc()],
                },
                primitive: wgpu::PrimitiveState {
                    topology: wgpu::PrimitiveTopology::TriangleList,
                    strip_index_format: None,
                    front_face: wgpu::FrontFace::Ccw,
                    cull_mode: Some(wgpu::Face::Back),
                    unclipped_depth: false,
                    polygon_mode: wgpu::PolygonMode::Fill,
                    conservative: false,
                },
                depth_stencil: Some(wgpu::DepthStencilState {
                    format: renderer::Texture::DEPTH_FORMAT,
                    depth_write_enabled: Some(true),
                    depth_compare: Some(wgpu::CompareFunction::Less),
                    stencil: wgpu::StencilState::default(),
                    bias: wgpu::DepthBiasState::default(),
                }),
                multisample: wgpu::MultisampleState {
                    count: 1,
                    mask: !0,
                    alpha_to_coverage_enabled: false,
                },
                fragment: Some(wgpu::FragmentState {
                    module: &shader,
                    entry_point: Some("fs_main"),
                    compilation_options: wgpu::PipelineCompilationOptions::default(),
                    targets: &[Some(wgpu::ColorTargetState {
                        format: hdr.format(),
                        blend: Some(wgpu::BlendState::REPLACE),
                        write_mask: wgpu::ColorWrites::ALL,
                    })],
                }),
                cache: None,
                multiview_mask: None,
            });

        let (model_sender, model_receiver) = mpsc::channel::<(usize, Model)>();

        Ok(Self {
            window_surface,
            device,
            queue,
            config,
            is_surface_configuration: false,
            render_pipeline,
            camera,
            camera_uniform,
            camera_buffer,
            camera_bind_group,
            camera_controller,
            static_models: HashMap::new(),
            light_models: HashMap::new(),
            actual_ingot: AtomicUsize::new(0),
            model_sender,
            model_receiver,
            last_render_time: web_time::Instant::now(),
            texture_bind_group_layout,
            depth_texture,
            shadow_texture,
            shadow_bind_group,
            shadow_render_pipeline,
            light_uniform,
            light_buffer,
            light_bind_group,
            light_render_pipeline,
            light_last_update: web_time::Instant::now(),
            wind_uniform,
            wind_buffer,
            wind_bind_group,
            wind_start: web_time::Instant::now(),
            hdr,
            environment_bind_group,
            sky_pipeline,
        })
    }

    pub fn resize(&mut self, height: u32, width: u32) {
        if height > 0 && width > 0 {
            self.config.height = height;
            self.config.width = width;

            self.window_surface.configure(&self.device, &self.config);

            self.camera.aspect = self.config.width as f32 / self.config.height as f32;

            self.depth_texture = renderer::Texture::create_depth_texture(
                &self.device,
                &self.config,
                "depth_texture",
            );

            self.hdr.resize(&self.device, width, height);
            self.is_surface_configuration = true;
        }
    }

    pub fn render(&mut self) -> Result<(), SurfaceError> {
        self.render_with_overlay(&mut |_, _, _, _| {})
    }

    /// Igual que `render`, pero invoca `overlay` tras el tonemapping HDR con el
    /// encoder y la vista del frame final, para pintar UI (egui, debug, etc.)
    /// encima de la escena sin que el motor dependa de ninguna crate de UI.
    pub fn render_with_overlay(
        &mut self,
        overlay: &mut dyn FnMut(
            &wgpu::Device,
            &wgpu::Queue,
            &mut wgpu::CommandEncoder,
            &wgpu::TextureView,
        ),
    ) -> Result<(), SurfaceError> {
        if !self.is_surface_configuration {
            return Ok(());
        }

        let mut encoder: CommandEncoder =
            self.device
                .create_command_encoder(&wgpu::CommandEncoderDescriptor {
                    label: Some("encoder"),
                });

        {
            let mut shadow_render_pass: RenderPass =
                encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
                    label: Some("Shadow_render_pass"),
                    color_attachments: &[],
                    depth_stencil_attachment: Some(wgpu::RenderPassDepthStencilAttachment {
                        view: &self.shadow_texture.view,
                        depth_ops: Some(wgpu::Operations {
                            load: wgpu::LoadOp::Clear(1.0),
                            store: wgpu::StoreOp::Store,
                        }),
                        stencil_ops: None,
                    }),
                    timestamp_writes: None,
                    occlusion_query_set: None,
                    multiview_mask: None,
                });

            shadow_render_pass.set_pipeline(&self.shadow_render_pipeline);
            shadow_render_pass.set_bind_group(0, &self.light_bind_group, &[]);
            for bead in self.static_models.values() {
                if let Bead::Molten(model) = bead {
                    shadow_render_pass.draw_shadow_model(model, &self.light_bind_group);
                }
            }
        }
        {
            let mut render_pass: RenderPass =
                encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
                    label: Some("render_pass"),
                    color_attachments: &[Some(wgpu::RenderPassColorAttachment {
                        view: self.hdr.view(),
                        depth_slice: None,
                        resolve_target: None,
                        ops: wgpu::Operations {
                            load: wgpu::LoadOp::Clear(wgpu::Color::BLACK),
                            store: wgpu::StoreOp::Store,
                        },
                    })],
                    depth_stencil_attachment: Some(wgpu::RenderPassDepthStencilAttachment {
                        view: &self.depth_texture.view,
                        depth_ops: Some(wgpu::Operations {
                            load: wgpu::LoadOp::Clear(1.0),
                            store: wgpu::StoreOp::Store,
                        }),
                        stencil_ops: None,
                    }),
                    timestamp_writes: None,
                    occlusion_query_set: None,
                    multiview_mask: None,
                });

            use assets::DrawLight;
            render_pass.set_pipeline(&self.light_render_pipeline);
            for bead in self.light_models.values() {
                if let Bead::Molten(model) = bead {
                    render_pass.draw_light_model(
                        model,
                        &self.camera_bind_group,
                        &self.light_bind_group,
                    );
                }
            }

            use assets::DrawModel;
            render_pass.set_pipeline(&self.render_pipeline);
            for bead in self.static_models.values() {
                if let Bead::Molten(model) = bead {
                    render_pass.draw_model(
                        model,
                        &self.camera_bind_group,
                        &self.light_bind_group,
                        &self.shadow_bind_group,
                        &self.wind_bind_group,
                    );
                }
            }

            // Sky pipeline last: leverages the depth test (LessEqual with z=1.0)
            // to paint only the pixels where no geometry was drawn.
            render_pass.set_pipeline(&self.sky_pipeline);
            render_pass.set_bind_group(0, &self.camera_bind_group, &[]);
            render_pass.set_bind_group(1, &self.environment_bind_group, &[]);
            render_pass.draw(0..3, 0..1);
        }

        let ouput: SurfaceTexture = match self.window_surface.get_current_texture() {
            wgpu::CurrentSurfaceTexture::Success(t)
            | wgpu::CurrentSurfaceTexture::Suboptimal(t) => t,
            // Frame no disponible temporalmente (minimizada, timeout): se salta
            // el frame sin tratarlo como error.
            wgpu::CurrentSurfaceTexture::Timeout | wgpu::CurrentSurfaceTexture::Occluded => {
                return Ok(());
            }
            wgpu::CurrentSurfaceTexture::Outdated => return Err(SurfaceError::Outdated),
            wgpu::CurrentSurfaceTexture::Lost => return Err(SurfaceError::Lost),
            wgpu::CurrentSurfaceTexture::Validation => return Err(SurfaceError::Validation),
        };
        let view: TextureView = ouput.texture.create_view(&wgpu::TextureViewDescriptor {
            format: Some(self.config.format.add_srgb_suffix()),
            ..Default::default()
        });

        self.hdr.process(&mut encoder, &view);
        overlay(&self.device, &self.queue, &mut encoder, &view);
        self.queue.submit(std::iter::once(encoder.finish()));

        ouput.present();

        Ok(())
    }

    pub fn spawn_model(&mut self, model_desc: assets::ModelDesc) -> Ingot<assets::Model> {
        let id: usize = self.actual_ingot.fetch_add(1, Ordering::SeqCst);

        match model_desc.kind {
            TypeModel::StaticObj => self.static_models.insert(id, Bead::Burning),
            TypeModel::PointOfLight => self.light_models.insert(id, Bead::Burning),
        };

        let device: Arc<Device> = Arc::clone(&self.device);
        let queue: Arc<Queue> = Arc::clone(&self.queue);
        let layout: Arc<BindGroupLayout> = Arc::clone(&self.texture_bind_group_layout);
        let sender: Sender<(usize, Model)> = self.model_sender.clone();
        let path: String = model_desc.path.to_string();

        // La carga es asíncrona y el modelo llega por el canal cuando termina
        // (Bead::Burning hasta entonces). En nativo: hilo + block_on; en wasm
        // no hay hilos ni block_on, así que se encola en el event loop del
        // navegador con spawn_local (el fetch de assets ya es async).
        let instances: Vec<Instance> = model_desc.instances;
        let kind: TypeModel = model_desc.kind;

        #[cfg(not(target_arch = "wasm32"))]
        std::thread::spawn(move || {
            let result: Result<Model, anyhow::Error> = pollster::block_on(assets::load_model(
                &path, &device, &queue, &layout, instances, kind,
            ));
            if let Ok(model) = result {
                let _ = sender.send((id, model));
            }
        });

        #[cfg(target_arch = "wasm32")]
        wasm_bindgen_futures::spawn_local(async move {
            let result: Result<Model, anyhow::Error> =
                resources::load_model(&path, &device, &queue, &layout, instances, kind).await;
            if let Ok(model) = result {
                let _ = sender.send((id, model));
            }
        });

        Ingot {
            id,
            _marker: PhantomData,
        }
    }

    pub fn light_handle(&mut self) -> Light {
        Light
    }

    /// Fija el viento que anima el follaje. `direction` es un vector 2D en el
    /// plano XZ (x = derecha/izquierda, y = adelante/atrás) e `intensity` la
    /// fuerza [0, 1]. Solo guarda los valores; el `time` y la subida a GPU las
    /// hace `render` cada frame. Se normaliza la dirección para que la intensidad
    /// controle por sí sola la magnitud del balanceo.
    pub fn set_wind(&mut self, direction: [f32; 2], intensity: f32) {
        let len: f32 = (direction[0] * direction[0] + direction[1] * direction[1]).sqrt();
        self.wind_uniform.direction = if len > 1e-6 {
            [direction[0] / len, direction[1] / len]
        } else {
            [0.0, 0.0]
        };
        self.wind_uniform.intensity = intensity.clamp(0.0, 1.0);
    }

    pub fn evolbe(&mut self) {
        while let Ok((id, model)) = self.model_receiver.try_recv() {
            match model.type_model {
                TypeModel::StaticObj => self.static_models.insert(id, Bead::Molten(model)),
                TypeModel::PointOfLight => self.light_models.insert(id, Bead::Molten(model)),
            };
        }

        let now: web_time::Instant = web_time::Instant::now();
        let dt: web_time::Duration = now - self.last_render_time;
        self.last_render_time = now;
        self.camera_controller.update_camera(&mut self.camera, dt);
        self.camera_uniform.update_view_proj(&self.camera);
        self.queue.write_buffer(
            &self.camera_buffer,
            0,
            bytemuck::cast_slice(&[self.camera_uniform]),
        );

        let light_pos: cgmath::Point3<f32> = self.light_uniform.position.into();

        // Avoid degenerate look_at when the light is nearly aligned with the Y axis.
        let up: Vector3<f32> = if self.light_uniform.position[0].abs() < 0.01
            && self.light_uniform.position[2].abs() < 0.01
        {
            Vector3::unit_z()
        } else {
            Vector3::unit_y()
        };
        let light_view: Matrix4<f32> =
            Matrix4::look_at_rh(light_pos, Point3::new(0.0, 0.0, 0.0), up);
        let light_proj: Matrix4<f32> = ortho(-20.0, 20.0, -20.0, 20.0, 0.1, 100.0);

        let light_view_proj: Matrix4<f32> = light_proj * light_view;
        self.light_uniform.light_view_proj = cgmath::Matrix4::into(light_view_proj);

        self.queue.write_buffer(
            &self.light_buffer,
            0,
            bytemuck::cast_slice(&[self.light_uniform]),
        );

        // Avanzar la FASE del viento (la dirección/intensidad las fija el demo
        // vía `set_wind`) y subir el uniform a la GPU una vez por frame.
        // La fase se acumula escalada por la intensidad en vez de usar tiempo
        // real: soplar fuerte agita las hojas más rápido, no solo más lejos.
        // Acumular (en vez de multiplicar el tiempo) evita saltos de fase
        // cuando la intensidad cambia entre frames.
        let dt: f32 = self.wind_start.elapsed().as_secs_f32();
        self.wind_start = web_time::Instant::now();
        let speed: f32 = 0.6 + self.wind_uniform.intensity * 2.4;
        self.wind_uniform.time += dt * speed;
        self.queue.write_buffer(
            &self.wind_buffer,
            0,
            bytemuck::cast_slice(&[self.wind_uniform]),
        );
    }
}

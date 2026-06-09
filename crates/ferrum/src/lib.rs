mod camera;
pub mod config;
mod hdr;
mod light;
mod material;
pub mod math;
pub mod models;
mod pipeline;
mod resources;
mod structs;
mod texture;

use crate::{
    config::WindowSize,
    hdr::HdrPipeline,
    light::Light,
    models::{DrawShadow, InstanceRaw, Model, ModelVertex, Vertex},
    texture::CubeTexture,
};
pub use {
    cgmath::{Deg, Matrix4, Point3, Quaternion, Rotation3, Vector3, ortho},
    models::{Instance, TypeModel},
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
    pub camera: camera::Camera,
    pub camera_uniform: camera::CameraUniform,
    pub camera_buffer: wgpu::Buffer,
    pub camera_bind_group: wgpu::BindGroup,
    pub camera_controller: camera::CameraController,

    // Models
    static_models: HashMap<usize, Bead<Model>>,
    light_models: HashMap<usize, Bead<Model>>,
    actual_ingot: AtomicUsize,
    model_sender: mpsc::Sender<(usize, Model)>,
    model_receiver: mpsc::Receiver<(usize, Model)>,
    pub texture_bind_group_layout: Arc<wgpu::BindGroupLayout>,

    pub last_render_time: web_time::Instant,
    pub depth_texture: texture::Texture,

    // Light
    pub light_uniform: light::LightUniform,
    pub light_buffer: Buffer,
    pub light_bind_group: wgpu::BindGroup,
    pub light_render_pipeline: wgpu::RenderPipeline,

    // Shadow
    pub shadow_texture: texture::Texture,
    pub shadow_bind_group: wgpu::BindGroup,
    pub shadow_render_pipeline: wgpu::RenderPipeline,

    // HDR
    pub hdr: hdr::HdrPipeline,
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
        let backend_instance: wgpu::Instance = wgpu::Instance::new(&wgpu::InstanceDescriptor {
            #[cfg(target_arch = "wasm32")]
            backends: wgpu::Backends::GL | wgpu::Backends::BROWSER_WEBGPU,
            #[cfg(all(not(target_arch = "wasm32"), not(feature = "rpi")))]
            backends: wgpu::Backends::PRIMARY,
            #[cfg(all(not(target_arch = "wasm32"), feature = "rpi"))]
            backends: wgpu::Backends::GL,
            ..Default::default()
        });

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
                // every WebGPU feature as required — including texture-compression-astc,
                // which desktop GPUs (e.g. AMD Vega) don't support, so requestDevice fails.
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
        let camera: camera::Camera = camera::Camera {
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
            camera::Camera::build_camera_setup(&camera, &device, &camera_bind_group_layout);

        // Deth texture
        let depth_texture: texture::Texture =
            texture::Texture::create_depth_texture(&device, &config, "depth_texture");

        // Sky
        let hdr: HdrPipeline = hdr::HdrPipeline::new(&device, &config);

        let hdr_loader: hdr::HdrLoader = hdr::HdrLoader::new(&device);

        // Web caps max_texture_dimension_2d at 8192 and wasm32 has a 4 GiB address
        // space, so the 16K equirectangular (16384px, ~2 GiB decoded) cannot be
        // loaded in the browser. Use a 4K version on web and keep 16K on native.
        #[cfg(target_arch = "wasm32")]
        let sky_file: &str = "exr/NightSkyHDRI014_4K_HDR.exr";
        #[cfg(not(target_arch = "wasm32"))]
        let sky_file: &str = "exr/NightSkyHDRI014_16K_HDR.exr";

        let sky_bytes: Vec<u8> = resources::load_binary(sky_file).await?;

        let sky_texture: CubeTexture = hdr_loader.from_equirectangular_bytes(
            &device,
            &queue,
            &sky_bytes,
            if sky_file.ends_with(".exr") {
                hdr::SkyFormat::Exr
            } else {
                hdr::SkyFormat::Hdr
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
                    bind_group_layouts: &[&camera_bind_group_layout, &environment_layout],
                    immediate_size: 0,
                });
            let shader: ShaderModuleDescriptor = wgpu::include_wgsl!("shaders/sky.wgsl");
            pipeline::create_render_pipeline(
                &device,
                &layout,
                hdr.format(),
                Some(texture::Texture::DEPTH_FORMAT),
                &[],
                wgpu::PrimitiveTopology::TriangleList,
                shader,
            )
        };

        // Light
        let light_uniform: light::LightUniform = light::LightUniform {
            position: [15.0, 0.0, 0.0],
            color: [7.0, 6.95, 6.85],
            light_view_proj: [
                [1.0, 0.0, 0.0, 0.0],
                [0.0, 1.0, 0.0, 0.0],
                [0.0, 0.0, 1.0, 0.0],
                [0.0, 0.0, 0.0, 1.0],
            ],
            _padding: 0,
            _padding2: 0,
        };

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
                    &camera_bind_group_layout,
                    &light_bind_group_layout,
                    &texture_bind_group_layout,
                ],
                ..Default::default()
            });

        let light_render_pipeline: RenderPipeline = {
            let normal_shader: ShaderModuleDescriptor = wgpu::ShaderModuleDescriptor {
                label: Some("normal_shader"),
                source: wgpu::ShaderSource::Wgsl(include_str!("shaders/light.wgsl").into()),
            };

            light::LightUniform::create_render_pipeline(
                &device,
                &light_pipeline_layout,
                Some(hdr.format()),
                Some(texture::Texture::DEPTH_FORMAT),
                &[ModelVertex::desc()],
                normal_shader,
                Some(wgpu::Face::Back),
                wgpu::DepthBiasState::default(),
            )
        };

        // Shadow
        let shadow_texture: texture::Texture =
            texture::Texture::create_shadow_map(&device, 2048, "shadow_texture");

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
                bind_group_layouts: &[&light_bind_group_layout],
                ..Default::default()
            });

        let shadow_render_pipeline: RenderPipeline = {
            let normal_shader: ShaderModuleDescriptor = wgpu::ShaderModuleDescriptor {
                label: Some("shadow_normal_shader"),
                source: wgpu::ShaderSource::Wgsl(include_str!("shaders/shadow.wgsl").into()),
            };

            light::LightUniform::create_render_pipeline(
                &device,
                &shadow_pipeline_layout,
                None,
                Some(texture::Texture::DEPTH_FORMAT),
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

        // Render Pipeline
        let pipeline_render_layout: PipelineLayout =
            device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
                bind_group_layouts: &[
                    &texture_bind_group_layout,
                    &camera_bind_group_layout,
                    &light_bind_group_layout,
                    &shadow_bind_group_layout,
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
                    format: texture::Texture::DEPTH_FORMAT,
                    depth_write_enabled: true,
                    depth_compare: wgpu::CompareFunction::Less,
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

            self.depth_texture =
                texture::Texture::create_depth_texture(&self.device, &self.config, "depth_texture");

            self.hdr.resize(&self.device, width, height);
            self.is_surface_configuration = true;
        }
    }

    pub fn render(&mut self) -> Result<(), wgpu::SurfaceError> {
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

            use models::DrawLight;
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

            use models::DrawModel;
            render_pass.set_pipeline(&self.render_pipeline);
            for bead in self.static_models.values() {
                if let Bead::Molten(model) = bead {
                    render_pass.draw_model(
                        model,
                        &self.camera_bind_group,
                        &self.light_bind_group,
                        &self.shadow_bind_group,
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

        let ouput: SurfaceTexture = self.window_surface.get_current_texture()?;
        let view: TextureView = ouput.texture.create_view(&wgpu::TextureViewDescriptor {
            format: Some(self.config.format.add_srgb_suffix()),
            ..Default::default()
        });

        self.hdr.process(&mut encoder, &view);
        self.queue.submit(std::iter::once(encoder.finish()));

        ouput.present();

        Ok(())
    }

    pub fn spawn_model(&mut self, model_desc: models::ModelDesc) -> Ingot<models::Model> {
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

        std::thread::spawn(move || {
            let result: Result<Model, anyhow::Error> = pollster::block_on(resources::load_model(
                &path,
                &device,
                &queue,
                &layout,
                model_desc.instances,
                model_desc.kind,
            ));
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

        // TODO: Light rotation animation, eliminado solo era para hacer pruebas, en un futuro
        // utilizar para movimientoi del sol en la demo
        //let old_position: cgmath::Vector3<f32> = self.light_uniform.position.into();
        //
        //self.light_uniform.position = (cgmath::Quaternion::from_axis_angle(
        //    (0.0, 0.0, 1.0).into(),
        //    cgmath::Deg(30.0 * dt.as_secs_f32()),
        //) * old_position)
        //    .into();

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
    }
}

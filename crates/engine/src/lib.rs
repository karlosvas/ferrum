mod camera;
mod hdr;
mod light;
mod material;
mod models;
mod pipeline;
mod resources;
mod structs;
mod texture;

use cgmath::{Deg, Quaternion, Vector3};
#[cfg(target_arch = "wasm32")]
use wasm_bindgen::prelude::*;
use wgpu::TextureView;
#[cfg(target_arch = "wasm32")]
use winit::event_loop::EventLoopProxy;

use crate::{
    hdr::HdrPipeline,
    models::{InstanceRaw, ModelVertex, Vertex},
    texture::CubeTexture,
};
use {
    cgmath::Rotation3,
    image::ImageBuffer,
    std::sync::Arc,
    wgpu::{
        Adapter, BindGroup, BindGroupLayout, Buffer, CommandEncoder, Device, Instance,
        PipelineLayout, Queue, RenderPass, RenderPipeline, ShaderModule, ShaderModuleDescriptor,
        Surface, SurfaceCapabilities, SurfaceTexture, TextureFormat, util::DeviceExt,
        wgt::SurfaceConfiguration,
    },
    winit::{
        application::ApplicationHandler,
        dpi::PhysicalSize,
        event::KeyEvent,
        event::WindowEvent,
        event_loop::{ActiveEventLoop, EventLoop},
        keyboard::{KeyCode, PhysicalKey},
        window::Window,
        window::{WindowAttributes, WindowId},
    },
};

pub struct State {
    pub window_surface: wgpu::Surface<'static>,
    pub device: wgpu::Device,
    pub queue: wgpu::Queue,
    pub config: wgpu::SurfaceConfiguration,
    pub is_surface_configuration: bool,
    pub render_pipeline: wgpu::RenderPipeline,
    pub camera: camera::Camera,
    pub camera_uniform: camera::CameraUniform,
    pub camera_buffer: wgpu::Buffer,
    pub camera_bind_group: wgpu::BindGroup,
    pub camera_controller: camera::CameraController,
    pub obj_model: models::Model,
    pub obj_model_2: models::Model,
    pub obj_light: models::Model,
    pub last_render_time: web_time::Instant,
    pub depth_texture: texture::Texture,

    // Light
    pub light_uniform: light::LightUniform,
    pub light_buffer: Buffer,
    pub light_bind_group: wgpu::BindGroup,
    pub light_render_pipeline: wgpu::RenderPipeline,

    // HDR
    pub hdr: hdr::HdrPipeline,
    pub environment_bind_group: wgpu::BindGroup,
    pub sky_pipeline: wgpu::RenderPipeline,

    pub window: Arc<Window>,
}

impl State {
    pub async fn new(window: Arc<Window>) -> anyhow::Result<Self> {
        let window_size: PhysicalSize<u32> = window.inner_size();

        let backend_instance: Instance = wgpu::Instance::new(&wgpu::InstanceDescriptor {
            #[cfg(target_arch = "wasm32")]
            backends: wgpu::Backends::GL | wgpu::Backends::BROWSER_WEBGPU,
            #[cfg(all(not(target_arch = "wasm32"), not(feature = "rpi")))]
            backends: wgpu::Backends::PRIMARY,
            #[cfg(all(not(target_arch = "wasm32"), feature = "rpi"))]
            backends: wgpu::Backends::GL,
            ..Default::default()
        });

        let window_surface: Surface = backend_instance.create_surface(window.clone())?;

        // Representation of the system's physical GPU
        let adapter: Adapter = backend_instance
            .request_adapter(&wgpu::RequestAdapterOptions {
                power_preference: wgpu::PowerPreference::default(),
                force_fallback_adapter: false,
                compatible_surface: Some(&window_surface),
            })
            .await?;

        // Logic interface for creating resources and a command queue that is sent to the GPU
        let (device, queue): (Device, Queue) = adapter
            .request_device(&wgpu::DeviceDescriptor {
                label: None,
                required_features: if cfg!(target_arch = "wasm32") {
                    wgpu::Features::all_webgpu_mask()
                } else {
                    wgpu::Features::empty()
                },
                required_limits: if cfg!(target_arch = "wasm32") {
                    wgpu::Limits::downlevel_webgl2_defaults()
                } else {
                    adapter.limits()
                },
                experimental_features: wgpu::ExperimentalFeatures::disabled(),
                memory_hints: Default::default(),
                trace: wgpu::Trace::Off,
            })
            .await?;

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

        let texture_bind_group_layout: BindGroupLayout =
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
            });

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

        // Load the 3D model (.obj inside the /res folder)
        let obj_model: models::Model = resources::load_model(
            "plant/plant.obj",
            &device,
            &queue,
            &texture_bind_group_layout,
            vec![models::Instance::new(
                Vector3::new(0.0, 1.5, 0.0),
                Quaternion::from_angle_y(Deg(0.0)),
                Vector3::new(1.0, 1.0, 1.0),
            )],
        )
        .await
        .expect("Error cargando plant/plant.obj");

        let obj_model_2: models::Model = resources::load_model(
            "floor/floor.obj",
            &device,
            &queue,
            &texture_bind_group_layout,
            vec![models::Instance::default()],
        )
        .await
        .expect("Error cargando plant/plant.obj");

        let obj_light: models::Model = resources::load_model(
            "sun/venus.obj",
            &device,
            &queue,
            &texture_bind_group_layout,
            vec![models::Instance::default()],
        )
        .await
        .unwrap();

        let depth_texture: texture::Texture =
            texture::Texture::create_depth_texture(&device, &config, "depth_texture");

        // HDR
        let hdr: HdrPipeline = hdr::HdrPipeline::new(&device, &config);

        let hdr_loader: hdr::HdrLoader = hdr::HdrLoader::new(&device);
        let sky_file: &str = "NightSkyHDRI014_16K_HDR.exr";
        let sky_format: hdr::SkyFormat = if sky_file.ends_with(".exr") {
            hdr::SkyFormat::Exr
        } else {
            hdr::SkyFormat::Hdr
        };
        let sky_bytes: Vec<u8> = resources::load_binary(sky_file).await?;

        let sky_texture: CubeTexture = hdr_loader.from_equirectangular_bytes(
            &device,
            &queue,
            &sky_bytes,
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

        // Light section
        let light_uniform: light::LightUniform = light::LightUniform {
            position: [10.0, 0.0, 0.0],
            _padding: 0,
            color: [10.0, 8.0, 6.0],
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

        let pipeline_render_layout: PipelineLayout =
            device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
                bind_group_layouts: &[
                    &texture_bind_group_layout,
                    &camera_bind_group_layout,
                    &light_bind_group_layout,
                ],
                label: Some("render_pipeline_layout"),
                ..Default::default()
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
                hdr.format(),
                Some(texture::Texture::DEPTH_FORMAT),
                &[ModelVertex::desc()],
                normal_shader,
            )
        };

        // Render Pipeline
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
                // Fragment shader
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
            obj_model,
            obj_model_2,
            obj_light,
            last_render_time: web_time::Instant::now(),
            depth_texture,
            light_uniform,
            light_buffer,
            light_bind_group,
            light_render_pipeline,
            hdr,
            environment_bind_group,
            sky_pipeline,
            window,
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
        // TODO: delete after
        self.window.request_redraw();

        if !self.is_surface_configuration {
            return Ok(());
        }

        let mut encoder: CommandEncoder =
            self.device
                .create_command_encoder(&wgpu::CommandEncoderDescriptor {
                    label: Some("encoder"),
                });

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
            render_pass.draw_light_model(
                &self.obj_light,
                &self.camera_bind_group,
                &self.light_bind_group,
            );

            use models::DrawModel;
            render_pass.set_pipeline(&self.render_pipeline);
            render_pass.draw_model(
                &self.obj_model,
                &self.camera_bind_group,
                &self.light_bind_group,
            );
            render_pass.draw_model(
                &self.obj_model_2,
                &self.camera_bind_group,
                &self.light_bind_group,
            );

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

    pub fn handle_key(&mut self, event_loop: &ActiveEventLoop, key: KeyCode, is_pressed: bool) {
        if key == KeyCode::Escape && is_pressed {
            #[cfg(not(target_arch = "wasm32"))]
            event_loop.exit();
        } else {
            self.camera_controller.handle_key(key, is_pressed);
        }
    }

    pub fn update(&mut self) {
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

        // Light rotation animation
        let old_position: cgmath::Vector3<f32> = self.light_uniform.position.into();
        self.light_uniform.position = (cgmath::Quaternion::from_axis_angle(
            (0.0, 0.0, 1.0).into(),
            cgmath::Deg(30.0 * dt.as_secs_f32()),
        ) * old_position)
            .into();

        self.queue.write_buffer(
            &self.light_buffer,
            0,
            bytemuck::cast_slice(&[self.light_uniform]),
        );
    }
}

pub struct App {
    pub state: Option<State>,
    #[cfg(target_arch = "wasm32")]
    pub proxy: Option<EventLoopProxy<State>>,
}

impl App {
    pub fn new(#[cfg(target_arch = "wasm32")] proxy: EventLoopProxy<State>) -> Self {
        Self {
            state: None,
            #[cfg(target_arch = "wasm32")]
            proxy: Some(proxy),
        }
    }
}

impl ApplicationHandler<State> for App {
    fn resumed(&mut self, event_loop: &ActiveEventLoop) {
        use winit::window::Icon;
        let icon: Icon = {
            let bytes: &[u8] = include_bytes!("../assets/logo.ico");
            let img: ImageBuffer<image::Rgba<u8>, Vec<u8>> =
                image::load_from_memory(bytes).unwrap().to_rgba8();
            let (w, h): (u32, u32) = img.dimensions();
            Icon::from_rgba(img.into_raw(), w, h)
        }
        .unwrap();

        #[allow(unused_mut)]
        let mut window_attributes: WindowAttributes =
            Window::default_attributes().with_title("Ferrum");

        #[cfg(target_arch = "wasm32")]
        {
            window_attributes = window_attributes.with_window_icon(Some(icon.clone()));
        }

        #[cfg(target_arch = "wasm32")]
        {
            use wasm_bindgen::JsCast;
            use winit::{platform::web::WindowAttributesExtWebSys, window};

            const CANVAS_ID: &str = "canvas";
            let window = wgpu::web_sys::window().unwrap_throw();
            let document = window.document().unwrap_throw();

            let canvas = document.get_element_by_id(CANVAS_ID).unwrap_throw();
            let html_canvas = canvas.unchecked_into();
            window_attributes = window_attributes.with_canvas(Some(html_canvas));
        }

        let window: Arc<Window> = Arc::new(event_loop.create_window(window_attributes).unwrap());

        #[cfg(not(target_arch = "wasm32"))]
        {
            self.state = Some(pollster::block_on(State::new(window)).unwrap());
        }

        #[cfg(target_arch = "wasm32")]
        {
            if let Some(proxy) = self.proxy.take() {
                wasm_bindgen_futures::spawn_local(async move {
                    assert!(
                        proxy
                            .send_event(
                                State::new(window)
                                    .await
                                    .expect("Unable te creeate canvas!!")
                            )
                            .is_ok()
                    )
                })
            }
        }
    }

    fn window_event(
        &mut self,
        event_loop: &ActiveEventLoop,
        _window_id: WindowId,
        event: WindowEvent,
    ) {
        let state: &mut State = match &mut self.state {
            Some(s) => s,
            None => return,
        };

        match event {
            WindowEvent::CloseRequested => event_loop.exit(),
            WindowEvent::RedrawRequested => {
                state.update();
                match state.render() {
                    Ok(_) => {}
                    Err(wgpu::SurfaceError::Lost | wgpu::SurfaceError::Outdated) => {
                        let size: PhysicalSize<u32> = state.window.inner_size();
                        state.resize(size.height, size.width);
                    }
                    Err(e) => log::error!("No se ha podido renderizar {}", e),
                }
            }
            WindowEvent::Resized(size) => state.resize(size.height, size.width),
            WindowEvent::KeyboardInput {
                event:
                    KeyEvent {
                        physical_key: PhysicalKey::Code(code),
                        state: key_state,
                        ..
                    },
                ..
            } => state.handle_key(event_loop, code, key_state.is_pressed()),
            _ => {}
        }
    }

    #[allow(unused_mut)]
    fn user_event(&mut self, _event_loop: &ActiveEventLoop, mut event: State) {
        #[cfg(target_arch = "wasm32")]
        {
            event.window.request_redraw();
            event.resize(
                event.window.inner_size().height,
                event.window.inner_size().width,
            );
        }
        self.state = Some(event)
    }
}

pub fn run() -> anyhow::Result<()> {
    #[cfg(not(target_arch = "wasm32"))]
    {
        env_logger::init();
    }
    #[cfg(target_arch = "wasm32")]
    {
        console_log::init_with_level(log::Level::Info).unwrap_throw();
    }

    let event_loop: EventLoop<State> = EventLoop::<State>::with_user_event().build()?;
    let mut app: App = App::new(
        #[cfg(target_arch = "wasm32")]
        {
            event_loop.create_proxy()
        },
    );
    event_loop.run_app(&mut app)?;

    Ok(())
}

#[cfg(target_arch = "wasm32")]
#[wasm_bindgen(start)]
pub fn run_web() -> Result<(), wasm_bindgen::JsValue> {
    console_error_panic_hook::set_once();
    run().unwrap_throw();
    Ok(())
}

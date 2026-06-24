pub mod assets;
pub mod config;
mod error;
pub mod math;
mod renderer;
mod scene;

// Pricvate use
use {
    crate::{
        assets::{
            DrawLight, DrawModel, DrawShadow, InstanceRaw, Model, ModelDesc, ModelStore,
            ModelVertex, Vertex,
        },
        config::{WindowSize, config::FerrumConfig},
        renderer::{CameraRig, HdrPipeline, Material, ShadowRig, SkyRig},
        scene::{Light, LightRig, WindRig},
    },
    std::sync::Arc,
    wgpu::{
        Adapter, BindGroupLayout, CommandEncoder, Device, PipelineLayout, Queue, RenderPass,
        RenderPipeline, Surface, SurfaceCapabilities, SurfaceConfiguration, SurfaceTexture,
        TextureFormat, TextureView,
    },
};

// Public use
pub use {
    assets::{Ingot, Instance, TypeModel},
    cgmath::{Deg, Matrix4, Point3, Quaternion, Rotation3, Vector3, ortho},
    error::SurfaceError,
    renderer::{EnviroimentDesc, SkyFormat},
    winit::{dpi::PhysicalSize, keyboard::KeyCode},
};

pub struct State {
    pub window_surface: wgpu::Surface<'static>,
    pub device: Arc<wgpu::Device>,
    pub queue: Arc<wgpu::Queue>,
    pub ferrum_config: FerrumConfig,
    pub is_surface_configuration: bool,
    pub render_pipeline: wgpu::RenderPipeline,
    pub texture_bind_group_layout: Arc<wgpu::BindGroupLayout>,
    pub depth_texture: renderer::Texture,
    pub last_render_time: web_time::Instant,
    pub camera: CameraRig,
    pub light: LightRig,
    pub wind: WindRig,
    pub shadow: ShadowRig,
    pub hdr: HdrPipeline,
    pub sky: Option<SkyRig>,
    pub sky_desc: Option<EnviroimentDesc>,
    pub(crate) models: ModelStore,
}

impl State {
    pub async fn new(
        target: impl raw_window_handle::HasWindowHandle
        + raw_window_handle::HasDisplayHandle
        + wgpu::WasmNotSendSync
        + 'static,
        window_size: WindowSize,
        asset: crate::assets::Asset,
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
            .unwrap_or(surface_caps.formats[0]); // Fallback to the first suirface

        // Describe the surface configuration, which includes the format, size, and present mode
        let surface_config: SurfaceConfiguration = wgpu::SurfaceConfiguration {
            usage: wgpu::TextureUsages::RENDER_ATTACHMENT,
            format: surface_format,
            width: window_size.width,
            height: window_size.height,
            present_mode: surface_caps.present_modes[0],
            desired_maximum_frame_latency: 2,
            alpha_mode: surface_caps.alpha_modes[0],
            view_formats: vec![surface_format.add_srgb_suffix()],
        };
        let ferrum_config: FerrumConfig = FerrumConfig {
            surface_config: Some(surface_config.clone()),
            asset,
            ..Default::default()
        };

        // Each subsystem builds its own GPU resources; State only wires the
        // layouts they need from one another.
        let texture_bind_group_layout: Arc<BindGroupLayout> =
            Arc::new(Material::bind_group_layout(&device));

        let camera: CameraRig = CameraRig::new(
            &device,
            surface_config.width as f32 / surface_config.height as f32,
        );

        let depth_texture: renderer::Texture =
            renderer::Texture::create_depth_texture(&device, &surface_config, "depth_texture");

        // Global HDR
        let hdr: HdrPipeline = HdrPipeline::new(&device, &surface_config);

        let light: LightRig = LightRig::new(
            &device,
            &camera.layout,
            &texture_bind_group_layout,
            hdr.format(),
        );

        let shadow: ShadowRig = ShadowRig::new(&device, &light.layout);

        let wind: WindRig = WindRig::new(&device);

        // Main render pipeline (textured geometry with light, shadow and wind)
        let pipeline_render_layout: PipelineLayout =
            device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
                bind_group_layouts: &[
                    Some(&texture_bind_group_layout),
                    Some(&camera.layout),
                    Some(&light.layout),
                    Some(&shadow.layout),
                    Some(&wind.layout),
                ],
                label: Some("render_pipeline_layout"),
                ..Default::default()
            });

        let render_pipeline: RenderPipeline = renderer::create_render_pipeline(
            &device,
            &pipeline_render_layout,
            hdr.format(),
            Some(renderer::Texture::DEPTH_FORMAT),
            &[ModelVertex::desc(), InstanceRaw::desc()],
            wgpu::PrimitiveTopology::TriangleList,
            wgpu::include_wgsl!("shaders/shaders.wgsl"),
            wgpu::CompareFunction::Less,
        );

        Ok(Self {
            window_surface,
            device,
            queue,
            ferrum_config,
            is_surface_configuration: false,
            render_pipeline,
            texture_bind_group_layout,
            depth_texture,
            last_render_time: web_time::Instant::now(),
            camera,
            light,
            wind,
            shadow,
            hdr,
            sky: None,
            sky_desc: None,
            models: ModelStore::new(),
        })
    }

    pub fn resize(&mut self, height: u32, width: u32) {
        if height > 0 && width > 0 {
            self.ferrum_config.size.height = height;
            self.ferrum_config.size.width = width;

            self.camera.set_aspect(
                self.ferrum_config.size.width as f32 / self.ferrum_config.size.height as f32,
            );

            if let Some(sc) = &self.ferrum_config.surface_config {
                self.window_surface.configure(&self.device, sc);

                self.depth_texture =
                    renderer::Texture::create_depth_texture(&self.device, sc, "depth_texture");
            };

            self.hdr.resize(&self.device, width, height);
            self.is_surface_configuration = true;
        }
    }

    pub fn render(&mut self) -> Result<(), SurfaceError> {
        self.render_with_overlay(&mut |_, _, _, _| {})
    }

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
                        view: &self.shadow.texture.view,
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

            shadow_render_pass.set_pipeline(&self.shadow.pipeline);
            shadow_render_pass.set_bind_group(0, &self.light.bind_group, &[]);
            for model in self.models.static_loaded() {
                shadow_render_pass.draw_shadow_model(model, &self.light.bind_group);
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

            render_pass.set_pipeline(&self.light.pipeline);
            for model in self.models.light_loaded() {
                render_pass.draw_light_model(
                    model,
                    &self.camera.bind_group,
                    &self.light.bind_group,
                );
            }

            render_pass.set_pipeline(&self.render_pipeline);
            for model in self.models.static_loaded() {
                render_pass.draw_model(
                    model,
                    &self.camera.bind_group,
                    &self.light.bind_group,
                    &self.shadow.bind_group,
                    &self.wind.bind_group,
                );
            }

            // Sky pipeline last: leverages the depth test (LessEqual with z=1.0)
            // to paint only the pixels where no geometry was drawn.
            if let Some(sky) = &self.sky {
                render_pass.set_pipeline(&sky.pipeline);
                render_pass.set_bind_group(0, &self.camera.bind_group, &[]);
                render_pass.set_bind_group(1, &sky.bind_group, &[]);
                render_pass.draw(0..3, 0..1);
            };
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

        if let Some(sc) = &self.ferrum_config.surface_config {
            let view: TextureView = ouput.texture.create_view(&wgpu::TextureViewDescriptor {
                format: Some(sc.format.add_srgb_suffix()),
                ..Default::default()
            });
            self.hdr.process(&mut encoder, &view);
            overlay(&self.device, &self.queue, &mut encoder, &view);
        }
        self.queue.submit(std::iter::once(encoder.finish()));

        ouput.present();

        Ok(())
    }

    pub fn spawn_model(&mut self, model_desc: ModelDesc) -> Ingot<Model> {
        self.models.spawn(
            &self.device,
            &self.queue,
            &self.texture_bind_group_layout,
            model_desc,
            &self.ferrum_config,
        )
    }

    pub fn light_handle(&mut self) -> Light {
        Light
    }

    pub fn spawn_enviroiment(&mut self, enviroiment: EnviroimentDesc) {
        self.sky_desc = Some(enviroiment);
    }

    /// See [`WindRig::set`]: stores the wind direction/intensity that animates
    /// the foliage; the GPU upload happens once per frame in `evolbe`.
    pub fn set_wind(&mut self, direction: [f32; 2], intensity: f32) {
        self.wind.set(direction, intensity);
    }

    /// Per-frame engine tick: integrates freshly loaded models and updates the
    /// camera, light and wind uniforms on the GPU.
    pub fn evolbe(&mut self) {
        self.models.collect_loaded();

        let now: web_time::Instant = web_time::Instant::now();
        let dt: web_time::Duration = now - self.last_render_time;
        self.last_render_time = now;

        if let (Some(desc), Some(sc)) = (self.sky_desc.take(), &self.ferrum_config.surface_config) {
            // take() → consume y deja None
            self.sky = Some(
                SkyRig::new(
                    &self.device,
                    &self.queue,
                    sc,
                    &self.camera.layout,
                    self.hdr.format(),
                    desc,
                )
                .expect("Error with load enviroiment"),
            );
        }

        self.camera.update(&self.queue, dt);
        self.light.update(&self.queue);
        self.wind.update(&self.queue);
    }
}

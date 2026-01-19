mod structs;

use wgpu::{
    Adapter, CommandEncoder, Instance, Surface, SurfaceCapabilities, SurfaceTexture, TextureFormat,
    TextureView,
};
use winit::dpi::PhysicalSize;
#[cfg(target_arch = "wasm32")]
use winit::event_loop::EventLoop;
use {
    std::sync::Arc,
    structs::{App, State},
    winit::window::Window,
    winit::{
        application::ApplicationHandler,
        event::WindowEvent,
        event_loop::{ActiveEventLoop, EventLoop},
        window::{WindowAttributes, WindowId},
    },
};

impl State {
    pub async fn new(window: Arc<Window>) -> anyhow::Result<Self> {
        // Tamaño de la pallaa
        let size: PhysicalSize<u32> = window.inner_size();

        // Nuestro punto de entarda para nuestro bakend
        let instance: Instance = wgpu::Instance::new(&wgpu::InstanceDescriptor {
            backends: wgpu::Backends::PRIMARY,
            ..Default::default()
        });

        // Representa la superficie de la ventana
        let surface: Surface = instance.create_surface(window.clone()).unwrap();

        // Representa la GPU física del sistema
        let adapter: Adapter = instance
            .request_adapter(&wgpu::RequestAdapterOptions {
                power_preference: wgpu::PowerPreference::default(),
                force_fallback_adapter: false,
                compatible_surface: Some(&surface),
            })
            .await?;

        // Interfaz lógica para crear recursos y una cola de comandos que se envian a la GPU
        let (device, queue) = adapter
            .request_device(&wgpu::DeviceDescriptor {
                label: None,
                required_features: wgpu::Features::empty(),
                required_limits: wgpu::Limits::default(),
                experimental_features: wgpu::ExperimentalFeatures::disabled(),
                memory_hints: Default::default(),
                trace: wgpu::Trace::Off,
            })
            .await?;

        // Una consulta dinamica de las capacidades que varía segun el adaptador que tengas
        let surface_caps: SurfaceCapabilities = surface.get_capabilities(&adapter);

        // Define como se almacenan los píxeles en memoria
        let surface_format: TextureFormat = surface_caps
            .formats
            .iter()
            .find(|f| f.is_srgb())
            .copied()
            .unwrap_or(surface_caps.formats[0]);

        // Definir como se renderiz da (swapchain)
        let config = wgpu::SurfaceConfiguration {
            usage: wgpu::TextureUsages::RENDER_ATTACHMENT,
            format: surface_format,
            width: size.width,
            height: size.height,
            present_mode: surface_caps.present_modes[0],
            desired_maximum_frame_latency: 2,
            alpha_mode: surface_caps.alpha_modes[0],
            view_formats: vec![],
        };

        Ok(Self {
            surface,
            device,
            queue,
            config,
            is_surface_configuration: false,
            window,
        })
    }

    pub fn resize(&mut self, height: u32, width: u32) {
        if height > 0 && width > 0 {
            self.config.height = height;
            self.config.width = width;

            self.surface.configure(&self.device, &self.config);

            self.is_surface_configuration = true;
        }
    }

    pub fn render(&mut self) -> Result<(), wgpu::SurfaceError> {
        self.window.request_redraw();

        if !self.is_surface_configuration {
            return Ok(());
        }

        let ouput: SurfaceTexture = self.surface.get_current_texture()?;

        let view: TextureView = ouput
            .texture
            .create_view(&wgpu::TextureViewDescriptor::default());

        let mut encoder: CommandEncoder =
            self.device
                .create_command_encoder(&wgpu::CommandEncoderDescriptor {
                    label: Some("Render Encoder"),
                });

        {
            let _render_pass = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
                label: Some("Render Pass"),
                color_attachments: &[Some(wgpu::RenderPassColorAttachment {
                    view: &view,
                    depth_slice: None,
                    resolve_target: None,
                    ops: wgpu::Operations {
                        load: wgpu::LoadOp::Clear(wgpu::Color::BLUE),
                        store: wgpu::StoreOp::Store,
                    },
                })],
                depth_stencil_attachment: None,
                timestamp_writes: None,
                occlusion_query_set: None,
            });
        }

        self.queue.submit(std::iter::once(encoder.finish()));
        ouput.present();

        Ok(())
    }
}

impl App {
    pub fn new(#[cfg(target_arch = "wasm32")] event_loop: EventLoop<State>) -> Self {
        #[cfg(target_arch = "wasm32")]
        let proxy = event_loop.create_proxy();

        Self {
            state: None,
            #[cfg(target_arch = "wasm32")]
            proxy,
        }
    }
}

impl ApplicationHandler<State> for App {
    fn resumed(&mut self, event_loop: &ActiveEventLoop) {
        let window_attributes: WindowAttributes = Window::default_attributes();
        let window: Arc<Window> = Arc::new(event_loop.create_window(window_attributes).unwrap());
        self.state = Some(pollster::block_on(State::new(window)).unwrap());
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
            WindowEvent::RedrawRequested => match state.render() {
                Ok(_) => {}
                Err(wgpu::SurfaceError::Lost | wgpu::SurfaceError::Outdated) => {
                    let size: PhysicalSize<u32> = state.window.inner_size();
                    state.resize(size.height, size.width);
                }
                Err(e) => log::error!("No se ha podido renderizar {}", e),
            },
            WindowEvent::Resized(size) => state.resize(size.height, size.width),
            _ => {}
        }
    }

    fn user_event(&mut self, event_loop: &ActiveEventLoop, event: State) {
        self.state = Some(event)
    }
}

pub fn run() -> anyhow::Result<()> {
    #[cfg(not(target_arch = "wasm32"))]
    {
        env_logger::init();
    }

    let event_loop: EventLoop<State> = EventLoop::<State>::with_user_event().build()?;
    let mut app: App = App::new();
    event_loop.run_app(&mut app);

    Ok(())
}

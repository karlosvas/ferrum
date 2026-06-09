pub mod config;

use crate::config::AppConfig;

use ferrum::KeyCode;
#[cfg(target_arch = "wasm32")]
use wasm_bindgen::prelude::*;
#[cfg(target_arch = "wasm32")]
use winit::event_loop::EventLoopProxy;
use {
    ferrum::{State, config::WindowSize},
    std::sync::Arc,
    winit::{
        application::ApplicationHandler,
        dpi::PhysicalSize,
        event::{KeyEvent, WindowEvent},
        event_loop::{ActiveEventLoop, EventLoop},
        keyboard::PhysicalKey,
        window::{Window, WindowAttributes, WindowId},
    },
};

pub type SetupFn = Box<dyn FnOnce(&mut State)>;
pub type UpdateFn = Box<dyn FnMut(&mut State)>;

#[derive(Default)]
pub struct App {
    pub state: Option<State>,
    setup: Option<SetupFn>,
    update: Option<UpdateFn>,
    window: Option<Arc<Window>>,
    config: AppConfig,
    #[cfg(target_arch = "wasm32")]
    pub proxy: Option<EventLoopProxy<State>>,
}

impl AppConfig {
    pub fn new(size: Option<PhysicalSize<u32>>) -> Self {
        Self {
            size: size.unwrap_or_default(),
        }
    }
}

impl App {
    pub fn new(config: AppConfig) -> Self {
        Self {
            state: None,
            setup: None,
            update: None,
            window: None,
            config,
            #[cfg(target_arch = "wasm32")]
            proxy: None,
        }
    }

    pub fn ferrum_setup<F>(mut self, f: F) -> Self
    where
        F: FnOnce(&mut State) + 'static,
    {
        self.setup = Some(Box::new(f));
        self
    }

    pub fn ferrum_update<F>(mut self, f: F) -> Self
    where
        F: FnMut(&mut State) + 'static,
    {
        self.update = Some(Box::new(f));
        self
    }

    pub fn run(mut self) -> anyhow::Result<()> {
        #[cfg(not(target_arch = "wasm32"))]
        {
            env_logger::Builder::from_env(env_logger::Env::default().default_filter_or("info"))
                .init();
        }

        let event_loop: EventLoop<State> = EventLoop::<State>::with_user_event().build()?;

        #[cfg(target_arch = "wasm32")]
        {
            self.proxy = Some(event_loop.create_proxy());
        }

        event_loop.run_app(&mut self)?;

        Ok(())
    }

    #[cfg(target_arch = "wasm32")]
    #[wasm_bindgen(start)]
    pub fn run_web() -> Result<(), wasm_bindgen::JsValue> {
        console_error_panic_hook::set_once();

        let level: log::Level = if cfg!(debug_assertions) {
            log::Level::Debug
        } else {
            log::Level::Warn
        };

        console_log::init_with_level(level).unwrap_throw();
        log::info!("Ferrum engine loaded successfully");

        App::new().run().unwrap_throw();
        Ok(())
    }
}

impl ApplicationHandler<State> for App {
    fn resumed(&mut self, event_loop: &ActiveEventLoop) {
        #[allow(unused_mut)]
        let mut window_attributes: WindowAttributes = Window::default_attributes()
            .with_title("Ferrum")
            .with_inner_size(ferrum::PhysicalSize::new(
                self.config.size.width,
                self.config.size.height,
            ));

        #[cfg(target_arch = "wasm32")]
        {
            use image::ImageBuffer;
            use winit::window::Icon;

            let icon: Icon = {
                let bytes: &[u8] = include_bytes!("../assets/logo.ico");
                let img: ImageBuffer<image::Rgba<u8>, Vec<u8>> =
                    image::load_from_memory(bytes).unwrap().to_rgba8();
                let (w, h): (u32, u32) = img.dimensions();
                Icon::from_rgba(img.into_raw(), w, h)
            }
            .unwrap();

            window_attributes = window_attributes.with_window_icon(Some(icon.clone()));
            use wasm_bindgen::JsCast;
            use winit::platform::web::WindowAttributesExtWebSys;

            const CANVAS_ID: &str = "canvas";
            let window = wgpu::web_sys::window().unwrap_throw();
            let document = window.document().unwrap_throw();

            let canvas = document.get_element_by_id(CANVAS_ID).unwrap_throw();
            let html_canvas = canvas.unchecked_into();
            window_attributes = window_attributes.with_canvas(Some(html_canvas));
        }

        let window: Arc<Window> = Arc::new(event_loop.create_window(window_attributes).unwrap());
        self.window = Some(Arc::clone(&window));
        let inner_size: ferrum::PhysicalSize<u32> = window.inner_size();
        let setup = self.setup.take();

        #[cfg(not(target_arch = "wasm32"))]
        {
            let size: WindowSize = WindowSize::new(inner_size.width, inner_size.height);

            self.state = Some(
                pollster::block_on(async move {
                    let mut state: State = State::new(window, size).await?;
                    if let Some(s) = setup {
                        s(&mut state);
                    }
                    anyhow::Ok(state)
                })
                .unwrap(),
            );
        }
        #[cfg(target_arch = "wasm32")]
        {
            if let Some(proxy) = self.proxy.take() {
                let setup = self.setup.take();
                wasm_bindgen_futures::spawn_local(async move {
                    let mut state = State::new(window, size)
                        .await
                        .expect("Unable to create canvas");
                    if let Some(s) = setup {
                        s(&mut state);
                    }
                    assert!(proxy.send_event(state).is_ok());
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
                state.evolbe();
                if let Some(update) = &mut self.update {
                    update(state);
                }
                match state.render() {
                    Ok(_) => {}
                    Err(wgpu::SurfaceError::Lost | wgpu::SurfaceError::Outdated) => {
                        if let Some(window) = &self.window {
                            let size: PhysicalSize<u32> = window.inner_size();
                            state.resize(size.height, size.width);
                        } else {
                            log::error!("Window not initialized yet");
                        }
                    }
                    Err(e) => log::error!("The app could not be rendered => {}", e),
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
            } => {
                if code == KeyCode::Escape && key_state.is_pressed() {
                    #[cfg(not(target_arch = "wasm32"))]
                    event_loop.exit();
                } else {
                    state
                        .camera_controller
                        .handle_key(code, key_state.is_pressed());
                }
            }
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

    fn about_to_wait(&mut self, _event_loop: &ActiveEventLoop) {
        if let Some(window) = &self.window {
            window.request_redraw();
        }
    }
}

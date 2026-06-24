use ferrum_wgpu::{
    config::{config::FerrumConfig, WindowSize},
    State,
};
use std::sync::Arc;
use winit::{
    application::ApplicationHandler,
    event::WindowEvent,
    event_loop::{ActiveEventLoop, EventLoop},
    window::{Window, WindowId},
};

fn main() -> anyhow::Result<()> {
    let config = FerrumConfig {
        size: WindowSize::new(1000, 500),
        asset: ferrum_wgpu::assets::Asset::new("/res".to_string()),
        ..Default::default()
    };
    App::new(config).run()
}

#[derive(Default)]
struct App {
    state: Option<State>,
    window: Option<Arc<Window>>,
    config: FerrumConfig,
}

impl App {
    fn new(config: FerrumConfig) -> Self {
        Self {
            config,
            ..Default::default()
        }
    }

    fn run(mut self) -> anyhow::Result<()> {
        env_logger::init();
        EventLoop::<()>::new()?.run_app(&mut self)?;
        Ok(())
    }
}

impl ApplicationHandler for App {
    fn resumed(&mut self, event_loop: &ActiveEventLoop) {
        let window = Arc::new(
            event_loop
                .create_window(
                    Window::default_attributes()
                        .with_title("Ferrum")
                        .with_inner_size(ferrum_wgpu::PhysicalSize::new(
                            self.config.size.width,
                            self.config.size.height,
                        )),
                )
                .unwrap(),
        );

        self.window = Some(Arc::clone(&window));
        let size = WindowSize::new(window.inner_size().width, window.inner_size().height);
        let asset = self.config.asset.clone();

        self.state = Some(pollster::block_on(State::new(window, size, asset)).unwrap());
    }

    fn window_event(&mut self, event_loop: &ActiveEventLoop, _: WindowId, event: WindowEvent) {
        let Some(state) = &mut self.state else { return };

        match event {
            WindowEvent::CloseRequested => event_loop.exit(),
            WindowEvent::Resized(s) => state.resize(s.height, s.width),
            WindowEvent::RedrawRequested => {
                state.evolbe();
                if let Err(e) = state.render() {
                    eprintln!("Render error: {e}");
                }
            }
            _ => {}
        }
    }

    fn about_to_wait(&mut self, _: &ActiveEventLoop) {
        if let Some(w) = &self.window {
            w.request_redraw();
        }
    }
}

# ferrum-wgpu

A 3D rendering engine library built with **Rust** and **wgpu** (WebGPU).

[![Crates.io](https://img.shields.io/crates/v/ferrum-wgpu?style=flat-square)](https://crates.io/crates/ferrum-wgpu)
[![Rust](https://img.shields.io/badge/Rust-edition%202024-dea584?logo=rust&style=flat-square)](https://rustup.rs/)
[![wgpu](https://img.shields.io/badge/wgpu-29-76B900?logo=webgpu&style=flat-square)](https://wgpu.rs/)
[![License: GPL v3](https://img.shields.io/badge/License-GPLv3-blue?style=flat-square)](../../LICENSE)

Cross-platform PBR rendering engine that runs on **desktop** (Windows, Linux, macOS), **browser** (WebAssembly via WebGPU) and **Raspberry Pi**.

## Installation

```bash
cargo add ferrum-wgpu
```

Or in `Cargo.toml`:

```toml
[dependencies]
ferrum-wgpu = "x.x.x"
```

## Features

| Feature | Default | Description                                                             |
| ------- | ------- | ----------------------------------------------------------------------- |
| `rpi`   | no      | Enables OpenGL ES backend for Raspberry Pi. Disables Vulkan/Metal/DX12. |

Enable with:

```toml
ferrum-wgpu = { version = "x.x.x", features = ["rpi"] }
```

## Quick Start

In this case you can see the basic window example to setup your app.
For more complete examples, see the [`/examples`](./examples) directory.

```rust
use ferrum_wgpu::{
    State,
    config::{WindowSize, config::FerrumConfig},
};
use std::collections::HashMap;
use {
    ferrum_wgpu::KeyCode,
    std::sync::Arc,
    winit::{
        application::ApplicationHandler,
        event::{KeyEvent, WindowEvent},
        event_loop::{ActiveEventLoop, EventLoop},
        keyboard::PhysicalKey,
        window::{Window, WindowId},
    },
};

fn main() -> anyhow::Result<()> {
    let demo_models: HashMap<&str, usize> = HashMap::new();

    let app_config: FerrumConfig = FerrumConfig {
        size: WindowSize::new(1000, 500),
        asset: ferrum_wgpu::assets::Asset::new("/res".to_string()),
        ..Default::default()
    };

    App::new(app_config)
        .ferrum_setup(move |state: &mut State| setup(state, &demo_models))
        .ferrum_update(|state: &mut State| update(state))
        .run()?;

    Ok(())
}

pub fn setup(_state: &mut State, _demo_models: &HashMap<&str, usize>) {}
pub fn update(_state: &mut State) {}

pub type SetupFn = Box<dyn FnOnce(&mut State)>;
pub type UpdateFn = Box<dyn FnMut(&mut State)>;

#[derive(Default)]
pub struct App {
    pub state: Option<State>,
    setup: Option<SetupFn>,
    update: Option<UpdateFn>,
    window: Option<Arc<Window>>,
    config: FerrumConfig,
}

impl App {
    pub fn new(config: FerrumConfig) -> Self {
        Self {
            state: None,
            setup: None,
            update: None,
            window: None,
            config,
        }
    }

    pub fn ferrum_setup<F: FnOnce(&mut State) + 'static>(mut self, f: F) -> Self {
        self.setup = Some(Box::new(f));
        self
    }

    pub fn ferrum_update<F: FnMut(&mut State) + 'static>(mut self, f: F) -> Self {
        self.update = Some(Box::new(f));
        self
    }

    pub fn run(mut self) -> anyhow::Result<()> {
        env_logger::Builder::from_env(env_logger::Env::default().default_filter_or("info")).init();

        let event_loop: EventLoop<State> = EventLoop::<State>::with_user_event().build()?;
        event_loop.run_app(&mut self)?;
        Ok(())
    }
}

impl ApplicationHandler<State> for App {
    fn resumed(&mut self, event_loop: &ActiveEventLoop) {
        let window_attributes = Window::default_attributes()
            .with_title("Ferrum")
            .with_inner_size(ferrum_wgpu::PhysicalSize::new(
                self.config.size.width,
                self.config.size.height,
            ));

        let window: Arc<Window> = Arc::new(event_loop.create_window(window_attributes).unwrap());
        self.window = Some(Arc::clone(&window));

        let inner_size = window.inner_size();
        let size = WindowSize::new(inner_size.width, inner_size.height);
        let setup = self.setup.take();
        let asset = self.config.asset.clone();

        self.state = Some(
            pollster::block_on(async move {
                let mut state = State::new(window, size, asset).await?;
                if let Some(s) = setup {
                    s(&mut state);
                }
                anyhow::Ok(state)
            })
            .unwrap(),
        );
    }

    fn window_event(
        &mut self,
        event_loop: &ActiveEventLoop,
        _window_id: WindowId,
        event: WindowEvent,
    ) {
        let state = match &mut self.state {
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
                    Err(ferrum_wgpu::SurfaceError::Lost | ferrum_wgpu::SurfaceError::Outdated) => {
                        if let Some(window) = &self.window {
                            let size = window.inner_size();
                            state.resize(size.height, size.width);
                        }
                    }
                    Err(e) => eprint!("Render error: {e}"),
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
                        .camera
                        .controller
                        .handle_key(code, key_state.is_pressed());
                }
            }
            _ => {}
        }
    }

    fn user_event(&mut self, _event_loop: &ActiveEventLoop, event: State) {
        self.state = Some(event);
    }

    fn about_to_wait(&mut self, _event_loop: &ActiveEventLoop) {
        if let Some(window) = &self.window {
            window.request_redraw();
        }
    }
}
```

## Docs

Full documentation is not available yet. For now, refer to the demo example
to understand basic usage. Below is a brief summary of the main concepts:

## API Overview

| Method                                 | Description                                            |
| -------------------------------------- | ------------------------------------------------------ |
| `State::new(window, size)`             | Initialize GPU device, surface, pipelines and sky      |
| `state.spawn_model(desc)`              | Async-load a `.obj` model and add it to the scene      |
| `state.evolbe()`                       | Per-frame tick: collect loaded models, update uniforms |
| `state.render()`                       | Submit render pass and present the frame               |
| `state.render_with_overlay(callback)`  | Render with an egui overlay pass                       |
| `state.set_wind(direction, intensity)` | Set wind vector that animates foliage                  |
| `state.resize(width, height)`          | Handle window resize                                   |

## Capabilities

- PBR rendering with diffuse/specular lighting, tangent-space normal maps, HDR pipeline and ACES tonemapping
- Skybox from equirectangular HDR/EXR images converted to cubemap via compute shaders
- Animated directional light with orbital rotation and shadow maps
- Instancing for efficient multi-object rendering
- Free camera with WASD / arrow key controls
- Async resource loading on both native and WASM targets

## Graphics Backends

| Platform                | Backend                          |
| ----------------------- | -------------------------------- |
| Windows / macOS / Linux | Vulkan, Metal, DX12              |
| Web (WASM)              | WebGPU (required — not WebGL2)   |
| Raspberry Pi            | OpenGL ES (enable `rpi` feature) |

## Demo & Source

Full project, live demo and Raspberry Pi integration: [github.com/karlosvas/ferrum](https://github.com/karlosvas/ferrum)

## License

GNU General Public License v3.0 — see [LICENSE](../../LICENSE).

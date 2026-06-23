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
ferrum = { package = "ferrum-wgpu", version = "0.1" }
```

## Features

| Feature | Default | Description |
|---------|---------|-------------|
| `rpi`   | no      | Enables OpenGL ES backend for Raspberry Pi. Disables Vulkan/Metal/DX12. |

Enable with:

```toml
ferrum = { package = "ferrum-wgpu", version = "0.1", features = ["rpi"] }
```

## Quick Start

```rust
use ferrum::{State, config::WindowSize};

// Create the engine state (requires a winit Window or equivalent handle)
let state = State::new(&window, WindowSize { width: 1280, height: 720 }).await?;

// Load a 3D model
let _model = state.spawn_model(ModelDesc { path: "res/plant/plant.obj", .. });

// Main loop
loop {
    state.evolbe();          // update camera, light and wind uniforms
    state.render()?;         // draw frame
}
```

## API Overview

| Method | Description |
|--------|-------------|
| `State::new(window, size)` | Initialize GPU device, surface, pipelines and sky |
| `state.spawn_model(desc)` | Async-load a `.obj` model and add it to the scene |
| `state.evolbe()` | Per-frame tick: collect loaded models, update uniforms |
| `state.render()` | Submit render pass and present the frame |
| `state.render_with_overlay(callback)` | Render with an egui overlay pass |
| `state.set_wind(direction, intensity)` | Set wind vector that animates foliage |
| `state.resize(width, height)` | Handle window resize |

## Capabilities

- PBR rendering with diffuse/specular lighting, tangent-space normal maps, HDR pipeline and ACES tonemapping
- Skybox from equirectangular HDR/EXR images converted to cubemap via compute shaders
- Animated directional light with orbital rotation and shadow maps
- Instancing for efficient multi-object rendering
- Free camera with WASD / arrow key controls
- Async resource loading on both native and WASM targets

## Graphics Backends

| Platform                | Backend                              |
|-------------------------|--------------------------------------|
| Windows / macOS / Linux | Vulkan, Metal, DX12                  |
| Web (WASM)              | WebGPU (required — not WebGL2)       |
| Raspberry Pi            | OpenGL ES (enable `rpi` feature)     |

## Demo & Source

Full project, live demo and Raspberry Pi integration: [github.com/karlosvas/ferrum](https://github.com/karlosvas/ferrum)

## License

GNU General Public License v3.0 — see [LICENSE](../../LICENSE).

<div align="center">
  <img src="www/public/logo/favicon.svg" alt="Ferrum Logo" width="120"/>

# Ferrum ⚙️

**A 3D rendering engine built from scratch in Rust + wgpu (WebGPU)**

[![Rust](https://img.shields.io/badge/Rust-edition%202024-dea584?logo=rust&style=flat-square)](https://rustup.rs/)
[![wgpu](https://img.shields.io/badge/wgpu-0.28-76B900?logo=webgpu&style=flat-square)](https://wgpu.rs/)
[![Astro](https://img.shields.io/badge/Astro-5-BC52EE?logo=astro&style=flat-square)](https://astro.build/)
[![License: GPL v3](https://img.shields.io/badge/License-GPLv3-blue?style=flat-square)](LICENSE)
[![Vercel](https://img.shields.io/badge/Deploy-Vercel-000000?logo=vercel&style=flat-square)](https://vercel.com)

  <br/>

<a href="#features">Features</a> •
<a href="#live-demo">Live Demo</a> •
<a href="#browser-requirements-webgpu">Browser Requirements</a> •
<a href="#getting-started">Getting Started</a> •
<a href="#architecture">Architecture</a> •
<a href="#license">License</a>

  <br/>
</div>

Ferrum is a general-purpose 3D rendering engine built with **Rust** and **wgpu** (WebGPU). It implements its own graphics pipeline from scratch — every effect is hand-written — and lets you load your own `.obj` models, place them in a scene and render them with dynamic lighting and wind in real time.

Developed as a final degree project, it combines a hand-built lighting pipeline (HDR + ACES, shadow mapping with PCF, tangent-space normal mapping) with a cross-platform architecture that runs natively on **desktop** (Windows, Linux, macOS), in the **browser** (WebAssembly / WebGPU), and ships with an interactive demo driven by a **Raspberry Pi** with physical sensors.

---

## Features

- **Real-time lighting** (Blinn-Phong diffuse/specular) with tangent-space normal mapping, an HDR pipeline and ACES tonemapping
- **Shadow mapping** with 3×3 PCF for soft shadow edges
- **Skybox** from equirectangular HDR/EXR images converted to a cubemap with a compute shader
- **GPU wind animation** (height-weighted vertex sway) for foliage and similar geometry
- **Animated directional light** with orbital rotation
- **Instancing** for efficient multi-object rendering
- **Free camera** with WASD / arrow key controls
- **Asynchronous model loading** with typed handles — the render loop never blocks on meshes (native and WASM)
- **Interactive demo** with Raspberry Pi and physical sensors
- **Web visualization** via WebAssembly powered by WebGPU
- **Stripe payment** integration for commercial licensing
- **WebSockets** for real-time sensor data streaming

---

## Live Demo

Try the engine directly from your browser on the [demo page](https://ferrum.dev/demo). The engine compiles to WebAssembly and runs inside a `<canvas>` element via wgpu on **WebGPU** — no installation required.

> ⚠️ **The demo requires a WebGPU-capable browser.** See [Browser Requirements](#browser-requirements-webgpu) below before reporting a blank canvas.

### Raspberry Pi Demo

The included demo connects a **Raspberry Pi** with the following sensors to drive the 3D scene from the real world (the shipped demo scene happens to be a potted plant, but the engine itself is model-agnostic):

| Component                  | Connection         | Purpose                                                                         |
| -------------------------- | ------------------ | ------------------------------------------------------------------------------- |
| **TSL2591**                | I2C (`/dev/i2c-1`) | Ambient light sensor — measures lux, full spectrum and infrared                 |
| **ADS1115**                | External ADC       | Analog-to-digital converter for the microphones                                 |
| **4× MAX446**              | ADC input pins     | Microphones that detect wind (blowing) and translate it into the scene's wind   |
| **Wide Angle 120º Camera** | CSI                | Detects light direction to orient the scene illumination                        |

Sensor data is streamed to the engine in real time via **WebSockets**, so real-world light and wind directly affect the 3D scene.

---

## Browser Requirements (WebGPU)

> **The web demo requires WebGPU. It will _not_ run on WebGL2.**

This is not optional. The skybox is built by converting an equirectangular HDR/EXR
into a cubemap using a **compute shader** (`shaders/equirectangular.wgsl`). Compute
shaders **do not exist in WebGL2**, so the engine genuinely needs the WebGPU backend.
If the browser cannot provide a WebGPU adapter, `request_adapter()` returns `None`,
`State::new` fails, and the canvas stays black.

The demo page detects this in `Demo.astro` (it calls `navigator.gpu.requestAdapter()`
before loading the WASM) and shows an on-screen warning instead of a blank canvas.

### Supported browsers

| Browser                   | WebGPU status                                                                       |
| ------------------------- | ----------------------------------------------------------------------------------- |
| **Chrome / Edge / Brave** | ✅ Enabled by default on Windows/macOS. **On Linux it must be enabled via a flag.** |
| **Firefox**               | ⚠️ Behind a flag on Linux (`dom.webgpu.enabled`); most reliable on Nightly.         |
| **Safari**                | ✅ WebGPU on recent versions (macOS / iOS 18+).                                     |

### Enabling WebGPU on Linux (Chromium — Chrome/Brave/Edge)

Chromium does **not** enable WebGPU by default on Linux, even when the system GPU and
Vulkan work perfectly. Enable it:

1. Open `chrome://flags` (Brave: `brave://flags`, Edge: `edge://flags`).
2. Set **`#enable-unsafe-webgpu`** → **Enabled** (the important one).
3. If present, set **`#enable-vulkan`** → **Enabled**.
4. **Fully restart** the browser (close every window).

Or launch from a terminal with the flags:

```bash
brave --enable-unsafe-webgpu --enable-features=Vulkan
# or
google-chrome-stable --enable-unsafe-webgpu --enable-features=Vulkan
```

On Wayland, if it still fails, force the backend:

```bash
brave --enable-unsafe-webgpu --enable-features=Vulkan --use-angle=vulkan --ozone-platform=x11
```

### Enabling WebGPU on Firefox (Linux)

1. Open `about:config`.
2. Set `dom.webgpu.enabled` → `true` (and `gfx.webgpu.force-enabled` → `true` if available).
3. Restart Firefox. Consider **Firefox Nightly** for better Linux WebGPU support.

### System prerequisites

Browser WebGPU on Linux is implemented on top of **Vulkan**, so you need working
Vulkan drivers:

```bash
vulkaninfo --summary   # must list your GPU
```

> The native build (`cargo run -p demo`) uses Vulkan directly, so if the desktop
> app renders but the browser does not, the problem is browser-side WebGPU enablement,
> **not** your drivers or the engine.

### Verifying

- Check your browser's GPU diagnostics page (e.g. `chrome://gpu` in Chromium-based
  browsers, `about:support` in Firefox) and look for the **WebGPU** line — it should
  report that it is hardware accelerated / enabled.
- Or visit **[webgpureport.org](https://webgpureport.org)**: if it reports no adapter,
  the issue is browser configuration, not Ferrum.

### Quality on web vs. native

The native build adapts to your hardware and renders at **full quality**. The web build
is intentionally **scaled down** — textures, sky and other assets are swapped for
lighter, lower-resolution versions (selected via `cfg(target_arch = "wasm32")`), so the
demo looks noticeably less detailed in the browser than the desktop application. This
trade-off keeps the experience within the limits any browser engine can render:

- Browser WebGPU caps `max_texture_dimension_2d` at **8192**, so very large textures would
  fail validation.
- Full-resolution assets decode to several GiB, exceeding the **4 GiB** wasm32 address
  space.

---

## Pricing

| Plan           | Price           | Includes                                                               |
| -------------- | --------------- | ---------------------------------------------------------------------- |
| **FREE**       | $0              | 3 active models, WebGL export, Discord community                       |
| **STUDIO**     | $100 (lifetime) | Unlimited models, WASM+WebGL export, source code access                |
| **ENTERPRISE** | Custom          | Multi-site licenses, 24/7 support, custom integrations                 |

Every user can upload their own 3D models and render them inside the engine.

---

## Tech Stack

| Layer           | Technology                                                                   |
| --------------- | ---------------------------------------------------------------------------- |
| **Language**    | Rust (edition 2024)                                                          |
| **Graphics**    | [wgpu](https://wgpu.rs/) 0.28                                                |
| **Windowing**   | [winit](https://github.com/rust-windowing/winit) 0.30                        |
| **Shading**     | WGSL (Blinn-Phong, HDR + ACES, shadow mapping, skybox, equirectangular → cubemap) |
| **Math**        | cgmath 0.18                                                                  |
| **3D Models**   | Wavefront .obj (async loading via tobj)                                      |
| **Textures**    | PNG, JPEG, HDR, EXR, ICO                                                     |
| **Frontend**    | [Astro](https://astro.build/) 5 + [Tailwind CSS](https://tailwindcss.com/) 4 |
| **WebAssembly** | wasm-bindgen + wasm-pack                                                     |
| **Web assets**  | Cloudflare R2                                                                |
| **Pi sensors**  | linux-embedded-hal + tsl2591-rs (I2C)                                        |
| **Build tools** | Cargo xtask                                                                  |
| **Deploy**      | Vercel                                                                       |
| **Payments**    | Stripe                                                                       |

### Graphics Backends

| Platform                | wgpu Backend                                                                     |
| ----------------------- | -------------------------------------------------------------------------------- |
| Windows / macOS / Linux | Vulkan, Metal, DX12                                                              |
| Web (WASM)              | **WebGPU (required)** — see [Browser Requirements](#browser-requirements-webgpu) |
| Raspberry Pi            | OpenGL ES                                                                        |

---

## Getting Started

### Prerequisites

- [Rust](https://rustup.rs/) — `rustup update`
- [wasm-pack](https://rustwasm.github.io/wasm-pack/) — `cargo install wasm-pack`
- [Node.js](https://nodejs.org/) 18+
- For Raspberry Pi: [`cross`](https://github.com/cross-rs/cross) and `aarch64-unknown-linux-gnu` toolchain

### Desktop

```bash
cargo run -p demo
```

### Web (WASM)

```bash
cargo xtask web
```

Compiles the engine to WebAssembly and outputs it to `www/public/pkg/` for the demo page.

### Frontend (dev server)

```bash
cd www && npm install && npm run dev
```

### Raspberry Pi

```bash
export PI_USER="pi"
export PI_HOST="192.168.1.x"
cargo xtask rpi
```

Cross-compiles with `cross`, deploys via SCP, and runs the binary on the Raspberry Pi.

### Deploy web

```bash
cargo xtask vercel-deploy
```

### All together (Pi + desktop)

```bash
cargo xtask run
```

---

## Architecture

Ferrum is a Cargo workspace split into focused crates. The engine itself
(`ferrum`) is independent of the demo and the sensor firmware — everything else
just consumes its public API.

| Crate / dir | Role |
| ----------- | ---- |
| **`crates/ferrum`** | The rendering engine: graphics pipeline, lighting (HDR + ACES, shadows, normal mapping), skybox, camera, async model loading and the WGSL shaders. This is the reusable library. |
| **`crates/demo`** | The interactive application that drives the engine — windowing, UI overlay, and the real-time scene fed by Raspberry Pi sensor data over WebSockets. |
| **`crates/rpi`** | Firmware that runs on the Raspberry Pi: reads the I²C sensors (light, microphones, camera) and streams readings to the demo. |
| **`crates/shared`** | Data structures shared between the demo and the Pi firmware (serialized over the wire). |
| **`crates/xtask`** | Build automation — `web`, `rpi`, `vercel-deploy`, `run` commands. |
| **`www/`** | Astro + Tailwind web frontend that hosts the WASM build and the demo page. |

> The `ferrum` engine crate has no dependency on `demo`, `rpi` or `shared`, so it
> can be reused as a standalone 3D renderer.

---

## License

**GNU General Public License v3.0** — see [LICENSE](LICENSE).

---

<div align="center">
  <sub>Built as a Bachelor's Thesis project</sub>
</div>

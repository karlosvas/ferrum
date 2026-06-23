use {
    crate::renderer::uniform_layout,
    wgpu::{BindGroup, BindGroupLayout, Buffer, Device, Queue, util::DeviceExt},
};

/// Wind state and its GPU resources. The demo sets direction/intensity via
/// [`WindRig::set`]; the phase advances and is uploaded once per frame in
/// [`WindRig::update`].
pub struct WindRig {
    pub uniform: WindUniform,
    pub buffer: Buffer,
    pub bind_group: BindGroup,
    pub layout: BindGroupLayout,
    /// Instant of the last frame to accumulate the wind phase. `time` is not
    /// real time: it advances faster the greater the intensity, so that a
    /// strong gust shakes the leaves faster and bends them more.
    pub start: web_time::Instant,
}

impl WindRig {
    pub fn new(device: &Device) -> Self {
        let uniform: WindUniform = WindUniform {
            intensity: 0.8,
            ..Default::default()
        };

        let buffer: Buffer = device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
            label: Some("wind_buffer"),
            contents: bytemuck::cast_slice(&[uniform]),
            usage: wgpu::BufferUsages::UNIFORM | wgpu::BufferUsages::COPY_DST,
        });

        let layout: BindGroupLayout =
            uniform_layout(device, "wind_bind_group_layout", wgpu::ShaderStages::VERTEX);

        let bind_group: BindGroup = device.create_bind_group(&wgpu::BindGroupDescriptor {
            label: Some("wind_bind_group"),
            layout: &layout,
            entries: &[wgpu::BindGroupEntry {
                binding: 0,
                resource: buffer.as_entire_binding(),
            }],
        });

        Self {
            uniform,
            buffer,
            bind_group,
            layout,
            start: web_time::Instant::now(),
        }
    }

    /// Fixes the wind that animates the foliage. `direction` is a 2D vector in
    /// the XZ plane (x = right/left, y = forward/backward) and `intensity` the
    /// force [0, 1]. It only stores the values; `time` and the GPU upload are
    /// handled each frame by [`WindRig::update`]. The direction is normalized
    /// so that the intensity alone controls the magnitude of the swaying.
    pub fn set(&mut self, direction: [f32; 2], intensity: f32) {
        let len: f32 = (direction[0] * direction[0] + direction[1] * direction[1]).sqrt();
        self.uniform.direction = if len > 1e-6 {
            [direction[0] / len, direction[1] / len]
        } else {
            [0.0, 0.0]
        };
        self.uniform.intensity = intensity.clamp(0.0, 1.0);
    }

    /// Advances the wind PHASE and uploads the uniform to the GPU.
    /// The phase is accumulated based on intensity instead of using real-world
    /// time: blowing hard stirs the leaves faster, not just farther.
    /// Accumulating (instead of multiplying by time) prevents phase jumps when
    /// the intensity changes between frames.
    pub fn update(&mut self, queue: &Queue) {
        let dt: f32 = self.start.elapsed().as_secs_f32();
        self.start = web_time::Instant::now();
        let speed: f32 = 0.6 + self.uniform.intensity * 2.4;
        self.uniform.time += dt * speed;
        queue.write_buffer(&self.buffer, 0, bytemuck::cast_slice(&[self.uniform]));
    }
}

/// Datos de viento que recibe el vertex shader para animar el follaje.
///
/// `direction` es un vector 2D en el plano XZ (suelo) ya normalizado, `intensity`
/// la fuerza del viento [0, 1] y `time` segundos acumulados para la animación.
/// Los 4 f32 ocupan exactamente 16 bytes => alineación válida de uniform.
#[repr(C)]
#[derive(Debug, Clone, Copy, bytemuck::Pod, bytemuck::Zeroable)]
pub struct WindUniform {
    pub direction: [f32; 2],
    pub intensity: f32,
    pub time: f32,
}

impl Default for WindUniform {
    fn default() -> Self {
        Self {
            direction: [1.0, 0.0],
            intensity: 0.0,
            time: 0.0,
        }
    }
}

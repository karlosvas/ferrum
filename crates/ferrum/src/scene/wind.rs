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

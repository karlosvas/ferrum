/// Fallos al adquirir el frame de la superficie. Espejo simplificado del
/// `CurrentSurfaceTexture` de wgpu (que en la v29 dejó de ser un `Result`)
/// para que los consumidores del motor no dependan de los detalles de wgpu.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SurfaceError {
    /// La superficie cambió (resize); reconfigura con `State::resize`.
    Outdated,
    /// La superficie se perdió; reconfigura con `State::resize`.
    Lost,
    /// Error de validación interno de wgpu.
    Validation,
}

impl std::fmt::Display for SurfaceError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            SurfaceError::Outdated => write!(f, "surface outdated"),
            SurfaceError::Lost => write!(f, "surface lost"),
            SurfaceError::Validation => write!(f, "surface validation error"),
        }
    }
}

/// Failed to get surface frame. Simplificated mirror of `wgpu::CurrentSurfaceTexture`
/// (because in v29 its not `Result`)
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SurfaceError {
    /// The surface has changed
    Outdated,
    /// The suface has lossed
    Lost,
    /// Error validatios of intenal wgpu
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

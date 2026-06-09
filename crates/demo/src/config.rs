pub struct AppConfig {
    pub size: ferrum::PhysicalSize<u32>,
}

impl Default for AppConfig {
    fn default() -> Self {
        Self {
            size: ferrum::PhysicalSize::new(500, 500),
        }
    }
}

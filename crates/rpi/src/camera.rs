use crate::ComunicationError;
use image::{GenericImageView, ImageReader};
use shared::structs::Camera3Wide;
use std::process::Command;

const IMAGE_WIDTH: u32 = 2304;
const IMAGE_HEIGHT: u32 = 1296;
const DEFAULT_BORDER_PERCENT: u32 = 10;
const CAPTURE_PATH: &str = "/tmp/light_detect.jpg";

/// Radius (in world units) covered by half the image when projecting.
const WORLD_RADIUS: f64 = 10.0;
/// Fixed height at which the light sits above the plane (camera pointing UP).
const LIGHT_HEIGHT: f64 = 10.0;

pub struct CameraSensor {
    /// Percentage of the border excluded from the analysis
    pub border_percent: u32,
}

impl CameraSensor {
    pub fn new() -> Self {
        Self {
            border_percent: DEFAULT_BORDER_PERCENT,
        }
    }

    /// Captures an image and returns the angle of the brightest light.
    /// Meant to be used as a data producer in the `main` loop.
    pub fn read(&self) -> Camera3Wide {
        match self.capture_and_analyze() {
            Ok(camera) => camera,
            Err(e) => {
                log::error!("Camera error: {e}");
                Camera3Wide::default()
            }
        }
    }

    fn capture_and_analyze(&self) -> Result<Camera3Wide, ComunicationError> {
        self.capture_locally(CAPTURE_PATH)?;
        self.analyze_image(CAPTURE_PATH)
    }

    fn capture_locally(&self, output_path: &str) -> Result<(), ComunicationError> {
        let status = Command::new("rpicam-jpeg")
            .args([
                "-o",
                output_path,
                "--width",
                &IMAGE_WIDTH.to_string(),
                "--height",
                &IMAGE_HEIGHT.to_string(),
                "--timeout",
                "2000",
                "--nopreview",
            ])
            .status()
            .map_err(|e| ComunicationError::Camera(format!("Failed to run rpicam-jpeg: {e}")))?;

        if !status.success() {
            return Err(ComunicationError::Camera(format!(
                "rpicam-jpeg failed with status: {status}"
            )));
        }
        Ok(())
    }

    fn analyze_image(&self, image_path: &str) -> Result<Camera3Wide, ComunicationError> {
        let img = ImageReader::open(image_path)
            .map_err(|e| ComunicationError::Camera(format!("Failed to open image: {e}")))?
            .decode()
            .map_err(|e| ComunicationError::Camera(format!("Failed to decode image: {e}")))?;

        let (w, h) = img.dimensions();

        // Convert to grayscale for the brightness analysis
        let gray = img.to_luma8();

        // Border exclusion
        let bx = (w * self.border_percent / 100).max(1);
        let by = (h * self.border_percent / 100).max(1);
        let sx = bx;
        let ex = w - bx;
        let sy = by;
        let ey = h - by;

        // Find the brightest pixel
        let mut max_bright = 0.0f32;
        let mut max_x = 0;
        let mut max_y = 0;

        for y in sy..ey {
            for x in sx..ex {
                let pixel = gray.get_pixel(x, y);
                let brightness = pixel[0] as f32;
                if brightness > max_bright {
                    max_bright = brightness;
                    max_x = x;
                    max_y = y;
                }
            }
        }

        let (x, y, z) = project_pixel_to_world(max_x, max_y, w, h);

        Ok(Camera3Wide { x, y, z })
    }
}

/// Projects an image pixel into the 3D world space.
///
/// This function isolates the "position source" heuristic (currently, the
/// projection of the brightest pixel) so it can be replaced in the future by
/// another system (e.g. ArUco marker detection) without touching the rest of the flow.
///
/// Convention: the camera points UP, so the image plane maps to the world's
/// horizontal X-Z plane and the light sits at a fixed height `Y`.
/// The image `y` coordinate grows downwards, hence the negative sign on Z.
fn project_pixel_to_world(px: u32, py: u32, width: u32, height: u32) -> (f64, f64, f64) {
    // Normalize to [-1, 1] relative to the image center.
    let nx = (px as f64 / width as f64) * 2.0 - 1.0;
    let ny = (py as f64 / height as f64) * 2.0 - 1.0;

    let world_x = nx * WORLD_RADIUS;
    let world_z = -ny * WORLD_RADIUS;

    (world_x, LIGHT_HEIGHT, world_z)
}

impl Default for CameraSensor {
    fn default() -> Self {
        Self::new()
    }
}

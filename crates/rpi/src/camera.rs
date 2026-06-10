use crate::ComunicationError;
use image::{GenericImageView, ImageReader};
use shared::structs::Camera3Wide;
use std::process::Command;

const IMAGE_WIDTH: u32 = 2304;
const IMAGE_HEIGHT: u32 = 1296;
const DEFAULT_BORDER_PERCENT: u32 = 10;
const CAPTURE_PATH: &str = "/tmp/light_detect.jpg";

/// Radio (en unidades de mundo) que abarca media imagen al proyectar.
const WORLD_RADIUS: f64 = 10.0;
/// Altura fija a la que se sitúa la luz sobre el plano (cámara apuntando hacia ARRIBA).
const LIGHT_HEIGHT: f64 = 10.0;

pub struct CameraSensor {
    /// Porcentaje de borde excluido del análisis
    pub border_percent: u32,
}

impl CameraSensor {
    pub fn new() -> Self {
        Self {
            border_percent: DEFAULT_BORDER_PERCENT,
        }
    }

    /// Captura una imagen y devuelve el ángulo de la luz más brillante.
    /// Pensado para usarse como productor de datos en el bucle de `main`.
    pub fn read(&self) -> Camera3Wide {
        match self.capture_and_analyze() {
            Ok(camera) => camera,
            Err(e) => {
                eprintln!("Camera error: {e}");
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

        // Convertir a escala de grises para el análisis de brillo
        let gray = img.to_luma8();

        // Exclusión de borde
        let bx = (w * self.border_percent / 100).max(1);
        let by = (h * self.border_percent / 100).max(1);
        let sx = bx;
        let ex = w - bx;
        let sy = by;
        let ey = h - by;

        // Buscar el píxel más brillante
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

/// Proyecta un píxel de la imagen al espacio 3D del mundo.
///
/// Esta función aísla la heurística de "origen de la posición" (actualmente, la
/// proyección del píxel más brillante) para que pueda sustituirse en el futuro por
/// otro sistema (p. ej. detección de marcadores ArUco) sin tocar el resto del flujo.
///
/// Convención: la cámara apunta hacia ARRIBA, por lo que el plano de la imagen
/// se mapea al plano horizontal X-Z del mundo y la luz se sitúa a una altura fija `Y`.
/// La coordenada `y` de la imagen crece hacia abajo, de ahí el signo negativo en Z.
fn project_pixel_to_world(px: u32, py: u32, width: u32, height: u32) -> (f64, f64, f64) {
    // Normalizar a [-1, 1] respecto al centro de la imagen.
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

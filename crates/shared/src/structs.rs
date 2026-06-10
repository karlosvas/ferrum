use serde::{Deserialize, Serialize};
use tsl2591_rs::driver::SensorReading;

#[derive(Serialize, Deserialize, Default, Debug, Clone, Copy)]
pub struct MicReading {
    pub channel: u8,
    pub raw: u16,
}

/// Posición 3D estimada de la fuente de luz a partir de la imagen de la cámara.
/// Se obtiene proyectando el píxel más brillante al espacio del mundo.
#[derive(Serialize, Deserialize, Default, Debug, Clone, Copy)]
pub struct Camera3Wide {
    pub x: f64,
    pub y: f64,
    pub z: f64,
}

#[derive(Serialize, Deserialize, Debug)]
pub struct RpiDemo {
    pub light: SensorReading,
    pub microphone: [MicReading; 4],
    pub camera: Camera3Wide,
}

impl RpiDemo {
    pub fn new(light: SensorReading, microphone: [MicReading; 4], camera: Camera3Wide) -> Self {
        Self {
            light,
            microphone,
            camera,
        }
    }
}

impl Default for RpiDemo {
    fn default() -> Self {
        Self {
            light: SensorReading {
                lux: 0.0,
                infrared: 0,
                full_spectrum: 0,
            },
            microphone: std::array::from_fn(|i| MicReading {
                channel: i as u8,
                raw: 0,
            }),
            camera: Camera3Wide::default(),
        }
    }
}

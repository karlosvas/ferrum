use serde::{Deserialize, Serialize};
use tsl2591_rs::driver::SensorReading;

#[derive(Serialize, Deserialize, Default, Debug, Clone, Copy)]
pub struct MicReading {
    pub channel: u8,
    pub raw: u16,
}

#[derive(Serialize, Deserialize, Default, Debug)]
pub struct Camera3Wide {
    angule_of_max_light: f64,
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

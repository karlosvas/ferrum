use serde::{Deserialize, Serialize};
use tsl2591_rs::driver::SensorReading;

#[derive(Serialize, Deserialize, Default, Debug)]
pub struct Microphone {
    intensity: f64,
}

#[derive(Serialize, Deserialize, Default, Debug)]
pub struct Camera3Wide {
    angule_of_max_light: f64,
}

#[derive(Serialize, Deserialize, Debug)]
pub struct RpiDemo {
    pub light: SensorReading,
    pub microphone: Vec<Microphone>,
    pub camera: Camera3Wide,
}

impl RpiDemo {
    pub fn new(light: SensorReading, microphone: Vec<Microphone>, camera: Camera3Wide) -> Self {
        Self {
            light,
            microphone,
            camera,
        }
    }
}

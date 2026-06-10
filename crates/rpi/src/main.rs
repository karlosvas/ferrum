mod camera;
mod light;
mod microphone;

use futures_util::SinkExt;
use linux_embedded_hal::{Delay, I2cdev};
use shared::structs::{Camera3Wide, RpiDemo};
use std::sync::{Arc, Mutex};
use std::time::Duration;
use tokio_tungstenite::{connect_async, tungstenite::Message};
use tsl2591_rs::{AdafruitTSL2591, driver::SensorReading};

use crate::{
    camera::CameraSensor,
    light::LightSensor,
    microphone::{MICROPHONES_SIZE, MicrophoneSensor},
};
use {thiserror::Error, tsl2591_rs::driver::Tsl2591Error};

#[tokio::main]
async fn main() -> Result<(), ComunicationError> {
    dotenvy::dotenv().ok();

    let ip_host: String = std::env::var("IP_HOST").unwrap_or_else(|_| "127.0.0.1".to_string());
    let url: String = format!("ws://{}:3000/demo", ip_host);

    let mut socket = loop {
        match connect_async(&url).await {
            Ok((s, _)) => break s,
            Err(e) => {
                eprintln!("Error conection to {url} ({e}), retrying in 2s...");
                tokio::time::sleep(Duration::from_secs(2)).await;
            }
        }
    };

    let mut light_sensor: AdafruitTSL2591<I2cdev, Delay> = LightSensor::setup().await?;
    let mut microphone_sensor: MicrophoneSensor = MicrophoneSensor::new();

    eprintln!("Calibrating microphone noise floor, keep quiet...");
    microphone_sensor.calibrate();

    // La cámara captura con rpicam-jpeg (~2-3s por foto), así que vive en su
    // propio hilo y publica la última posición; el bucle de muestreo nunca se
    // bloquea esperándola. Sin esto los micros solo se actualizaban cada ~3.5s
    // y era imposible detectar un soplido.
    let camera_pos: Arc<Mutex<Camera3Wide>> = Arc::new(Mutex::new(Camera3Wide::default()));
    {
        let camera_pos = Arc::clone(&camera_pos);
        std::thread::spawn(move || {
            let camera_sensor: CameraSensor = CameraSensor::new();
            loop {
                let reading: Camera3Wide = camera_sensor.read();
                if let Ok(mut pos) = camera_pos.lock() {
                    *pos = reading;
                }
                std::thread::sleep(Duration::from_millis(500));
            }
        });
    }

    let handle = tokio::spawn(async move {
        // El sensor de luz bloquea ~130ms por lectura (espera de integración),
        // así que se lee cada N iteraciones y se cachea; los micros marcan la
        // cadencia del bucle para que el soplido se detecte con poca latencia.
        const LIGHT_EVERY_N_TICKS: u32 = 3;
        let mut tick: u32 = 0;
        let mut last_light: SensorReading = SensorReading {
            lux: 0.0,
            full_spectrum: 0,
            infrared: 0,
        };

        loop {
            if tick % LIGHT_EVERY_N_TICKS == 0 {
                last_light = light_sensor.get_event().unwrap_or(SensorReading {
                    lux: 0.0,
                    full_spectrum: 0,
                    infrared: 0,
                });
            }
            tick = tick.wrapping_add(1);

            for ch in 0..MICROPHONES_SIZE {
                microphone_sensor.read_amplitude(ch as u8);
            }

            let camera_reading: Camera3Wide =
                camera_pos.lock().map(|pos| *pos).unwrap_or_default();

            let data: RpiDemo = RpiDemo {
                light: last_light,
                microphone: microphone_sensor.microphones,
                camera: camera_reading,
            };

            let bytes: Vec<u8> =
                bincode::serde::encode_to_vec(&data, bincode::config::standard()).unwrap_or(vec![]);

            let msg: Message = Message::Binary(bytes.into());
            if let Err(e) = socket.send(msg).await {
                eprintln!("Error sending data: {e}, stopping loop");
                break;
            }

            // Cadencia del bucle de muestreo/envío. Los micros ya tardan ~100ms
            // en muestrearse, así que el ciclo efectivo queda en ~150-250ms.
            tokio::time::sleep(Duration::from_millis(50)).await;
        }
    });

    // Mantener vivo el proceso mientras la tarea de muestreo siga ejecutándose.
    handle.await.ok();

    Ok(())
}

#[derive(Error, Debug)]
pub enum ComunicationError {
    #[error("I2C device error: {0}")]
    I2cDevice(#[from] linux_embedded_hal::i2cdev::linux::LinuxI2CError),
    #[error("Light sensor error: {0}")]
    LightSensor(#[from] Tsl2591Error<linux_embedded_hal::I2CError>),
    #[error("Microphone error; {0}")]
    Microphone(String),
    #[error("Camera error; {0}")]
    Camera(String),
    #[error("Connection Errror")]
    Connection(#[from] tokio_tungstenite::tungstenite::Error),
    #[error("Internal Error")]
    InternalServer(#[from] std::io::Error),
    #[error("Internal Error")]
    Parse(#[from] bincode::error::EncodeError),
}

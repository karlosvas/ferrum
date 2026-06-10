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
    env_logger::Builder::from_env(env_logger::Env::default().default_filter_or("info")).init();

    let ip_host: String = std::env::var("IP_HOST").unwrap_or_else(|_| "127.0.0.1".to_string());
    let url: String = format!("ws://{}:3000/demo", ip_host);

    let mut socket = loop {
        match connect_async(&url).await {
            Ok((s, _)) => break s,
            Err(e) => {
                log::error!("Error connecting to {url} ({e}), retrying in 2s...");
                tokio::time::sleep(Duration::from_secs(2)).await;
            }
        }
    };

    let mut light_sensor: AdafruitTSL2591<I2cdev, Delay> = LightSensor::setup().await?;
    let mut microphone_sensor: MicrophoneSensor = MicrophoneSensor::new();

    log::info!("Calibrating microphone noise floor, keep quiet...");
    microphone_sensor.calibrate();

    // The camera captures with rpicam-jpeg (~2-3s per photo), so it lives in its
    // own thread and publishes the latest position; the sampling loop never
    // blocks waiting for it. Without this the mics only updated every ~3.5s
    // and detecting a blow was impossible.
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
        // The light sensor blocks ~130ms per read (integration wait), so it is
        // read every N iterations and cached; the mics set the loop cadence so
        // a blow is detected with low latency.
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
                log::error!("Error sending data: {e}, stopping loop");
                break;
            }

            // Sampling/send loop cadence. The mics already take ~100ms to
            // sample, so the effective cycle ends up at ~150-250ms.
            tokio::time::sleep(Duration::from_millis(50)).await;
        }
    });

    // Keep the process alive while the sampling task is still running.
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

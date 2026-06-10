mod camera;
mod light;
mod microphone;

use futures_util::SinkExt;
use linux_embedded_hal::{Delay, I2cdev};
use shared::structs::RpiDemo;
use std::time::Duration;
use tokio_tungstenite::{connect_async, tungstenite::Message};
use tsl2591_rs::{AdafruitTSL2591, driver::SensorReading};

use crate::{
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

    tokio::spawn(async move {
        loop {
            let sensor_reading: SensorReading = light_sensor.get_event().unwrap_or(SensorReading {
                lux: 0.0,
                full_spectrum: 0,
                infrared: 0,
            });

            for ch in 0..MICROPHONES_SIZE {
                microphone_sensor.read_channel(ch as u8);
            }

            let data: RpiDemo = RpiDemo {
                light: sensor_reading,
                microphone: microphone_sensor.microphones,
                ..Default::default()
            };

            let bytes: Vec<u8> =
                bincode::serde::encode_to_vec(&data, bincode::config::standard()).unwrap_or(vec![]);

            let msg: Message = Message::Binary(bytes.into());
            socket.send(msg).await;
        }
    });

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

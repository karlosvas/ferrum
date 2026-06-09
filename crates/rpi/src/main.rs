mod camera;
mod light;
mod microphone;

use crate::light::LightSensor;
use {thiserror::Error, tsl2591_rs::driver::Tsl2591Error};

#[tokio::main]
async fn main() -> Result<(), ComunicationError> {
    dotenvy::dotenv().ok();
    LightSensor::setup().await?;

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

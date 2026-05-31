use {
    futures_util::SinkExt,
    linux_embedded_hal::{Delay, I2cdev},
    shared::structs::{Camera3Wide, Microphone, RpiDemo},
    std::time::Duration,
    thiserror::Error,
    tokio,
    tokio_tungstenite::{connect_async, tungstenite::Message},
    tsl2591_rs::{
        AdafruitTSL2591, Gain, IntegrationTime, TSL2591_ADDR,
        driver::{SensorReading, Tsl2591Error},
    },
};

#[tokio::main]
async fn main() -> Result<(), ComunicationError> {
    dotenvy::dotenv().ok();
    let ip_host: String = std::env::var("IP_HOST").unwrap_or_else(|_| "127.0.0.1".to_string());

    let i2c: I2cdev = I2cdev::new("/dev/i2c-1")?;
    let mut sensor: AdafruitTSL2591<I2cdev, Delay> = AdafruitTSL2591::new(
        i2c,
        Delay,
        IntegrationTime::OneHundredMS,
        Gain::Medium,
        TSL2591_ADDR,
    );
    sensor.begin()?;

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

    loop {
        let sensor_reading: SensorReading = sensor
            .get_event()
            .map_err(|e| ComunicationError::LightSensor(e))?;

        let mic: Vec<Microphone> = vec![Microphone::default()];
        let camera: Camera3Wide = Camera3Wide::default();

        let data: RpiDemo = RpiDemo::new(sensor_reading, mic, camera);
        let bytes: Vec<u8> = bincode::serde::encode_to_vec(&data, bincode::config::standard())?;

        let msg: Message = Message::Binary(bytes.into());
        socket.send(msg).await?;
        tokio::time::sleep(Duration::from_millis(100)).await;
    }
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

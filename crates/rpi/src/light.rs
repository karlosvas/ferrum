use {
    crate::ComunicationError,
    linux_embedded_hal::{self, Delay, I2cdev},
    shared::structs::{Camera3Wide, Microphone, RpiDemo},
    std::time::Duration,
    tokio_tungstenite::tungstenite::Message,
    tsl2591_rs::{AdafruitTSL2591, Gain, IntegrationTime, TSL2591_ADDR, driver::SensorReading},
};

pub struct LightSensor;

impl LightSensor {
    pub async fn setup() -> Result<(), ComunicationError> {
        let ip_host: String = std::env::var("IP_HOST").unwrap_or_else(|_| "127.0.0.1".to_string());

        let i2c: I2cdev = I2cdev::new("/dev/i2c-1")?;
        let mut sensor: AdafruitTSL2591<I2cdev, Delay> = AdafruitTSL2591::new(
            i2c,
            Delay,
            IntegrationTime::OneHundredMS,
            Gain::Medium,
            TSL2591_ADDRR,
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
        }
    }
}

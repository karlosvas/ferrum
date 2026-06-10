use {
    crate::ComunicationError,
    linux_embedded_hal::{self, Delay, I2cdev},
    tsl2591_rs::{AdafruitTSL2591, Gain, IntegrationTime, TSL2591_ADDR},
};

pub struct LightSensor;

impl LightSensor {
    pub async fn setup() -> Result<AdafruitTSL2591<I2cdev, Delay>, ComunicationError> {
        let i2c: I2cdev = I2cdev::new("/dev/i2c-1")?;
        let mut sensor: AdafruitTSL2591<I2cdev, Delay> = AdafruitTSL2591::new(
            i2c,
            Delay,
            IntegrationTime::OneHundredMS,
            Gain::Medium,
            TSL2591_ADDR,
        );
        sensor.begin()?;
        Ok(sensor)
    }
}

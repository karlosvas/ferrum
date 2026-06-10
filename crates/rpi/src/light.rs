use {
    crate::ComunicationError,
    linux_embedded_hal::{self, Delay, I2cdev},
    tsl2591_rs::{AdafruitTSL2591, Gain, IntegrationTime, TSL2591_ADDR},
};

pub struct LightSensor;

impl LightSensor {
    pub async fn setup() -> Result<AdafruitTSL2591<I2cdev, Delay>, ComunicationError> {
        let i2c: I2cdev = I2cdev::new("/dev/i2c-1")?;
        // Low gain (1x) and short integration so the sensor is LESS sensitive:
        // it does not saturate under strong light and keeps a useful range.
        // With `Gain::Medium` (~25x) the channels saturated and the computed lux
        // dropped, which is why "lots of light" showed up as little.
        let mut sensor: AdafruitTSL2591<I2cdev, Delay> = AdafruitTSL2591::new(
            i2c,
            Delay,
            IntegrationTime::OneHundredMS,
            Gain::Low,
            TSL2591_ADDR,
        );
        sensor.begin()?;
        Ok(sensor)
    }
}

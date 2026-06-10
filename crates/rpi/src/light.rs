use {
    crate::ComunicationError,
    linux_embedded_hal::{self, Delay, I2cdev},
    tsl2591_rs::{AdafruitTSL2591, Gain, IntegrationTime, TSL2591_ADDR},
};

pub struct LightSensor;

impl LightSensor {
    pub async fn setup() -> Result<AdafruitTSL2591<I2cdev, Delay>, ComunicationError> {
        let i2c: I2cdev = I2cdev::new("/dev/i2c-1")?;
        // Ganancia baja (1x) e integración corta para que el sensor sea MENOS
        // sensible: así no se satura ante luz fuerte y mantiene rango útil.
        // Con `Gain::Medium` (~25x) los canales se saturaban y el lux calculado
        // caía, por eso "mucha luz" se representaba como poca.
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

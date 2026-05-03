use linux_embedded_hal::{Delay, I2cdev};
use std::time::{Duration, Instant};
use tsl2591_rs::{AdafruitTSL2591, Gain, IntegrationTime, TSL2591_ADDR, driver::SensorReading};

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let i2c: I2cdev = I2cdev::new("/dev/i2c-1")?;
    let mut delay: Delay = Delay;
    let mut sensor: AdafruitTSL2591<I2cdev, Delay> = AdafruitTSL2591::new(
        i2c,
        delay,
        IntegrationTime::OneHundredMS,
        Gain::Medium,
        TSL2591_ADDR,
    );

    sensor.begin()?;

    let start: Instant = std::time::Instant::now();
    loop {
        let reading: SensorReading = sensor.get_event()?;
        println!("Lux: {:.2}", reading.lux);
        println!("Full spectrum: {}", reading.full_spectrum);
        println!("Infrared: {}", reading.infrared);
        std::thread::sleep(Duration::from_millis(500));
    }

    Ok(())
}

use rppal::i2c::I2c;
use shared::structs::MicReading;

const ADS1115_ADDR: u16 = 0x48;
const REG_CONVERSION: u8 = 0x00;
const REG_CONFIG: u8 = 0x01;
pub const MICROPHONES_SIZE: usize = 4;

pub struct MicrophoneSensor {
    pub i2c: I2c,
    pub microphones: [MicReading; MICROPHONES_SIZE],
}

impl MicrophoneSensor {
    pub fn new() -> Self {
        let mut i2c: I2c = I2c::new().unwrap();
        i2c.set_slave_address(ADS1115_ADDR).unwrap();

        Self {
            i2c,
            microphones: Default::default(),
        }
    }

    pub fn read_channel(&mut self, channel: u8) -> i16 {
        let mux: u16 = 0x04u16 + channel as u16;

        // OS=1, MUX=single-ended, PGA=±2.048V, MODE=single-shot, DR=128SPS, COMP_QUE=disabled
        let config: u16 = 0x8000 | (mux << 12) | 0x0400 | 0x0100 | 0x0080 | 0x0003;

        // smbus envía little-endian, ADS1115 espera big-endian → swap antes de escribir
        self.i2c
            .smbus_write_word(REG_CONFIG, config.swap_bytes())
            .unwrap();

        let raw = self.i2c.smbus_read_word(REG_CONVERSION).unwrap();
        // ADS1115 envía big-endian, smbus lee little-endian → swap al leer
        raw.swap_bytes() as i16
    }
}

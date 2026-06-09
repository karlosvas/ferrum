use rppal::i2c::I2c;
use std::{thread, time::Duration};

const ADS1115_ADDR: u16 = 0x48;
const REG_CONVERSION: u8 = 0x00;
const REG_CONFIG: u8 = 0x01;

fn read_channel(i2c: &mut I2c, channel: u8) -> i16 {
    let mux = 0x04u16 + channel as u16;

    // OS=1, MUX=single-ended, PGA=±2.048V, MODE=single-shot, DR=128SPS, COMP_QUE=disabled
    let config: u16 = 0x8000 | (mux << 12) | 0x0400 | 0x0100 | 0x0080 | 0x0003;

    // smbus envía little-endian, ADS1115 espera big-endian → swap antes de escribir
    i2c.smbus_write_word(REG_CONFIG, config.swap_bytes())
        .unwrap();
    thread::sleep(Duration::from_millis(10));

    let raw = i2c.smbus_read_word(REG_CONVERSION).unwrap();
    // ADS1115 envía big-endian, smbus lee little-endian → swap al leer
    raw.swap_bytes() as i16
}

fn main() {
    let mut i2c = I2c::new().unwrap();
    i2c.set_slave_address(ADS1115_ADDR).unwrap();
    println!("Leyendo micro...");
    loop {
        for ch in 0..4 {
            let val = read_channel(&mut i2c, ch);
            print!("MIC{}: {:6} | ", ch, val);
        }
        println!();
        thread::sleep(Duration::from_millis(100));
    }
}

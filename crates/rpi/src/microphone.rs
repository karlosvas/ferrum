use rppal::i2c::I2c;
use shared::structs::MicReading;

const ADS1115_ADDR: u16 = 0x48;
const REG_CONVERSION: u8 = 0x00;
const REG_CONFIG: u8 = 0x01;
pub const MICROPHONES_SIZE: usize = 4;

pub struct MicrophoneSensor {
    pub i2c: I2c,
    pub microphones: [MicReading; MICROPHONES_SIZE],
    /// Noise floor (ambient amplitude) per channel. Each mic has a different
    /// sensitivity and offset; without this, a "noisy" channel would always
    /// look blown into and bias the wind direction.
    baseline: [f32; MICROPHONES_SIZE],
}

impl MicrophoneSensor {
    pub fn new() -> Self {
        let mut i2c: I2c = I2c::new().unwrap();
        i2c.set_slave_address(ADS1115_ADDR).unwrap();

        Self {
            i2c,
            microphones: Default::default(),
            baseline: [0.0; MICROPHONES_SIZE],
        }
    }

    /// Calibrates each channel's noise floor by averaging several bursts.
    /// Call at startup, with the environment silent (no blowing).
    pub fn calibrate(&mut self) {
        const ROUNDS: usize = 6;
        let mut acc: [f32; MICROPHONES_SIZE] = [0.0; MICROPHONES_SIZE];
        for _ in 0..ROUNDS {
            for ch in 0..MICROPHONES_SIZE {
                acc[ch] += self.burst_amplitude(ch as u8) as f32;
            }
        }
        for ch in 0..MICROPHONES_SIZE {
            self.baseline[ch] = acc[ch] / ROUNDS as f32;
        }
        log::info!("Mic baseline: {:?}", self.baseline);
    }

    pub fn read_channel(&mut self, channel: u8) -> i16 {
        let mux: u16 = 0x04u16 + channel as u16;

        // OS=1, MUX=single-ended, PGA=±2.048V, MODE=single-shot, DR=860SPS, COMP_QUE=disabled.
        // DR at 860SPS (bits 111 = 0x00E0) to sample the amplitude quickly.
        let config: u16 = 0x8000 | (mux << 12) | 0x0400 | 0x0100 | 0x00E0 | 0x0003;

        // smbus sends little-endian, ADS1115 expects big-endian → swap before writing
        self.i2c
            .smbus_write_word(REG_CONFIG, config.swap_bytes())
            .unwrap();

        // Wait until the conversion ACTUALLY finishes: the OS bit (15) of the
        // config register flips to 1. With a fixed sleep, the previous
        // conversion was sometimes read — from the OTHER channel after switching
        // the MUX — and that showed up as ghost spikes of thousands of counts
        // on silent channels.
        for _ in 0..50 {
            let cfg: u16 = self
                .i2c
                .smbus_read_word(REG_CONFIG)
                .unwrap_or(0)
                .swap_bytes();
            if cfg & 0x8000 != 0 {
                break;
            }
            std::thread::sleep(std::time::Duration::from_micros(200));
        }

        let raw = self.i2c.smbus_read_word(REG_CONVERSION).unwrap();
        // ADS1115 sends big-endian, smbus reads little-endian → swap when reading
        raw.swap_bytes() as i16
    }

    /// Amplitude of a burst of samples. The mic delivers an AC signal around a
    /// DC level, so a single sample does not represent the "volume"; the spread
    /// of the burst does grow with the blow.
    ///
    /// It is a ROBUST amplitude: the burst is sorted and the 2 highest and
    /// lowest values are discarded, so a spurious sample (ADS1115 mux residue)
    /// cannot fabricate thousands of counts of ghost amplitude. A real blow
    /// keeps the signal large across many samples and survives the trimming.
    fn burst_amplitude(&mut self, channel: u8) -> u16 {
        const SAMPLES: usize = 20;
        const TRIM: usize = 2;

        // Discard the first conversion after switching channels (mux residue).
        let _ = self.read_channel(channel);

        let mut buf: [i16; SAMPLES] = [0; SAMPLES];
        for sample in buf.iter_mut() {
            *sample = self.read_channel(channel);
        }
        buf.sort_unstable();
        (buf[SAMPLES - 1 - TRIM] as i32 - buf[TRIM] as i32).clamp(0, u16::MAX as i32) as u16
    }

    /// Measures the channel's activity (amplitude above the noise floor) and
    /// stores it in `microphones`. This is the value the demo consumes to derive
    /// wind direction and intensity.
    pub fn read_amplitude(&mut self, channel: u8) -> u16 {
        let ch: usize = channel as usize;
        let amplitude: f32 = self.burst_amplitude(channel) as f32;

        // Slow noise-floor adaptation: drops quickly (if the environment calms
        // down) and rises very slowly (so a sustained blow is not absorbed).
        if amplitude < self.baseline[ch] {
            self.baseline[ch] = self.baseline[ch] * 0.9 + amplitude * 0.1;
        } else {
            self.baseline[ch] += (amplitude - self.baseline[ch]) * 0.005;
        }

        let net: u16 = (amplitude - self.baseline[ch]).max(0.0) as u16;
        self.microphones[ch] = MicReading { channel, raw: net };
        net
    }
}

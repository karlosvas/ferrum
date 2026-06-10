use rppal::i2c::I2c;
use shared::structs::MicReading;

const ADS1115_ADDR: u16 = 0x48;
const REG_CONVERSION: u8 = 0x00;
const REG_CONFIG: u8 = 0x01;
pub const MICROPHONES_SIZE: usize = 4;

pub struct MicrophoneSensor {
    pub i2c: I2c,
    pub microphones: [MicReading; MICROPHONES_SIZE],
    /// Suelo de ruido (amplitud ambiente) por canal. Cada micro tiene una
    /// sensibilidad y offset distintos; sin esto, un canal "ruidoso" parecería
    /// siempre soplado y sesgaría la dirección del viento.
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

    /// Calibra el suelo de ruido de cada canal promediando varias ráfagas.
    /// Llamar al arrancar, con el ambiente en silencio (sin soplar).
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
        eprintln!("Mic baseline: {:?}", self.baseline);
    }

    pub fn read_channel(&mut self, channel: u8) -> i16 {
        let mux: u16 = 0x04u16 + channel as u16;

        // OS=1, MUX=single-ended, PGA=±2.048V, MODE=single-shot, DR=860SPS, COMP_QUE=disabled.
        // DR a 860SPS (bits 111 = 0x00E0) para poder muestrear rápido la amplitud.
        let config: u16 = 0x8000 | (mux << 12) | 0x0400 | 0x0100 | 0x00E0 | 0x0003;

        // smbus envía little-endian, ADS1115 espera big-endian → swap antes de escribir
        self.i2c
            .smbus_write_word(REG_CONFIG, config.swap_bytes())
            .unwrap();

        // Esperar a que la conversión TERMINE de verdad: el bit OS (15) del
        // registro de config pasa a 1. Con un sleep fijo a veces se leía la
        // conversión anterior — del OTRO canal tras cambiar el MUX — y eso
        // aparecía como picos fantasma de miles de cuentas en canales en silencio.
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
        // ADS1115 envía big-endian, smbus lee little-endian → swap al leer
        raw.swap_bytes() as i16
    }

    /// Amplitud de una ráfaga de muestras. El micro entrega una señal AC
    /// alrededor de un nivel DC, así que una sola muestra no representa el
    /// "volumen"; la dispersión de la ráfaga sí crece con el soplido.
    ///
    /// Es una amplitud ROBUSTA: se ordena la ráfaga y se descartan los 2 valores
    /// más altos y más bajos, de modo que una muestra espuria (residuos del mux
    /// del ADS1115) no fabrique miles de cuentas de amplitud fantasma. Un soplido
    /// real mantiene la señal grande durante muchas muestras y sobrevive al recorte.
    fn burst_amplitude(&mut self, channel: u8) -> u16 {
        const SAMPLES: usize = 20;
        const TRIM: usize = 2;

        // Descartar la primera conversión tras cambiar de canal (residuos del mux).
        let _ = self.read_channel(channel);

        let mut buf: [i16; SAMPLES] = [0; SAMPLES];
        for sample in buf.iter_mut() {
            *sample = self.read_channel(channel);
        }
        buf.sort_unstable();
        (buf[SAMPLES - 1 - TRIM] as i32 - buf[TRIM] as i32).clamp(0, u16::MAX as i32) as u16
    }

    /// Mide la actividad del canal (amplitud por encima del suelo de ruido) y la
    /// guarda en `microphones`. Es el valor que consume el demo para derivar
    /// dirección e intensidad del viento.
    pub fn read_amplitude(&mut self, channel: u8) -> u16 {
        let ch: usize = channel as usize;
        let amplitude: f32 = self.burst_amplitude(channel) as f32;

        // Adaptación lenta del suelo de ruido: baja rápido (si el ambiente se
        // calma) y sube muy despacio (para no absorber un soplido sostenido).
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

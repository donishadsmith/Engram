// https://www.chciken.com/tlmboy/2025/03/24/gameboy-apu-noise.html
const DIVISORS: [u16; 8] = [8, 16, 32, 48, 64, 80, 96, 112];

use crate::components::apu::sound_control::{Envelope, EnvelopeDirection, Length};

struct LFSR {
    width: u8,
    value: u16,
}

impl LFSR {
    fn new() -> Self {
        Self { width: 0, value: 0 }
    }

    fn step(&mut self) {
        let feedback = (self.value ^ (self.value >> 1)) & 1;
        self.value >>= 1;
        self.value = (self.value & !(1 << 14)) | (feedback << 14);
        if self.width == 1 {
            self.value = (self.value & !(1 << 6)) | (feedback << 6);
        }
    }
}

pub struct NoiseChannel {
    pub enabled: bool,
    pub length: Length,
    pub envelope: Envelope,
    clock_shift: u8,
    clock_divider: u8,
    frequency_timer: u32,
    lfsr: LFSR,
}

impl NoiseChannel {
    pub fn new() -> Self {
        Self {
            enabled: false,
            length: Length::new(),
            envelope: Envelope::new(),
            clock_shift: 0,
            clock_divider: 0,
            frequency_timer: 0,
            lfsr: LFSR::new(),
        }
    }

    pub fn read_nr41(&self) -> u8 {
        0xFF
    }

    pub fn write_nr41(&mut self, value: u8) {
        self.length.write(value);
    }

    pub fn read_nr42(&self) -> u8 {
        self.envelope.read()
    }

    pub fn write_nr42(&mut self, value: u8) {
        self.envelope.set(value);
    }

    pub fn read_nr43(&self) -> u8 {
        (self.clock_shift << 4) | (self.lfsr.width << 3) | self.clock_divider
    }

    pub fn write_nr43(&mut self, value: u8) {
        self.clock_shift = (value & 0xF0) >> 4;
        self.lfsr.width = (value & 0x08) >> 3;
        self.clock_divider = value & 0x07;
    }

    pub fn read_nr44(&self) -> u8 {
        self.length.read()
    }

    pub fn write_nr44(&mut self, value: u8) {
        self.length.enabled = (value & 0x40) != 0;

        if (value >> 7) & 0x01 == 1 {
            self.enabled = self.dac_enabled();

            if self.length.timer == 0 {
                self.length.timer = 64;
            }

            self.frequency_timer =
                ((DIVISORS[self.clock_divider as usize]) as u32) << self.clock_shift;
            self.envelope.timer = self.envelope.period;
            self.envelope.current_volume = self.envelope.initial_volume;
            self.lfsr.value = 0x7FFF;
        }
    }

    fn dac_enabled(&self) -> bool {
        self.envelope.initial_volume != 0
            || matches!(self.envelope.direction, EnvelopeDirection::Increment)
    }

    pub fn tick(&mut self) {
        if self.frequency_timer > 0 {
            self.frequency_timer -= 1;
        }

        if self.frequency_timer == 0 {
            self.frequency_timer =
                (DIVISORS[self.clock_divider as usize] as u32) << self.clock_shift;
            if self.clock_shift < 14 {
                self.lfsr.step();
            }
        }
    }

    pub fn sample(&self) -> u8 {
        if self.enabled {
            (!(self.lfsr.value) as u8 & 0x01) * self.envelope.current_volume
        } else {
            0
        }
    }
}

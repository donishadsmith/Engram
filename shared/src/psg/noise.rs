// https://www.chciken.com/tlmboy/2025/03/24/gameboy-apu-noise.html
const DIVISORS: [u16; 8] = [8, 16, 32, 48, 64, 80, 96, 112];

use crate::traits::BitOps;

use crate::psg::sound_control::{Envelope, Length};

struct LFSR {
    width: u8,
    register: u16,
}

impl LFSR {
    fn new() -> Self {
        Self {
            width: 0,
            register: 0x0000,
        }
    }

    fn step(&mut self) {
        let feedback = (self.register ^ (self.register >> 1)).get_bit_range(0..1);

        self.register.set_bit_range_value(15..16, feedback);
        if self.width == 1 {
            self.register.set_bit_range_value(7..8, feedback);
        }

        self.register >>= 1;
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

    pub fn read_nrx1(&self) -> u8 {
        0xFF
    }

    pub fn write_nrx1(&mut self, value: u8) {
        self.length.write(value);
    }

    pub fn read_nrx2(&self) -> u8 {
        self.envelope.read()
    }

    pub fn write_nrx2(&mut self, value: u8) {
        self.envelope.set(value);

        if value.get_bit_range(3..8) == 0 {
            self.enabled = false;
        }
    }

    pub fn read_nrx3(&self) -> u8 {
        (self.clock_shift << 4) | (self.lfsr.width << 3) | self.clock_divider
    }

    pub fn write_nrx3(&mut self, value: u8) {
        self.clock_shift = value.get_bit_range(4..8);
        self.lfsr.width = value.get_bit(3);
        self.clock_divider = value.get_bit_range(0..3);
    }

    pub fn read_nrx4(&self) -> u8 {
        self.length.read()
    }

    pub fn write_nrx4(&mut self, value: u8) {
        self.length.enabled = value.is_set(6);

        if value.is_set(7) {
            self.enabled = self.envelope.dac_enabled();

            if self.length.timer == 0 {
                self.length.timer = 64;
            }

            self.frequency_timer =
                (DIVISORS[self.clock_divider as usize] as u32) << self.clock_shift;
            self.envelope.timer = self.envelope.period;
            self.envelope.current_volume = self.envelope.initial_volume;
            self.lfsr.register = 0x7FFF;
        }
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
            (self.lfsr.register.is_clear(0) as u8) * self.envelope.current_volume
        } else {
            0
        }
    }
}

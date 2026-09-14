// https://www.chciken.com/tlmboy/2025/03/24/gameboy-apu-noise.html
const DIVISORS: [u16; 8] = [8, 16, 32, 48, 64, 80, 96, 112];

use crate::components::apu::{
    global_control::AudioChannel,
    sound_control::{Envelope, Length},
};
use shared::traits::BitOps;

struct LFSR {
    width: u8,
    register: u16,
}

impl LFSR {
    fn new() -> Self {
        Self {
            width: 0,
            register: 0,
        }
    }

    fn step(&mut self) {
        let feedback = (self.register ^ (self.register >> 1)).get_bit(0) as u16;
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
    clock_shift: u16,
    clock_divider: u16,
    frequency_timer: u32,
    lfsr: LFSR,
    pub soundcnt_l: u16,
    pub soundcnt_h: u16,
    pub history: Vec<u8>,
    pub mute: bool,
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
            soundcnt_l: 0,
            soundcnt_h: 0,
            history: Vec::with_capacity(2048),
            mute: false,
        }
    }

    pub fn update_from_register(&mut self, address: u32) {
        match address {
            0x4000078 => {
                self.length
                    .set_timer(self.soundcnt_l, AudioChannel::Channel4);
                self.envelope.set(self.soundcnt_l);
            }
            0x400007C => {
                let value = self.soundcnt_h;
                self.length.enabled = value.is_set(14);
                self.clock_divider = value.get_bit_range(0..3);
                self.clock_shift = value.get_bit_range(4..8);
                self.lfsr.width = value.get_bit(3);

                if value.is_set(15) {
                    self.trigger_reset_event();
                }
            }
            _ => {}
        }
    }

    pub fn read_from_register(&self, address: u32) -> u16 {
        match address {
            0x4000078 => self.envelope.read(),
            0x400007C => {
                (self.clock_shift << 4)
                    | (self.lfsr.width << 3) as u16
                    | self.clock_divider
                    | (self.length.enabled as u16) << 14
            }
            _ => 0,
        }
    }

    fn trigger_reset_event(&mut self) {
        self.enabled = self.envelope.dac_enabled();

        if self.length.timer == 0 {
            self.length.timer = 64;
        }

        self.frequency_timer = ((DIVISORS[self.clock_divider as usize]) as u32) << self.clock_shift;
        self.envelope.timer = self.envelope.step_time;
        self.envelope.current_volume = self.envelope.initial_volume;
        self.lfsr.register = 0x7FFF;
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

    pub fn get_sample(&mut self) -> u8 {
        let sample = if self.enabled {
            (!self.lfsr.register.is_set(0)) as u8 * self.envelope.current_volume as u8
        } else {
            0
        };

        self.history.push(sample);

        sample
    }
}

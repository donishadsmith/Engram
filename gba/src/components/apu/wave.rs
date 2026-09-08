use crate::components::{
    apu::{global_control::AudioChannel, sound_control::Length},
    utils::{BitOps, GroupedRegisters},
};

#[derive(Clone, Copy, PartialEq, Eq)]
enum CoarseVolume {
    Mute,
    Full,
    Half,
    Quarter,
    ThreeFourths,
}

impl CoarseVolume {
    fn from_register(value: u16) -> CoarseVolume {
        match value.get_bit_range(13..15) {
            0b00 => CoarseVolume::Mute,
            0b01 => CoarseVolume::Full,
            0b10 => CoarseVolume::Half,
            0b11 => CoarseVolume::Quarter,
            _ => unreachable!(),
        }
    }

    fn to_shift(self) -> u8 {
        match self {
            CoarseVolume::Mute => 4,
            CoarseVolume::Full => 0,
            CoarseVolume::Half => 1,
            CoarseVolume::Quarter => 2,
            _ => unreachable!(),
        }
    }
}

pub struct WaveChannel {
    pub enabled: bool,
    pub dac_enabled: bool,
    pub ram: [u8; 32],
    pub length: Length,
    volume: CoarseVolume,
    frequency_period: u16,
    frequency_timer: u16,
    position: u16,
    pub history: Vec<u8>,
    bank: usize,
    dimension: usize,
    pub soundcnt: GroupedRegisters<u16>,
    pub mute: bool,
}

impl WaveChannel {
    pub fn new() -> Self {
        Self {
            enabled: false,
            dac_enabled: false,
            ram: [0; 32],
            volume: CoarseVolume::Mute,
            length: Length::new(),
            frequency_period: 0,
            frequency_timer: 0,
            position: 0,
            history: Vec::with_capacity(2048),
            bank: 0,
            dimension: 32,
            soundcnt: GroupedRegisters::new(3, 0x4000070),
            mute: false,
        }
    }

    pub fn write_wave_ram(&mut self, address: u32, value: u16, psg_enabled: bool) {
        let bytes = u16::to_le_bytes(value);
        let offset = (address - 0x4000090) as usize;
        let index = self.bank_offset(psg_enabled) * 16 + offset;

        self.ram[index] = bytes[0];
        self.ram[index + 1] = bytes[1];
    }

    pub fn read_wave_ram(&self, address: u32, psg_enabled: bool) -> u16 {
        let offset = (address - 0x4000090) as usize;
        let index = self.bank_offset(psg_enabled) + offset;
        let halfword = u16::from_le_bytes([self.ram[index], self.ram[index + 1]]);

        halfword
    }

    fn bank_offset(&self, psg_enabled: bool) -> usize {
        let bank = if psg_enabled { self.bank } else { 0 };
        (bank ^ 1) as usize * 16
    }

    pub fn update_from_register(&mut self, address: u32) {
        match address {
            0x4000070 => {
                let value = self.soundcnt.from_index(0);
                self.dimension = if value.is_set(5) { 64 } else { 32 };
                self.bank = value.get_bit(6) as usize;
                self.dac_enabled = value.is_set(7);
                if !self.dac_enabled {
                    self.enabled = false;
                }
            }
            0x4000072 => {
                let value = self.soundcnt.from_index(1);
                self.volume = if value.is_set(15) {
                    CoarseVolume::ThreeFourths
                } else {
                    CoarseVolume::from_register(value)
                };
                self.length.set_timer(value, AudioChannel::Channel3);
            }
            0x4000074 => {
                let value = self.soundcnt.from_index(2);
                self.length.enabled = value.is_set(14);
                self.frequency_period = self.soundcnt.from_index(2).get_bit_range(0..11);
                self.soundcnt.write_u16(address, value & !0x8000);
                if value.is_set(15) {
                    self.trigger_reset_event();
                }
            }
            _ => {}
        }
    }

    pub fn read_from_register(&self, address: u32) -> u16 {
        match address {
            0x4000070 => self.soundcnt.from_index(0),
            0x04000072 => {
                let value = self.soundcnt.from_index(1);
                value.get_bit_range(13..15) << 13 | ((value.get_bit(15) as u16) << 15)
            }
            0x4000074 => (self.length.enabled as u16) << 14,
            _ => 0,
        }
    }

    fn pitch_adjustment(&mut self) {
        self.frequency_timer = (2048 - self.frequency_period) * 2;
    }

    pub fn tick(&mut self) {
        if self.frequency_timer > 0 {
            self.frequency_timer -= 1;
        }

        if self.frequency_timer == 0 {
            self.pitch_adjustment();
            self.position = (self.position + 1) % self.dimension as u16;
        }
    }

    fn trigger_reset_event(&mut self) {
        self.enabled = self.dac_enabled;

        if self.length.timer == 0 {
            self.length.timer = 256;
        }

        self.pitch_adjustment();
        self.position = 0;
    }

    pub fn get_sample(&mut self) -> u8 {
        if !self.enabled {
            return 0;
        }

        let index = (self.bank * 32 + self.position as usize) % 64;
        let byte = self.ram[(index / 2) as usize];
        let nibble = if self.position % 2 == 0 {
            byte.get_bit_range(4..8)
        } else {
            byte.get_bit_range(0..4)
        };

        let sample = if self.volume == CoarseVolume::ThreeFourths {
            (nibble * 3) >> 2
        } else {
            nibble >> self.volume.to_shift()
        } as u8;

        self.history.push(sample);

        sample
    }
}

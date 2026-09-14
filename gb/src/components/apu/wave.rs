use crate::components::apu::sound_control::Length;
use shared::traits::BitOps;

#[derive(Clone, Copy)]
#[repr(u8)]
enum CoarseVolume {
    Mute = 0b00,
    Full = 0b01,
    Half = 0b10,
    Quarter = 0b11,
}

impl CoarseVolume {
    fn from_register(value: u8) -> CoarseVolume {
        match value.get_bit_range(5..7) {
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
        }
    }
}

pub struct WaveChannel {
    pub enabled: bool,
    pub dac_enabled: bool,
    pub ram: [u8; 16],
    pub length: Length,
    volume: CoarseVolume,
    frequency_period: u16,
    frequency_timer: u16,
    position: u8,
}

impl WaveChannel {
    pub fn new() -> Self {
        Self {
            enabled: false,
            dac_enabled: false,
            ram: [0u8; 16],
            volume: CoarseVolume::Mute,
            length: Length::new(),
            frequency_period: 0,
            frequency_timer: 0,
            position: 0,
        }
    }

    pub fn write_nr30(&mut self, value: u8) {
        self.dac_enabled = value.is_set(7);
        if !self.dac_enabled {
            self.enabled = false;
        }
    }

    pub fn read_nr30(&self) -> u8 {
        (if self.dac_enabled { 0x80 } else { 0 }) | 0x7F
    }

    pub fn read_nr31(&self) -> u8 {
        0xFF
    }

    pub fn write_nr31(&mut self, value: u8) {
        self.length.timer = 256 - value as u16;
    }

    pub fn read_nr32(&self) -> u8 {
        ((self.volume as u8) << 5) | 0x9F
    }

    pub fn write_nr32(&mut self, value: u8) {
        self.volume = CoarseVolume::from_register(value);
    }

    pub fn read_nr33(&self) -> u8 {
        0xFF
    }

    pub fn write_nr33(&mut self, value: u8) {
        self.frequency_period = (self.frequency_period & 0x0700) | value as u16;
    }

    pub fn read_nr34(&self) -> u8 {
        self.length.read()
    }

    pub fn write_nr34(&mut self, value: u8) {
        self.frequency_period =
            self.frequency_period.get_bit_range(0..8) | ((value.get_bit_range(0..3) as u16) << 8);
        self.length.enabled = value.is_set(6);

        if value.is_set(7) {
            self.enabled = self.dac_enabled;

            if self.length.timer == 0 {
                self.length.timer = 256;
            }

            self.frequency_timer = (2048 - self.frequency_period) * 2;
            self.position = 0;
        }
    }

    pub fn tick(&mut self) {
        if self.frequency_timer > 0 {
            self.frequency_timer -= 1;
        }

        if self.frequency_timer == 0 {
            self.frequency_timer = (2048 - self.frequency_period) * 2;
            self.position = (self.position + 1).get_bit_range(0..5);
        }
    }

    pub fn sample(&self) -> u8 {
        if !self.enabled {
            return 0;
        }

        let byte = self.ram[(self.position / 2) as usize];
        let nibble = if self.position.is_clear(0) {
            byte.get_bit_range(4..8)
        } else {
            byte.get_bit_range(0..4)
        };

        nibble >> self.volume.to_shift()
    }
}

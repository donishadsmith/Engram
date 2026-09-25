use crate::EmulatorId;
use crate::psg::sound_control::Length;
use crate::traits::BitOps;

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
    pub ram: Box<[u8]>,
    bank: usize,
    pub length: Length,
    volume: CoarseVolume,
    frequency_period: u16,
    frequency_timer: u16,
    position: u8,
    emulator_id: EmulatorId,
    dimension: u8,
    base_address: u32,
    force_75: bool,
}

impl WaveChannel {
    pub fn new(emulator_id: EmulatorId) -> Self {
        Self {
            enabled: false,
            dac_enabled: false,
            ram: if emulator_id == EmulatorId::Gb {
                Box::new([0; 16])
            } else {
                Box::new([0; 32])
            },
            bank: 0,
            volume: CoarseVolume::Mute,
            length: Length::new(),
            frequency_period: 0,
            frequency_timer: 0,
            position: 0,
            emulator_id,
            dimension: 32,
            base_address: if emulator_id == EmulatorId::Gb {
                0xFF30
            } else {
                0x4000090
            },
            force_75: false,
        }
    }

    pub fn write_wave_ram(&mut self, address: u32, value: u16, psg_enabled: bool) {
        let bytes = u16::to_le_bytes(value);
        let offset = (address - self.base_address) as usize;
        let index = self.bank_offset(psg_enabled) + offset;

        self.ram[index] = bytes[0];
        if self.emulator_id == EmulatorId::Gb {
            return;
        }

        self.ram[index + 1] = bytes[1];
    }

    pub fn read_wave_ram(&self, address: u32, psg_enabled: bool) -> u16 {
        let offset = (address - self.base_address) as usize;
        let index = self.bank_offset(psg_enabled) + offset;

        if self.emulator_id == EmulatorId::Gb {
            self.ram[index] as u16
        } else {
            u16::from_le_bytes([self.ram[index], self.ram[index + 1]])
        }
    }

    fn bank_offset(&self, psg_master_enabled: bool) -> usize {
        if self.emulator_id == EmulatorId::Gb {
            return 0;
        }

        let bank = if psg_master_enabled { self.bank } else { 0 };
        (bank ^ 1) as usize * 16
    }

    pub fn write_nrx0(&mut self, value: u8) {
        if self.emulator_id == EmulatorId::Gba {
            self.dimension = if value.is_set(5) { 64 } else { 32 };
            self.bank = value.get_bit(6) as usize;
        }

        self.dac_enabled = value.is_set(7);
        if !self.dac_enabled {
            self.enabled = false;
        }
    }

    pub fn read_nrx0(&self) -> u8 {
        (if self.dac_enabled { 0x80 } else { 0 }) | 0x7F
    }

    pub fn read_nrx1(&self) -> u8 {
        0xFF
    }

    pub fn write_nrx1(&mut self, value: u8) {
        self.length.timer = 256 - value as u16;
    }

    pub fn read_nrx2(&self) -> u8 {
        ((self.volume as u8) << 5) | 0x9F
    }

    pub fn write_nrx2(&mut self, value: u8) {
        self.volume = CoarseVolume::from_register(value);
        self.force_75 = value.is_set(7) && self.emulator_id == EmulatorId::Gba;
    }

    pub fn read_nrx3(&self) -> u8 {
        0xFF
    }

    pub fn write_nrx3(&mut self, value: u8) {
        self.frequency_period = (self.frequency_period & 0x0700) | value as u16;
    }

    pub fn read_nrx4(&self) -> u8 {
        self.length.read()
    }

    pub fn write_nrx4(&mut self, value: u8) {
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
            self.position = (self.position + 1) % self.dimension;
        }
    }

    pub fn sample(&self) -> u8 {
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

        let sample = if self.force_75 {
            (nibble * 3) >> 2
        } else {
            nibble >> self.volume.to_shift()
        };

        sample
    }
}

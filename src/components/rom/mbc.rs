// TODO: currently placeholder logic for mbc1-5 logic

pub mod prelude {
    use crate::components::cpu::core::ByteOps8;

    pub trait MBC {
        fn read(&self, address: u16) -> u8;

        fn write(&mut self, address: u16, value: u8);

        fn get_rom(&self) -> &[u8];

        fn get_ram(&self) -> &[u8];

        fn n_rom_banks(&self) -> usize {
            self.get_rom().len() / (16 * 1024)
        }

        fn n_ram_banks(&self) -> usize {
            (self.get_ram().len() / (8 * 1024)).max(1)
        }

        fn rom_size(&self) -> usize {
            self.get_rom().len() * 1024
        }

        fn ram_size(&self) -> usize {
            self.get_ram().len() * 1024
        }
    }

    pub struct RomOnly {
        rom: Vec<u8>,
        ram: Vec<u8>,
    }

    impl RomOnly {
        pub fn new(rom: Vec<u8>, ram: Vec<u8>) -> Self {
            Self { rom, ram }
        }
    }

    impl MBC for RomOnly {
        fn read(&self, address: u16) -> u8 {
            *self.rom.get(address as usize).unwrap_or(&0xFF)
        }

        fn write(&mut self, _address: u16, _value: u8) {}

        fn get_rom(&self) -> &[u8] {
            &self.rom
        }

        fn get_ram(&self) -> &[u8] {
            &self.ram
        }
    }

    // https://gbdev.io/pandocs/MBC1.html; theres a 5 + a 2 bit register for this
    pub struct MBC1 {
        rom: Vec<u8>,
        ram: Vec<u8>,
        register_5bit: u8,
        register_2bit: u8,
        mode: bool,
        ram_enabled: bool,
    }

    impl MBC1 {
        pub fn new(rom: Vec<u8>, ram: Vec<u8>) -> Self {
            Self {
                rom,
                ram,
                register_5bit: 1,
                register_2bit: 0,
                mode: false,
                ram_enabled: false,
            }
        }

        fn rom_bank(&self) -> usize {
            ((self.register_2bit as usize) << 5) | (self.register_5bit as usize)
        }

        fn ram_bank(&self) -> usize {
            if self.mode {
                self.register_2bit as usize
            } else {
                0
            }
        }

        fn ram_index(&self, address: u16) -> usize {
            (self.ram_bank() * 0x2000 + (address as usize - 0xA000)) % self.ram.len()
        }
    }

    impl MBC for MBC1 {
        fn read(&self, address: u16) -> u8 {
            match address {
                0x0000..=0x3FFF => self.rom[address as usize % self.rom.len()],
                0x4000..=0x7FFF => {
                    let offset = self.rom_bank() * 0x4000 + (address as usize - 0x4000);
                    self.rom[offset % self.rom.len()]
                }
                0xA000..=0xBFFF => {
                    if self.ram.is_empty() || !self.ram_enabled {
                        0xFF
                    } else {
                        self.ram[self.ram_index(address)]
                    }
                }
                _ => 0xFF,
            }
        }

        fn write(&mut self, address: u16, value: u8) {
            match address {
                0x0000..=0x1FFF => self.ram_enabled = value.mask(0x0F) == 0x0A,
                0x2000..=0x3FFF => {
                    let bits = value.mask(0x1F);
                    self.register_5bit = if bits == 0 { 1 } else { bits };
                }
                0x4000..=0x5FFF => self.register_2bit = value.mask(0x03),
                0x6000..=0x7FFF => self.mode = value.mask(0x01) != 0,
                0xA000..=0xBFFF => {
                    if !self.ram.is_empty() && self.ram_enabled {
                        let index = self.ram_index(address);
                        self.ram[index] = value;
                    }
                }
                _ => {}
            }
        }

        fn get_rom(&self) -> &[u8] {
            &self.rom
        }

        fn get_ram(&self) -> &[u8] {
            &self.ram
        }
    }

    pub struct MBC2 {
        rom: Vec<u8>,
        ram: Vec<u8>,
        rom_bank: usize,
        ram_bank: usize,
    }

    impl MBC2 {
        pub fn new(rom: Vec<u8>, ram: Vec<u8>) -> Self {
            Self {
                rom,
                ram,
                rom_bank: 0,
                ram_bank: 0,
            }
        }
    }

    impl MBC for MBC2 {
        fn read(&self, address: u16) -> u8 {
            match address {
                0x0000..=0x3FFF => self.rom[address as usize],
                0x4000..=0x7FFF => {
                    let offset = self.rom_bank.max(1) * 0x4000 + (address as usize - 0x4000);
                    self.rom[offset % self.rom.len()]
                }
                0xA000..=0xBFFF => {
                    if self.ram.is_empty() {
                        0xFF
                    } else {
                        self.ram[address as usize - 0xA000]
                    }
                }
                _ => 0xFF,
            }
        }

        fn write(&mut self, address: u16, value: u8) {
            match address {
                0x2000..=0x3FFF => {
                    let bank = value.mask(0x1F) as usize;
                    self.rom_bank = if bank == 0 { 1 } else { bank };
                }
                _ => {}
            }
        }

        fn get_rom(&self) -> &[u8] {
            &self.rom
        }

        fn get_ram(&self) -> &[u8] {
            &self.ram
        }
    }

    pub struct MBC3 {
        rom: Vec<u8>,
        ram: Vec<u8>,
        rom_bank: usize,
        ram_bank: usize,
    }

    impl MBC3 {
        pub fn new(rom: Vec<u8>, ram: Vec<u8>) -> Self {
            Self {
                rom,
                ram,
                rom_bank: 0,
                ram_bank: 0,
            }
        }
    }

    impl MBC for MBC3 {
        fn read(&self, address: u16) -> u8 {
            match address {
                0x0000..=0x3FFF => self.rom[address as usize],
                0x4000..=0x7FFF => {
                    let offset = self.rom_bank.max(1) * 0x4000 + (address as usize - 0x4000);
                    self.rom[offset % self.rom.len()]
                }
                0xA000..=0xBFFF => {
                    if self.ram.is_empty() {
                        0xFF
                    } else {
                        self.ram[address as usize - 0xA000]
                    }
                }
                _ => 0xFF,
            }
        }

        fn write(&mut self, address: u16, value: u8) {
            match address {
                0x2000..=0x3FFF => {
                    let bank = value.mask(0x1F) as usize;
                    self.rom_bank = if bank == 0 { 1 } else { bank };
                }
                _ => {}
            }
        }

        fn get_rom(&self) -> &[u8] {
            &self.rom
        }

        fn get_ram(&self) -> &[u8] {
            &self.ram
        }
    }

    pub struct MBC5 {
        rom: Vec<u8>,
        ram: Vec<u8>,
        rom_bank: usize,
        ram_bank: usize,
    }

    impl MBC5 {
        pub fn new(rom: Vec<u8>, ram: Vec<u8>) -> Self {
            Self {
                rom,
                ram,
                rom_bank: 0,
                ram_bank: 0,
            }
        }
    }

    impl MBC for MBC5 {
        fn read(&self, address: u16) -> u8 {
            match address {
                0x0000..=0x3FFF => self.rom[address as usize],
                0x4000..=0x7FFF => {
                    let offset = self.rom_bank.max(1) * 0x4000 + (address as usize - 0x4000);
                    self.rom[offset % self.rom.len()]
                }
                0xA000..=0xBFFF => {
                    if self.ram.is_empty() {
                        0xFF
                    } else {
                        self.ram[address as usize - 0xA000]
                    }
                }
                _ => 0xFF,
            }
        }

        fn write(&mut self, address: u16, value: u8) {
            match address {
                0x2000..=0x3FFF => {
                    let bank = value.mask(0x1F) as usize;
                    self.rom_bank = if bank == 0 { 1 } else { bank };
                }
                _ => {}
            }
        }

        fn get_rom(&self) -> &[u8] {
            &self.rom
        }

        fn get_ram(&self) -> &[u8] {
            &self.ram
        }
    }
}

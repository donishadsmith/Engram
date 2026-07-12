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

        fn ram_changed(&mut self) -> &mut bool;
    }

    pub struct RomOnly {
        rom: Vec<u8>,
        ram: Vec<u8>,
        ram_updated: bool,
    }

    impl RomOnly {
        pub fn new(rom: Vec<u8>, ram: Vec<u8>) -> Self {
            Self {
                rom,
                ram,
                ram_updated: false,
            }
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

        fn ram_changed(&mut self) -> &mut bool {
            &mut self.ram_updated
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
        ram_updated: bool,
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
                ram_updated: false,
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
                        let index = self.ram_index(address);
                        self.ram[index]
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
                        self.ram_updated = true;
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

        fn ram_changed(&mut self) -> &mut bool {
            &mut self.ram_updated
        }
    }

    pub struct MBC2 {
        rom: Vec<u8>,
        ram: Vec<u8>,
        rom_bank: usize,
        ram_enabled: bool,
        ram_updated: bool,
    }

    impl MBC2 {
        const MBC2_RAM_SIZE: usize = 512;

        pub fn new(rom: Vec<u8>, ram: Vec<u8>) -> Self {
            let mut internal_ram = vec![0u8; Self::MBC2_RAM_SIZE];
            let n = ram.len().min(Self::MBC2_RAM_SIZE);
            internal_ram[..n].copy_from_slice(&ram[..n]);

            Self {
                rom,
                ram: internal_ram,
                rom_bank: 0,
                ram_enabled: false,
                ram_updated: false,
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
                    if !self.ram_enabled {
                        return 0xFF;
                    }
                    self.ram[(address & 0x1FF) as usize] | 0xF0
                }
                _ => 0xFF,
            }
        }

        fn write(&mut self, address: u16, value: u8) {
            match address {
                0x0000..=0x3FFF => {
                    let bit = address & 0x0100;
                    if bit == 0 {
                        self.ram_enabled = value.mask(0x0F) == 0x0A;
                    } else {
                        self.rom_bank = value.mask(0x0F) as usize;
                    }
                }
                0xA000..=0xBFFF => {
                    if self.ram.is_empty() || !self.ram_enabled {
                        return;
                    }
                    self.ram[(address & 0x1FF) as usize] = value & 0x0F;
                    self.ram_updated = true;
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

        fn ram_changed(&mut self) -> &mut bool {
            &mut self.ram_updated
        }
    }

    struct RTCRegister {
        seconds: u8,
        minutes: u8,
        hours: u8,
        dl: u8,
        dh: u8,
    }

    // For clock the plan will be to intercept with unix timestamp
    pub struct MBC3 {
        rom: Vec<u8>,
        ram: Vec<u8>,
        register_7bit: u8,
        ram_bank: usize,
        ram_enabled: bool,
        ram_updated: bool,
        rtc_register: u8,
        current_bank_value: u8,
        timer_enabled: bool,
    }

    impl MBC3 {
        pub fn new(rom: Vec<u8>, ram: Vec<u8>) -> Self {
            Self {
                rom,
                ram,
                register_7bit: 0,
                ram_bank: 0,
                ram_updated: false,
                ram_enabled: false,
                rtc_register: 0,
                current_bank_value: 0,
                timer_enabled: false,
            }
        }

        fn rom_bank(&self) -> usize {
            self.register_7bit as usize
        }

        fn ram_index(&self, address: u16) -> usize {
            (self.ram_bank * 0x2000 + (address as usize - 0xA000)) % self.ram.len()
        }
    }

    impl MBC for MBC3 {
        fn read(&self, address: u16) -> u8 {
            match address {
                0x0000..=0x3FFF => self.rom[address as usize],
                0x4000..=0x7FFF => {
                    let offset = self.rom_bank().max(1) * 0x4000 + (address as usize - 0x4000);
                    self.rom[offset % self.rom.len()]
                }
                0xA000..=0xBFFF => {
                    if self.current_bank_value <= 7 {
                        if self.ram.is_empty() || !self.ram_enabled {
                            0xFF
                        } else {
                            self.ram[self.ram_index(address)]
                        }
                    } else {
                        if self.timer_enabled {
                            return 0xFF;
                        } else {
                            return 0xFF;
                        }
                    }
                }
                _ => 0xFF,
            }
        }

        fn write(&mut self, address: u16, value: u8) {
            match address {
                0x0000..=0x1FFF => {
                    if value.mask(0x0F) == 0x0A {
                        self.ram_enabled = true;
                        self.timer_enabled = true;
                    }
                }
                0x2000..=0x3FFF => {
                    let bits = value.mask(0x7F);
                    self.register_7bit = if bits == 0 { 1 } else { bits };
                }
                0x4000..=0x5FFF => {
                    self.current_bank_value = value.mask(0x7F);
                    if self.current_bank_value <= 7 {
                        self.ram_bank = self.current_bank_value as usize;
                    } else {
                        self.rtc_register = self.current_bank_value as u8;
                    }
                }
                0xA000..=0xBFFF => {
                    if self.current_bank_value <= 7 && !self.ram.is_empty() && self.ram_enabled {
                        let index = self.ram_index(address);
                        self.ram[index] = value;
                        self.ram_updated = true;
                    }
                    // 0x08..=0x0C RTC stuff later
                }
                0x6000..=0x7FFF => {} // latch clock stuff
                _ => {}
            }
        }

        fn get_rom(&self) -> &[u8] {
            &self.rom
        }

        fn get_ram(&self) -> &[u8] {
            &self.ram
        }

        fn ram_changed(&mut self) -> &mut bool {
            &mut self.ram_updated
        }
    }

    pub struct MBC5 {
        rom: Vec<u8>,
        ram: Vec<u8>,
        register_8bit: u8,
        register_1bit: u8,
        ram_bank: usize,
        ram_enabled: bool,
        ram_updated: bool,
    }

    impl MBC5 {
        pub fn new(rom: Vec<u8>, ram: Vec<u8>) -> Self {
            Self {
                rom,
                ram,
                register_8bit: 0,
                register_1bit: 0,
                ram_bank: 0,
                ram_enabled: false,
                ram_updated: false,
            }
        }

        fn rom_bank(&self) -> usize {
            let bit = self.register_1bit as u16;
            (bit << 8).wrapping_add(self.register_8bit as u16) as usize
        }

        fn ram_index(&self, address: u16) -> usize {
            (self.ram_bank * 0x2000 + (address as usize - 0xA000)) % self.ram.len()
        }
    }

    impl MBC for MBC5 {
        fn read(&self, address: u16) -> u8 {
            match address {
                0x0000..=0x3FFF => self.rom[address as usize],
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
                0x2000..=0x2FFF => self.register_8bit = value,
                0x3000..=0x3FFF => self.register_1bit = value.mask(0x01),
                0x4000..=0x5FFF => self.ram_bank = (value & 0x0F) as usize,
                0xA000..=0xBFFF => {
                    if !self.ram.is_empty() && self.ram_enabled {
                        let index = self.ram_index(address);
                        self.ram[index] = value;
                        self.ram_updated = true;
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

        fn ram_changed(&mut self) -> &mut bool {
            &mut self.ram_updated
        }
    }
}

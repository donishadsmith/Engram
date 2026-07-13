pub mod prelude {
    use crate::components::{cpu::core::ByteOps8, rom::cartridge::MBCType};
    use chrono::Utc;

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
            self.get_rom().len()
        }

        fn ram_size(&self) -> usize {
            self.get_ram().len()
        }

        fn ram_changed(&mut self) -> &mut bool;

        fn id(&self) -> MBCType;

        fn is_timer_enabled(&self) -> bool {
            false
        }

        fn tick(&mut self) {}

        fn rtc_save_state(&self) -> Option<RTCSaveState> {
            None
        }
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

        fn id(&self) -> MBCType {
            MBCType::RomOnly
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
                0x0000..=0x1FFF => self.ram_enabled = (value & 0x0F) == 0x0A,
                0x2000..=0x3FFF => {
                    let bits = value & 0x1F;
                    self.register_5bit = if bits == 0 { 1 } else { bits };
                }
                0x4000..=0x5FFF => self.register_2bit = value & 0x03,
                0x6000..=0x7FFF => self.mode = (value & 0x01) != 0,
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

        fn id(&self) -> MBCType {
            MBCType::MBC1
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
                        self.ram_enabled = (value & 0x0F) == 0x0A;
                    } else {
                        self.rom_bank = (value & 0x0F) as usize;
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

        fn id(&self) -> MBCType {
            MBCType::MBC2
        }
    }

    #[derive(Clone, Copy)]
    pub struct RTCSaveState {
        pub previous_unix_timestamp: i64,
        pub seconds: u8,
        pub minutes: u8,
        pub hours: u8,
        pub dl: u8,
        pub dh: u8,
        pub latched_seconds: u8,
        pub latched_minutes: u8,
        pub latched_hours: u8,
        pub latched_dl: u8,
        pub latched_dh: u8,
    }

    impl RTCSaveState {
        pub const BYTE_SIZE: usize = 18;

        pub fn to_bytes(&self) -> [u8; Self::BYTE_SIZE] {
            let mut bytes = [0u8; Self::BYTE_SIZE];
            bytes[0..8].copy_from_slice(&self.previous_unix_timestamp.to_be_bytes());
            bytes[8] = self.seconds;
            bytes[9] = self.minutes;
            bytes[10] = self.hours;
            bytes[11] = self.dl;
            bytes[12] = self.dh;
            bytes[13] = self.latched_seconds;
            bytes[14] = self.latched_minutes;
            bytes[15] = self.latched_hours;
            bytes[16] = self.latched_dl;
            bytes[17] = self.latched_dh;

            bytes
        }

        pub fn from_bytes(bytes: &[u8]) -> Self {
            let timestamp_bytes: [u8; 8] = bytes[0..8].try_into().unwrap();

            Self {
                previous_unix_timestamp: i64::from_be_bytes(timestamp_bytes),
                seconds: bytes[8],
                minutes: bytes[9],
                hours: bytes[10],
                dl: bytes[11],
                dh: bytes[12],
                latched_seconds: bytes[13],
                latched_minutes: bytes[14],
                latched_hours: bytes[15],
                latched_dl: bytes[16],
                latched_dh: bytes[17],
            }
        }
    }

    struct LatchedClockData {
        seconds: u8,
        minutes: u8,
        hours: u8,
        dl: u8,
        dh: u8,
    }

    pub struct RTCRegister {
        previous_unix_timestamp: i64,
        bank: u8,
        seconds: u8,
        minutes: u8,
        hours: u8,
        dl: u8,
        dh: u8,
        latch_clock_value: u8,
        latched_clock_data: LatchedClockData,
    }

    impl RTCRegister {
        fn new(save_state: Option<RTCSaveState>) -> Self {
            let mut rtc = match save_state {
                Some(state) => Self {
                    previous_unix_timestamp: state.previous_unix_timestamp,
                    bank: 0x08,
                    seconds: state.seconds,
                    minutes: state.minutes,
                    hours: state.hours,
                    dl: state.dl,
                    dh: state.dh,
                    latch_clock_value: 0,
                    latched_clock_data: LatchedClockData {
                        seconds: state.latched_seconds,
                        minutes: state.latched_minutes,
                        hours: state.latched_hours,
                        dl: state.latched_dl,
                        dh: state.latched_dh,
                    },
                },
                None => Self {
                    previous_unix_timestamp: Utc::now().timestamp(),
                    bank: 0x08,
                    seconds: 0,
                    minutes: 0,
                    hours: 0,
                    dl: 0,
                    dh: 0,
                    latch_clock_value: 0,
                    latched_clock_data: LatchedClockData {
                        seconds: 0,
                        minutes: 0,
                        hours: 0,
                        dl: 0,
                        dh: 0,
                    },
                },
            };

            rtc.tick();

            rtc
        }

        fn is_halted(&self) -> bool {
            (self.dh & 0x40) != 0
        }

        fn read(&self) -> u8 {
            match self.bank {
                0x08 => self.latched_clock_data.seconds,
                0x09 => self.latched_clock_data.minutes,
                0x0A => self.latched_clock_data.hours,
                0x0B => self.latched_clock_data.dl,
                0x0C => self.latched_clock_data.dh & 0xC1,
                _ => 0xFF,
            }
        }

        fn write(&mut self, value: u8) {
            self.tick();

            match self.bank {
                0x08 => self.seconds = value % 60,
                0x09 => self.minutes = value % 60,
                0x0A => self.hours = value % 24,
                0x0B => self.dl = value,
                0x0C => self.dh = value & 0xC1,
                _ => {}
            }
        }

        fn days(&self) -> u16 {
            ((self.dh & 0x01) as u16) << 8 | self.dl as u16
        }

        fn set_days(&mut self, total_days: i64) {
            if total_days > 511 {
                self.dh |= 0x80;
            }

            let days = (total_days % 512) as u16;
            self.dl = (days & 0xFF) as u8;
            self.dh = (self.dh & 0xFE) | ((days >> 8) & 0x01) as u8;
        }

        fn latch(&mut self) {
            self.tick();

            self.latched_clock_data = LatchedClockData {
                seconds: self.seconds,
                minutes: self.minutes,
                hours: self.hours,
                dl: self.dl,
                dh: self.dh,
            };
        }

        fn tick(&mut self) {
            let current_timestamp = Utc::now().timestamp();
            if self.is_halted() {
                self.previous_unix_timestamp = current_timestamp; // don't accumulate time while halted
                return;
            }

            let seconds_passed = current_timestamp - self.previous_unix_timestamp;
            self.previous_unix_timestamp = current_timestamp;

            let seconds = self.seconds as i64 + seconds_passed;
            self.seconds = (seconds % 60) as u8;

            let minutes = self.minutes as i64 + seconds / 60;
            self.minutes = (minutes % 60) as u8;

            let hours = self.hours as i64 + minutes / 60;
            self.hours = (hours % 24) as u8;

            let days = self.days() as i64 + hours / 24;
            self.set_days(days);
        }

        fn save_state(&self) -> RTCSaveState {
            RTCSaveState {
                previous_unix_timestamp: self.previous_unix_timestamp,
                seconds: self.seconds,
                minutes: self.minutes,
                hours: self.hours,
                dl: self.dl,
                dh: self.dh,
                latched_seconds: self.latched_clock_data.seconds,
                latched_minutes: self.latched_clock_data.minutes,
                latched_hours: self.latched_clock_data.hours,
                latched_dl: self.latched_clock_data.dl,
                latched_dh: self.latched_clock_data.dh,
            }
        }
    }

    pub struct MBC3 {
        rom: Vec<u8>,
        ram: Vec<u8>,
        register_7bit: u8,
        ram_bank: usize,
        ram_enabled: bool,
        ram_updated: bool,
        rtc_register: RTCRegister,
        current_bank_value: u8,
        timer_enabled: bool,
    }

    impl MBC3 {
        pub fn new(rom: Vec<u8>, ram: Vec<u8>, rtc_save_state: Option<RTCSaveState>) -> Self {
            Self {
                rom,
                ram,
                register_7bit: 0,
                ram_bank: 0,
                ram_updated: false,
                ram_enabled: false,
                rtc_register: RTCRegister::new(rtc_save_state),
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
                        if !self.timer_enabled {
                            return 0xFF;
                        } else {
                            return self.rtc_register.read();
                        }
                    }
                }
                _ => 0xFF,
            }
        }

        fn write(&mut self, address: u16, value: u8) {
            match address {
                0x0000..=0x1FFF => {
                    let enabled = (value & 0x0F) == 0x0A;
                    self.ram_enabled = enabled;
                    self.timer_enabled = enabled;
                }
                0x2000..=0x3FFF => {
                    let bits = value & 0x7F;
                    self.register_7bit = if bits == 0 { 1 } else { bits };
                }
                0x4000..=0x5FFF => {
                    self.current_bank_value = value & 0x7F;
                    if self.current_bank_value <= 7 {
                        self.ram_bank = self.current_bank_value as usize;
                    } else {
                        self.rtc_register.bank = self.current_bank_value;
                    }
                }
                0xA000..=0xBFFF => {
                    if self.current_bank_value <= 7 && !self.ram.is_empty() && self.ram_enabled {
                        let index = self.ram_index(address);
                        self.ram[index] = value;
                        self.ram_updated = true;
                    } else if self.current_bank_value > 7 && self.timer_enabled {
                        self.rtc_register.write(value);
                        self.ram_updated = true;
                    }
                }
                0x6000..=0x7FFF => {
                    if self.rtc_register.latch_clock_value == 0x00 && value == 0x01 {
                        self.rtc_register.latch();
                    }

                    self.rtc_register.latch_clock_value = value;
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

        fn id(&self) -> MBCType {
            MBCType::MBC3
        }

        fn is_timer_enabled(&self) -> bool {
            self.timer_enabled
        }

        fn tick(&mut self) {
            self.rtc_register.tick();
        }

        fn rtc_save_state(&self) -> Option<RTCSaveState> {
            Some(self.rtc_register.save_state())
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
                0x0000..=0x1FFF => self.ram_enabled = (value & 0x0F) == 0x0A,
                0x2000..=0x2FFF => self.register_8bit = value,
                0x3000..=0x3FFF => self.register_1bit = value & 0x01,
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

        fn id(&self) -> MBCType {
            MBCType::MBC5
        }
    }
}

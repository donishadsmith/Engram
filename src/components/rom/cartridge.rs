/*
    https://tonisagrista.com/blog/2026/playkid/

    https://gbdev.io/pandocs/The_Cartridge_Header.html
    0147 — Cartridge type, indicates the memory bank controller based on some 8 bit value

    https://gbdev.io/pandocs/MBCs.html
    Gameboy can only see 64 KB but some Roms can be up to 1 MB, bank switching required
*/

use std::path::PathBuf;

use crate::components::rom::mbc::prelude::*;

// "MBC3" in ASCII
const MAGIC_NUMBERS: [u8; 4] = [0x4D, 0x42, 0x43, 0x33];
// 4 magic numebers for MBC3 with timer enabled + 18 RTC states = 22 bytes before the RAM save data
const SAV_HEADER_SIZE: usize = MAGIC_NUMBERS.len() + RTCSaveState::BYTE_SIZE;

#[derive(Clone, Copy, PartialEq, Debug)]
pub enum MBCType {
    RomOnly,
    MBC1,
    MBC2,
    MBC3,
    MBC5,
    Unknown(u8),
}

impl MBCType {
    fn byte_to_id(rom: &[u8]) -> Self {
        match rom[0x0147] {
            0x00 | 0x08 | 0x09 => MBCType::RomOnly,
            0x01..=0x03 => MBCType::MBC1,
            0x05 | 0x06 => MBCType::MBC2,
            0x0F..=0x13 => MBCType::MBC3,
            0x19..=0x1E => MBCType::MBC5,
            /*
                For now we will stick with these and maybe
                implement others such as HuC3 later, a lot of
                banks are chinese or japan exclusives or custom banks
            */
            other => MBCType::Unknown(other),
        }
    }

    pub fn to_str(&self) -> &str {
        match self {
            MBCType::RomOnly => "RomOnly",
            MBCType::MBC1 => "MBC1",
            MBCType::MBC2 => "MBC2",
            MBCType::MBC3 => "MBC3",
            MBCType::MBC5 => "MBC5",
            MBCType::Unknown(_) => "Unknown",
        }
    }

    pub fn to_struct(
        &self,
        rom: Vec<u8>,
        ram: Vec<u8>,
        rtc_save_state: Option<RTCSaveState>,
    ) -> Option<Box<dyn MBC>> {
        match self {
            MBCType::RomOnly => Some(Box::new(RomOnly::new(rom, ram))),
            MBCType::MBC1 => Some(Box::new(MBC1::new(rom, ram))),
            MBCType::MBC2 => Some(Box::new(MBC2::new(rom, ram))),
            MBCType::MBC3 => Some(Box::new(MBC3::new(rom, ram, rtc_save_state))),
            MBCType::MBC5 => Some(Box::new(MBC5::new(rom, ram))),
            MBCType::Unknown(_) => None,
        }
    }
}

#[derive(Clone, Copy, PartialEq, Debug)]
pub enum CGBFlag {
    CGB,
    DMG,
}

impl CGBFlag {
    fn byte_to_id(rom: &[u8]) -> Self {
        match rom[0x0143] {
            0x80 | 0xC0 => CGBFlag::CGB,
            _ => CGBFlag::DMG,
        }
    }

    pub fn to_str(&self) -> &str {
        match self {
            CGBFlag::CGB => "Color",
            CGBFlag::DMG => "Monochrome",
        }
    }
}

pub struct Header {
    pub title: String,
    pub mbc_type: MBCType,
    pub rom_size: usize,
    pub ram_size: usize,
    pub cgb_flag: CGBFlag,
    pub has_battery: bool,
    pub has_rumble: bool,
    pub has_timer: bool,
    pub checksum: u8,
}

impl Header {
    fn new(rom: &[u8]) -> Self {
        let mbc_type = Self::mbc(&rom);
        let title = Self::title(&rom);
        let rom_size = Self::rom_size(&rom);
        let ram_size = Self::ram_size(&rom);
        let cgb_flag = Self::mode(&rom);
        let has_battery = Self::has_battery(&rom);
        let has_rumble = Self::has_rumble(&rom);
        let has_timer = Self::has_timer(&rom);
        let checksum = Self::checksum(&rom);

        Self {
            mbc_type,
            title,
            rom_size,
            ram_size,
            cgb_flag,
            has_battery,
            has_rumble,
            has_timer,
            checksum,
        }
    }

    /*
        https://gbdev.io/pandocs/The_Cartridge_Header.html#footnote-mbc30
        0104-0133 — Nintendo logo; valid rom contains this
        0134-0143 — Title - in uppercase ASCII, if the title is less than 16 characters, it gets zero padded, which is NULL in ASCII
    */
    fn title(rom: &[u8]) -> String {
        rom[0x0134..=0x0143]
            .iter()
            .take_while(|&&byte| byte != 0)
            .map(|&byte| byte as char)
            .collect()
    }
    /*
        uint8_t - wraps
        uint8_t checksum = 0;
        for (uint16_t address = 0x0134; address <= 0x014C; address++) {
            checksum = checksum - rom[address] - 1;
        }
    */
    fn checksum(rom: &[u8]) -> u8 {
        let mut checksum: u8 = 0;
        for address in 0x0134..=0x014C {
            checksum = checksum.wrapping_sub(rom[address]).wrapping_sub(1);
        }

        checksum
    }

    fn mbc(rom: &[u8]) -> MBCType {
        MBCType::byte_to_id(rom)
    }

    fn mode(rom: &[u8]) -> CGBFlag {
        CGBFlag::byte_to_id(rom)
    }

    fn has_battery(rom: &[u8]) -> bool {
        match rom[0x0147] {
            0x03 | 0x06 | 0x09 | 0x0D..=0x10 | 0x13 | 0x1B | 0x1E | 0x22 | 0xFF => true,
            _ => false,
        }
    }

    fn has_rumble(rom: &[u8]) -> bool {
        match rom[0x0147] {
            0x1C..=0x1E => true,
            _ => false,
        }
    }

    fn has_timer(rom: &[u8]) -> bool {
        match rom[0x0147] {
            0x0F | 0x10 => true,
            _ => false,
        }
    }

    // 0148 — ROM size: 32 KiB * (1 << <value>)
    fn rom_size(rom: &[u8]) -> usize {
        32 * 1024 * (1usize << rom[0x0148] as usize)
    }

    // 0149 — RAM size
    fn ram_size(rom: &[u8]) -> usize {
        match rom[0x0149] {
            0x02 => 8 * 1024,   // 1 bank; bank size is multiple of 8
            0x03 => 32 * 1024,  // 4 banks of 8 KiB each
            0x04 => 128 * 1024, // 16 banks of 8 KiB each
            0x05 => 64 * 1024,  // 8 banks of 8 KiB each
            _ => 0,
        }
    }

    fn fake() -> Self {
        Self {
            title: String::from("Test"),
            mbc_type: MBCType::MBC3,
            rom_size: 0,
            ram_size: 0,
            cgb_flag: CGBFlag::DMG,
            has_battery: false,
            has_rumble: false,
            has_timer: false,
            checksum: 0,
        }
    }
}

pub struct Cartridge {
    pub header: Header,
    pub sav_path: PathBuf,
    pub mbc: Box<dyn MBC>,
}

impl Cartridge {
    pub fn load(filename: Option<std::path::PathBuf>) -> Result<Self, std::io::Error> {
        let Some(rom_path) = filename else {
            let file_error_msg = "Issue occured with file selection".to_string();

            return Err(Self::error_message(file_error_msg));
        };

        let rom = std::fs::read(&rom_path)?;
        if rom.len() < 0x150 {
            return Err(Self::error_message(
                "File too small to be a valid ROM".to_string(),
            ));
        }

        let header = Header::new(&rom);
        let sav_path = rom_path.with_extension("sav");
        let (ram, rtc_save_state) = Self::read_sav(&sav_path, &header)?;

        let mbc = Self::get_mbc(&header, rom, ram, rtc_save_state)?;

        Ok(Self {
            header,
            sav_path,
            mbc,
        })
    }

    pub fn error_message(message: String) -> std::io::Error {
        std::io::Error::new(std::io::ErrorKind::InvalidData, message)
    }

    fn get_mbc(
        header: &Header,
        rom: Vec<u8>,
        ram: Vec<u8>,
        rtc_save_state: Option<RTCSaveState>,
    ) -> Result<Box<dyn MBC>, std::io::Error> {
        header
            .mbc_type
            .to_struct(rom, ram, rtc_save_state)
            .ok_or_else(|| {
                Self::error_message(
                    "Only MBC1, MBC2, MBC3, MBC5, and RomOnly are supported.".to_string(),
                )
            })
    }

    pub fn read_sav(
        sav_path: &PathBuf,
        header: &Header,
    ) -> Result<(Vec<u8>, Option<RTCSaveState>), std::io::Error> {
        let mut ram = vec![0; header.ram_size];
        let mut rtc_save_state = None;

        if ram.is_empty() || !sav_path.exists() {
            return Ok((ram, rtc_save_state));
        }

        let mut sav_buffer = std::fs::read(sav_path)?;
        if sav_buffer.len() >= SAV_HEADER_SIZE && sav_buffer[0..4] == MAGIC_NUMBERS {
            rtc_save_state = Some(RTCSaveState::from_bytes(
                &sav_buffer[MAGIC_NUMBERS.len()..SAV_HEADER_SIZE],
            ));

            sav_buffer.drain(0..SAV_HEADER_SIZE);
        }

        let n = ram.len().min(sav_buffer.len());
        ram[..n].copy_from_slice(&sav_buffer[..n]);

        Ok((ram, rtc_save_state))
    }

    pub fn write_sav(&self) -> Result<(), std::io::Error> {
        if !self.header.has_battery || self.mbc.get_ram().is_empty() {
            return Ok(());
        }

        match self.mbc.rtc_save_state() {
            Some(state) => {
                let ram = self.mbc.get_ram();
                let mut buffer = Vec::with_capacity(SAV_HEADER_SIZE + ram.len());
                buffer.extend_from_slice(&MAGIC_NUMBERS);
                buffer.extend_from_slice(&state.to_bytes());
                buffer.extend_from_slice(ram);

                std::fs::write(&self.sav_path, buffer)?;
            }
            None => std::fs::write(&self.sav_path, self.mbc.get_ram())?,
        }

        Ok(())
    }

    // Just for testing purposes
    pub fn fake() -> Result<Self, std::io::Error> {
        let rom = Vec::new();
        let ram = Vec::new();
        let header = Header::fake();

        let mbc = Self::get_mbc(&header, rom, ram, None)?;

        Ok(Self {
            header: header,
            sav_path: PathBuf::new(),
            mbc,
        })
    }
}

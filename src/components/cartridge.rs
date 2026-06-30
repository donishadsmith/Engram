/*
    https://tonisagrista.com/blog/2026/playkid/

    https://gbdev.io/pandocs/The_Cartridge_Header.html
    0147 — Cartridge type, indicates the memory bank controller based on some 8 bit value

    https://gbdev.io/pandocs/MBCs.html
    Gameboy can only see 64 KB but some Roms can be up to 1 MB, bank switching required
*/

use std::path::PathBuf;

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
}

#[derive(Clone, Copy, PartialEq, Debug)]
pub enum CGBFlag {
    CBG,
    DMG,
}

impl CGBFlag {
    fn byte_to_id(rom: &[u8]) -> Self {
        match rom[0x0143] {
            0x80 | 0xC0 => CGBFlag::CBG,
            _ => CGBFlag::DMG,
        }
    }

    pub fn to_str(&self) -> &str {
        match self {
            CGBFlag::CBG => "Color",
            CGBFlag::DMG => "Monochrome",
        }
    }
}

pub struct Header {
    pub title: String,
    pub mbc_type: MBCType,
    pub rom_size: usize,
    pub ram_size: usize,
    pub cbc_flag: CGBFlag,
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
        let cbc_flag = Self::mode(&rom);
        let has_battery = Self::has_battery(&rom);
        let has_rumble = Self::has_rumble(&rom);
        let has_timer = Self::has_timer(&rom);
        let checksum = Self::checksum(&rom);

        Self {
            mbc_type,
            title,
            rom_size,
            ram_size,
            cbc_flag,
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
            checksum = checksum.wrapping_sub(rom[address].wrapping_sub(1));
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

    // 0148 — RAM size: 32 KiB * (1 << <value>)
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
            cbc_flag: CGBFlag::DMG,
            has_battery: false,
            has_rumble: false,
            has_timer: false,
            checksum: 0,
        }
    }
}

pub struct Cartridge {
    pub rom: Vec<u8>,
    pub header: Header,
    pub sram: Vec<u8>,
    pub sav_path: PathBuf,
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
        let mut sram = vec![0; header.ram_size];
        let sav_path = rom_path.with_extension("sav");
        sram = Self::read_sav(sram, &sav_path)?;

        Ok(Self {
            rom,
            header,
            sram,
            sav_path,
        })
    }

    pub fn error_message(message: String) -> std::io::Error {
        std::io::Error::new(std::io::ErrorKind::InvalidData, message)
    }

    pub fn read_sav(mut sram: Vec<u8>, sav_path: &PathBuf) -> Result<Vec<u8>, std::io::Error> {
        if sram.is_empty() || !sav_path.exists() {
            return Ok(sram);
        }

        let sav_buffer = std::fs::read(sav_path)?;
        sram.copy_from_slice(&sav_buffer);

        Ok(sram)
    }

    pub fn write_sav(&self) -> Result<(), std::io::Error> {
        if self.header.has_battery && !self.sram.is_empty() {
            std::fs::write(&self.sav_path, &self.sram)?;
        }

        Ok(())
    }

    // Just for testing purposes
    pub fn fake() -> Self {
        Self {
            rom: Vec::new(),
            header: Header::fake(),
            sram: Vec::new(),
            sav_path: PathBuf::new(),
        }
    }
}

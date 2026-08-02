// https://www.advanscene.com/html/dbstart.php#

use std::{
    fs::{read, write},
    io::Error,
    path::PathBuf,
};
// SRAM_F

// https://problemkaputt.de/gbatek-gba-cart-backup-ids.htm
#[derive(Clone, Copy, PartialEq, Eq)]
pub enum BackupType {
    Eeprom,
    Sram,
    SramF, // Apparently, there is an SRAM_F_V variant (e.g. Hamtaro Ham Ham Heartbreak)
    Flash,
    Flash512,
    Flash1M,
    None,
}

impl BackupType {
    fn to_vec(self) -> Vec<u8> {
        match self {
            BackupType::Sram | BackupType::SramF => vec![0u8; kilobytes(32)],
            BackupType::Flash | BackupType::Flash512 => vec![0u8; kilobytes(64)],
            BackupType::Flash1M => vec![0u8; kilobytes(128)],
            _ => Vec::<u8>::new(),
        }
    }

    // Just for checking purposes
    pub fn to_str(self) -> &'static str {
        match self {
            BackupType::Eeprom => "EEPROM_V",
            BackupType::Sram => "SRAM_V",
            BackupType::SramF => "SRAM_F_V",
            BackupType::Flash => "FLASH_V",
            BackupType::Flash512 => "FLASH512_V",
            BackupType::Flash1M => "FLASH1M_V",
            BackupType::None => "None",
        }
    }
}

fn detect_save_type(rom: &[u8]) -> BackupType {
    for (needle, kind) in [
        (b"FLASH1M_V".as_slice(), BackupType::Flash1M),
        (b"FLASH512_V".as_slice(), BackupType::Flash512),
        (b"FLASH_V".as_slice(), BackupType::Flash),
        (b"SRAM_F_V".as_slice(), BackupType::SramF),
        (b"SRAM_V".as_slice(), BackupType::Sram),
        (b"EEPROM_V".as_slice(), BackupType::Eeprom),
    ] {
        if rom.windows(needle.len()).any(|x| x == needle) {
            return kind;
        }
    }

    BackupType::None
}

fn error_message(message: String) -> std::io::Error {
    std::io::Error::new(std::io::ErrorKind::InvalidData, message)
}

fn kilobytes(value: usize) -> usize {
    value * 1024
}

pub struct GamePak {
    pub rom: Vec<u8>,
    sav_path: PathBuf,
    pub backup_memory: Vec<u8>,
    pub backup_type: BackupType,
    pub ram_updated: bool,
}

impl GamePak {
    pub fn load(filename: Option<PathBuf>) -> Result<Self, Error> {
        let Some(rom_path) = filename else {
            let file_error_msg = "Issue occured with file selection".to_string();

            return Err(error_message(file_error_msg));
        };

        let rom = std::fs::read(&rom_path)?;

        let sav_path = rom_path.with_extension("sav");
        let backup_type = detect_save_type(&rom);
        let backup_memory = Self::read_sav(&sav_path, backup_type)?;

        Ok(Self {
            rom,
            sav_path,
            backup_type,
            backup_memory,
            ram_updated: false,
        })
    }

    // Eventually append RTC data to the end
    pub fn read_sav(sav_path: &PathBuf, backup_type: BackupType) -> Result<Vec<u8>, Error> {
        let mut backup_memory = backup_type.to_vec();

        if !sav_path.exists() {
            return Ok(backup_memory);
        }

        let buffer = read(sav_path)?;

        if backup_type == BackupType::Eeprom {
            return Ok(buffer);
        }

        let n = buffer.len().min(backup_memory.len());
        backup_memory[..n].copy_from_slice(&buffer[..n]);

        Ok(backup_memory)
    }

    pub fn write_sav(&mut self) -> Result<(), Error> {
        if self.ram_updated && self.has_backup() {
            write(&self.sav_path, &self.backup_memory)?;
        }

        self.ram_updated = false;

        Ok(())
    }

    pub fn has_backup(&self) -> bool {
        self.backup_type != BackupType::None
    }

    // https://densinh.github.io/DenSinH/emulation/2021/02/01/gba-eeprom.html
    // https://problemkaputt.de/gbatek.htm#gbacartbackupeeprom
    // We will leave this eeprom for later, apparently, the hardware is a
    // single wire that writes one bit at a time and the number of bits determined
    // the size but only somewhat reliably, this seems to also require DMA, since DMA3
    // can access rom, maybe this should be done closer to the emulator end than trying
    // to wire up right now
    fn is_eeprom_address(&self, address: u32) -> bool {
        if self.backup_type != BackupType::Eeprom {
            return false;
        }

        if self.rom.len() > 0x1000000 {
            address >= 0x0DFFFF00 && address <= 0x0DFFFFFF
        } else {
            // anywhere in 0x0D000000-0x0DFFFFFF
            (address >> 24) == 0x0D
        }
    }

    pub fn read_rom_region(&self, address: u32) -> u8 {
        if self.is_eeprom_address(address) {
            //self.eeprom_read_bit()
            0
        } else {
            self.rom_byte(address)
        }
    }

    #[inline]
    fn rom_byte(&self, address: u32) -> u8 {
        let index = (address & 0x01FFFFFF) as usize;
        self.rom.get(index).copied().unwrap_or(0)
    }

    pub fn mock() -> Self {
        Self { rom: vec![0u8; kilobytes(32000)], sav_path: PathBuf::from("mock.sav"), backup_memory: vec![0u8; kilobytes(32)], backup_type: BackupType::Sram, ram_updated: false }
    }
}

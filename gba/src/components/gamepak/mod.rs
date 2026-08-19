// https://www.advanscene.com/html/dbstart.php#
//** REMEMBER TO IMPLEMENT EEPROM*****

mod eeprom;
mod flash;
mod sram;

use eeprom::{EEPROM_4KBIT, Eeprom};
use flash::Flash;
use sram::Sram;

use crate::components::utils::BitOps;
use std::{fs::read, io::Error, path::PathBuf};

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
    fn to_enum(self) -> BackupChip {
        match self {
            BackupType::Sram | BackupType::SramF => {
                BackupChip::Sram(Sram::new(vec![0u8; kilobytes(32)]))
            }
            BackupType::Flash | BackupType::Flash512 => {
                BackupChip::Flash(Flash::new(vec![0u8; kilobytes(64)]))
            }
            BackupType::Flash1M => BackupChip::Flash(Flash::new(vec![0u8; kilobytes(128)])),
            BackupType::Eeprom => BackupChip::Eeprom(Eeprom::new(vec![0u8; EEPROM_4KBIT])), // default to the small version
            BackupType::None => BackupChip::None,
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

#[derive(PartialEq, Eq)]
pub enum BackupChip {
    None,
    Eeprom(Eeprom),
    Sram(Sram),
    Flash(Flash),
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
    pub backup_chip: BackupChip,
}

impl GamePak {
    pub fn load(filename: Option<PathBuf>) -> Result<Self, Error> {
        let Some(rom_path) = filename else {
            let file_error_msg = "Issue occured with file selection".to_string();

            return Err(error_message(file_error_msg));
        };

        let rom = std::fs::read(&rom_path)?;

        let sav_path = rom_path.with_extension("sav");
        let mut backup_chip = detect_save_type(&rom).to_enum();
        Self::read_sav(&sav_path, &mut backup_chip)?;

        Ok(Self {
            rom,
            sav_path,
            backup_chip,
        })
    }

    // Eventually incorporate RTC data
    pub fn read_sav(sav_path: &PathBuf, backup_chip: &mut BackupChip) -> Result<(), Error> {
        if !sav_path.exists() {
            return Ok(());
        }

        let buffer = read(sav_path)?;

        match backup_chip {
            BackupChip::Eeprom(eeprom) => {
                if buffer.len() > eeprom.memory.len() {
                    eeprom.increase_capacity();
                }

                Self::copy_sav_data(buffer, &mut eeprom.memory)
            }
            BackupChip::Flash(flash) => {
                Self::copy_sav_data(buffer, &mut flash.memory);
            }
            BackupChip::Sram(sram) => {
                Self::copy_sav_data(buffer, &mut sram.memory);
            }
            BackupChip::None => {}
        }

        Ok(())
    }

    pub fn copy_sav_data(save_buffer: Vec<u8>, memory: &mut Vec<u8>) {
        let n = save_buffer.len().min(memory.len());

        memory[..n].copy_from_slice(&save_buffer[..n]);
    }

    pub fn write_sav(&mut self) -> Result<(), Error> {
        match &mut self.backup_chip {
            BackupChip::Eeprom(eeprom) => eeprom.write_sav(&self.sav_path)?,
            BackupChip::Sram(sram) => sram.write_sav(&self.sav_path)?,
            BackupChip::Flash(flash) => flash.write_sav(&self.sav_path)?,
            BackupChip::None => {}
        }

        Ok(())
    }

    // https://densinh.github.io/DenSinH/emulation/2021/02/01/gba-eeprom.html
    // https://problemkaputt.de/gbatek.htm#gbacartbackupeeprom
    // **** REMEMBER TO IMPLEMENT EEPROM***** the hardware is a
    // single wire that writes one bit at a time and the number of bits determined
    // the size but only somewhat reliably, this seems to also require DMA, since DMA3
    // can access rom, maybe this should be done closer to the emulator end than trying
    // to wire up right now
    fn is_eeprom_address(&self, address: u32) -> bool {
        if !matches!(self.backup_chip, BackupChip::Eeprom(_)) {
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
            0
        } else {
            self.rom_byte(address)
        }
    }

    #[inline]
    fn rom_byte(&self, address: u32) -> u8 {
        let index = (address.get_bit_range(0..25)) as usize;
        self.rom.get(index).copied().unwrap_or(0)
    }

    pub fn mock() -> Self {
        Self {
            rom: vec![8u8; kilobytes(32000)],
            sav_path: PathBuf::from("mock.sav"),
            backup_chip: BackupType::to_enum(BackupType::Flash1M),
        }
    }
}

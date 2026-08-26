// https://www.advanscene.com/html/dbstart.php#

mod eeprom;
mod flash;
mod sram;

use eeprom::{EEPROM_4KBIT, Eeprom};
use flash::Flash;
use sram::Sram;

use crate::components::{gamepak::flash::FlashSize, utils::BitOps};
use shared::utils::error_message;
use std::{
    fs::{read, write},
    io::Error,
    path::PathBuf,
};

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
                BackupChip::Sram(Sram::new(vec![0; kilobytes(32)]))
            }
            BackupType::Flash | BackupType::Flash512 => {
                BackupChip::Flash(Flash::new(vec![0xFF; kilobytes(64)], FlashSize::Flash64k))
            }
            BackupType::Flash1M => {
                BackupChip::Flash(Flash::new(vec![0xFF; kilobytes(128)], FlashSize::Flash128k))
            }
            BackupType::Eeprom => BackupChip::Eeprom(Eeprom::new(vec![0xFF; EEPROM_4KBIT], false)), // default to the small version
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

fn kilobytes(value: usize) -> usize {
    value * 1024
}

pub struct GamePak {
    pub rom: Vec<u8>,
    sav_path: PathBuf,
    pub backup_chip: BackupChip,
}

impl GamePak {
    pub fn load(rom_path: PathBuf) -> Result<Self, Error> {
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

    // Eventually incorporate RTC data; https://problemkaputt.de/gbatek-gba-cart-backup-eeprom.htm
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

                eeprom.size_known = true;
                copy_sav_data(buffer, &mut eeprom.memory)
            }
            BackupChip::Flash(flash) => {
                copy_sav_data(buffer, &mut flash.memory);
            }
            BackupChip::Sram(sram) => {
                copy_sav_data(buffer, &mut sram.memory);
            }
            BackupChip::None => {}
        }

        Ok(())
    }

    pub fn write_sav(&self) -> Result<(), Error> {
        match &self.backup_chip {
            BackupChip::Eeprom(eeprom) => write(&self.sav_path, &eeprom.memory)?,
            BackupChip::Sram(sram) => write(&self.sav_path, &sram.memory)?,
            BackupChip::Flash(flash) => write(&self.sav_path, &flash.memory)?,
            BackupChip::None => {}
        }

        Ok(())
    }

    #[inline]
    pub fn read_rom_region(&self, address: u32) -> u8 {
        let index = (address.get_bit_range(0..25)) as usize;
        self.rom.get(index).copied().unwrap_or(0)
    }

    pub fn mock(backup_type: BackupType) -> Self {
        Self {
            rom: vec![8u8; kilobytes(32000)],
            sav_path: PathBuf::from("mock.sav"),
            backup_chip: BackupType::to_enum(backup_type),
        }
    }
}

pub fn copy_sav_data(save_buffer: Vec<u8>, memory: &mut Vec<u8>) {
    let n = save_buffer.len().min(memory.len());

    memory[..n].copy_from_slice(&save_buffer[..n]);
}

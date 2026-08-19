use std::{fs::write, io::Error, path::PathBuf};

const EEPROM_64KBIT: usize = 8192;
pub const EEPROM_4KBIT: usize = 512;

#[derive(PartialEq, Eq)]
pub struct Eeprom {
    pub memory: Vec<u8>,
    pub updated: bool,
}

impl Eeprom {
    pub fn new(memory: Vec<u8>) -> Self {
        Self {
            memory,
            updated: false,
        }
    }

    pub fn write_sav(&mut self, sav_path: &PathBuf) -> Result<(), Error> {
        if self.updated {
            write(&sav_path, &self.memory)?;
            self.updated = false;
        }

        Ok(())
    }

    pub fn increase_capacity(&mut self) {
        if self.memory.len() == EEPROM_4KBIT {
            self.memory.reserve_exact(EEPROM_64KBIT - EEPROM_4KBIT);
        }
    }

    pub fn write(&mut self, index: usize, value: u8) {
        self.updated = true;

        self.memory[index] = value;
    }
}

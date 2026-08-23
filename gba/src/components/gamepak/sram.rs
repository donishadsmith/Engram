use std::{fs::write, io::Error, path::PathBuf};

// https://problemkaputt.de/gbatek-gba-cart-backup-sram-fram.htm
#[derive(PartialEq, Eq)]
pub struct Sram {
    pub memory: Vec<u8>,
    pub updated: bool,
}

impl Sram {
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

    pub fn write(&mut self, index: usize, value: u8) {
        self.updated = true;

        self.memory[index] = value;
    }
}

use std::{fs::write, io::Error, path::PathBuf};

#[derive(PartialEq, Eq)]
pub struct Flash {
    pub memory: Vec<u8>,
    pub updated: bool,
}

impl Flash {
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

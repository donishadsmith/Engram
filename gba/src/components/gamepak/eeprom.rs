// https://problemkaputt.de/gbatek-gba-cart-backup-eeprom.htm
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

    pub fn increase_capacity(&mut self) {
        if self.memory.len() == EEPROM_4KBIT {
            self.memory.resize(EEPROM_64KBIT, 0);
        }
    }
}

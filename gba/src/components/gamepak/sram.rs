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

    pub fn read(&self, address: u32) {}

    pub fn write(&mut self, index: usize, value: u8) {
        self.updated = true;

        self.memory[index] = value;
    }
}

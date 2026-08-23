// https://problemkaputt.de/gbatek-gba-cart-real-time-clock-rtc.htm
// https://problemkaputt.de/gbatek-gba-cart-backup-flash-rom.htm
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

    pub fn read(&self, address: u32) {}

    pub fn write(&mut self, index: usize, value: u8) {
        self.updated = true;

        self.memory[index] = value;
    }
}

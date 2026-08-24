// https://problemkaputt.de/gbatek-gba-cart-backup-sram-fram.htm
// check this and bus if hamtaro ham ham heartbreak or any megaman battle networks saves dont work
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

    pub fn read(&self, index: usize) -> u8 {
        self.memory[index]
    }

    pub fn write(&mut self, index: usize, value: u8) {
        self.updated = true;

        self.memory[index] = value;
    }
}

#[cfg(test)]
mod tests {
    use crate::components::{bus::AccessType, gamepak::BackupType, utils::create_bus};

    #[test]
    fn test_sram_u16_write() {
        let mut bus = create_bus(BackupType::Sram);

        bus.write_u16(0x0E000001, 0xAABB, AccessType::Nonsequential);
        assert_eq!(bus.read_u8(0x0E000001, AccessType::Nonsequential), 0xAA);
    }

    #[test]
    fn test_sram_u16_read() {
        let mut bus = create_bus(BackupType::Sram);

        bus.write_u8(0x0E000000, 0x42, AccessType::Nonsequential);
        assert_eq!(bus.read_u16(0x0E000000, AccessType::Nonsequential), 0x4242);
    }

    #[test]
    fn test_sram_mirror() {
        let mut bus = create_bus(BackupType::Sram);

        bus.write_u8(0x0E000000, 0x42, AccessType::Nonsequential);
        assert_eq!(bus.read_u8(0x0E008000, AccessType::Nonsequential), 0x42);
        assert_eq!(bus.read_u8(0x0F000000, AccessType::Nonsequential), 0x42);
    }
}

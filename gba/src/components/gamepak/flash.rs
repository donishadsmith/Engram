// https://github.com/michelhe/rustboyadvance-ng/blob/master/core/src/cartridge/backup/flash.rs
// https://github.com/ioncodes/ayyboy-advance/blob/master/gba-core/src/cartridge/flash.rs
// https://problemkaputt.de/gbatek-gba-cart-real-time-clock-rtc.htm
// https://problemkaputt.de/gbatek-gba-cart-backup-flash-rom.htm
// https://nexusindustrialmemory.com/guides/what-is-nand-memory/
// https://dev.to/amanprasad/flash-memory-explained-nand-vs-nor-architecture-and-memory-organization-3abf

// https://dillonbeliveau.com/2020/06/05/GBA-FLASH.html <- amazing resource!!!

// pokemon uses this
use crate::components::utils::BitOps;

const MACRONIX_64K_ID: u16 = 0x1CC2;
const MACRONIX_128K_ID: u16 = 0x09C2;

const SECTOR_SIZE: usize = 4096;
const BANK_SIZE: usize = 0x10000;

#[derive(PartialEq, Eq)]
enum FlashMode {
    Normal,
    ChipId,
    EraseArmed,
    Write,
    Bank,
}

#[derive(PartialEq, Eq)]
pub enum FlashSize {
    Flash64k,
    Flash128k,
}

#[derive(PartialEq, Eq)]
enum WriteSequence {
    Ready,
    ReceivedAA,
    ReceivedAA55,
    AwaitArgument,
}

#[derive(PartialEq, Eq)]
pub struct Flash {
    pub memory: Vec<u8>,
    pub updated: bool,
    pub bank: usize,
    mode: FlashMode,
    write_sequence: WriteSequence,
    size: FlashSize,
}

impl Flash {
    pub fn new(memory: Vec<u8>, size: FlashSize) -> Self {
        Self {
            memory,
            updated: false,
            bank: 0,
            mode: FlashMode::Normal,
            write_sequence: WriteSequence::Ready,
            size,
        }
    }

    fn manufacturer_id(&self) -> u16 {
        match self.size {
            FlashSize::Flash64k => MACRONIX_64K_ID.get_bit_range(0..8),
            FlashSize::Flash128k => MACRONIX_128K_ID.get_bit_range(0..8),
        }
    }

    fn device_id(&self) -> u16 {
        match self.size {
            FlashSize::Flash64k => MACRONIX_64K_ID.get_bit_range(8..16),
            FlashSize::Flash128k => MACRONIX_128K_ID.get_bit_range(8..16),
        }
    }

    pub fn read(&self, address: u32) -> u8 {
        let offset = address & 0xFFFF;
        match self.mode {
            FlashMode::Normal | FlashMode::EraseArmed | FlashMode::Write | FlashMode::Bank => {
                self.memory[self.offset(address)]
            }
            FlashMode::ChipId => {
                if offset == 0 {
                    return self.manufacturer_id() as u8;
                }

                if offset == 1 {
                    return self.device_id() as u8;
                } else {
                    return 0xFF;
                }
            }
        }
    }

    pub fn write(&mut self, address: u32, value: u8) {
        let offset = address & 0xFFFF;
        match self.write_sequence {
            WriteSequence::Ready => {
                if offset == 0x5555 && value == 0xAA {
                    self.write_sequence = WriteSequence::ReceivedAA;
                } else if offset == 0x5555 && value == 0xF0 {
                    self.mode = FlashMode::Normal;
                }
            }
            WriteSequence::ReceivedAA => {
                if offset == 0x2AAA && value == 0x55 {
                    self.write_sequence = WriteSequence::ReceivedAA55;
                } else {
                    self.write_sequence = WriteSequence::Ready;
                }
            }
            WriteSequence::ReceivedAA55 => {
                if value == 0x30 && self.mode == FlashMode::EraseArmed {
                    self.erase_sector(address);
                    self.mode = FlashMode::Normal;
                    self.reset_sequence();
                } else if offset == 0x5555 {
                    match value {
                        0x90 => {
                            self.mode = FlashMode::ChipId;
                            self.reset_sequence();
                        }
                        0xF0 => {
                            self.mode = FlashMode::Normal;
                            self.reset_sequence()
                        }
                        0x10 if self.mode == FlashMode::EraseArmed => {
                            self.erase_full_chip();
                            self.mode = FlashMode::Normal;
                            self.reset_sequence();
                        }
                        0x80 => {
                            self.mode = FlashMode::EraseArmed;
                            self.reset_sequence();
                        }
                        0xA0 => {
                            self.mode = FlashMode::Write;
                            self.write_sequence = WriteSequence::AwaitArgument
                        }
                        0xB0 => {
                            self.mode = FlashMode::Bank;
                            self.write_sequence = WriteSequence::AwaitArgument
                        }
                        _ => self.reset_sequence(),
                    }
                }
            }
            WriteSequence::AwaitArgument => {
                match self.mode {
                    FlashMode::Write => {
                        self.store_byte(address, value);
                        self.reset_sequence();
                    }
                    FlashMode::Bank => {
                        if self.size == FlashSize::Flash128k {
                            self.bank = (value & 1) as usize;
                        }
                    }
                    _ => {}
                }

                self.mode = FlashMode::Normal;
                self.reset_sequence();
            }
        }
    }

    fn store_byte(&mut self, address: u32, value: u8) {
        let index = self.offset(address);
        self.memory[index] = value;

        self.updated = true;
    }

    fn erase_full_chip(&mut self) {
        self.memory.fill(0xFF);

        self.updated = true;
    }

    fn erase_sector(&mut self, address: u32) {
        let start = self.offset(address & 0xF000);
        self.memory[start..start + SECTOR_SIZE].fill(0xFF);

        self.updated = true;
    }

    fn offset(&self, address: u32) -> usize {
        self.bank * BANK_SIZE + (address & 0xFFFF) as usize
    }

    fn reset_sequence(&mut self) {
        self.write_sequence = WriteSequence::Ready
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_chip_id_sequence() {
        let mut flash = Flash::new(vec![0xFF; BANK_SIZE], FlashSize::Flash64k);

        flash.write(0x5555, 0xAA);
        flash.write(0x2AAA, 0x55);
        flash.write(0x5555, 0x90);
        assert_eq!(flash.read(0), 0xC2, "the manufacturer id");
        assert_eq!(flash.read(1), 0x1C, "the device id");

        flash.write(0x5555, 0xAA);
        flash.write(0x2AAA, 0x55);
        flash.write(0x5555, 0xF0);
        assert_eq!(flash.read(0), 0xFF, "go back to normal read ops");
    }

    #[test]
    fn test_erase_sector() {
        let mut flash = Flash::new(vec![0xAB; BANK_SIZE], FlashSize::Flash64k);

        flash.write(0x5555, 0xAA);
        flash.write(0x2AAA, 0x55);
        flash.write(0x5555, 0x80);

        flash.write(0x5555, 0xAA);
        flash.write(0x2AAA, 0x55);
        flash.write(0x3000, 0x30);

        assert_eq!(flash.read(0x3000), 0xFF, "beginning of sector erased");
        assert_eq!(flash.read(0x3FFF), 0xFF, "end of sector erased");
        assert_eq!(flash.read(0x2FFF), 0xAB, "lower sector not erased");
        assert_eq!(flash.read(0x4000), 0xAB, "higher sector not erased");
        assert!(flash.updated, "ram updated");
    }

    #[test]
    fn test_erase_full_chip() {
        let mut flash = Flash::new(vec![0xAB; BANK_SIZE], FlashSize::Flash64k);

        flash.write(0x5555, 0xAA);
        flash.write(0x2AAA, 0x55);
        flash.write(0x5555, 0x80);

        flash.write(0x5555, 0xAA);
        flash.write(0x2AAA, 0x55);
        flash.write(0x5555, 0x10);

        assert!(flash.memory.iter().all(|&b| b == 0xFF), "earase all chip");
    }

    #[test]
    fn test_ignore_erase_sequence() {
        let mut flash = Flash::new(vec![0xAB; BANK_SIZE], FlashSize::Flash64k);

        flash.write(0x5555, 0xAA);
        flash.write(0x2AAA, 0x55);
        flash.write(0x3000, 0x30);

        assert_eq!(flash.read(0x3000), 0xAB, "shouldnt erase cause not armed");
    }
}

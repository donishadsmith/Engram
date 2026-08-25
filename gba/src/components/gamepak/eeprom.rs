use crate::components::utils::BitOps;

// https://github.com/ioncodes/ayyboy-advance/blob/master/gba-core/src/cartridge/eeprom.rs
// https://densinh.github.io/DenSinH/emulation/2021/02/01/gba-eeprom.html
// https://www.lenovo.com/us/en/glossary/what-is-eeprom/?orgRef=https%253A%252F%252Fwww.google.com%252F&srsltid=AfmBOooBU-eWhO4G0SFCYlmG2TaVtcwn-JZNVt--8QjeWm4twFoXAZ2y
const EEPROM_64KBIT: usize = 8192;
pub const EEPROM_4KBIT: usize = 512;

#[derive(Clone, Copy, PartialEq, Eq)]
enum Command {
    Read,
    Write,
}

impl Command {
    fn from_command_bits(command: u8) -> Command {
        match command {
            0b10 => Command::Write,
            0b11 => Command::Read,
            _ => unreachable!(),
        }
    }
}

#[derive(Clone, Copy, PartialEq, Eq)]
enum EepromMode {
    Idle,
    Command(u8),
    Address {
        command: Command,
        bits_received: u8,
        address: u16,
    },
    WriteData {
        block_start_index: usize,
        bits_received: u8,
        data: u64,
    },
    ReadData {
        block_start_index: usize,
        bits_sent: u8,
    },
}

#[derive(PartialEq, Eq)]
pub struct Eeprom {
    pub memory: Vec<u8>,
    pub updated: bool,
    mode: EepromMode,
    pub size_known: bool,
}

impl Eeprom {
    pub fn new(memory: Vec<u8>, size_known: bool) -> Self {
        Self {
            memory,
            updated: false,
            mode: EepromMode::Idle,
            size_known,
        }
    }

    pub fn increase_capacity(&mut self) {
        if self.memory.len() == EEPROM_4KBIT {
            self.memory.resize(EEPROM_64KBIT, 0);
        }
    }

    pub fn read_bit(&mut self) -> u16 {
        match &mut self.mode {
            EepromMode::ReadData {
                block_start_index,
                bits_sent,
            } => {
                if *bits_sent < 4 {
                    *bits_sent += 1;
                    return 0;
                } else {
                    let full_word = u64::from_be_bytes([
                        self.memory[*block_start_index],
                        self.memory[*block_start_index + 1],
                        self.memory[*block_start_index + 2],
                        self.memory[*block_start_index + 3],
                        self.memory[*block_start_index + 4],
                        self.memory[*block_start_index + 5],
                        self.memory[*block_start_index + 6],
                        self.memory[*block_start_index + 7],
                    ]);

                    let index = (*bits_sent - 4) as usize;
                    *bits_sent += 1;

                    if *bits_sent == 68 {
                        self.mode = EepromMode::Idle;
                    }

                    return full_word.get_bit(63 - index) as u16;
                }
            }
            _ => 1,
        }
    }

    pub fn write_bit(&mut self, value: u16) {
        let value = value.get_bit_range(0..1) as u8;
        let expected_address_size = self.expected_address_size();

        match &mut self.mode {
            EepromMode::Idle => {
                if value == 1 {
                    self.mode = EepromMode::Command(1);
                }
            }
            EepromMode::Command(command) => {
                *command = (*command << 1) | value;
                self.mode = EepromMode::Address {
                    command: Command::from_command_bits(*command),
                    bits_received: 0,
                    address: 0,
                }
            }
            EepromMode::Address {
                command,
                bits_received,
                address,
            } => {
                *address = (*address << 1) | value as u16;

                *bits_received += 1;
                if *bits_received == expected_address_size {
                    if expected_address_size == 14 {
                        *address = *address & 0x3FF
                    } else {
                        *address = *address & 0x3F
                    }

                    self.mode = match command {
                        Command::Write => EepromMode::WriteData {
                            block_start_index: (*address * 8) as usize,
                            bits_received: 0,
                            data: 0,
                        },
                        Command::Read => EepromMode::ReadData {
                            block_start_index: (*address * 8) as usize,
                            bits_sent: 0,
                        },
                    }
                }
            }
            EepromMode::WriteData {
                block_start_index,
                bits_received,
                data,
            } => {
                // use the stop bit to go back to idle
                if *bits_received == 64 {
                    self.mode = EepromMode::Idle;

                    return;
                }

                *data = (*data << 1) | value as u64;
                *bits_received += 1;
                if *bits_received == 64 {
                    self.updated = true;

                    for (offset, byte) in u64::to_be_bytes(*data).iter().enumerate() {
                        self.memory[*block_start_index + offset] = *byte;
                    }
                }
            }
            EepromMode::ReadData { .. } => {} // waste the stop bit
        }
    }

    fn expected_address_size(&self) -> u8 {
        if self.memory.len() == EEPROM_4KBIT {
            6
        } else {
            14
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_write1() {
        let mut eeprom = Eeprom::new(vec![0xFF; EEPROM_4KBIT], true);
        for bit in [1, 0] {
            eeprom.write_bit(bit);
        }

        for bit in [0, 0, 0, 0, 1, 0] {
            eeprom.write_bit(bit);
        }

        for bit in [1, 0, 1, 0, 1, 0, 1, 1] {
            eeprom.write_bit(bit);
        }

        for _ in 0..56 {
            eeprom.write_bit(0);
        }
        eeprom.write_bit(0); // stop

        assert_eq!(
            eeprom.memory[16], 0xAB,
            "first byte shuld be at block start"
        );
        assert_eq!(eeprom.memory[17..24], [0; 7]);
        assert!(eeprom.updated);
    }

    #[test]
    fn test_read() {
        let mut eeprom = Eeprom::new(vec![0xFF; EEPROM_4KBIT], true);

        eeprom.memory[16] = 0xAB;
        eeprom.memory[17] = 0x01;

        for bit in [1, 1] {
            eeprom.write_bit(bit);
        }

        for bit in [0, 0, 0, 0, 1, 0] {
            eeprom.write_bit(bit);
        }

        eeprom.write_bit(0);

        for i in 0..4 {
            assert_eq!(eeprom.read_bit(), 0, "dummy bit {i}");
        }

        for (i, expected) in [1, 0, 1, 0, 1, 0, 1, 1].iter().enumerate() {
            assert_eq!(eeprom.read_bit(), *expected, "bit {i} for 0xAB");
        }

        for i in 0..7 {
            assert_eq!(eeprom.read_bit(), 0, "leading zero {i} for 0x01");
        }

        assert_eq!(eeprom.read_bit(), 1, "last bit for 0x01");

        for _ in 0..48 {
            assert_eq!(eeprom.read_bit(), 1);
        }

        assert_eq!(eeprom.read_bit(), 1, "should be ready");
        assert!(!eeprom.updated, "no update");
    }

    #[test]
    fn test_write_64k() {
        let mut eeprom = Eeprom::new(vec![0xFF; EEPROM_64KBIT], true);

        for bit in [1, 0] {
            eeprom.write_bit(bit);
        }

        for bit in [1, 1, 1, 1, 0, 0, 0, 0, 0, 0, 0, 0, 1, 1] {
            eeprom.write_bit(bit);
        }

        for bit in [0u16, 1, 0, 0, 0, 0, 1, 0] {
            eeprom.write_bit(bit);
        }

        for _ in 0..56 {
            eeprom.write_bit(0);
        }

        eeprom.write_bit(0);

        assert_eq!(eeprom.memory[24], 0x42, "block 3 starts at 24");
        assert_eq!(eeprom.memory[25..32], [0; 7]);
    }
}

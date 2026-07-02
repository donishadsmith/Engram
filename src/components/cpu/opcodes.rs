use crate::components::cpu::core::{
    AddressBus, ArithmeticOperation, BitwiseOperation, ByteOps8, CPU, FlagDelta, FlagType,
    MergeByteOps, Register8Bits, Register16Bits, StatusFlag, half_carry_add, half_carry_sub,
};

impl<A> CPU<A>
where
    A: AddressBus,
{
    pub fn decode_and_execute(&mut self) {
        let opcode = self.registers.instruction_register.unwrap();
        eprintln!(
            "Opcode {:#0x}; PC={:#06x}",
            opcode,
            self.registers.program_counter.address.wrapping_sub(1)
        );

        // https://izik1.github.io/gbops/
        // https://gekkio.fi/files/gb-docs/gbctr.pdf
        // https://archive.gbdev.io/salvage/decoding_gbz80_opcodes/Decoding%20Gamboy%20Z80%20Opcodes.html
        match opcode {
            0x00 => return,
            0x01 | 0x11 | 0x21 | 0x31 => {
                let value = self.fetch_2bytes();

                match opcode {
                    0x01 => self.registers.set_16bit(Register16Bits::BC, value),
                    0x11 => self.registers.set_16bit(Register16Bits::DE, value),
                    0x21 => self.registers.set_16bit(Register16Bits::HL, value),
                    _ => self.registers.set_16bit(Register16Bits::SP, value),
                }
            }
            0x02 | 0x12 | 0x22 | 0x32 | 0x0A | 0x1A | 0x2A | 0x3A => {
                let (_, _, _, p, q) = self.decoder(opcode);

                let address = match p {
                    0 => self.registers.get_16bit(Register16Bits::BC),
                    1 => self.registers.get_16bit(Register16Bits::DE),
                    _ => self.registers.get_16bit(Register16Bits::HL),
                };

                if q == 0 {
                    self.bus.write(address, self.registers.a);
                } else {
                    self.registers.a = self.bus.read(address);
                }

                match p {
                    2 => self
                        .registers
                        .set_16bit(Register16Bits::HL, address.wrapping_add(1)),
                    3 => self
                        .registers
                        .set_16bit(Register16Bits::HL, address.wrapping_sub(1)),
                    _ => {}
                }
            }
            0x03 | 0x13 | 0x23 | 0x33 | 0x0B | 0x1B | 0x2B | 0x3B => {
                let (_, _, _, p, q) = self.decoder(opcode);

                let register = match p {
                    0 => Register16Bits::BC,
                    1 => Register16Bits::DE,
                    2 => Register16Bits::HL,
                    _ => Register16Bits::SP,
                };

                let value = if q == 0 {
                    self.registers.get_16bit(register).wrapping_add(1)
                } else {
                    self.registers.get_16bit(register).wrapping_sub(1)
                };

                self.registers.set_16bit(register, value);
            }

            0x04 | 0x14 | 0x24 | 0x34 | 0x05 | 0x15 | 0x25 | 0x35 | 0x0C | 0x1C | 0x2C | 0x3C
            | 0x0D | 0x1D | 0x2D | 0x3D => {
                let (_, destination, z, _, _) = self.decoder(opcode);
                let is_inc = z == 4;

                if destination == 6 {
                    let address = self.registers.get_16bit(Register16Bits::HL);
                    let old = self.bus.read(address);
                    let new = self.inc_dec_instruction(old, is_inc);
                    self.bus.write(address, new);
                } else {
                    let register = match destination {
                        0 => Register8Bits::B,
                        1 => Register8Bits::C,
                        2 => Register8Bits::D,
                        3 => Register8Bits::E,
                        4 => Register8Bits::H,
                        5 => Register8Bits::L,
                        7 => Register8Bits::A,
                        _ => unreachable!(),
                    };

                    let old = self.registers.get_8bit(register);
                    let new = self.inc_dec_instruction(old, is_inc);
                    self.registers.set_8bit(register, new);
                }
            }
            0x06 | 0x16 | 0x26 | 0x36 => {
                let value = self.fetch_byte();

                match opcode {
                    0x06 => self.registers.set_8bit(Register8Bits::B, value),
                    0x16 => self.registers.set_8bit(Register8Bits::D, value),
                    0x26 => self.registers.set_8bit(Register8Bits::H, value),
                    _ => self
                        .bus
                        .write(self.registers.get_16bit(Register16Bits::HL), value),
                }
            }
            0x08 => {
                let value = self.registers.get_16bit(Register16Bits::SP);
                let low_byte = value as u8;
                let high_byte = (value >> 8) as u8;
                let address = self.fetch_2bytes();

                self.bus.write(address, low_byte);
                self.bus.write(address + 1, high_byte);
            }
            0xC1 | 0xD1 | 0xE1 | 0xF1 => {
                let (low_byte, high_byte) = self.pop();
                let value = if opcode == 0xF1 {
                    high_byte.merge_bytes(low_byte.mask(0xF0))
                } else {
                    high_byte.merge_bytes(low_byte)
                };

                match opcode {
                    0xC1 => self.registers.set_16bit(Register16Bits::BC, value),
                    0xD1 => self.registers.set_16bit(Register16Bits::DE, value),
                    0xE1 => self.registers.set_16bit(Register16Bits::HL, value),
                    _ => self.registers.set_16bit(Register16Bits::AF, value),
                }
            }
            0xC5 | 0xD5 | 0xE5 | 0xF5 => {
                let address = match opcode {
                    0xC5 => self.registers.get_16bit(Register16Bits::BC),
                    0xD5 => self.registers.get_16bit(Register16Bits::DE),
                    0xE5 => self.registers.get_16bit(Register16Bits::HL),
                    _ => self.registers.get_16bit(Register16Bits::AF),
                };

                self.push(address);
            }
            0x40..=0x75 | 0x77..=0x7F => {
                let (_, destination, source, _, _) = self.decoder(opcode);

                let value = self.fetch_source_value(source);

                match destination {
                    0 => self.registers.set_8bit(Register8Bits::B, value),
                    1 => self.registers.set_8bit(Register8Bits::C, value),
                    2 => self.registers.set_8bit(Register8Bits::D, value),
                    3 => self.registers.set_8bit(Register8Bits::E, value),
                    4 => self.registers.set_8bit(Register8Bits::H, value),
                    5 => self.registers.set_8bit(Register8Bits::L, value),
                    6 => self
                        .bus
                        .write(self.registers.get_16bit(Register16Bits::HL), value),
                    7 => self.registers.set_8bit(Register8Bits::A, value),
                    _ => unreachable!(),
                }
            }
            0x76 => {}
            0x80..=0xBF => {
                let (_, destination, source, _, _) = self.decoder(opcode);

                let value = self.fetch_source_value(source);

                match destination {
                    0 => self.add_instruction(value),
                    1 => self.adc_instruction(value),
                    2 => self.sub_instruction(value),
                    3 => self.sbc_instruction(value),
                    4 => self.bitwise_and_instruction(value),
                    5 => self.bitwise_xor_instruction(value),
                    6 => self.bitwise_or_instruction(value),
                    7 => self.cp_instruction(value),
                    _ => unreachable!(),
                }
            }
            0x0E | 0x1E | 0x2E | 0x3E => {
                let value = self.fetch_byte();

                match opcode {
                    0x0E => self.registers.set_8bit(Register8Bits::C, value),
                    0x1E => self.registers.set_8bit(Register8Bits::E, value),
                    0x2E => self.registers.set_8bit(Register8Bits::L, value),
                    _ => self.registers.set_8bit(Register8Bits::A, value),
                }
            }
            0xC6 | 0xD6 | 0xE6 | 0xF6 | 0xCE | 0xDE | 0xEE | 0xFE => {
                let value = self.fetch_byte();

                match opcode {
                    0xC6 => self.add_instruction(value),
                    0xCE => self.adc_instruction(value),
                    0xD6 => self.sub_instruction(value),
                    0xDE => self.sbc_instruction(value),
                    0xE6 => self.bitwise_and_instruction(value),
                    0xEE => self.bitwise_xor_instruction(value),
                    0xF6 => self.bitwise_or_instruction(value),
                    _ => self.cp_instruction(value),
                }
            }
            0xCB => {
                let (x, y, z, _, _) = self.decoder(opcode);
            }
            _ => unimplemented!("Opcode {:#04x}", opcode),
        }
    }

    fn decoder(&self, opcode: u8) -> (u8, u8, u8, u8, u8) {
        let x: u8 = (opcode >> 6) & 0x3; // category
        let y: u8 = (opcode >> 3) & 0x7; // destination register
        let z: u8 = opcode & 7; // source register
        let p: u8 = y >> 1; // 16 bit register pair
        let q: u8 = y & 0x1; // boolean toggle

        (x, y, z, p, q)
    }

    fn fetch_byte(&mut self) -> u8 {
        let byte = self.bus.read(self.registers.program_counter.address);
        self.registers.program_counter.increment(1);

        byte
    }

    fn fetch_2bytes(&mut self) -> u16 {
        let low_byte = self.fetch_byte();
        let high_byte = self.fetch_byte();
        high_byte.merge_bytes(low_byte)
    }

    fn fetch_source_value(&self, source: u8) -> u8 {
        match source {
            0 => self.registers.b,
            1 => self.registers.c,
            2 => self.registers.d,
            3 => self.registers.e,
            4 => self.registers.h,
            5 => self.registers.l,
            6 => self.bus.read(self.registers.get_16bit(Register16Bits::HL)),
            7 => self.registers.a,
            _ => unreachable!(),
        }
    }

    fn add_instruction(&mut self, value: u8) {
        let (result, delta) = ArithmeticOperation::Add
            .operation(self.registers.a, Some(value), false)
            .unwrap();
        self.apply_alu_results(result, delta);
    }

    fn adc_instruction(&mut self, value: u8) {
        let carry = self.registers.flag(StatusFlag::C);
        let (result, delta) = ArithmeticOperation::Adc
            .operation(self.registers.a, Some(value), carry)
            .unwrap();
        self.apply_alu_results(result, delta);
    }

    fn sub_instruction(&mut self, value: u8) {
        let (result, delta) = ArithmeticOperation::Sub
            .operation(self.registers.a, Some(value), false)
            .unwrap();
        self.apply_alu_results(result, delta);
    }

    fn sbc_instruction(&mut self, value: u8) {
        let carry = self.registers.flag(StatusFlag::C);
        let (result, delta) = ArithmeticOperation::Sbc
            .operation(self.registers.a, Some(value), carry)
            .unwrap();
        self.apply_alu_results(result, delta);
    }

    fn cp_instruction(&mut self, value: u8) {
        let (_, delta) = ArithmeticOperation::Sub
            .operation(self.registers.a, Some(value), false)
            .unwrap();
        self.registers.apply_flags(delta);
    }

    fn bitwise_and_instruction(&mut self, value: u8) {
        let (result, delta) = ArithmeticOperation::Bit(BitwiseOperation::And)
            .operation(self.registers.a, Some(value), false)
            .unwrap();
        self.apply_alu_results(result, delta);
    }

    fn bitwise_or_instruction(&mut self, value: u8) {
        let (result, delta) = ArithmeticOperation::Bit(BitwiseOperation::Or)
            .operation(self.registers.a, Some(value), false)
            .unwrap();
        self.apply_alu_results(result, delta);
    }

    fn bitwise_xor_instruction(&mut self, value: u8) {
        let (result, delta) = ArithmeticOperation::Bit(BitwiseOperation::Xor)
            .operation(self.registers.a, Some(value), false)
            .unwrap();
        self.apply_alu_results(result, delta);
    }

    fn apply_alu_results(&mut self, result: u8, delta: FlagDelta) {
        self.registers.set_8bit(Register8Bits::A, result);
        self.registers.apply_flags(delta);
    }

    fn inc_dec_instruction(&mut self, old: u8, is_inc: bool) -> u8 {
        let (new, half_carry) = if is_inc {
            (old.wrapping_add(1), half_carry_add(old, 1, false))
        } else {
            (old.wrapping_sub(1), half_carry_sub(old, 1, false))
        };

        let delta = FlagDelta {
            z: if new == 0 {
                FlagType::Set
            } else {
                FlagType::Unset
            },
            n: if is_inc {
                FlagType::Unset
            } else {
                FlagType::Set
            },
            h: if half_carry {
                FlagType::Set
            } else {
                FlagType::Unset
            },
            c: FlagType::Unmodified,
        };
        self.registers.apply_flags(delta);

        new
    }
}

// For JSON tests
pub struct TestBus {
    ram: Vec<u8>,
}

impl TestBus {
    pub fn new() -> Self {
        Self {
            ram: vec![0u8; 0x10000],
        }
    }
}

impl AddressBus for TestBus {
    fn read(&self, address: u16) -> u8 {
        self.ram[address as usize]
    }
    fn write(&mut self, address: u16, value: u8) {
        self.ram[address as usize] = value;
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::components::cpu::core::Registers;

    use serde::Deserialize;

    #[derive(Deserialize)]
    struct CpuState {
        a: u8,
        b: u8,
        c: u8,
        d: u8,
        e: u8,
        f: u8,
        h: u8,
        l: u8,
        pc: u16,
        sp: u16,
        ram: Vec<(u16, u8)>,
    }

    #[derive(Deserialize)]
    struct TestCase {
        name: String,
        initial: CpuState,
        #[serde(rename = "final")]
        final_state: CpuState,
    }

    #[test]
    fn add_a_b_runs() {
        let registers = Registers::from_state(255, 0, 2, 0, 0, 0, 0, 0, 0xC001, 0xFFFE);
        let mut bus = TestBus::new();
        bus.write(0xC000, 0x80);
        let mut cpu = CPU::from_state(registers, bus);

        cpu.cycle();

        assert_eq!(cpu.registers.a, 1);
        assert_eq!(cpu.registers.f, 0x30)
    }

    #[test]
    fn json_instructions() {
        let mut implemented_instructions = vec![
            "02", "12", "22", "32", "0a", "1a", "2a", "3a", "c1", "d1", "e1", "f1", "c6", "d6",
            "e6", "f6", "ce", "de", "ee", "fe", "01", "11", "21", "31", "06", "16", "26", "36",
            "0e", "1e", "2e", "3e", "08", "c5", "d5", "e5", "f5", "03", "13", "23", "33", "0b",
            "1b", "2b", "3b", "14", "24", "05", "15", "25", "35", "0c", "1c", "2c", "3c", "0d",
            "1d", "2d", "3d",
        ]
        .into_iter()
        .map(String::from)
        .collect::<Vec<_>>();
    
        for hex in 0x80..=0xBFu8 {
            implemented_instructions.push(format!("{:0x}", hex));
        }

        for hex in 0x40..=0x75 {
            implemented_instructions.push(format!("{:0x}", hex));
        }

        for hex in 0x77..=0x7F {
            implemented_instructions.push(format!("{:0x}", hex));
        }

        for opcode in implemented_instructions.iter() {
            let text = std::fs::read_to_string(format!("tests/sm83/v2/{}.json", opcode)).unwrap();
            let cases: Vec<TestCase> = serde_json::from_str(&text).unwrap();

            for case in &cases {
                let mut bus = TestBus::new();
                for (address, value) in &case.initial.ram {
                    bus.write(*address, *value);
                }

                let registers = Registers::from_state(
                    case.initial.a,
                    case.initial.f,
                    case.initial.b,
                    case.initial.c,
                    case.initial.d,
                    case.initial.e,
                    case.initial.h,
                    case.initial.l,
                    case.initial.pc,
                    case.initial.sp,
                );
                let mut cpu = CPU::from_state(registers, bus);

                cpu.cycle();

                let final_state = &case.final_state;

                assert_eq!(cpu.registers.a, final_state.a, "{}: A", case.name);
                assert_eq!(cpu.registers.b, final_state.b, "{}: B", case.name);
                assert_eq!(cpu.registers.c, final_state.c, "{}: C", case.name);
                assert_eq!(cpu.registers.d, final_state.d, "{}: D", case.name);
                assert_eq!(cpu.registers.e, final_state.e, "{}: E", case.name);
                assert_eq!(cpu.registers.h, final_state.h, "{}: H", case.name);
                assert_eq!(cpu.registers.l, final_state.l, "{}: L", case.name);
                assert_eq!(cpu.registers.f, final_state.f, "{}: F", case.name);
                assert_eq!(
                    cpu.registers.program_counter.address, final_state.pc,
                    "{}: PC",
                    case.name
                );
                assert_eq!(
                    cpu.registers.stack_pointer, final_state.sp,
                    "{}: SP",
                    case.name
                );

                for (address, value) in &final_state.ram {
                    assert_eq!(
                        cpu.bus.read(*address),
                        *value,
                        "{}: mem[{:#06x}]",
                        case.name,
                        address
                    );
                }
            }
        }
    }
}

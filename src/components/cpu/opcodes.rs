use crate::components::{
    bus::AddressBus,
    cpu::core::{
        ArithmeticOperation, BitwiseOperation, ByteOps8, CPU, FlagDelta, FlagType, MergeByteOps,
        Register8Bits, Register16Bits, StatusFlag, half_carry_add, half_carry_sub,
    },
    cpu::cycles::{PREFIX_CYCLES, UNPREFIX_CYCLES},
};

#[derive(PartialEq)]
enum RPTable {
    RP,
    RP2,
}

#[derive(PartialEq)]
enum BitDirection {
    Right,
    Left,
}

// https://archive.gbdev.io/salvage/decoding_gbz80_opcodes/Decoding%20Gamboy%20Z80%20Opcodes.html
fn opcode_decoder(opcode: u8) -> (u8, u8, u8, u8, u8) {
    let x = (opcode >> 6).mask(0x03); // category; the opcode's 1st octal digit (i.e. bits 7-6)
    let y = (opcode >> 3).mask(0x07); // destination register; the opcode's 2nd octal digit (i.e. bits 5-3)
    let z = opcode.mask(0x07); // source register; the opcode's 3rd octal digit (i.e. bits 2-0)
    let p = y >> 1; // 16 bit register pair; y rightshifted one position (i.e. bits 5-4)
    let q = y.mask(0x01); // boolean toggle; y modulo 2 (i.e. bit 3)

    (x, y, z, p, q)
}

impl<A> CPU<A>
where
    A: AddressBus,
{
    pub fn decode_and_execute(&mut self) -> u8 {
        let opcode = self.registers.instruction_register.unwrap();
        let base = UNPREFIX_CYCLES[opcode as usize];

        //println!("{:02x}", opcode);

        // https://izik1.github.io/gbops/
        // https://gekkio.fi/files/gb-docs/gbctr.pdf
        match opcode {
            0x00 => return base,
            0x01 | 0x11 | 0x21 | 0x31 => {
                let value = self.fetch_2bytes();

                match opcode {
                    0x01 => self.registers.set_16bit(Register16Bits::BC, value),
                    0x11 => self.registers.set_16bit(Register16Bits::DE, value),
                    0x21 => self.registers.set_16bit(Register16Bits::HL, value),
                    _ => self.registers.set_16bit(Register16Bits::SP, value),
                }
            }
            0x02 | 0x0A | 0x12 | 0x1A | 0x22 | 0x2A | 0x32 | 0x3A => {
                let (_, _, _, p, q) = opcode_decoder(opcode);

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
            0x03 | 0x0B | 0x13 | 0x1B | 0x23 | 0x2B | 0x33 | 0x3B => {
                let (_, _, _, p, q) = opcode_decoder(opcode);

                let register = self.select_16bit_register(p, RPTable::RP);

                let value = if q == 0 {
                    self.registers.get_16bit(register).wrapping_add(1)
                } else {
                    self.registers.get_16bit(register).wrapping_sub(1)
                };

                self.registers.set_16bit(register, value);
            }
            0x04 | 0x05 | 0x0C | 0x0D | 0x14 | 0x15 | 0x1C | 0x1D | 0x24 | 0x25 | 0x2C | 0x2D
            | 0x34 | 0x35 | 0x3C | 0x3D => {
                let (_, destination, z, _, _) = opcode_decoder(opcode);
                let is_inc = z == 4;

                if destination == 6 {
                    let old = self.read_from_hl_address();
                    let new = self.inc_dec_instruction(old, is_inc);
                    self.write_to_hl_address(new);
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
            0x07 | 0x0F | 0x17 | 0x1F => {
                let (_, y, _, _, _) = opcode_decoder(opcode);

                match y {
                    0 => self.circular_rotate(7, BitDirection::Left), // Rotate left circular accumulator
                    1 => self.circular_rotate(7, BitDirection::Right), // Rotate right circular accumulator
                    2 => self.rotate(7, BitDirection::Left),           // Rotate left accumulator
                    _ => self.rotate(7, BitDirection::Right),          // Rotate right accumulator
                }

                self.registers.apply_flags(FlagDelta {
                    z: FlagType::Unset,
                    n: FlagType::Unmodified,
                    h: FlagType::Unmodified,
                    c: FlagType::Unmodified,
                });
            }
            0x08 => {
                let value = self.registers.get_16bit(Register16Bits::SP);
                let low_byte = value as u8;
                let high_byte = (value >> 8) as u8;
                let address = self.fetch_2bytes();

                self.bus.write(address, low_byte);
                self.bus.write(address.wrapping_add(1), high_byte);
            }
            0x09 | 0x19 | 0x29 | 0x39 => {
                let (_, _, _, p, _) = opcode_decoder(opcode);
                let register = self.select_16bit_register(p, RPTable::RP);

                self.add_16bit(register, Register16Bits::HL);
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
            0x10 => {
                // STOP-for now just consume byte
                self.fetch_byte();
            }
            0x18 | 0x20 | 0x28 | 0x30 | 0x38 => {
                let displacement = self.fetch_byte().i16() as u16;
                let address = self
                    .registers
                    .program_counter
                    .address
                    .wrapping_add(displacement);

                if opcode == 0x18 {
                    self.registers.program_counter.jump(address);

                    return base;
                }

                let (_, y, _, _, _) = opcode_decoder(opcode);

                if self.conditional_move(y - 4) {
                    self.registers.program_counter.jump(address);
                } else {
                    return base - 1;
                }
            }
            0x27 => {
                self.daa_instruction();
            }
            0x2F => {
                // CPL - complement accumulator
                self.registers.a = !self.registers.a;
                self.registers.apply_flags(FlagDelta {
                    z: FlagType::Unmodified,
                    n: FlagType::Set,
                    h: FlagType::Set,
                    c: FlagType::Unmodified,
                });
            }
            0x37 => {
                // SCF - set carry flag
                self.registers.apply_flags(FlagDelta {
                    z: FlagType::Unmodified,
                    n: FlagType::Unset,
                    h: FlagType::Unset,
                    c: FlagType::Set,
                });
            }
            0x3F => {
                // CCF - complement carry flag
                let carry = self.registers.flag(StatusFlag::C);
                self.registers.apply_flags(FlagDelta {
                    z: FlagType::Unmodified,
                    n: FlagType::Unset,
                    h: FlagType::Unset,
                    c: if !carry {
                        FlagType::Set
                    } else {
                        FlagType::Unset
                    },
                });
            }
            0x40..=0x75 | 0x77..=0x7F => {
                let (_, destination, source, _, _) = opcode_decoder(opcode);

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
            0x76 => {
                if !self.interrupt.master_enable && self.bus.pending_interrupt() != 0 {
                    self.halt_bug = true
                } else {
                    self.halted = true
                }
            }
            0x80..=0xBF => {
                let (_, destination, source, _, _) = opcode_decoder(opcode);

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
            0xC0 | 0xC8 | 0xC9 | 0xD0 | 0xD8 => {
                if opcode == 0xC9 {
                    let (low_byte, high_byte) = self.pop();
                    let address = high_byte.merge_bytes(low_byte);
                    self.registers.program_counter.jump(address);

                    return base;
                }

                let (_, y, _, _, _) = opcode_decoder(opcode);

                if self.conditional_move(y) {
                    let (low_byte, high_byte) = self.pop();
                    let address = high_byte.merge_bytes(low_byte);
                    self.registers.program_counter.jump(address);
                } else {
                    return base - 3;
                }
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
            0xC2 | 0xC3 | 0xCA | 0xD2 | 0xDA => {
                let address = self.fetch_2bytes();

                if opcode == 0xC3 {
                    self.registers.program_counter.jump(address);

                    return base;
                }

                let (_, y, _, _, _) = opcode_decoder(opcode);

                if self.conditional_move(y) {
                    self.registers.program_counter.jump(address);
                } else {
                    return base - 1;
                }
            }
            0xC4 | 0xCC | 0xCD | 0xD4 | 0xDC => {
                let address = self.fetch_2bytes();

                if opcode == 0xCD {
                    self.call(address);

                    return base;
                }

                let (_, y, _, _, _) = opcode_decoder(opcode);

                if self.conditional_move(y) {
                    self.call(address);
                } else {
                    return base - 3;
                }
            }
            0xC5 | 0xD5 | 0xE5 | 0xF5 => {
                let (_, _, _, p, _) = opcode_decoder(opcode);
                let register = self.select_16bit_register(p, RPTable::RP2);
                let address = self.registers.get_16bit(register);

                self.push(address);
            }
            0xC6 | 0xCE | 0xD6 | 0xDE | 0xE6 | 0xEE | 0xF6 | 0xFE => {
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
            0xC7 | 0xCF | 0xD7 | 0xDF | 0xE7 | 0xEF | 0xF7 | 0xFF => {
                let (_, y, _, _, _) = opcode_decoder(opcode);
                let address = (y as u16) * 8;
                self.call(address);
            }
            0xCB => {
                let opcode = self.fetch_byte();
                let (x, y, z, _, _) = opcode_decoder(opcode);

                match x {
                    0 => match y {
                        0 => self.circular_rotate(z, BitDirection::Left),
                        1 => self.circular_rotate(z, BitDirection::Right),
                        2 => self.rotate(z, BitDirection::Left),
                        3 => self.rotate(z, BitDirection::Right),
                        4 => self.shift_arithmetic(z, BitDirection::Left),
                        5 => self.shift_arithmetic(z, BitDirection::Right),
                        6 => self.swap(z),
                        _ => self.srl(z),
                    },
                    1 => self.test_bit(y, z),
                    2 => self.reset_bit(y, z),
                    _ => self.set_bit(y, z),
                }

                return PREFIX_CYCLES[opcode as usize];
            }
            0xD9 => {
                // RETI - return and enable interrupts
                self.ret();
                self.interrupt.master_enable = true;
            }
            0xE0 | 0xE2 | 0xEA | 0xF0 | 0xF2 | 0xFA => {
                let (_, y, _, _, _) = opcode_decoder(opcode);

                if opcode == 0xE0 || opcode == 0xF0 {
                    let address = 0xFF00u16.wrapping_add(self.fetch_byte() as u16);

                    match y {
                        4 => {
                            let value = self.registers.get_8bit(Register8Bits::A);
                            self.bus.write(address, value);
                        }
                        _ => {
                            let value = self.bus.read(address);
                            self.registers.set_8bit(Register8Bits::A, value);
                        }
                    }

                    return base;
                }

                match y {
                    4 => {
                        let a = self.registers.get_8bit(Register8Bits::A);
                        let c = self.registers.get_8bit(Register8Bits::C) as u16;
                        self.bus.write(0xFF00u16.wrapping_add(c), a);
                    }
                    5 => {
                        let value = self.registers.get_8bit(Register8Bits::A);
                        let address = self.fetch_2bytes();
                        self.bus.write(address, value);
                    }
                    6 => {
                        let c = self.registers.get_8bit(Register8Bits::C) as u16;
                        let address = 0xFF00u16.wrapping_add(c);
                        let value = self.bus.read(address);
                        self.registers.set_8bit(Register8Bits::A, value);
                    }
                    _ => {
                        let address = self.fetch_2bytes();
                        let value = self.bus.read(address);
                        self.registers.set_8bit(Register8Bits::A, value);
                    }
                };
            }
            0xE8 => {
                let sp = self.registers.get_16bit(Register16Bits::SP);
                let byte = self.fetch_byte();
                let value = sp.wrapping_add(byte.i16() as u16);

                self.registers.set_16bit(Register16Bits::SP, value);
                self.registers.apply_flags(FlagDelta {
                    z: FlagType::Unset,
                    n: FlagType::Unset,
                    h: if (sp & 0x0F) + (byte as u16 & 0x0F) > 0x0F {
                        FlagType::Set
                    } else {
                        FlagType::Unset
                    },
                    c: if (sp & 0xFF) + (byte as u16) > 0xFF {
                        FlagType::Set
                    } else {
                        FlagType::Unset
                    },
                });
            }
            0xE9 => {
                let address = self.registers.get_16bit(Register16Bits::HL);
                self.registers.program_counter.jump(address);
            }
            0xF3 => {
                self.interrupt.master_enable = false;
                self.interrupt.pending_enable = false;
            }
            0xF8 => {
                let byte = self.fetch_byte().i16() as u16;
                let sp = self.registers.get_16bit(Register16Bits::SP);
                let value = sp.wrapping_add(byte);

                self.registers.set_16bit(Register16Bits::HL, value);
                self.registers.apply_flags(FlagDelta {
                    z: FlagType::Unset,
                    n: FlagType::Unset,
                    h: if (sp & 0x000F) + (byte & 0x000F) > 0x000F {
                        FlagType::Set
                    } else {
                        FlagType::Unset
                    },
                    c: if (sp & 0x00FF) + (byte & 0x00FF) > 0x00FF {
                        FlagType::Set
                    } else {
                        FlagType::Unset
                    },
                });
            }
            0xF9 => {
                let value = self.registers.get_16bit(Register16Bits::HL);
                self.registers.set_16bit(Register16Bits::SP, value);
            }
            0xFB => self.interrupt.pending_enable = true,
            _ => unreachable!("Remaining opcodes are illegal"),
        }

        base
    }

    fn conditional_move(&self, y: u8) -> bool {
        match y {
            0 => !self.registers.flag(StatusFlag::Z),
            1 => self.registers.flag(StatusFlag::Z),
            2 => !self.registers.flag(StatusFlag::C),
            _ => self.registers.flag(StatusFlag::C),
        }
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

    fn select_16bit_register(&self, p: u8, table: RPTable) -> Register16Bits {
        match p {
            0 => Register16Bits::BC,
            1 => Register16Bits::DE,
            2 => Register16Bits::HL,
            _ => {
                if table == RPTable::RP {
                    Register16Bits::SP
                } else {
                    Register16Bits::AF
                }
            }
        }
    }

    fn add_16bit(&mut self, source: Register16Bits, destination: Register16Bits) {
        let a = self.registers.get_16bit(source);
        let b = self.registers.get_16bit(destination);
        // 0000 | 0000 | 0000 | 0000;
        //        ^ bit 11
        let half_carry = (a & 0x0FFF) + (b & 0x0FFF) > 0x0FFF;
        let (value, overflow) = a.overflowing_add(b);

        self.registers.set_16bit(destination, value);
        self.registers.apply_flags(FlagDelta {
            z: FlagType::Unmodified,
            n: FlagType::Unset,
            h: if half_carry {
                FlagType::Set
            } else {
                FlagType::Unset
            },
            c: if overflow {
                FlagType::Set
            } else {
                FlagType::Unset
            },
        });
    }

    fn inc_dec_instruction(&mut self, old: u8, is_inc: bool) -> u8 {
        let (new, half_carry) = if is_inc {
            (old.wrapping_add(1), half_carry_add(old, 1, false))
        } else {
            (old.wrapping_sub(1), half_carry_sub(old, 1, false))
        };

        self.registers.apply_flags(FlagDelta {
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
        });

        new
    }

    fn circular_rotate(&mut self, z: u8, direction: BitDirection) {
        // rotate left circular, bit 7 wraps to bit 0
        // rotate right circular, bit 0 wraps to bit 7
        let value = self.get_value_for_cb_op(z);

        let (result, carry) = if direction == BitDirection::Left {
            (value.rotate_left(1), value.mask(0x80) != 0)
        } else {
            (value.rotate_right(1), value.mask(0x01) != 0)
        };

        self.write_value_for_cb_op(z, result);
        self.apply_rotate_flags(result, carry);
    }

    fn rotate(&mut self, z: u8, direction: BitDirection) {
        // Rotate left - old carry becomes bit 0 and bit 7 is new carry
        // Rotate right - old carry becomes bit 7 and the bit 0 is new carry
        let value = self.get_value_for_cb_op(z);

        let old_carry = self.registers.flag(StatusFlag::C) as u8;

        let (result, carry) = if direction == BitDirection::Left {
            ((value << 1) | old_carry, value.mask(0x80) != 0)
        } else {
            ((value >> 1) | (old_carry << 7), value.mask(0x01) != 0)
        };

        self.write_value_for_cb_op(z, result);
        self.apply_rotate_flags(result, carry);
    }

    fn shift_arithmetic(&mut self, z: u8, direction: BitDirection) {
        // Shift left arithmetic - shift bit 7 out and let it become carry; let bit 0 be zero
        // Shift right arithmetic - bit 7 is duplicated and bit zero becomes carry
        let value = self.get_value_for_cb_op(z);

        let (result, carry) = if direction == BitDirection::Left {
            (value << 1, value.mask(0x80) != 0)
        } else {
            ((value >> 1) | value.mask(0x80), value.mask(0x01) != 0)
        };

        self.write_value_for_cb_op(z, result);
        self.apply_rotate_flags(result, carry);
    }

    fn swap(&mut self, z: u8) {
        // Swap high nibble and low nibble
        let value = self.get_value_for_cb_op(z);
        let carry = false;
        let result = value.rotate_left(4);

        self.write_value_for_cb_op(z, result);
        self.apply_rotate_flags(result, carry);
    }

    fn srl(&mut self, z: u8) {
        // Shift right logical - bit 0 becomes carry, bit 7 becomes zero
        let value = self.get_value_for_cb_op(z);
        let carry = value.mask(0x01) != 0;
        let result = value >> 1;

        self.write_value_for_cb_op(z, result);
        self.apply_rotate_flags(result, carry);
    }

    fn apply_rotate_flags(&mut self, result: u8, carry: bool) {
        self.registers.apply_flags(FlagDelta {
            z: if result == 0 {
                FlagType::Set
            } else {
                FlagType::Unset
            },
            n: FlagType::Unset,
            h: FlagType::Unset,
            c: if carry {
                FlagType::Set
            } else {
                FlagType::Unset
            },
        });
    }

    fn test_bit(&mut self, y: u8, z: u8) {
        let value = self.get_value_for_cb_op(z);
        let is_zero = value & (1 << y) == 0;

        self.registers.apply_flags(FlagDelta {
            z: if is_zero {
                FlagType::Set
            } else {
                FlagType::Unset
            },
            n: FlagType::Unset,
            h: FlagType::Set,
            c: FlagType::Unmodified,
        });
    }

    fn reset_bit(&mut self, y: u8, z: u8) {
        let value = self.get_value_for_cb_op(z);
        let result = value & !(1 << y);

        self.write_value_for_cb_op(z, result);
    }

    fn set_bit(&mut self, y: u8, z: u8) {
        let value = self.get_value_for_cb_op(z);
        let result = value | (1 << y);

        self.write_value_for_cb_op(z, result);
    }

    fn select_8bit_register(&self, z: u8) -> Option<Register8Bits> {
        match z {
            0 => Some(Register8Bits::B),
            1 => Some(Register8Bits::C),
            2 => Some(Register8Bits::D),
            3 => Some(Register8Bits::E),
            4 => Some(Register8Bits::H),
            5 => Some(Register8Bits::L),
            6 => None,
            7 => Some(Register8Bits::A),
            _ => unreachable!(),
        }
    }

    fn read_from_hl_address(&self) -> u8 {
        let address = self.registers.get_16bit(Register16Bits::HL);
        self.bus.read(address)
    }

    fn write_to_hl_address(&mut self, value: u8) {
        let address = self.registers.get_16bit(Register16Bits::HL);
        self.bus.write(address, value);
    }

    fn get_value_for_cb_op(&mut self, z: u8) -> u8 {
        match self.select_8bit_register(z) {
            Some(register) => self.registers.get_8bit(register),
            None => self.read_from_hl_address(),
        }
    }

    fn write_value_for_cb_op(&mut self, z: u8, result: u8) {
        match self.select_8bit_register(z) {
            Some(register) => self.registers.set_8bit(register, result),
            None => self.write_to_hl_address(result),
        }
    }

    fn daa_instruction(&mut self) {
        // applying flags to the accumulator value, subtraction
        // determined by negative flag

        /*
            DAA-decimal adjust accumulator
            https://forums.nesdev.org/viewtopic.php?t=15944
            https://blog.ollien.com/posts/gb-daa/
            BCD-binary coded decimal
            Byte=8 bits, nibble is 4 bits, high nibble + low nibble.
            In base 2, each nibble has 2^4 valid combinations, meaning
            (2^4)*(2^4) = 256 total.
            BCD only goes from 0-99, each nibble goes from 0-9,
            since 10+ is not a valid decimal digit, hence 10*10=100 combinations.
            If a nibble overflows past 9 (nibble > 9 or half-carry set), add an
            offset of 6 to the ones place, derived from 16 - 10 (base gap, skips
            the invalid hex codes A-F). Same for the 10s place, if high nibble > 9
            or carry set, add 0x60, then wrap and carry
        */

        let mut update_carry_flag = false;
        let mut a = self.registers.get_8bit(Register8Bits::A);
        let half_carry = self.registers.flag(StatusFlag::H);
        let carry = self.registers.flag(StatusFlag::C);

        if !self.registers.flag(StatusFlag::N) {
            if a > 0x99 || carry {
                update_carry_flag = true;
                a = a.wrapping_add(0x60);
            }

            if a.mask(0x0F) > 0x09 || half_carry {
                a = a.wrapping_add(0x06);
            }
        } else {
            if carry {
                a = a.wrapping_sub(0x60);
            }

            if half_carry {
                a = a.wrapping_sub(0x06);
            }
        }

        self.registers.set_8bit(Register8Bits::A, a);
        self.registers.apply_flags(FlagDelta {
            z: if a == 0 {
                FlagType::Set
            } else {
                FlagType::Unset
            },
            n: FlagType::Unmodified,
            h: FlagType::Unset,
            c: if update_carry_flag {
                FlagType::Set
            } else {
                FlagType::Unmodified
            },
        });
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

    fn pending_interrupt(&self) -> u8 {
        0
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
        cycles: Vec<serde_json::Value>,
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

    const SKIPPED_OPCODES: &[u8] = &[
        0x76, 0xFB, 0x10, 0xF3, // No json files
        0xD3, 0xDB, 0xDD, 0xE3, 0xE4, 0xEB, 0xEC, 0xED, 0xF4, 0xFC, 0xFD, // illegal
    ];

    #[test]
    fn json_instructions() {
        let implemented_instructions: Vec<String> = (0x00..=0xFFu8)
            .filter(|opcode| !SKIPPED_OPCODES.contains(opcode))
            .map(|opcode| format!("{:02x}", opcode))
            .collect();

        for opcode in implemented_instructions.iter() {
            let path = format!("tests/sm83/v2/{}.json", opcode);
            let text = std::fs::read_to_string(&path)
                .unwrap_or_else(|e| panic!("Failed to read {path}: {e}"));
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

                let m_cycles = cpu.cycle();

                let final_state = &case.final_state;

                assert_eq!(
                    m_cycles as usize,
                    case.cycles.len(),
                    "{}: cycles",
                    case.name
                );
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

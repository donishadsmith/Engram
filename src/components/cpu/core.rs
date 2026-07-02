use crate::components::cartridge::{CGBFlag, Cartridge};

const STARTING_ADDRESS: u16 = 0x0000;
const AFTER_BOOT_STARTING_ADDRESS: u16 = 0x0100;

pub trait ByteOps8 {
    fn set_bit(&self, mask: u8, flag: bool) -> u8;

    fn mask(&self, hex: u8) -> u8;
}

impl ByteOps8 for u8 {
    fn set_bit(&self, mask: u8, flag: bool) -> u8 {
        if flag { self | mask } else { self & !mask }
    }

    fn mask(&self, hex: u8) -> u8 {
        self & hex
    }
}

// Nice trait high and low byte for u16 from https://aquova.net/emudev/, will be using that pattern
// for my implementation
pub trait ByteOps16 {
    fn high_byte(&self) -> u8;
    fn low_byte(&self) -> u8;
}

impl ByteOps16 for u16 {
    fn high_byte(&self) -> u8 {
        (self >> 8) as u8
    }

    fn low_byte(&self) -> u8 {
        (self & 0xFF) as u8
    }
}

pub trait MergeByteOps {
    fn merge_bytes<L: Into<u16>>(self, low_bytes: L) -> u16;
}

impl<T: Into<u16>> MergeByteOps for T {
    fn merge_bytes<L: Into<u16>>(self, low_bytes: L) -> u16 {
        (self.into() << 8) | low_bytes.into()
    }
}

#[derive(Copy, Clone, Debug)]
pub enum StatusFlag {
    Z = 0x80, // 7 bit is 1; Zero flag - condition in which the operation resulted in 0
    N = 0x40, // 6 bit is 1; Subtraction flag (BCD) - Most significant bit is 1, which is a negative value
    H = 0x20, // 5 bit is 1; Half Carry flag (BCD) - a carry or borrow occured between the high and low nibble
    C = 0x10, // 4 bit is 1; Carry flag - overflow or underflow occured
}

impl StatusFlag {
    pub fn u8(self) -> u8 {
        self as u8
    }

    pub fn boot(checksum: u8) -> u8 {
        if checksum == 0x00 {
            StatusFlag::Z.u8()
        } else {
            StatusFlag::Z.u8() | StatusFlag::H.u8() | StatusFlag::C.u8()
        }
    }
}

/*
    https://gbdev.io/gb-asm-tutorial/part1/operations.html

    After every operation, each flag has three states (4 options).
    The CPU status flags, Z, N, H, C are essentially masks, in which the bitwise and operation
    will zero out the bits of the value in the f register for that is not 1 in the specific
    bit position (i.e., 7 but position for Z). These are used to track the side effects of certain operations

    Flag Operations includes:
        - Set: Where the bit is changed to 1 by using a bitwise or operation, to ensure that
        the other bits do not get zeroed out but the bit of interest becomes 1 if it was previosly zero
        - Unset: In which the flag is turned off
        - Dependent: Flag is set or unset based on a specific condition
        - Unmodified: The flag is left as is

    Example of flags purpose:

    0x01FF + 0x0001 = 0x200(512)

    Each register can only hold 8 bit values, (2^8-1) which is 0-255, to represent larger numbers with this restriction
    Low byte add is 0xFF (255) + 0x01 = 0x100, due to the 8 bit restriction, register A gets 0x00 and the C flag carries the overflow.

    The high bytes can be summed 0x01 + 0x00 = 0x01, so the final 16 bit representation would be 0x0100 (256), which is incorrect; however
    adding the overflow 0x01 + 0x00 + 0x01 = 0x02, resulting in 0x0200, which is the correct 512. This is the ADC instruction.
*/
#[derive(PartialEq, Debug)]
pub enum FlagType {
    Set,
    Unset,
    Unmodified,
}

pub struct FlagDelta {
    pub z: FlagType,
    pub n: FlagType,
    pub h: FlagType,
    pub c: FlagType,
}

impl FlagDelta {
    pub fn apply(self, mut f: u8) -> u8 {
        if self.z != FlagType::Unmodified {
            f = f.set_bit(StatusFlag::Z.u8(), self.z == FlagType::Set);
        }

        if self.n != FlagType::Unmodified {
            f = f.set_bit(StatusFlag::N.u8(), self.n == FlagType::Set);
        }

        if self.h != FlagType::Unmodified {
            f = f.set_bit(StatusFlag::H.u8(), self.h == FlagType::Set);
        }

        if self.c != FlagType::Unmodified {
            f = f.set_bit(StatusFlag::C.u8(), self.c == FlagType::Set);
        }

        f.mask(0xF0)
    }
}

// Op table: https://izik1.github.io/gbops/
pub enum BitwiseOperation {
    And,
    Or,
    Xor,
    Not,
}

impl BitwiseOperation {
    pub fn operation(&self, a: u8, b: Option<u8>) -> Option<(u8, FlagDelta)> {
        Some(match self {
            BitwiseOperation::And => {
                let value = a & b?;
                (
                    value,
                    FlagDelta {
                        z: if value == 0 {
                            FlagType::Set
                        } else {
                            FlagType::Unset
                        },
                        n: FlagType::Unset,
                        h: FlagType::Set,
                        c: FlagType::Unset,
                    },
                )
            }
            BitwiseOperation::Or => {
                let value = a | b?;
                (
                    value,
                    FlagDelta {
                        z: if value == 0 {
                            FlagType::Set
                        } else {
                            FlagType::Unset
                        },
                        n: FlagType::Unset,
                        h: FlagType::Unset,
                        c: FlagType::Unset,
                    },
                )
            }
            BitwiseOperation::Xor => {
                let value = a ^ b?;
                (
                    value,
                    FlagDelta {
                        z: if value == 0 {
                            FlagType::Set
                        } else {
                            FlagType::Unset
                        },
                        n: FlagType::Unset,
                        h: FlagType::Unset,
                        c: FlagType::Unset,
                    },
                )
            }
            BitwiseOperation::Not => {
                let value = !a;
                (
                    value,
                    FlagDelta {
                        z: FlagType::Unmodified,
                        n: FlagType::Set,
                        h: FlagType::Set,
                        c: FlagType::Unmodified,
                    },
                )
            }
        })
    }
}
pub enum ArithmeticOperation {
    Add,
    Adc,
    Sub,
    Sbc,
    Bit(BitwiseOperation),
}

fn half_carry_add(a: u8, b: u8, carry: bool) -> bool {
    a.mask(0x0F) + b.mask(0x0F) + carry as u8 > 0x0F
}

fn half_carry_sub(a: u8, b: u8, carry: bool) -> bool {
    a.mask(0x0F) < b.mask(0x0f) + carry as u8
}

impl ArithmeticOperation {
    pub fn operation(&self, a: u8, b: Option<u8>, carry: bool) -> Option<(u8, FlagDelta)> {
        Some(match self {
            ArithmeticOperation::Add => {
                let b = b?;
                let (value, overflow) = a.overflowing_add(b);
                (
                    value,
                    FlagDelta {
                        z: if value == 0 {
                            FlagType::Set
                        } else {
                            FlagType::Unset
                        },
                        n: FlagType::Unset,
                        h: if half_carry_add(a, b, false) {
                            FlagType::Set
                        } else {
                            FlagType::Unset
                        },
                        c: if overflow {
                            FlagType::Set
                        } else {
                            FlagType::Unset
                        },
                    },
                )
            }
            ArithmeticOperation::Adc => {
                let b = b?;
                let adjusted_value_u16 = a as u16 + b as u16 + carry as u16;
                let adjusted_value_u8 = adjusted_value_u16 as u8;
                let half_carry = half_carry_add(a, b, carry);

                (
                    adjusted_value_u8 as u8,
                    FlagDelta {
                        z: if (adjusted_value_u8 as u8) == 0 {
                            FlagType::Set
                        } else {
                            FlagType::Unset
                        },
                        n: FlagType::Unset,
                        h: if half_carry {
                            FlagType::Set
                        } else {
                            FlagType::Unset
                        },
                        c: if adjusted_value_u16 > 0xFF {
                            FlagType::Set
                        } else {
                            FlagType::Unset
                        },
                    },
                )
            }
            ArithmeticOperation::Sub => {
                let b = b?;
                let (value, underflow) = a.overflowing_sub(b);

                (
                    value,
                    FlagDelta {
                        z: if value == 0 {
                            FlagType::Set
                        } else {
                            FlagType::Unset
                        },
                        n: FlagType::Set,
                        h: if half_carry_sub(a, b, false) {
                            FlagType::Set
                        } else {
                            FlagType::Unset
                        },
                        c: if underflow {
                            FlagType::Set
                        } else {
                            FlagType::Unset
                        },
                    },
                )
            }
            ArithmeticOperation::Sbc => {
                let b = b?;
                let adjusted_value = a.wrapping_sub(b).wrapping_sub(carry as u8);
                let underflow = (a as u16) < (b as u16) + (carry as u16);

                (
                    adjusted_value,
                    FlagDelta {
                        z: if adjusted_value == 0 {
                            FlagType::Set
                        } else {
                            FlagType::Unset
                        },
                        n: FlagType::Set,
                        h: if half_carry_sub(a, b, carry) {
                            FlagType::Set
                        } else {
                            FlagType::Unset
                        },
                        c: if underflow {
                            FlagType::Set
                        } else {
                            FlagType::Unset
                        },
                    },
                )
            }
            ArithmeticOperation::Bit(bit_op) => bit_op.operation(a, b)?,
        })
    }
}

pub struct ProgramCounter {
    pub address: u16,
}

impl ProgramCounter {
    pub fn start(next_starting_address: u16) -> Self {
        Self {
            address: next_starting_address,
        }
    }

    // Instructions can be 1 to 3 bytes, presented in 8 bits
    fn increment(&mut self, step: u16) {
        self.address = self.address.wrapping_add(step);
    }

    fn jump(&mut self, address: u16) {
        self.address = address
    }
}

pub enum Register8Bits {
    A,
    F,
    B,
    C,
    D,
    E,
    H,
    L,
}

pub enum Register16Bits {
    AF,
    BC,
    DE,
    HL,
    SP,
}

// https://gbdev.io/pandocs/Power_Up_Sequence.html
// DMG & CGB
pub struct Registers {
    pub a: u8, // output always goes to A register
    pub f: u8,
    pub b: u8,
    pub c: u8,
    pub d: u8,
    pub e: u8,
    pub h: u8,
    pub l: u8,
    pub instruction_register: Option<u8>,
    pub program_counter: ProgramCounter,
    pub stack_pointer: u16,
}

impl Registers {
    pub fn new(cartridge: &Cartridge) -> Self {
        match cartridge.header.cgb_flag {
            CGBFlag::DMG => Self {
                a: 0x01,
                f: StatusFlag::boot(cartridge.header.checksum),
                b: 0x00,
                c: 0x13,
                d: 0x00,
                e: 0xD8,
                h: 0x01,
                l: 0x4D,
                program_counter: ProgramCounter::start(STARTING_ADDRESS),
                stack_pointer: 0xFFFE,
                instruction_register: None,
            },

            CGBFlag::CBG => Self {
                a: 0x11,
                f: StatusFlag::Z.u8(),
                b: 0x00,
                c: 0x00,
                d: 0xFF,
                e: 0x56,
                h: 0x00,
                l: 0x0D,
                program_counter: ProgramCounter::start(STARTING_ADDRESS),
                stack_pointer: 0xFFFE,
                instruction_register: None,
            },
        }
    }

    pub fn from_state(
        a: u8,
        f: u8,
        b: u8,
        c: u8,
        d: u8,
        e: u8,
        h: u8,
        l: u8,
        program_counter: u16,
        stack_pointer: u16,
    ) -> Self {
        Self {
            a,
            f,
            b,
            c,
            d,
            e,
            h,
            l,
            program_counter: ProgramCounter {
                address: program_counter,
            },
            stack_pointer: stack_pointer,
            instruction_register: None,
        }
    }

    pub fn flag(&self, flag: StatusFlag) -> bool {
        self.f & flag.u8() != 0
    }

    pub fn apply_flags(&mut self, delta: FlagDelta) {
        self.f = delta.apply(self.f);
    }

    pub fn set_8bit(&mut self, register_8bits: Register8Bits, value: u8) {
        match register_8bits {
            Register8Bits::A => self.a = value,
            Register8Bits::F => self.f = value.mask(0xF0),
            Register8Bits::B => self.b = value,
            Register8Bits::C => self.c = value,
            Register8Bits::D => self.d = value,
            Register8Bits::E => self.e = value,
            Register8Bits::H => self.h = value,
            Register8Bits::L => self.l = value,
        }
    }

    pub fn get_8bit(&mut self, register_8bits: Register8Bits) -> u8 {
        match register_8bits {
            Register8Bits::A => self.a,
            Register8Bits::F => self.f,
            Register8Bits::B => self.b,
            Register8Bits::C => self.c,
            Register8Bits::D => self.d,
            Register8Bits::E => self.e,
            Register8Bits::H => self.h,
            Register8Bits::L => self.l,
        }
    }

    pub fn set_16bit(&mut self, register_16bits: Register16Bits, value: u16) {
        match register_16bits {
            Register16Bits::AF => {
                self.a = value.high_byte();
                self.f = value.low_byte() & 0xF0;
            }
            Register16Bits::BC => {
                self.b = value.high_byte();
                self.c = value.low_byte();
            }
            Register16Bits::DE => {
                self.d = value.high_byte();
                self.e = value.low_byte();
            }
            Register16Bits::HL => {
                self.h = value.high_byte();
                self.l = value.low_byte();
            }
            Register16Bits::SP => {
                self.stack_pointer = value;
            }
        };
    }

    pub fn get_16bit(&self, register_16bits: Register16Bits) -> u16 {
        match register_16bits {
            Register16Bits::AF => self.a.merge_bytes(self.f),
            Register16Bits::BC => self.b.merge_bytes(self.c),
            Register16Bits::DE => self.d.merge_bytes(self.e),
            Register16Bits::HL => self.h.merge_bytes(self.l),
            Register16Bits::SP => self.stack_pointer,
        }
    }
}

pub trait AddressBus {
    fn read(&self, address: u16) -> u8;
    fn write(&mut self, address: u16, value: u8);
}

pub struct CPU<A>
where
    A: AddressBus,
{
    pub registers: Registers,
    pub bus: A,
}

impl<A> CPU<A>
where
    A: AddressBus,
{
    pub fn start(cartridge: Cartridge, bus: A) -> Self {
        let mut cpu = Self {
            registers: Registers::new(&cartridge),
            bus,
        };
        cpu.fetch();
        cpu
    }

    pub fn from_state(registers: Registers, bus: A) -> Self {
        let mut cpu = Self { registers, bus };
        let opcode_address = cpu.registers.program_counter.address.wrapping_sub(1);
        cpu.registers.instruction_register = Some(cpu.bus.read(opcode_address));
        cpu
    }

    /*
        Interrupts - Break in program execution by hardware when a condition is met

        Steps:
            - Push current address to the stack
            - Jump to some fixed address
            - Execute
            - Pop address from stack and jump back to it

        Types:
            Screen finished a frame (V-Blank): 0x0040
            LCD condition: 0x0048
            Timer overflowed: 0x0050
            Serial link: 0x0058
            Button pressed: 0x0060
    */
    pub fn push(&mut self, value: u16) {
        // High byte stored first, stack grows down
        self.registers.stack_pointer = self.registers.stack_pointer.wrapping_sub(1);
        self.bus
            .write(self.registers.stack_pointer, (value >> 8) as u8);
        self.registers.stack_pointer = self.registers.stack_pointer.wrapping_sub(1);
        self.bus.write(self.registers.stack_pointer, value as u8);
    }

    pub fn pop(&mut self) -> (u8, u8) {
        // Low byte poppped first
        let low_byte = self.bus.read(self.registers.stack_pointer);
        self.registers.stack_pointer = self.registers.stack_pointer.wrapping_add(1);
        let high_byte = self.bus.read(self.registers.stack_pointer);
        self.registers.stack_pointer = self.registers.stack_pointer.wrapping_add(1);

        (low_byte, high_byte)
    }

    pub fn call(&mut self, target: u16) {
        self.push(self.registers.program_counter.address);
        self.registers.program_counter.jump(target);
    }

    pub fn ret(&mut self) {
        let (low_byte, high_byte) = self.pop();
        self.registers
            .program_counter
            .jump(high_byte.merge_bytes(low_byte));
    }

    pub fn cycle(&mut self) {
        self.decode_and_execute();
        self.fetch();
    }

    pub fn fetch(&mut self) {
        self.registers.instruction_register =
            Some(self.bus.read(self.registers.program_counter.address));

        self.registers.program_counter.increment(1u16);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_arithmetic_add() {
        let a: u8 = 255;
        let b: Option<u8> = Some(2);
        let carry: bool = false;
        let (value, delta) = ArithmeticOperation::Add.operation(a, b, carry).unwrap();

        assert_eq!(value, 1);
        assert_eq!(delta.z, FlagType::Unset);
        assert_eq!(delta.n, FlagType::Unset);
        assert_eq!(delta.h, FlagType::Set);
        assert_eq!(delta.c, FlagType::Set);
    }

    #[test]
    fn test_arithmetic_sub() {
        let a: u8 = 2;
        let b: Option<u8> = Some(255);
        let carry: bool = false;
        let (value, delta) = ArithmeticOperation::Sub.operation(a, b, carry).unwrap();

        assert_eq!(value, 3); // 2 - (-1) = 3 -> 2 - 255 = -253 -> (-253 + 256 = 3)
        assert_eq!(delta.z, FlagType::Unset);
        assert_eq!(delta.n, FlagType::Set);
        assert_eq!(delta.h, FlagType::Set);
        assert_eq!(delta.c, FlagType::Set);
    }

    #[test]
    fn test_arithmetic_adc() {
        let a: u8 = 255;
        let b: Option<u8> = Some(2);
        let carry: bool = true;
        let (value, delta) = ArithmeticOperation::Adc.operation(a, b, carry).unwrap();

        assert_eq!(value, 2);
        assert_eq!(delta.z, FlagType::Unset);
        assert_eq!(delta.n, FlagType::Unset);
        assert_eq!(delta.h, FlagType::Set);
        assert_eq!(delta.c, FlagType::Set);
    }

    #[test]
    fn test_arithmetic_sbc() {
        let a: u8 = 2;
        let b: Option<u8> = Some(255);
        let carry: bool = true;
        let (value, delta) = ArithmeticOperation::Sbc.operation(a, b, carry).unwrap();

        assert_eq!(value, 2); // 2 - (-1) - 1 = 2
        assert_eq!(delta.z, FlagType::Unset);
        assert_eq!(delta.n, FlagType::Set);
        assert_eq!(delta.h, FlagType::Set);
        assert_eq!(delta.c, FlagType::Set);
    }

    #[test]
    fn test_flag_setting() -> Result<(), std::io::Error> {
        // Test flag setting
        let monochrome_cartridge = Cartridge::fake()?;
        // Default checksum is 0, so f register is set to 0x10000000
        let mut register = Registers::new(&monochrome_cartridge);

        let a: u8 = 255;
        let b: Option<u8> = Some(2);
        let carry: bool = register.flag(StatusFlag::C);
        assert_eq!(carry, false);
        let (value, delta) = ArithmeticOperation::Add.operation(a, b, carry).unwrap();

        assert_eq!(value, 1);
        assert_eq!(delta.z, FlagType::Unset);
        assert_eq!(delta.n, FlagType::Unset);
        assert_eq!(delta.h, FlagType::Set);
        assert_eq!(delta.c, FlagType::Set);

        register.apply_flags(delta);
        assert_eq!(register.f, 0b00110000);

        Ok(())
    }
}

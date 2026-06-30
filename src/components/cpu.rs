use crate::components::{
    bus::Bus,
    cartridge::{CGBFlag, Cartridge},
};

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

#[derive(Copy, Clone)]
pub enum CPUFlag {
    Z = 0x80, // 7 bit is 1; Zero flag
    N = 0x40, // 6 bit is 1; Subtraction flag (BCD)
    H = 0x20, // 5 bit is 1; Half Carry flag (BCD)
    C = 0x10, // 4 bit is 1; Carry flag
}

#[derive(PartialEq)]
pub enum FlagOp {
    Set,
    Unset,
    Unmodified,
}

pub struct FlagDelta {
    pub z: FlagOp,
    pub n: FlagOp,
    pub h: FlagOp,
    pub c: FlagOp,
}

impl FlagDelta {
    pub fn apply(self, mut f: u8) -> u8 {
        if self.z != FlagOp::Unmodified {
            f = f.set_bit(CPUFlag::Z.u8(), self.z == FlagOp::Set);
        }

        if self.n != FlagOp::Unmodified {
            f = f.set_bit(CPUFlag::N.u8(), self.n == FlagOp::Set);
        }

        if self.h != FlagOp::Unmodified {
            f = f.set_bit(CPUFlag::H.u8(), self.h == FlagOp::Set);
        }

        if self.c != FlagOp::Unmodified {
            f = f.set_bit(CPUFlag::C.u8(), self.c == FlagOp::Set);
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
                            FlagOp::Set
                        } else {
                            FlagOp::Unset
                        },
                        n: FlagOp::Unset,
                        h: FlagOp::Set,
                        c: FlagOp::Unset,
                    },
                )
            }
            BitwiseOperation::Or => {
                let value = a | b?;
                (
                    value,
                    FlagDelta {
                        z: if value == 0 {
                            FlagOp::Set
                        } else {
                            FlagOp::Unset
                        },
                        n: FlagOp::Unset,
                        h: FlagOp::Unset,
                        c: FlagOp::Unset,
                    },
                )
            }
            BitwiseOperation::Xor => {
                let value = a ^ b?;
                (
                    value,
                    FlagDelta {
                        z: if value == 0 {
                            FlagOp::Set
                        } else {
                            FlagOp::Unset
                        },
                        n: FlagOp::Unset,
                        h: FlagOp::Unset,
                        c: FlagOp::Unset,
                    },
                )
            }
            BitwiseOperation::Not => {
                let value = !a;
                (
                    value,
                    FlagDelta {
                        z: FlagOp::Unmodified,
                        n: FlagOp::Set,
                        h: FlagOp::Set,
                        c: FlagOp::Unmodified,
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

fn half_carry_add(a: u8, b: u8) -> bool {
    a.mask(0x0F) + b.mask(0x0F) > 0x0F
}

fn half_carry_adc(a: u8, b: u8, carry_in: bool) -> bool {
    a.mask(0x0F) + b.mask(0x0F) + carry_in as u8 > 0x0F
}

fn half_carry_sub(a: u8, b: u8) -> bool {
    a.mask(0x0F) < b.mask(0x0F)
}

fn half_carry_sbc(a: u8, b: u8, carry_in: bool) -> bool {
    a.mask(0x0F) < b.mask(0x0f) + carry_in as u8
}

impl ArithmeticOperation {
    pub fn operation(&self, a: u8, b: Option<u8>, carry_in: bool) -> Option<(u8, FlagDelta)> {
        Some(match self {
            ArithmeticOperation::Add => {
                let b = b?;
                let (value, overflow) = a.overflowing_add(b);
                (
                    value,
                    FlagDelta {
                        z: if value == 0 {
                            FlagOp::Set
                        } else {
                            FlagOp::Unset
                        },
                        n: FlagOp::Unset,
                        h: if half_carry_add(a, b) {
                            FlagOp::Set
                        } else {
                            FlagOp::Unset
                        },
                        c: if overflow { FlagOp::Set } else { FlagOp::Unset },
                    },
                )
            }
            ArithmeticOperation::Adc => {}
            ArithmeticOperation::Sub => {
                let b = b?;
                let (value, underflow) = a.overflowing_sub(b);
                (
                    value,
                    FlagDelta {
                        z: if value == 0 {
                            FlagOp::Set
                        } else {
                            FlagOp::Unset
                        },
                        n: FlagOp::Set,
                        h: if half_carry_sub(a, b) {
                            FlagOp::Set
                        } else {
                            FlagOp::Unset
                        },
                        c: if underflow {
                            FlagOp::Set
                        } else {
                            FlagOp::Unset
                        },
                    },
                )
            }
            ArithmeticOperation::Sbc => {}
            ArithmeticOperation::Bit(bit_op) => bit_op.operation(a, b)?,
        })
    }
}

pub struct ProgramCounter {
    address: u16,
}

impl ProgramCounter {
    fn start() -> Self {
        Self {
            address: STARTING_ADDRESS,
        }
    }

    // Instructions can be 1 to 3 bytes, presented in 8 bits
    fn increment<T: Into<u16>>(&mut self, step: T) {
        self.address += step.into();
    }

    fn call(&mut self, address: u16) -> u16 {
        let return_address = self.address;
        self.address = address;

        return_address
    }

    fn jump(&mut self, address: u16) {
        self.address = address
    }
}

impl CPUFlag {
    pub fn u8(self) -> u8 {
        self as u8
    }

    pub fn boot(checksum: u8) -> u8 {
        if checksum == 0x00 {
            CPUFlag::Z.u8()
        } else {
            CPUFlag::Z.u8() | CPUFlag::H.u8() | CPUFlag::C.u8()
        }
    }
}

pub enum Register16Bits {
    AF,
    BC,
    DE,
    HL,
    SP,
}

// https://gbdev.io/pandocs/Power_Up_Sequence.html
// DMG
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
    pub fn new(cartridge: Cartridge) -> Self {
        match cartridge.cbc_flag {
            CGBFlag::Monochrome => Self {
                a: 0x01,
                f: CPUFlag::boot(cartridge.checksum),
                b: 0x00,
                c: 0x13,
                d: 0x00,
                e: 0xD8,
                h: 0x01,
                l: 0x4D,
                program_counter: ProgramCounter::start(),
                stack_pointer: 0xFFFE,
                instruction_register: None,
            },

            CGBFlag::Color => Self {
                a: 0x11,
                f: CPUFlag::Z.u8(),
                b: 0x00,
                c: 0x00,
                d: 0xFF,
                e: 0x56,
                h: 0x00,
                l: 0x0D,
                program_counter: ProgramCounter::start(),
                stack_pointer: 0xFFFE,
                instruction_register: None,
            },
        }
    }

    pub fn out<T: Into<u8>>(&mut self, value: T) {
        self.a = value.into();
    }

    pub fn flag(&self, flag: CPUFlag) -> bool {
        self.f & flag.u8() != 0
    }

    pub fn apply_flags(&mut self, delta: FlagDelta) {
        self.f = delta.apply(self.f);
    }

    pub fn set_16bit(&mut self, register_bits: Register16Bits, value: u16) {
        match register_bits {
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

    pub fn get_16bit(&mut self, register_bits: Register16Bits) -> u16 {
        match register_bits {
            Register16Bits::AF => self.a.merge_bytes(self.f),
            Register16Bits::BC => self.b.merge_bytes(self.c),
            Register16Bits::DE => self.d.merge_bytes(self.e),
            Register16Bits::HL => self.h.merge_bytes(self.l),
            Register16Bits::SP => self.stack_pointer,
        }
    }
}

pub struct CPU {
    registers: Registers,
    bus: Bus,
}

impl CPU {
    pub fn start(cartridge: Cartridge) -> Self {
        Self {
            registers: Registers::new(cartridge),
            bus: Bus::start(),
        }
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
    fn push(&mut self, address: u16) {}

    fn pop(&mut self) {}

    pub fn cycle(&mut self) {
        self.fetch();
        self.execute();
    }

    fn fetch(&mut self) {
        let index = self.registers.program_counter.address as usize;
        //self.registers.instruction_register = Some(self.bus.read(index));

        //self.registers.program_counter.increment(1);
    }

    fn execute(&mut self) {}
}

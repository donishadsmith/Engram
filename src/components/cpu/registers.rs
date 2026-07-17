use crate::components::{
    cpu::{FlagDelta, ProgramCounter, STARTING_ADDRESS, StatusFlag},
    rom::cartridge::CGBFlag,
    utils::{ByteOps16, MergeByteOps},
};
#[derive(Clone, Copy)]
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

#[derive(Clone, Copy)]
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
    pub fn new(cgb_flag: CGBFlag, checksum: u8) -> Self {
        match cgb_flag {
            CGBFlag::DMG => Self {
                a: 0x01,
                f: StatusFlag::boot(checksum),
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

            CGBFlag::CGB => Self {
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
            Register8Bits::F => self.f = value & 0xF0,
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

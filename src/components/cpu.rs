const STARTING_ADDRESS: u16 = 0x0000;

pub enum BitwiseOperation {
    And,
    Or,
    Xor,
    Not,
}

impl BitwiseOperation {
    pub fn operation(&self, first: u8, second: Option<u8>) -> Option<u8> {
        Some(match self {
            BitwiseOperation::And => first & second?,
            BitwiseOperation::Or => first | second?,
            BitwiseOperation::Xor => first ^ second?,
            BitwiseOperation::Not => !first,
        })
    }
}

enum ArithmeticOperation {
    Add,
    Sub,
    Bit(BitwiseOperation),
}

impl ArithmeticOperation {
    pub fn operation(&self, first: u8, second: Option<u8>) -> Option<(u8, bool, bool)> {
        Some(match self {
            ArithmeticOperation::Add => {
                let b = second?;
                let (value, overflow) = first.overflowing_add(b);
                let (_, half_carry) = (first << 4).overflowing_add(b << 4);
                (value, overflow, half_carry)
            }
            ArithmeticOperation::Sub => {
                let b = second?;
                let (value, underflow) = first.overflowing_sub(b);
                let (_, half_carry) = (first << 4).overflowing_sub(b << 4);
                (value, underflow, half_carry)
            }
            ArithmeticOperation::Bit(bit_op) => (bit_op.operation(first, second)?, false, false),
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
    fn increment(&mut self, instruction_byte_len: u8) {
        self.address += instruction_byte_len as u16;
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

#[derive(Copy, Clone)]
pub enum Flag {
    Z = 0x80, // 7 bit is 1; Zero flag
    N = 0x40, // 6 bit is 1; Subtraction flag (BCD)
    H = 0x20, // 5 bit is 1; Half Carry flag (BCD)
    C = 0x10, // 4 bit is 1; Carry flag
}

impl Flag {
    pub fn set(self) -> u8 {
        self as u8
    }

    pub fn boot() -> u8 {
        Flag::Z.set() | Flag::H.set() | Flag::C.set()
    }
}

pub struct Registers {
    pub a: u8, // output always goes to A register
    pub f: u8,
    pub b: u8,
    pub c: u8,
    pub d: u8,
    pub e: u8,
    pub h: u8,
    pub l: u8,
    pub program_counter: ProgramCounter,
    pub stack_pointer: u16,
}

impl Registers {
    pub fn new() -> Self {
        Self {
            a: 0x01,
            f: Flag::boot(), // DMG Z=1 N=0 H=? C=?; If the header checksum is $00, then the carry and half-carry flags are clear; otherwise, they are both set.
            b: 0xFF,
            c: 0x13,
            d: 0x00,
            e: 0xC1,
            h: 0x84,
            l: 0x03,
            program_counter: ProgramCounter::start(),
            stack_pointer: 0xFFFE,
        }
    }

    pub fn out(&mut self, value: u8) {
        self.a = value;
    }
}

pub struct ControlUnit {
    //instruction_register:,
    //stack_pointer,
}

impl ControlUnit {}

pub struct CPU {
    control_unit: ControlUnit,
    //registers:
}

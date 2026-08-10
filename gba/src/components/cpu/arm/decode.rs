use crate::components::{cpu::Condition, utils::BitOps};

#[derive(Clone, Copy, Debug, PartialEq)]
pub enum DataOp {
    And,
    Eor,
    Sub,
    Rsb,
    Add,
    Adc,
    Sbc,
    Rsc,
    Tst,
    Teq,
    Cmp,
    Cmn,
    Orr,
    Mov,
    Bic,
    Mvn,
}

impl DataOp {
    // [24:21]
    fn from_bits(bits: u32) -> Self {
        match bits & 0xF {
            0b0000 => DataOp::And,
            0b0001 => DataOp::Eor,
            0b0010 => DataOp::Sub,
            0b0011 => DataOp::Rsb,
            0b0100 => DataOp::Add,
            0b0101 => DataOp::Adc,
            0b0110 => DataOp::Sbc,
            0b0111 => DataOp::Rsc,
            0b1000 => DataOp::Tst,
            0b1001 => DataOp::Teq,
            0b1010 => DataOp::Cmp,
            0b1011 => DataOp::Cmn,
            0b1100 => DataOp::Orr,
            0b1101 => DataOp::Mov,
            0b1110 => DataOp::Bic,
            0b1111 => DataOp::Mvn,
            _ => unreachable!(),
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub enum ShiftType {
    LogicalLeft,
    LogicalRight,
    ArithmeticRight,
    RotateRight,
}

// [6:5]
fn from_bits(bits: u32) -> ShiftType {
    match bits.get_bit_range(5..7) {
        0b00 => ShiftType::LogicalLeft,
        0b01 => ShiftType::LogicalRight,
        0b10 => ShiftType::ArithmeticRight,
        0b11 => ShiftType::RotateRight,
        _ => unreachable!(),
    }
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub enum ShiftAmount {
    Immediate(u8),
    Register(u8),
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub enum Operand2 {
    Immediate {
        value: u8,
        rotate: u8,
    },
    Register {
        rm: u8,
        shift_type: ShiftType,
        shift_amount: ShiftAmount,
    },
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub enum ArmInstruction {
    DataProcessing {
        opcode: DataOp,
        set_flags: bool,
        rn: u8,
        rd: u8,
        operand2: Operand2,
    },
    Undefined,
}

pub struct DecodedArm {
    pub condition: Condition,
    pub instruction: ArmInstruction,
}

pub fn decode_arm(instruction: u32) -> DecodedArm {
    DecodedArm {
        condition: Condition::Al,
        instruction: ArmInstruction::Undefined,
    }
}

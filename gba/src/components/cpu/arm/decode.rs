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
        match bits {
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

impl ShiftType {
    // [6:5] for arm
    fn from_bits(bits: u32) -> ShiftType {
        match bits {
            0b00 => ShiftType::LogicalLeft,
            0b01 => ShiftType::LogicalRight,
            0b10 => ShiftType::ArithmeticRight,
            0b11 => ShiftType::RotateRight,
            _ => unreachable!(),
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct ShiftedRegister {
    pub rm: u8,
    pub shift_type: ShiftType,
    pub shift_amount: ShiftAmount,
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub enum ShiftAmount {
    Immediate(u8),
    Register(u8),
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub enum SdtOffset {
    Immediate(u16),
    Register(ShiftedRegister),
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub enum HalfwordOffset {
    Immediate(u8),
    Register(u8),
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub enum Operand2 {
    Immediate { value: u8, rotate: u8 },
    Register(ShiftedRegister),
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub enum BitSize {
    Word,
    Byte,
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub enum TransferAction {
    Load,
    Store,
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub enum AddressingMode {
    PostDecrement,
    PreDecrement,
    PostIncrement,
    PreIncrement,
}

impl AddressingMode {
    fn from_bits(bits: u8) -> AddressingMode {
        match bits {
            0b00 => AddressingMode::PostDecrement,
            0b01 => AddressingMode::PostIncrement,
            0b10 => AddressingMode::PreDecrement,
            0b11 => AddressingMode::PreIncrement,
            _ => unreachable!(),
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub enum TransferKind {
    UnsignedHalfword,
    SignedByte,
    SignedHalfword,
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub enum MsrSource {
    Register(u8),
    Immediate { value: u8, rotate: u8 },
}

// 4-2; Table 4.1.1
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum ArmInstruction {
    DataProcessing {
        opcode: DataOp,
        set_flags: bool,
        rn: u8,
        rd: u8,
        operand2: Operand2,
    },
    Mrs {
        rd: u8,
        use_spsr: bool,
    },
    Msr {
        source: MsrSource,
        use_spsr: bool,
        flags_only: bool,
    },
    Multiply {
        rm: u8,
        rs: u8,
        rn: u8,
        rd: u8,
        accumulate: bool,
        set_flags: bool,
    },
    MultiplyLong {
        rm: u8,
        rn: u8,
        rdlo: u8,
        rdhi: u8,
        accumulate: bool,
        signed: bool,
        set_flags: bool,
    },
    SingleDataSwap {
        rn: u8,
        rd: u8,
        rm: u8,
        swap_size: BitSize,
    },
    BranchExchange {
        rn: u8,
    },
    HalfwordDataTransfer {
        offset: HalfwordOffset,
        transfer_kind: TransferKind,
        rd: u8,
        rn: u8,
        write_back: bool,
        addressing_mode: AddressingMode,
        transfer_action: TransferAction,
    },
    SingleDataTransfer {
        rn: u8,
        rd: u8,
        transfer_action: TransferAction,
        write_back: bool,
        transfer_size: BitSize,
        addressing_mode: AddressingMode,
        offset: SdtOffset,
    },
    Undefined,
    BlockDataTransfer {
        rn: u8,
        transfer_action: TransferAction,
        write_back: bool,
        psr: bool,
        addressing_mode: AddressingMode,
        register_list: u16,
    },
    Branch {
        link: bool,
        offset: i32,
    },
    SoftwareInterrupt {
        comment: u32,
    },
    ThumbBlHigh {
        offset: i32,
    },
    ThumbBlLow {
        offset: u32,
    },
}

pub struct DecodedArm {
    pub condition: Condition,
    pub instruction: ArmInstruction,
}

pub fn decode_arm(instruction: u32) -> DecodedArm {
    let condition = Condition::from_arm_instruction(instruction);

    match instruction.get_bit_range(25..28) {
        0b000 | 0b001 => DecodedArm {
            condition,
            instruction: ArmInstruction::Undefined,
        }, // placeholder, remember to actually decode the instructions in here
        0b010 | 0b011 => {
            if instruction.is_set(4) && instruction.is_set(25) {
                DecodedArm {
                    condition,
                    instruction: ArmInstruction::Undefined,
                }
            } else {
                let offset = if instruction.is_set(25) {
                    SdtOffset::Register(ShiftedRegister {
                        rm: instruction.get_bit_range(0..4) as u8,
                        shift_type: ShiftType::from_bits(instruction.get_bit_range(5..7)),
                        shift_amount: ShiftAmount::Immediate(instruction.get_bit_range(7..12) as u8),
                    })
                } else {
                    SdtOffset::Immediate(instruction.get_bit_range(0..12) as u16)
                };

                DecodedArm {
                    condition,
                    instruction: ArmInstruction::SingleDataTransfer {
                        rn: instruction.get_bit_range(16..20) as u8,
                        rd: instruction.get_bit_range(12..16) as u8,
                        transfer_action: if instruction.is_set(20) {
                            TransferAction::Load
                        } else {
                            TransferAction::Store
                        },
                        write_back: instruction.is_set(21),
                        transfer_size: if instruction.is_set(22) {
                            BitSize::Byte
                        } else {
                            BitSize::Word
                        },
                        addressing_mode: AddressingMode::from_bits(
                            instruction.get_bit_range(23..25) as u8,
                        ),
                        offset,
                    },
                }
            }
        }
        0b100 => DecodedArm {
            condition,
            instruction: ArmInstruction::BlockDataTransfer {
                rn: instruction.get_bit_range(16..20) as u8,
                transfer_action: if instruction.is_set(20) {
                    TransferAction::Load
                } else {
                    TransferAction::Store
                },
                write_back: instruction.is_set(21),
                psr: instruction.is_set(22),
                addressing_mode: AddressingMode::from_bits(instruction.get_bit_range(23..25) as u8),
                register_list: instruction.get_bit_range(0..16) as u16,
            },
        },
        0b101 => {
            let offset = (instruction.get_bit_range(0..24) << 8) as i32 >> 6; // section 4-8, shifted left by 2 bits and added to pc
            DecodedArm {
                condition,
                instruction: ArmInstruction::Branch {
                    link: instruction.is_set(24),
                    offset,
                },
            }
        }
        0b110 => DecodedArm {
            condition,
            instruction: ArmInstruction::Undefined,
        }, // No coprocessor on gba just replace with undegined
        0b111 => {
            if instruction.is_set(24) {
                DecodedArm {
                    condition,
                    instruction: ArmInstruction::SoftwareInterrupt {
                        comment: instruction.get_bit_range(0..24) >> 16,
                    },
                }
            } else {
                DecodedArm {
                    condition,
                    instruction: ArmInstruction::Undefined,
                } // another coprocessor
            }
        }
        _ => unreachable!(),
    }
}

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
    fn from_arm_bits(bits: u8) -> DataOp {
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

    pub fn from_thumb_bits(bits: u8) -> DataOp {
        match bits {
            0 => DataOp::Mov,
            1 => DataOp::Cmp,
            2 => DataOp::Add,
            3 => DataOp::Sub,
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
    pub fn from_bits(bits: u8) -> ShiftType {
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
    DecrementAfter,
    DecrementBefore,
    IncrementAfter,
    IncrementBefore,
}

impl AddressingMode {
    fn from_bits(bits: u8) -> AddressingMode {
        match bits {
            0b00 => AddressingMode::DecrementAfter,
            0b01 => AddressingMode::IncrementAfter,
            0b10 => AddressingMode::DecrementBefore,
            0b11 => AddressingMode::IncrementBefore,
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

impl TransferKind {
    pub fn from_bits(bits: u8) -> TransferKind {
        match bits {
            0b01 => TransferKind::UnsignedHalfword,
            0b10 => TransferKind::SignedByte,
            0b11 => TransferKind::SignedHalfword,
            _ => unreachable!(),
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub enum MsrSource {
    Register(u8),
    Immediate { value: u8, rotate: u8 },
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub enum ThumbBranchType {
    High,
    Low,
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
        rs: u8,
        rdlo: u8,
        rdhi: u8,
        accumulate: bool,
        signed: bool,
        set_flags: bool,
    },
    SingleDataSwap {
        rm: u8,
        rd: u8,
        rn: u8,
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
    ThumbBranch {
        branch_type: ThumbBranchType,
        offset: u32,
    },
}

#[derive(PartialEq)]
pub struct DecodedArm {
    pub condition: Condition,
    pub instruction: ArmInstruction,
}

pub fn decode_arm(instruction: u32) -> DecodedArm {
    let condition = Condition::from_bits(instruction.get_bit_range(28..32) as u8);

    match instruction.get_bit_range(25..28) {
        0b000 | 0b001 => {
            if instruction.is_set(4) && instruction.is_set(7) && instruction.is_clear(25) {
                if instruction.get_bit_range(5..7) != 0 {
                    if instruction.is_set(22) {
                        return DecodedArm {
                            condition,
                            instruction: ArmInstruction::HalfwordDataTransfer {
                                offset: HalfwordOffset::Immediate(
                                    instruction.get_bit_range(0..4) as u8
                                        | (instruction.get_bit_range(8..12) as u8) << 4,
                                ),
                                transfer_kind: TransferKind::from_bits(
                                    instruction.get_bit_range(5..7) as u8,
                                ),
                                rd: instruction.get_bit_range(12..16) as u8,
                                rn: instruction.get_bit_range(16..20) as u8,
                                write_back: instruction.is_set(21),
                                addressing_mode: AddressingMode::from_bits(
                                    instruction.get_bit_range(23..25) as u8,
                                ),
                                transfer_action: if instruction.is_set(20) {
                                    TransferAction::Load
                                } else {
                                    TransferAction::Store
                                },
                            },
                        };
                    } else {
                        return DecodedArm {
                            condition,
                            instruction: ArmInstruction::HalfwordDataTransfer {
                                offset: HalfwordOffset::Register(
                                    instruction.get_bit_range(0..4) as u8
                                ),
                                transfer_kind: TransferKind::from_bits(
                                    instruction.get_bit_range(5..7) as u8,
                                ),
                                rd: instruction.get_bit_range(12..16) as u8,
                                rn: instruction.get_bit_range(16..20) as u8,
                                write_back: instruction.is_set(21),
                                addressing_mode: AddressingMode::from_bits(
                                    instruction.get_bit_range(23..25) as u8,
                                ),
                                transfer_action: if instruction.is_set(20) {
                                    TransferAction::Load
                                } else {
                                    TransferAction::Store
                                },
                            },
                        };
                    }
                }

                if instruction.is_set(24) {
                    return DecodedArm {
                        condition,
                        instruction: ArmInstruction::SingleDataSwap {
                            rm: instruction.get_bit_range(0..4) as u8,
                            rd: instruction.get_bit_range(12..16) as u8,
                            rn: instruction.get_bit_range(16..20) as u8,
                            swap_size: if instruction.is_set(22) {
                                BitSize::Byte
                            } else {
                                BitSize::Word
                            },
                        },
                    };
                }

                if instruction.is_set(23) {
                    return DecodedArm {
                        condition,
                        instruction: ArmInstruction::MultiplyLong {
                            rm: instruction.get_bit_range(0..4) as u8,
                            rs: instruction.get_bit_range(8..12) as u8,
                            rdlo: instruction.get_bit_range(12..16) as u8,
                            rdhi: instruction.get_bit_range(16..20) as u8,
                            accumulate: instruction.is_set(21),
                            set_flags: instruction.is_set(20),
                            signed: instruction.is_set(22),
                        },
                    };
                } else {
                    return DecodedArm {
                        condition,
                        instruction: ArmInstruction::Multiply {
                            rm: instruction.get_bit_range(0..4) as u8,
                            rs: instruction.get_bit_range(8..12) as u8,
                            rn: instruction.get_bit_range(12..16) as u8,
                            rd: instruction.get_bit_range(16..20) as u8,
                            accumulate: instruction.is_set(21),
                            set_flags: instruction.is_set(20),
                        },
                    };
                }
            } else {
                if instruction.get_bit_range(4..28) == 0b000100101111111111110001 {
                    return DecodedArm {
                        condition,
                        instruction: ArmInstruction::BranchExchange {
                            rn: instruction.get_bit_range(0..4) as u8,
                        },
                    };
                }

                let bits = instruction.get_bit_range(12..22);
                if bits == 0b1010011111 || bits == 0b1010001111 {
                    let flags_only = instruction.is_clear(16);
                    let source = if instruction.is_set(25) {
                        MsrSource::Immediate {
                            value: instruction.get_bit_range(0..8) as u8,
                            rotate: instruction.get_bit_range(8..12) as u8,
                        }
                    } else {
                        MsrSource::Register(instruction.get_bit_range(0..4) as u8)
                    };

                    return DecodedArm {
                        condition,
                        instruction: ArmInstruction::Msr {
                            source,
                            use_spsr: instruction.is_set(22),
                            flags_only,
                        },
                    };
                }

                if instruction.get_bit_range(0..12) == 0
                    && instruction.get_bit_range(16..22) == 0b001111
                    && instruction.get_bit_range(23..28) == 0b00010
                {
                    return DecodedArm {
                        condition,
                        instruction: ArmInstruction::Mrs {
                            rd: instruction.get_bit_range(12..16) as u8,
                            use_spsr: instruction.is_set(22),
                        },
                    };
                } else {
                    let operand2 = if instruction.is_clear(25) {
                        let shift_amount = if instruction.is_set(4) {
                            ShiftAmount::Register(instruction.get_bit_range(8..12) as u8)
                        } else {
                            ShiftAmount::Immediate(instruction.get_bit_range(7..12) as u8)
                        };

                        Operand2::Register(ShiftedRegister {
                            rm: instruction.get_bit_range(0..4) as u8,
                            shift_type: ShiftType::from_bits(instruction.get_bit_range(5..7) as u8),
                            shift_amount: shift_amount,
                        })
                    } else {
                        Operand2::Immediate {
                            value: instruction.get_bit_range(0..8) as u8,
                            rotate: instruction.get_bit_range(8..12) as u8,
                        }
                    };

                    return DecodedArm {
                        condition,
                        instruction: ArmInstruction::DataProcessing {
                            opcode: DataOp::from_arm_bits(instruction.get_bit_range(21..25) as u8),
                            set_flags: instruction.is_set(20),
                            rn: instruction.get_bit_range(16..20) as u8,
                            rd: instruction.get_bit_range(12..16) as u8,
                            operand2,
                        },
                    };
                }
            }
        }
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
                        shift_type: ShiftType::from_bits(instruction.get_bit_range(5..7) as u8),
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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_multiply() {
        let multiply = 0b1110_0000_0000_0000_0000_0010_1001_0001;
        let instruction = decode_arm(multiply);

        assert!(matches!(
            instruction.instruction,
            ArmInstruction::Multiply {
                rm: 1,
                rs: 2,
                rn: 0,
                rd: 0,
                accumulate: false,
                set_flags: false
            }
        ))
    }

    #[test]
    fn test_multiply_long() {
        let multiply_long = 0b1110_0000_1000_0001_0000_0011_1001_0010;
        let instruction = decode_arm(multiply_long);

        assert!(matches!(
            instruction.instruction,
            ArmInstruction::MultiplyLong {
                rm: 2,
                rs: 3,
                rdlo: 0,
                rdhi: 1,
                accumulate: false,
                set_flags: false,
                signed: false
            }
        ))
    }

    #[test]
    fn test_single_data_swap() {
        let single_data_swap = 0b1110_0001_0000_0010_0000_0000_1001_0001;
        let instruction = decode_arm(single_data_swap);

        assert!(matches!(
            instruction.instruction,
            ArmInstruction::SingleDataSwap {
                rm: 1,
                rd: 0,
                rn: 2,
                swap_size: BitSize::Word
            }
        ))
    }

    #[test]
    fn test_branch_and_exchange() {
        let branch_and_exchange = 0b1110_0001_0010_1111_1111_1111_0001_0000;
        let instruction = decode_arm(branch_and_exchange);

        assert!(matches!(
            instruction.instruction,
            ArmInstruction::BranchExchange { rn: 0 }
        ))
    }

    #[test]
    fn test_halfword_data_transfer() {
        let register_offset = 0b1110_0001_1001_0010_0001_0000_1011_0011;
        let instruction = decode_arm(register_offset);

        assert!(matches!(
            instruction.instruction,
            ArmInstruction::HalfwordDataTransfer {
                offset: HalfwordOffset::Register(3),
                rn: 2,
                rd: 1,
                write_back: false,
                transfer_action: TransferAction::Load,
                transfer_kind: TransferKind::UnsignedHalfword,
                addressing_mode: AddressingMode::IncrementBefore
            }
        ));

        let immediate_offset: u32 = 0b1110_0001_1101_0010_0001_0000_1011_0011;
        let instruction = decode_arm(immediate_offset);
        assert!(matches!(
            instruction.instruction,
            ArmInstruction::HalfwordDataTransfer {
                offset: HalfwordOffset::Immediate(3),
                rn: 2,
                rd: 1,
                write_back: false,
                transfer_action: TransferAction::Load,
                transfer_kind: TransferKind::UnsignedHalfword,
                addressing_mode: AddressingMode::IncrementBefore
            }
        ));
    }

    #[test]
    fn test_single_data_transfer() {
        let single_data_transfer = 0b1110_0101_1001_0001_0000_0000_0000_0100;
        let instruction = decode_arm(single_data_transfer);

        assert!(matches!(
            instruction.instruction,
            ArmInstruction::SingleDataTransfer {
                offset: SdtOffset::Immediate(4),
                rn: 1,
                rd: 0,
                write_back: false,
                transfer_action: TransferAction::Load,
                transfer_size: BitSize::Word,
                addressing_mode: AddressingMode::IncrementBefore,
            }
        ))
    }

    #[test]
    fn test_undefined() {
        let undefined = 0b1110_0110_0000_0000_0000_0000_0001_0000;
        let instruction = decode_arm(undefined);
        assert!(matches!(instruction.instruction, ArmInstruction::Undefined));

        let coprocessors: [u32; 3] = [
            0b1110_1101_1001_0001_0000_1111_0000_0000,
            0b1110_1110_0001_0001_0000_1111_0000_0010,
            0b1110_1110_0001_0001_0000_1111_0001_0000,
        ];

        for i in coprocessors {
            let instruction = decode_arm(i);
            assert!(matches!(instruction.instruction, ArmInstruction::Undefined));
        }
    }

    #[test]
    fn test_block_data_transfer() {
        let block_data_transfer = 0b1110_1000_1011_1101_0000_0000_0000_1111;
        let instruction = decode_arm(block_data_transfer);

        assert!(matches!(
            instruction.instruction,
            ArmInstruction::BlockDataTransfer {
                rn: 13,
                write_back: true,
                transfer_action: TransferAction::Load,
                addressing_mode: AddressingMode::IncrementAfter,
                psr: false,
                register_list: 0b1111
            }
        ))
    }

    #[test]
    fn test_branch() {
        let branch = 0b1110_1011_0000_0000_0000_0000_0000_0010;
        assert_eq!(
            decode_arm(branch).instruction,
            ArmInstruction::Branch {
                link: true,
                offset: 8
            }
        );
    }

    #[test]
    fn test_branch_decrement_pc() {
        let branch = 0b1110_1010_1111_1111_1111_1111_1111_1100;
        assert_eq!(
            decode_arm(branch).instruction,
            ArmInstruction::Branch {
                link: false,
                offset: -16
            }
        );
    }

    #[test]
    fn test_software_interrupt() {
        let software_interrupt = 0b1110_1111_0000_1000_0000_0000_0000_0000;
        let instruction = decode_arm(software_interrupt);

        assert_eq!(
            instruction.instruction,
            ArmInstruction::SoftwareInterrupt { comment: 8 }
        );
    }

    #[test]
    fn test_data_processing() {
        let data_processing = 0b1110_0000_1000_0001_0000_0000_0000_0010;
        let instruction = decode_arm(data_processing);

        assert_eq!(
            instruction.instruction,
            ArmInstruction::DataProcessing {
                opcode: DataOp::Add,
                set_flags: false,
                rn: 1,
                rd: 0,
                operand2: Operand2::Register(ShiftedRegister {
                    rm: 2,
                    shift_type: ShiftType::LogicalLeft,
                    shift_amount: ShiftAmount::Immediate(0)
                })
            }
        );
    }

    #[test]
    fn test_mrs() {
        let mrs = 0b1110_0001_0000_1111_0000_0000_0000_0000;
        let instruction = decode_arm(mrs);

        assert_eq!(
            instruction.instruction,
            ArmInstruction::Mrs {
                rd: 0,
                use_spsr: false
            }
        );
    }

    #[test]
    fn test_msr_no_flag_only() {
        let msr = 0b1110_0001_0010_1001_1111_0000_0000_0000;
        let instruction = decode_arm(msr);

        assert_eq!(
            instruction.instruction,
            ArmInstruction::Msr {
                source: MsrSource::Register(0),
                use_spsr: false,
                flags_only: false
            }
        );
    }

    #[test]
    fn test_msr_flags_only() {
        let msr = 0b1110_0011_0010_1000_1111_0100_1111_0000;
        let instruction = decode_arm(msr);

        assert_eq!(
            instruction.instruction,
            ArmInstruction::Msr {
                source: MsrSource::Immediate {
                    value: 0b11110000,
                    rotate: 0b100
                },
                use_spsr: false,
                flags_only: true
            }
        );
    }

    #[test]
    fn test_data_processing_register_shift() {
        let data_processing = 0b1110_0000_1000_0001_0000_0011_0001_0010;
        let instruction = decode_arm(data_processing);

        assert_eq!(
            instruction.instruction,
            ArmInstruction::DataProcessing {
                opcode: DataOp::Add,
                set_flags: false,
                rn: 1,
                rd: 0,
                operand2: Operand2::Register(ShiftedRegister {
                    rm: 2,
                    shift_type: ShiftType::LogicalLeft,
                    shift_amount: ShiftAmount::Register(3),
                })
            }
        );
    }
}

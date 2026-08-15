use crate::components::{
    cpu::{
        Condition,
        arm::decode::{
            AddressingMode, ArmInstruction, BitSize, DataOp, DecodedArm, HalfwordOffset, Operand2,
            SdtOffset, ShiftAmount, ShiftType, ShiftedRegister, TransferAction, TransferKind,
        },
    },
    utils::BitOps,
};

fn thumb_dp(opcode: DataOp, rn: u8, rd: u8, set_flags: bool, operand2: Operand2) -> ArmInstruction {
    ArmInstruction::DataProcessing {
        opcode,
        set_flags: set_flags,
        rn: rn,
        rd: rd,
        operand2,
    }
}

//**Thumb instruction set format is in 5-2**/
// the condition will be Al so that it always executed, thumb is just a subset of arm anyway
// Probably fold that the bl instrucion into arm, based on programmers model 3-8, in thumb state
// only the branch instruction is capable of conditional execution - this is the conditional branch
pub fn decode_thumb(instruction: u16) -> DecodedArm {
    let condition = Condition::Al;

    match instruction.get_bit_range(13..16) {
        0b000 => {
            if instruction.is_set(11) && instruction.is_set(12) {
                // format 2
                let operand2 = if instruction.is_set(10) {
                    Operand2::Immediate {
                        value: instruction.get_bit_range(6..9) as u8,
                        rotate: 0,
                    }
                } else {
                    Operand2::Register(ShiftedRegister {
                        rm: instruction.get_bit_range(6..9) as u8,
                        shift_type: ShiftType::LogicalLeft,
                        shift_amount: ShiftAmount::Immediate(0),
                    })
                };

                let opcode = if instruction.is_set(9) {
                    DataOp::Sub
                } else {
                    DataOp::Add
                };

                let instruction = thumb_dp(
                    opcode,
                    instruction.get_bit_range(3..6) as u8,
                    instruction.get_bit_range(0..3) as u8,
                    true,
                    operand2,
                );

                return DecodedArm {
                    condition,
                    instruction,
                };
            } else {
                // format 1
                return DecodedArm {
                    condition,
                    instruction: ArmInstruction::DataProcessing {
                        opcode: DataOp::Mov,
                        set_flags: true,
                        rn: 0, // Move ignores rn
                        rd: instruction.get_bit_range(0..3) as u8,
                        operand2: Operand2::Register(ShiftedRegister {
                            rm: instruction.get_bit_range(3..6) as u8,
                            shift_type: ShiftType::from_bits(
                                instruction.get_bit_range(11..13) as u8
                            ),
                            shift_amount: ShiftAmount::Immediate(
                                (instruction.get_bit_range(6..11)) as u8,
                            ),
                        }),
                    },
                };
            }
        }
        0b001 => {
            // format 3
            return DecodedArm {
                condition,
                instruction: ArmInstruction::DataProcessing {
                    opcode: DataOp::from_thumb_bits(instruction.get_bit_range(11..13) as u8),
                    set_flags: true,
                    rn: instruction.get_bit_range(8..11) as u8,
                    rd: instruction.get_bit_range(8..11) as u8,
                    operand2: Operand2::Immediate {
                        value: instruction.get_bit_range(0..8) as u8,
                        rotate: 0,
                    },
                },
            };
        }
        0b010 => {
            if instruction.is_set(12) {
                if instruction.is_set(9) {
                    // formaat 8
                    let sh = (instruction.get_bit(10) as u8) << 1 | (instruction.get_bit(11) as u8);
                    let (transfer_action, transfer_kind) = if sh == 0 {
                        (TransferAction::Store, TransferKind::from_bits(1))
                    } else {
                        (TransferAction::Load, TransferKind::from_bits(sh))
                    };
                    return DecodedArm {
                        condition,
                        instruction: ArmInstruction::HalfwordDataTransfer {
                            offset: HalfwordOffset::Register(instruction.get_bit_range(6..9) as u8),
                            transfer_kind,
                            rd: instruction.get_bit_range(0..3) as u8,
                            rn: instruction.get_bit_range(3..6) as u8,
                            write_back: false,
                            addressing_mode: AddressingMode::IncrementBefore,
                            transfer_action,
                        },
                    };
                } else {
                    // format 7
                    return DecodedArm {
                        condition,
                        instruction: ArmInstruction::SingleDataTransfer {
                            rn: instruction.get_bit_range(3..6) as u8,
                            rd: instruction.get_bit_range(0..3) as u8,
                            transfer_action: if instruction.is_set(11) {
                                TransferAction::Load
                            } else {
                                TransferAction::Store
                            },
                            write_back: false,
                            transfer_size: if instruction.is_set(10) {
                                BitSize::Byte
                            } else {
                                BitSize::Word
                            },
                            addressing_mode: AddressingMode::IncrementBefore,
                            offset: SdtOffset::Register(ShiftedRegister {
                                rm: instruction.get_bit_range(6..9) as u8,
                                shift_type: ShiftType::LogicalLeft,
                                shift_amount: ShiftAmount::Immediate(0),
                            }),
                        },
                    };
                }
            }

            let unshifted_rs = |rs| {
                Operand2::Register(ShiftedRegister {
                    rm: rs,
                    shift_type: ShiftType::LogicalLeft,
                    shift_amount: ShiftAmount::Immediate(0),
                })
            };

            if instruction.is_set(11) {
                // format 6
                return DecodedArm {
                    condition,
                    instruction: ArmInstruction::SingleDataTransfer {
                        rn: 15,
                        rd: instruction.get_bit_range(8..11) as u8,
                        transfer_action: TransferAction::Load,
                        write_back: false,
                        transfer_size: BitSize::Word,
                        addressing_mode: AddressingMode::IncrementBefore,
                        offset: SdtOffset::Immediate(instruction.get_bit_range(0..8) << 2),
                    },
                };
            } else {
                if instruction.is_set(10) {
                    // format 5
                    let rd = (instruction.get_bit(7) << 3) | instruction.get_bit_range(0..3) as u8; // rd/hd
                    let rs = (instruction.get_bit(6) << 3) | instruction.get_bit_range(3..6) as u8; // rs/hs

                    let opcode = instruction.get_bit_range(8..10) as u8;

                    if opcode == 0b11 {
                        return DecodedArm {
                            condition,
                            instruction: ArmInstruction::BranchExchange { rn: rs },
                        };
                    } else {
                        let instruction = match opcode {
                            0b00 => thumb_dp(DataOp::Add, rd, rd, false, unshifted_rs(rs)),
                            0b01 => thumb_dp(DataOp::Cmp, rd, rd, true, unshifted_rs(rs)),
                            0b10 => thumb_dp(DataOp::Mov, 0, rd, false, unshifted_rs(rs)),
                            _ => unreachable!(),
                        };

                        return DecodedArm {
                            condition,
                            instruction,
                        };
                    }
                } else {
                    // format 4
                    let rd = instruction.get_bit_range(0..3) as u8;
                    let rs = instruction.get_bit_range(3..6) as u8;

                    let shift = |st| {
                        Operand2::Register(ShiftedRegister {
                            rm: rd,
                            shift_type: st,
                            shift_amount: ShiftAmount::Register(rs),
                        })
                    };

                    let instruction = match instruction.get_bit_range(6..10) {
                        0b0000 => thumb_dp(DataOp::And, rd, rd, true, unshifted_rs(rs)),
                        0b0001 => thumb_dp(DataOp::Eor, rd, rd, true, unshifted_rs(rs)),
                        0b0010 => thumb_dp(DataOp::Mov, 0, rd, true, shift(ShiftType::LogicalLeft)),
                        0b0011 => {
                            thumb_dp(DataOp::Mov, 0, rd, true, shift(ShiftType::LogicalRight))
                        }
                        0b0100 => {
                            thumb_dp(DataOp::Mov, 0, rd, true, shift(ShiftType::ArithmeticRight))
                        }
                        0b0101 => thumb_dp(DataOp::Adc, rd, rd, true, unshifted_rs(rs)),
                        0b0110 => thumb_dp(DataOp::Sbc, rd, rd, true, unshifted_rs(rs)),
                        0b0111 => thumb_dp(DataOp::Mov, 0, rd, true, shift(ShiftType::RotateRight)),
                        0b1000 => thumb_dp(DataOp::Tst, rd, rd, true, unshifted_rs(rs)),
                        0b1001 => thumb_dp(
                            DataOp::Rsb,
                            rs,
                            rd,
                            true,
                            Operand2::Immediate {
                                value: 0,
                                rotate: 0,
                            },
                        ),
                        0b1010 => thumb_dp(DataOp::Cmp, rd, rd, true, unshifted_rs(rs)),
                        0b1011 => thumb_dp(DataOp::Cmn, rd, rd, true, unshifted_rs(rs)),
                        0b1100 => thumb_dp(DataOp::Orr, rd, rd, true, unshifted_rs(rs)),
                        0b1101 => ArmInstruction::Multiply {
                            rd,
                            rm: rd,
                            rs,
                            rn: 0,
                            accumulate: false,
                            set_flags: true,
                        },
                        0b1110 => thumb_dp(DataOp::Bic, rd, rd, true, unshifted_rs(rs)),
                        0b1111 => thumb_dp(DataOp::Mvn, 0, rd, true, unshifted_rs(rs)),
                        _ => unreachable!(),
                    };

                    return DecodedArm {
                        condition,
                        instruction,
                    };
                }
            }
        }
        0b011 => {
            // format 9
            let offset5 = instruction.get_bit_range(6..11);
            let (transfer_size, offset) = if instruction.is_set(12) {
                (BitSize::Byte, offset5)
            } else {
                (BitSize::Word, offset5 << 2)
            };

            return DecodedArm {
                condition,
                instruction: ArmInstruction::SingleDataTransfer {
                    rn: instruction.get_bit_range(3..6) as u8,
                    rd: instruction.get_bit_range(0..3) as u8,
                    transfer_action: if instruction.is_set(11) {
                        TransferAction::Load
                    } else {
                        TransferAction::Store
                    },
                    write_back: false,
                    transfer_size,
                    addressing_mode: AddressingMode::IncrementBefore,
                    offset: SdtOffset::Immediate(offset),
                },
            };
        }
        0b100 => {
            if instruction.is_set(12) {
                return DecodedArm {
                    condition,
                    instruction: ArmInstruction::SingleDataTransfer {
                        rn: 13,
                        rd: instruction.get_bit_range(8..11) as u8,
                        transfer_action: if instruction.is_set(11) {
                            TransferAction::Load
                        } else {
                            TransferAction::Store
                        },
                        write_back: false,
                        transfer_size: BitSize::Word,
                        addressing_mode: AddressingMode::IncrementBefore,
                        offset: SdtOffset::Immediate(instruction.get_bit_range(0..8) << 2),
                    },
                };
            } else {
                return DecodedArm {
                    condition,
                    instruction: ArmInstruction::HalfwordDataTransfer {
                        offset: HalfwordOffset::Immediate(
                            (instruction.get_bit_range(6..11) as u8) << 1,
                        ),
                        transfer_kind: TransferKind::UnsignedHalfword,
                        rd: instruction.get_bit_range(0..3) as u8,
                        rn: instruction.get_bit_range(3..6) as u8,
                        write_back: false,
                        addressing_mode: AddressingMode::IncrementBefore,
                        transfer_action: if instruction.is_set(11) {
                            TransferAction::Load
                        } else {
                            TransferAction::Store
                        },
                    },
                };
            }
        }
        0b101 => {
            if instruction.is_clear(12) {
                // format 12
                let rn = if instruction.is_set(11) { 13 } else { 15 };
                let operand2 = Operand2::Immediate {
                    value: instruction.get_bit_range(0..8) as u8,
                    rotate: 15,
                };
                let instruction = thumb_dp(
                    DataOp::Add,
                    rn,
                    instruction.get_bit_range(8..11) as u8,
                    false,
                    operand2,
                );

                return DecodedArm {
                    condition,
                    instruction,
                };
            }

            if instruction.is_set(10) {
                // format 14
                let load = instruction.is_set(11);
                let mut register_list = instruction.get_bit_range(0..8);
                if instruction.is_set(8) {
                    register_list.set_bit(if load { 15 } else { 14 });
                }
                return DecodedArm {
                    condition,
                    instruction: ArmInstruction::BlockDataTransfer {
                        rn: 13,
                        transfer_action: if load {
                            TransferAction::Load
                        } else {
                            TransferAction::Store
                        },
                        write_back: true,
                        psr: false,
                        addressing_mode: if load {
                            AddressingMode::IncrementAfter
                        } else {
                            AddressingMode::DecrementBefore
                        },
                        register_list,
                    },
                };
            } else {
                // format 13
                let dataop = match instruction.is_set(7) {
                    true => DataOp::Sub,
                    false => DataOp::Add,
                };

                let operand2 = Operand2::Immediate {
                    value: instruction.get_bit_range(0..7) as u8,
                    rotate: 15,
                };
                let instruction = thumb_dp(dataop, 13, 13, false, operand2);

                return DecodedArm {
                    condition,
                    instruction,
                };
            }
        }
        0b110 => {
            if instruction.is_clear(12) {
                // format 15
                return DecodedArm {
                    condition,
                    instruction: ArmInstruction::BlockDataTransfer {
                        rn: instruction.get_bit_range(8..11) as u8,
                        transfer_action: if instruction.is_set(11) {
                            TransferAction::Load
                        } else {
                            TransferAction::Store
                        },
                        write_back: true,
                        psr: false,
                        addressing_mode: AddressingMode::IncrementAfter,
                        register_list: instruction.get_bit_range(0..8),
                    },
                };
            }

            if instruction.get_bit_range(8..12) == 0b1111 {
                // format 17
                return DecodedArm {
                    condition,
                    instruction: ArmInstruction::SoftwareInterrupt {
                        comment: instruction.get_bit_range(0..8) as u32,
                    },
                };
            } else {
                // format 16
                let offset = (((instruction.get_bit_range(0..8) as u32) << 24) as i32) >> 23;
                return DecodedArm {
                    condition: Condition::from_bits(instruction.get_bit_range(8..12) as u8),
                    instruction: ArmInstruction::Branch {
                        link: false,
                        offset,
                    },
                };
            }
        }
        0b111 => {
            if instruction.is_set(12) {
                if instruction.is_set(11) {
                    // format 19
                    return DecodedArm {
                        condition,
                        instruction: ArmInstruction::ThumbBlLow {
                            offset: (instruction.get_bit_range(0..11) as u32) << 1,
                        },
                    };
                } else {
                    let offset = ((instruction.get_bit_range(0..11) as u32) << 21) as i32 >> 9;
                    return DecodedArm {
                        condition,
                        instruction: ArmInstruction::ThumbBlHigh { offset },
                    };
                }
            } else {
                // format 18
                if instruction.is_set(11) {
                    return DecodedArm {
                        condition,
                        instruction: ArmInstruction::Undefined,
                    };
                } else {
                    return DecodedArm {
                        condition,
                        instruction: ArmInstruction::Branch {
                            link: false,
                            offset: ((instruction.get_bit_range(0..11) as u32) << 21) as i32 >> 20,
                        },
                    };
                }
            }
        }
        _ => unreachable!(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_thumb_format1() {
        let format1 = 0b0000100100010001;
        let instruction = decode_thumb(format1);

        assert_eq!(
            instruction.instruction,
            ArmInstruction::DataProcessing {
                opcode: DataOp::Mov,
                set_flags: true,
                rn: 0,
                rd: 1,
                operand2: Operand2::Register(ShiftedRegister {
                    rm: 2,
                    shift_type: ShiftType::LogicalRight,
                    shift_amount: ShiftAmount::Immediate(4),
                }),
            }
        );
    }

    #[test]
    fn test_thumb_format2() {
        let format2 = 0b0001100010001000;
        let instruction = decode_thumb(format2);

        assert_eq!(
            instruction.instruction,
            ArmInstruction::DataProcessing {
                opcode: DataOp::Add,
                set_flags: true,
                rn: 1,
                rd: 0,
                operand2: Operand2::Register(ShiftedRegister {
                    rm: 2,
                    shift_type: ShiftType::LogicalLeft,
                    shift_amount: ShiftAmount::Immediate(0),
                }),
            }
        );
    }

    #[test]
    fn test_thumb_format3() {
        let format3 = 0b0010101000001010;
        let instruction = decode_thumb(format3);

        assert_eq!(
            instruction.instruction,
            ArmInstruction::DataProcessing {
                opcode: DataOp::Cmp,
                set_flags: true,
                rn: 2,
                rd: 2,
                operand2: Operand2::Immediate {
                    value: 10,
                    rotate: 0
                },
            }
        );
    }

    #[test]
    fn test_thumb_format4() {
        let format4 = 0b0100000010010001;
        let instruction = decode_thumb(format4);

        assert_eq!(
            instruction.instruction,
            ArmInstruction::DataProcessing {
                opcode: DataOp::Mov,
                set_flags: true,
                rn: 0,
                rd: 1,
                operand2: Operand2::Register(ShiftedRegister {
                    rm: 1,
                    shift_type: ShiftType::LogicalLeft,
                    shift_amount: ShiftAmount::Register(2),
                }),
            }
        );
    }

    #[test]
    fn test_thumb_format5() {
        let format5 = 0b0100011101110000;
        let instruction = decode_thumb(format5);

        assert_eq!(
            instruction.instruction,
            ArmInstruction::BranchExchange { rn: 14 }
        );
    }

    #[test]
    fn test_thumb_format6() {
        let format6 = 0b0100101100000010;
        let instruction = decode_thumb(format6);

        assert_eq!(
            instruction.instruction,
            ArmInstruction::SingleDataTransfer {
                rn: 15,
                rd: 3,
                transfer_action: TransferAction::Load,
                write_back: false,
                transfer_size: BitSize::Word,
                addressing_mode: AddressingMode::IncrementBefore,
                offset: SdtOffset::Immediate(8),
            }
        );
    }

    #[test]
    fn test_thumb_format7() {
        let format7 = 0b0101000010001000;
        let instruction = decode_thumb(format7);

        assert_eq!(
            instruction.instruction,
            ArmInstruction::SingleDataTransfer {
                rn: 1,
                rd: 0,
                transfer_action: TransferAction::Store,
                write_back: false,
                transfer_size: BitSize::Word,
                addressing_mode: AddressingMode::IncrementBefore,
                offset: SdtOffset::Register(ShiftedRegister {
                    rm: 2,
                    shift_type: ShiftType::LogicalLeft,
                    shift_amount: ShiftAmount::Immediate(0),
                }),
            }
        );
    }

    #[test]
    fn test_thumb_format8() {
        let format8 = 0b0101001011010001;
        let instruction = decode_thumb(format8);

        assert_eq!(
            instruction.instruction,
            ArmInstruction::HalfwordDataTransfer {
                offset: HalfwordOffset::Register(3),
                transfer_kind: TransferKind::UnsignedHalfword,
                rd: 1,
                rn: 2,
                write_back: false,
                addressing_mode: AddressingMode::IncrementBefore,
                transfer_action: TransferAction::Store,
            }
        );
    }

    #[test]
    fn test_thumb_format9() {
        let format9 = 0b0110100001001000;
        let instruction = decode_thumb(format9);

        assert!(matches!(
            instruction.instruction,
            ArmInstruction::SingleDataTransfer {
                offset: SdtOffset::Immediate(4),
                rn: 1,
                rd: 0,
                ..
            }
        ));
    }

    #[test]
    fn test_thumb_format10() {
        let format10 = 0b1000000011010001;
        let instruction = decode_thumb(format10);

        assert_eq!(
            instruction.instruction,
            ArmInstruction::HalfwordDataTransfer {
                offset: HalfwordOffset::Immediate(6),
                transfer_kind: TransferKind::UnsignedHalfword,
                rd: 1,
                rn: 2,
                write_back: false,
                addressing_mode: AddressingMode::IncrementBefore,
                transfer_action: TransferAction::Store,
            }
        );
    }

    #[test]
    fn test_thumb_format11() {
        let format11 = 0b1001010000000100;
        let instruction = decode_thumb(format11);

        assert_eq!(
            instruction.instruction,
            ArmInstruction::SingleDataTransfer {
                rn: 13,
                rd: 4,
                transfer_action: TransferAction::Store,
                write_back: false,
                transfer_size: BitSize::Word,
                addressing_mode: AddressingMode::IncrementBefore,
                offset: SdtOffset::Immediate(16),
            }
        );
    }

    #[test]
    fn test_thumb_format12() {
        let format12 = 0b1010001000001010;
        let instruction = decode_thumb(format12);

        assert_eq!(
            instruction.instruction,
            ArmInstruction::DataProcessing {
                opcode: DataOp::Add,
                set_flags: false,
                rn: 15,
                rd: 2,
                operand2: Operand2::Immediate {
                    value: 10,
                    rotate: 15
                },
            }
        );
    }

    #[test]
    fn test_thumb_format13() {
        let format13 = 0b1011000010000001;
        let instruction = decode_thumb(format13);

        assert_eq!(
            instruction.instruction,
            ArmInstruction::DataProcessing {
                opcode: DataOp::Sub,
                set_flags: false,
                rn: 13,
                rd: 13,
                operand2: Operand2::Immediate {
                    value: 1,
                    rotate: 15
                },
            }
        );
    }

    #[test]
    fn test_thumb_format14() {
        let format14 = 0b1011010100000001;
        let instruction = decode_thumb(format14);

        assert_eq!(
            instruction.instruction,
            ArmInstruction::BlockDataTransfer {
                rn: 13,
                transfer_action: TransferAction::Store,
                write_back: true,
                psr: false,
                addressing_mode: AddressingMode::DecrementBefore,
                register_list: 0b0100000000000001,
            }
        );
    }

    #[test]
    fn test_thumb_format15() {
        let format15 = 0b1100000110000001;
        let instruction = decode_thumb(format15);

        assert_eq!(
            instruction.instruction,
            ArmInstruction::BlockDataTransfer {
                rn: 1,
                transfer_action: TransferAction::Store,
                write_back: true,
                psr: false,
                addressing_mode: AddressingMode::IncrementAfter,
                register_list: 0b10000001,
            }
        );
    }

    #[test]
    fn test_thumb_format16() {
        let format16 = 0b1101000111111110;
        let instruction = decode_thumb(format16);

        assert_eq!(instruction.condition, Condition::Ne);
        assert_eq!(
            instruction.instruction,
            ArmInstruction::Branch {
                link: false,
                offset: -4
            }
        );
    }

    #[test]
    fn test_thumb_format17() {
        let format17 = 0b1101111100010010;
        let instruction = decode_thumb(format17);

        assert_eq!(
            instruction.instruction,
            ArmInstruction::SoftwareInterrupt { comment: 18 }
        );
    }

    #[test]
    fn test_thumb_format18() {
        let format18 = 0b1110011111111110;
        let instruction = decode_thumb(format18);

        assert_eq!(
            instruction.instruction,
            ArmInstruction::Branch {
                link: false,
                offset: -4,
            }
        );
    }

    #[test]
    fn test_thumb_format19() {
        let format19 = 0b1111011111111111;
        let instruction = decode_thumb(format19);

        assert_eq!(
            instruction.instruction,
            ArmInstruction::ThumbBlHigh { offset: -4096 }
        );
    }
}

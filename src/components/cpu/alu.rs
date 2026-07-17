use crate::components::cpu::{FlagDelta, FlagType};

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

pub fn half_carry_add(a: u8, b: u8, carry: bool) -> bool {
    (a & 0x0F) + (b & 0x0F) + carry as u8 > 0x0F
}

pub fn half_carry_sub(a: u8, b: u8, carry: bool) -> bool {
    (a & 0x0F) < (b & 0x0f) + carry as u8
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

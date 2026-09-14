use shared::traits::BitOps;

use crate::components::{
    bus::Bus,
    cpu::arm::decode::ShiftType,
    gamepak::{BackupType, GamePak},
};

pub fn create_bus(backup_type: BackupType) -> Bus {
    let gamepak = GamePak::mock(backup_type);
    let bus = Bus::new(gamepak, 512);

    bus
}

// Pages 4-13 of arm7tdmi data sheet, special meanings for how assembler handles 32 for lsr, ror, asr
// Basically 0 treated as 32
pub trait ShiftOps: BitOps {
    fn handle_right_shift(self, shift_amount: u8) -> Self {
        if (shift_amount as usize) >= Self::BIT_WIDTH {
            Self::ZERO
        } else {
            self >> shift_amount as usize
        }
    }

    fn lsl_imm(self, shift_amount: u8) -> (Self, bool) {
        if shift_amount == 0 {
            panic!("a shift amount of 0 is not allowed here")
        }

        let carry_out = self.is_set(Self::BIT_WIDTH - shift_amount as usize);
        let value = self << shift_amount.into();

        (value, carry_out)
    }

    fn lsr_imm(self, mut shift_amount: u8) -> (Self, bool) {
        if shift_amount == 0 {
            shift_amount = Self::BIT_WIDTH as u8;
        }

        let carry_out = self.is_set((shift_amount - 1) as usize);
        let value = self.handle_right_shift(shift_amount);

        (value, carry_out)
    }

    fn asr_imm(self, mut shift_amount: u8) -> (Self, bool) {
        if shift_amount == 0 {
            shift_amount = Self::BIT_WIDTH as u8;
        }

        let set_bits = self.is_set(Self::BIT_WIDTH - 1);
        let (mut value, carry_out) = Self::lsr_imm(self, shift_amount);
        let start = Self::BIT_WIDTH - shift_amount as usize;

        if set_bits {
            value.set_bit_range(start..Self::BIT_WIDTH);
        }
        (value, carry_out)
    }

    fn perform_rotate_right(self, shift_amount: u32) -> Self;

    fn ror_imm(self, shift_amount: u8, c_set: bool) -> (Self, bool) {
        if shift_amount == 0 {
            return self.rrx_imm(c_set);
        }

        let carry_out = self.is_set((shift_amount - 1) as usize);
        let value = self.perform_rotate_right(shift_amount as u32);

        (value, carry_out)
    }

    fn rrx_imm(self, c_set: bool) -> (Self, bool) {
        let carry_out = self.is_set(0);

        let carry_in = if c_set { Self::ONE } else { Self::ZERO };
        let mut value = self >> 1;
        value |= carry_in << (Self::BIT_WIDTH - 1);

        (value, carry_out)
    }

    // Page 4-15 really hyper specific rules if the shift amount comes from
    // a register, the least significant byte of rs is taken
    // imm is only 0 to 31 but reg values cut to a byte go up to 255
    fn lsl_reg(self, shift_amount: u8, c_set: bool) -> (Self, bool) {
        match shift_amount {
            0 => (self, c_set),
            1..=31 => self.lsl_imm(shift_amount),
            32 => (Self::ZERO, self.is_set(0)),
            _ => (Self::ZERO, false),
        }
    }

    fn lsr_reg(self, shift_amount: u8, c_set: bool) -> (Self, bool) {
        match shift_amount {
            0 => (self, c_set),
            1..=31 => self.lsr_imm(shift_amount),
            32 => (Self::ZERO, self.is_set(31)),
            _ => (Self::ZERO, false),
        }
    }

    fn asr_reg(self, shift_amount: u8, c_set: bool) -> (Self, bool) {
        match shift_amount {
            0 => (self, c_set),
            1..=31 => self.asr_imm(shift_amount),
            _ => {
                let bit31 = self.is_set(31);
                (if bit31 { !Self::ZERO } else { Self::ZERO }, bit31)
            }
        }
    }

    fn ror_reg(self, shift_amount: u8, c_set: bool) -> (Self, bool) {
        match shift_amount {
            0 => (self, c_set),
            1..=31 => self.ror_imm(shift_amount, c_set),
            32 => (self, self.is_set(31)),
            _ => {
                let amount = ((shift_amount - 1) & 31) + 1;
                self.ror_imm(amount, c_set)
            }
        }
    }

    fn shift_imm(
        self,
        shift_type: ShiftType,
        shift_amount: u8,
        c_set: bool,
    ) -> (Self, Option<bool>) {
        match shift_type {
            ShiftType::LogicalLeft if shift_amount == 0 => (self, None),
            ShiftType::LogicalLeft => {
                let (value, carry_out) = self.lsl_imm(shift_amount);

                (value, Some(carry_out))
            }
            ShiftType::LogicalRight => {
                let (value, carry_out) = self.lsr_imm(shift_amount);

                (value, Some(carry_out))
            }
            ShiftType::ArithmeticRight => {
                let (value, carry_out) = self.asr_imm(shift_amount);

                (value, Some(carry_out))
            }
            ShiftType::RotateRight => {
                let (value, carry_out) = self.ror_imm(shift_amount, c_set);

                (value, Some(carry_out))
            }
        }
    }

    fn shift_reg(
        self,
        shift_type: ShiftType,
        shift_amount: u8,
        c_set: bool,
    ) -> (Self, Option<bool>) {
        match shift_type {
            ShiftType::LogicalLeft => {
                let (value, carry_out) = self.lsl_reg(shift_amount, c_set);

                (value, Some(carry_out))
            }
            ShiftType::LogicalRight => {
                let (value, carry_out) = self.lsr_reg(shift_amount, c_set);

                (value, Some(carry_out))
            }
            ShiftType::ArithmeticRight => {
                let (value, carry_out) = self.asr_reg(shift_amount, c_set);

                (value, Some(carry_out))
            }
            ShiftType::RotateRight => {
                let (value, carry_out) = self.ror_reg(shift_amount, c_set);

                (value, Some(carry_out))
            }
        }
    }
}

impl ShiftOps for u32 {
    fn perform_rotate_right(self, shift_amount: u32) -> Self {
        self.rotate_right(shift_amount)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_lsl() {
        let word = 0b1110_0000_0000_0000_0000_0010_1001_0001;
        let (value, carry) = word.lsl_imm(3);

        assert_eq!(value, 0b0000_0000_0000_0000_0001_0100_1000_1000);
        assert_eq!(carry, true);
    }

    #[test]
    fn test_lsr() {
        let word = 0b1110_0000_0000_0000_0000_0010_1001_0001;
        let (value, carry) = word.lsr_imm(5);

        assert_eq!(value, 0b0000_0111_0000_0000_0000_0000_0001_0100);
        assert_eq!(carry, true);

        let (value, carry) = word.lsr_imm(0);

        assert_eq!(value, 0);
        assert_eq!(carry, true);
    }

    #[test]
    fn test_asr() {
        let word = 0b1110_0000_0000_0000_0000_0010_1001_0001;
        let (value, carry) = word.asr_imm(5);

        assert_eq!(value, 0b1111_1111_0000_0000_0000_0000_0001_0100);
        assert_eq!(carry, true);

        let word = 0b0110_0000_0000_0000_0000_0010_1001_0001;
        let (value, carry) = word.asr_imm(5);

        assert_eq!(value, 0b0000_0011_0000_0000_0000_0000_0001_0100);
        assert_eq!(carry, true);

        let word = 0b1110_0000_0000_0000_0000_0010_1001_0001;
        let (value, carry) = word.asr_imm(0);

        assert_eq!(value, 0b1111_1111_1111_1111_1111_1111_1111_1111);
        assert_eq!(carry, true);
    }

    #[test]
    fn test_ror() {
        let word = 0b1110_0000_0000_0000_0000_0010_1001_0001;
        let (value, carry) = word.ror_imm(5, true);

        assert_eq!(value, 0b1000_1111_0000_0000_0000_0000_0001_0100);
        assert_eq!(carry, true);

        let (value, carry) = word.ror_imm(32, true);

        assert_eq!(value, 0b1110_0000_0000_0000_0000_0010_1001_0001);
        assert_eq!(carry, true);
    }

    #[test]
    fn test_ror_reg() {
        let word = 0b1110_0000_0000_0000_0000_0010_1001_0001;
        let (value, carry) = word.ror_reg(33, true);

        assert_eq!(value, 0b1111_0000_0000_0000_0000_0001_0100_1000);
        assert_eq!(carry, true);

        let (value, carry) = word.shift_reg(ShiftType::RotateRight, 33, true);
        assert_eq!(value, 0b1111_0000_0000_0000_0000_0001_0100_1000);
        assert_eq!(carry, Some(true));
    }

    #[test]
    fn test_rrx() {
        let word = 0b1110_0000_0000_0000_0000_0010_1001_0001;
        let (value, carry) = word.rrx_imm(false);

        assert_eq!(value, 0b0111_0000_0000_0000_0000_0001_0100_1000);
        assert_eq!(carry, true);

        let (value, carry) = word.rrx_imm(true);

        assert_eq!(value, 0b1111_0000_0000_0000_0000_0001_0100_1000);
        assert_eq!(carry, true);
    }
}

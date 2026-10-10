use std::{
    mem::size_of,
    ops::{BitAnd, BitAndAssign, BitOrAssign, Not, Range, Shl, Shr},
};

pub trait UnsignedInt: Copy {
    const ZERO: Self;
    const ONE: Self;
    const MAX: Self;
}

impl UnsignedInt for u8 {
    const ZERO: Self = 0;
    const ONE: Self = 1;
    const MAX: Self = 0xFF;
}

impl UnsignedInt for u16 {
    const ZERO: Self = 0;
    const ONE: Self = 1;
    const MAX: Self = 0xFFFF;
}

impl UnsignedInt for u32 {
    const ZERO: Self = 0;
    const ONE: Self = 1;
    const MAX: Self = 0xFFFFFFFF;
}

impl UnsignedInt for u64 {
    const ZERO: Self = 0;
    const ONE: Self = 1;
    const MAX: Self = 0xFFFFFFFFFFFFFFFF;
} // Just to give ceertain traits to u64 for multiply long

// Inspired to create a trait for bit setting after seeing this:
// https://github.com/michelhe/rustboyadvance-ng/blob/master/arm7tdmi/src/psr.rs
pub trait BitOps:
    UnsignedInt
    + Sized
    + Shl<usize, Output = Self>
    + Shr<usize, Output = Self>
    + BitAnd<Output = Self>
    + BitOrAssign
    + BitAndAssign
    + Not<Output = Self>
    + PartialEq
{
    const BIT_WIDTH: usize = size_of::<Self>() * 8;

    fn set_bit(&mut self, bit: usize) {
        debug_assert!(
            bit < Self::BIT_WIDTH,
            "bit {bit} is out of range for a {}-bit value",
            Self::BIT_WIDTH
        );

        *self |= Self::ONE << bit;
    }

    fn is_set(self, bit: usize) -> bool {
        debug_assert!(
            bit < Self::BIT_WIDTH,
            "bit {bit} is out of range for a {}-bit value",
            Self::BIT_WIDTH
        );

        ((self >> bit) & Self::ONE) == Self::ONE
    }

    fn clear_bit(&mut self, bit: usize) {
        debug_assert!(
            bit < Self::BIT_WIDTH,
            "bit {bit} is out of range for a {}-bit value",
            Self::BIT_WIDTH
        );

        *self &= !(Self::ONE << bit);
    }

    fn is_clear(self, bit: usize) -> bool {
        !Self::is_set(self, bit)
    }

    fn get_bit(self, bit: usize) -> Self {
        if Self::is_set(self, bit) {
            Self::ONE
        } else {
            Self::ZERO
        }
    }

    fn set_bit_range_value(&mut self, range: Range<usize>, value: Self) {
        let Range { start, end } = range;
        debug_assert!(start <= end, "invalid range {start}..{end}");
        debug_assert!(
            end <= Self::BIT_WIDTH,
            "range {start}..{end} exceeds {} bits",
            Self::BIT_WIDTH
        );

        if start == end {
            return;
        }

        let diff = end - start;
        let mask = (!Self::ZERO) >> (Self::BIT_WIDTH - diff);

        *self &= !(mask << start);

        *self |= (value & mask) << start;
    }

    fn set_bit_range(&mut self, range: Range<usize>) {
        self.set_bit_range_value(range, !Self::ZERO);
    }

    fn get_bit_range(self, range: Range<usize>) -> Self {
        let Range { start, end } = range;
        if start >= end {
            panic!("invalid range {start}..{end}")
        }

        if end > Self::BIT_WIDTH {
            panic!("range {start}..{end} exceeds {} bits", Self::BIT_WIDTH);
        }

        let diff = end - start;
        let mask = (!Self::ZERO) >> (Self::BIT_WIDTH - diff);
        let masked_val = self & (mask << start);

        masked_val >> start
    }

    fn clear_bit_range(&mut self, range: Range<usize>) {
        self.set_bit_range_value(range, Self::ZERO);
    }

    fn is_zero(self) -> bool {
        self == Self::ZERO
    }

    fn is_negative(self) -> bool {
        self.is_set(Self::BIT_WIDTH - 1)
    }
}

impl BitOps for u8 {}

impl BitOps for u16 {}

impl BitOps for u32 {}

impl BitOps for u64 {}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_is_zero() {
        assert_eq!(0u8.is_zero(), true);
    }

    #[test]
    fn test_is_negative() {
        let mut byte: u8 = 0b10000000;
        assert_eq!(byte.is_negative(), true);

        byte.clear_bit(7);
        assert_eq!(byte.is_negative(), false)
    }

    #[test]
    fn test_basic_bit_ops() {
        let mut x: u8 = 0;
        x.set_bit(2);

        assert_eq!(x, 0x04);
        assert!(x.is_set(2));
        assert!(!x.is_clear(2));
        assert_eq!(x.get_bit(2), 1);

        x.clear_bit(2);
        assert!(x.is_clear(2));
    }

    #[test]
    fn test_range_bit_ops() {
        let mut x: u8 = 0;
        x.set_bit_range(1..4);
        assert_eq!(x, 0x0E);

        let bits = x.get_bit_range(1..4);
        assert_eq!(bits, 0b111);

        x.clear_bit_range(1..4);
        assert_eq!(x, 0);
    }

    #[test]
    fn test_clear_range_additional() {
        let mut x: u16 = 0xFFFF;
        x.clear_bit_range(8..16);
        assert_eq!(x, 0x00FF);
    }
}

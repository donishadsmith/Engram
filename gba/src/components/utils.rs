use crate::components::bus::sealed;
use std::{
    mem::size_of,
    ops::{BitAnd, BitAndAssign, BitOrAssign, Not, Range, Shl, Shr},
};

pub fn zero_arr<const N: usize>() -> Box<[u8; N]> {
    vec![0u8; N].into_boxed_slice().try_into().unwrap()
}

// Inspired to create a trait for bit setting after seeing this:
// https://github.com/michelhe/rustboyadvance-ng/blob/master/arm7tdmi/src/psr.rs
pub trait BitOps:
    sealed::Sealed
    + Sized
    + Copy
    + Shl<usize, Output = Self>
    + Shr<usize, Output = Self>
    + BitAnd<Output = Self>
    + BitOrAssign
    + BitAndAssign
    + Not<Output = Self>
    + PartialEq
{
    const ZERO: Self;
    const ONE: Self;
    const BIT_WIDTH: usize = size_of::<Self>() * 8;

    fn set_bit(&mut self, bit: usize) {
        assert!(
            bit < Self::BIT_WIDTH,
            "bit {bit} is out of range for a {}-bit value",
            Self::BIT_WIDTH
        );

        *self |= Self::ONE << bit;
    }

    fn is_set(self, bit: usize) -> bool {
        assert!(
            bit < Self::BIT_WIDTH,
            "bit {bit} is out of range for a {}-bit value",
            Self::BIT_WIDTH
        );

        ((self >> bit) & Self::ONE) == Self::ONE
    }

    fn clear_bit(&mut self, bit: usize) {
        assert!(
            bit < Self::BIT_WIDTH,
            "bit {bit} is out of range for a {}-bit value",
            Self::BIT_WIDTH
        );

        *self &= !(Self::ONE << bit);
    }

    fn is_clear(self, bit: usize) -> bool {
        !Self::is_set(self, bit)
    }

    fn get_bit(self, bit: usize) -> u8 {
        if Self::is_set(self, bit) { 1u8 } else { 0u8 }
    }

    fn set_bit_range_value(&mut self, range: Range<usize>, value: Self) {
        let Range { start, end } = range;
        assert!(start <= end, "invalid range {start}..{end}");
        assert!(
            end <= Self::BIT_WIDTH,
            "range {start}..{end} exceeds {} bits",
            Self::BIT_WIDTH
        );

        if start == end {
            return;
        }

        let diff = end - start;
        let mask = (!Self::ZERO) >> (Self::BIT_WIDTH - diff);

        *self = *self & !(mask << start);

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
}

impl BitOps for u8 {
    const ZERO: Self = 0;
    const ONE: Self = 1;
}

impl BitOps for u16 {
    const ZERO: Self = 0;
    const ONE: Self = 1;
}

impl BitOps for u32 {
    const ZERO: Self = 0;
    const ONE: Self = 1;
}

#[cfg(test)]
mod tests {
    use super::*;

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

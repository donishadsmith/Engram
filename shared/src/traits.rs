use std::{
    mem::size_of,
    ops::{BitAnd, BitAndAssign, BitOrAssign, Not, Range, Shl, Shr},
};

pub trait UnsignedInt: Copy {
    const ZERO: Self;
    const ONE: Self;
}

impl UnsignedInt for u8 {
    const ZERO: Self = 0;
    const ONE: Self = 1;
}

impl UnsignedInt for u16 {
    const ZERO: Self = 0;
    const ONE: Self = 1;
}

impl UnsignedInt for u32 {
    const ZERO: Self = 0;
    const ONE: Self = 1;
}

impl UnsignedInt for u64 {
    const ZERO: Self = 0;
    const ONE: Self = 1;
} // Just to give ceertain traits to u64 for multiply long

pub fn zero_arr<const N: usize>() -> Box<[u8; N]> {
    vec![0u8; N].into_boxed_slice().try_into().unwrap()
}

pub fn get_halfword_shift(address: u32) -> u8 {
    if address & 2 == 0 { 0 } else { 16 }
}

pub fn get_word_mask(address: u32) -> u32 {
    !(0xFFFF << get_halfword_shift(address))
}

pub struct GroupedRegisters<T: UnsignedInt> {
    pub registers: Box<[T]>,
    pub base_address: usize,
}

impl<T: UnsignedInt> GroupedRegisters<T> {
    pub fn new(capacity: usize, base_address: u32) -> Self {
        Self {
            registers: vec![T::ZERO; capacity].into_boxed_slice(),
            base_address: base_address as usize,
        }
    }

    pub fn index(&self, address: u32) -> usize {
        (address as usize - self.base_address) / size_of::<T>()
    }
}

impl GroupedRegisters<u16> {
    pub fn read_u16(&self, address: u32) -> u16 {
        self.registers[self.index(address & !1)]
    }

    pub fn write_u16(&mut self, address: u32, value: u16) {
        let index = self.index(address & !1);
        self.registers[index] = value;
    }

    pub fn from_index(&self, index: usize) -> u16 {
        self.registers[index]
    }
}

impl GroupedRegisters<u32> {
    pub fn read_u32(&self, address: u32) -> u32 {
        self.registers[self.index(address & !3)]
    }

    pub fn write_u32(&mut self, address: u32, value: u32) {
        self.registers[self.index(address & !3)] = value;
    }

    pub fn read_u16(&self, address: u32) -> u16 {
        let address = address & !3;
        let index = self.index(address);

        let mask = get_word_mask(address);
        let shift = get_halfword_shift(address);

        ((self.registers[index] & mask) >> shift) as u16
    }

    pub fn write_u16(&mut self, address: u32, value: u16) {
        let index = self.index(address & !3);
        let shift = get_halfword_shift(address);
        let register = &mut self.registers[index];

        *register = (*register & get_word_mask(address)) | ((value as u32) << shift);
    }

    pub fn from_index(&self, index: usize) -> u32 {
        self.registers[index]
    }
}

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

    fn get_bit(self, bit: usize) -> u8 {
        if Self::is_set(self, bit) { 1u8 } else { 0u8 }
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

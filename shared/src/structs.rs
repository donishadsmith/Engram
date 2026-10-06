use crate::{
    traits::UnsignedInt,
    utils::{get_halfword_shift, get_word_mask, zero_arr},
};

pub struct GroupedRegisters<T: UnsignedInt> {
    pub registers: Box<[T]>,
    pub base_address: usize,
}

impl<T: UnsignedInt> GroupedRegisters<T> {
    pub fn new(capacity: usize, base_address: u32) -> Self {
        Self {
            registers: zero_arr::<T>(capacity),
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

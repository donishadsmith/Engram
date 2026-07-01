use crate::components::{cartridge::Cartridge, memory::Memory};

pub struct Bus {
    pub memory: Memory,
}

impl Bus {
    pub fn new(cartridge: Cartridge) -> Self {
        Self {
            memory: Memory::new(cartridge),
        }
    }

    pub fn read(&self, address: usize) {
        //self.ram.memory[address]
    }

    pub fn write(&mut self, address: usize, value: u8) {
        //self.ram.memory[address] = value
    }
}

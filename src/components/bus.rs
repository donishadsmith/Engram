use crate::components::ram::RAM;

pub struct Bus {
    //pub ram: RAM,
}

impl Bus {
    pub fn start() -> Self {
        Self {}
    }

    pub fn read(&self, address: usize) {
        //self.ram.memory[address]
    }

    pub fn write(&mut self, address: usize, value: u8) {
        //self.ram.memory[address] = value
    }
}

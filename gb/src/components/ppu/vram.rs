pub struct VRam {
    pub bank: u8,
    pub bank_size: u16,
    pub memory: Vec<u8>,
}

impl VRam {
    pub fn new(is_cgb: bool) -> Self {
        Self {
            bank: 0,
            bank_size: 8 * 1024,
            memory: vec![0u8; if is_cgb { 0x4000 } else { 0x2000 }],
        }
    }

    pub fn index_adjustment(&self, address: u16) -> usize {
        let index = (address - 0x8000) as usize;
        let offset = ((self.bank as u16) * self.bank_size) as usize;

        index + offset
    }

    pub fn read(&self, address: u16) -> u8 {
        let index = self.index_adjustment(address);
        self.memory[index]
    }

    pub fn write(&mut self, address: u16, value: u8) {
        let index = self.index_adjustment(address);
        self.memory[index] = value;
    }

    pub fn bank_swap(&mut self, value: u8) {
        self.bank = value & 0x01;
    }

    pub fn read_banked(&self, bank: usize, address: u16) -> u8 {
        self.memory[(address - 0x8000) as usize + bank * self.bank_size as usize]
    }
}

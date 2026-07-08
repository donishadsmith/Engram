pub struct APU {
    pub wave_ram: Vec<u8>,
}

impl APU {
    pub fn new() -> Self {
        Self {
            wave_ram: vec![0u8; 16],
        }
    }

    pub fn read(&self, address: u16) {}

    pub fn write(&mut self, address: u16, value: u8) {}

    //(address - 0xFF10) as usize
}

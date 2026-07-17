pub struct NoiseChannel {
    nr41: u8,
    nr42: u8,
    nr43: u8,
    nr44: u8,
    enabled: bool,
    length_enable: bool,
    length_timer: u8,
    volume: u8,
    lfsr: u16,
}

impl NoiseChannel {
    pub fn control(&mut self, value: u8) {
        if (value >> 7) & 0x01 == 1 {
            self.enabled = true;
            self.volume = self.nr42 >> 4;
        }
    }
}

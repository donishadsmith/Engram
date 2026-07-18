pub struct NoiseChannel {
    enabled: bool,
    length_enable: bool,
    length_timer: u8,
    volume: u8,
    lfsr: u16,
}

impl NoiseChannel {
    pub fn write_nr42(&mut self, value: u8) {
        if (value >> 7) & 0x01 == 1 {
            self.enabled = true;
            self.volume = value >> 4;
        }
    }
}

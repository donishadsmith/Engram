use crate::components::utils::zero_arr;

pub struct PPU {
    pub vram: Box<[u8; 0x18000]>,
    pub palette_ram: Box<[u8; 0x400]>,
    pub oam: Box<[u8; 0x400]>,
}

impl PPU {
    pub fn new() -> Self {
        Self {
            vram: zero_arr(),
            palette_ram: zero_arr(),
            oam: zero_arr(),
        }
    }
}

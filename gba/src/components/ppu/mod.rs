use crate::components::utils::{BitOps, zero_arr};

// https://www.patater.com/gbaguy/gba/ch5.htm
// https://gbadev.net/tonc/
// https://github.com/gbadev-org/awesome-gbadev/blob/master/README.md#tutorials
// https://problemkaputt.de/gbatek.htm#gbalcdvideocontroller

pub struct PPU {
    pub vram: Box<[u8; 0x18000]>,
    pub palette_ram: Box<[u8; 0x400]>,
    pub oam: Box<[u8; 0x400]>,
    pub dispcnt: u16,
}

impl PPU {
    pub fn new() -> Self {
        Self {
            vram: zero_arr(),
            palette_ram: zero_arr(),
            oam: zero_arr(),
            dispcnt: 0,
        }
    }

    pub fn current_mode(&self) -> u8 {
        self.dispcnt.get_bit_range(0..3) as u8
    }

    fn bitmap_starting_address(&self) -> usize {
        self.dispcnt.get_bit(4) as usize
    }

    // Need to explore this more
    fn force_processing_during_hblank(&self) -> bool {
        self.dispcnt.is_set(5)
    }

    fn sprite_n_dimensions(&self) -> u8 {
        if self.dispcnt.is_set(6) { 1 } else { 2 }
    }

    fn force_blank_display(&self) -> bool {
        self.dispcnt.is_set(7)
    }

    fn enable_bg0(&self) -> bool {
        self.dispcnt.is_set(8)
    }

    fn enable_bg1(&self) -> bool {
        self.dispcnt.is_set(9)
    }

    fn enable_bg2(&self) -> bool {
        self.dispcnt.is_set(10)
    }

    fn enable_bg3(&self) -> bool {
        self.dispcnt.is_set(11)
    }

    fn enable_sprites(&self) -> bool {
        self.dispcnt.is_set(12)
    }

    fn enable_window0(&self) -> bool {
        self.dispcnt.is_set(13)
    }

    fn enable_window1(&self) -> bool {
        self.dispcnt.is_set(14)
    }

    fn enable_sprite_windows(&self) -> bool {
        self.dispcnt.is_set(15)
    }
}

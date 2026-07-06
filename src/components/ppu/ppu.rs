// https://github.com/Ashiepaws/GBEDG/blob/master/ppu/index.md

pub const SCREEN_WIDTH: usize = 160;
pub const SCREEN_HEIGHT: usize = 144;

const M_CYCLES_PER_SCANLINE: u16 = 456;

use crate::components::cpu::core::{ByteOps8, InterruptMode};

pub struct Sprite {
    pub position_y: i16,
    pub position_x: i16,
    pub tile: u8,
    pub below_bg: bool,
    pub flip_y: bool,
    pub flip_x: bool,
    pub dmg_palette: bool,
}

impl Sprite {
    pub fn from_oam(bytes: &[u8]) -> Self {
        Self {
            position_y: bytes[0] as i16 - 16,
            position_x: bytes[1] as i16 - 8,
            tile: bytes[2],
            below_bg: bytes[3].mask(0x80) != 0,
            flip_y: bytes[3].mask(0x40) != 0,
            flip_x: bytes[3].mask(0x20) != 0,
            dmg_palette: bytes[3].mask(0x10) != 0,
        }
    }
}

#[derive(PartialEq)]
enum PPUState {
    OAMSearch,
    PixelTransfer,
    HBlank,
    VBlank,
}

pub struct PPU {
    dots: u16,
    pub ly: u8,
    pub vram: Vec<u8>,
    pub oam: Vec<u8>,
    pub screen: [[u8; SCREEN_WIDTH]; SCREEN_HEIGHT],
}

impl PPU {
    pub fn new(is_cgb: bool) -> Self {
        Self {
            dots: 0,
            ly: 0,
            vram: vec![0u8; if is_cgb { 0x4000 } else { 0x2000 }],
            oam: vec![0u8; 0x00A0],
            screen: [[0u8; SCREEN_WIDTH]; SCREEN_HEIGHT],
        }
    }

    pub fn tick(&mut self, t_cycles: u16, interrupt_flag: &mut u8) {
        self.dots += t_cycles;
        while self.dots >= M_CYCLES_PER_SCANLINE {
            self.ly = (self.ly + 1) % 154;
            // https://blog.tigris.fr/2019/09/15/writing-an-emulator-the-first-pixel/
            // LY is the current horrizontal line, values from 144 to 153 is the vblank period
            if self.ly == 144 {
                *interrupt_flag |= InterruptMode::VBlank.mask();
            }

            self.dots -= M_CYCLES_PER_SCANLINE;
        }
    }
}

use crate::components::{
    dma::Trigger,
    utils::{BitOps, zero_arr},
};
use shared::render::Frame;
// https://www.patater.com/gbaguy/gba/ch5.htm
// https://gbadev.net/tonc/
// https://github.com/gbadev-org/awesome-gbadev/blob/master/README.md#tutorials
// https://problemkaputt.de/gbatek.htm#gbalcdvideocontroller

const SCREEN_WIDTH: usize = 240;
const SCREEN_HEIGTH: usize = 160;

pub struct PPU {
    pub vram: Box<[u8; 0x18000]>,
    pub palette_ram: Box<[u8; 0x400]>,
    pub oam: Box<[u8; 0x400]>,
    pub dispcnt: u16,
    pub dispstat: u16,
    pub vcount: u8,
    pub frame: Frame,
    pub frame_ready: bool,
}

impl PPU {
    pub fn new() -> Self {
        Self {
            vram: zero_arr(),
            palette_ram: zero_arr(),
            oam: zero_arr(),
            dispcnt: 0,
            dispstat: 0,
            vcount: 0,
            frame: Frame {
                pixels: Box::new([0; SCREEN_HEIGTH * SCREEN_WIDTH]),
                width: SCREEN_WIDTH,
                height: SCREEN_HEIGTH,
            },
            frame_ready: false,
        }
    }

    pub fn read_dispcnt(&self) {}

    pub fn write_dispcnt(&mut self) {}

    pub fn read_dispstat(&self) {}

    pub fn write_dispstat(&mut self) {}

    pub fn read_vcount(&self) {}

    pub fn handle_hblank(&mut self, interrupt_flag: &mut u16) -> Option<Trigger> {
        Some(Trigger::Hblank)
    }
    pub fn handle_hblank_end(&mut self, interrupt_flag: &mut u16) -> Option<Trigger> {
        // reminders: here will contain vblank, vcount
        // need to evaluate the start of each scanline and check if ly == lyc, if so
        // return vcount and enable interrupt
        // 159 to 160 transition, vblank return and trigger vblank interrupt
        Some(Trigger::Vblank)
    }

    pub fn current_mode(&self) -> u8 {
        self.dispcnt.get_bit_range(0..3) as u8
    }

    fn render_scanline(&mut self) {}
}

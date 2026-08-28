mod affine;
mod background;

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
const SCREEN_HEIGHT: usize = 160;

enum DispstatBit {
    VblankFlag = 0,
    HblankFlag = 1,
    VcounterFlag = 2,
    VblankInterrupt = 3,
    HblankInterrupt = 4,
    VcounterInterrupt = 5,
}

pub struct ScanlineEvent {
    pub vblank: bool,
    pub vcounter_match: bool,
}

struct BitmapModeParams {
    mode: u8,
    width: u8,
    height: u8,
    bpp: u8,
    page_flip: bool,
}

fn get_bitmap_mode_params(mode: u8) -> BitmapModeParams {
    let (width, height, bpp, page_flip) = match mode {
        3 => (240, 160, 16, false),
        4 => (240, 160, 8, true),
        5 => (160, 128, 16, true),
        _ => unreachable!(),
    };

    BitmapModeParams {
        mode,
        width,
        height,
        bpp,
        page_flip,
    }
}

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
                pixels: Box::new([0; SCREEN_HEIGHT * SCREEN_WIDTH]),
                width: SCREEN_WIDTH,
                height: SCREEN_HEIGHT,
            },
            frame_ready: false,
        }
    }

    pub fn read_dispcnt(&self) -> u16 {
        self.dispcnt
    }

    pub fn write_dispcnt(&mut self, value: u16) {
        self.dispcnt = value;

        self.dispcnt.clear_bit(3);
    }

    pub fn read_dispstat(&self) -> u16 {
        self.dispstat
    }

    pub fn write_dispstat(&mut self, mut value: u16) {
        self.dispstat.clear_bit_range(3..16);
        value.clear_bit_range(0..3);

        self.dispstat |= value;
    }

    pub fn read_vcount(&self) -> u16 {
        self.vcount as u16
    }

    pub fn handle_hblank(&mut self, interrupt_flag: &mut u16) -> Option<Trigger> {
        self.dispstat.set_bit(DispstatBit::HblankFlag as usize);

        self.set_interrupt(DispstatBit::HblankInterrupt, interrupt_flag);

        if self.vcount < 160 {
            self.render_scanline();

            Some(Trigger::Hblank)
        } else {
            None
        }
    }

    fn render_scanline(&mut self) {
        let mode = self.current_mode();
        match mode {
            3 | 4 | 5 => {
                let bitmap_mode_params = get_bitmap_mode_params(mode);

                self.generate_bitmap_mode_line(bitmap_mode_params);
            }
            _ => {}
        }
    }

    // the roms pass but i completely missed that these modes can rotate and scale too
    fn generate_bitmap_mode_line(&mut self, bitmap_mode_params: BitmapModeParams) {
        let page = self.dispcnt.get_bit(4) as usize;
        let bitmap_row = (self.vcount as usize) * bitmap_mode_params.width as usize;
        let frame_row = (self.vcount as usize) * SCREEN_WIDTH;

        if self.dispcnt.is_clear(10) {
            self.frame.pixels[frame_row..(frame_row + SCREEN_WIDTH)].fill(u16::from_le_bytes([
                self.palette_ram[0],
                self.palette_ram[1],
            ]));

            return;
        }

        for pixel in 0..SCREEN_WIDTH {
            let rgb_555 = if bitmap_mode_params.bpp == 16 {
                if bitmap_mode_params.mode == 5
                    && (pixel >= bitmap_mode_params.width as usize
                        || self.vcount >= bitmap_mode_params.height)
                {
                    u16::from_le_bytes([self.palette_ram[0], self.palette_ram[1]])
                } else {
                    let page_offset = if bitmap_mode_params.page_flip {
                        page * 0xA000
                    } else {
                        0
                    };
                    let index = page_offset + (bitmap_row + pixel) * 2;

                    u16::from_le_bytes([self.vram[index], self.vram[index + 1]])
                }
            } else {
                let index = (page * 0xA000) + bitmap_row + pixel;
                let byte = self.vram[index] as usize;

                u16::from_le_bytes([
                    self.palette_ram[(byte * 2) as usize],
                    self.palette_ram[((byte * 2) + 1) as usize],
                ])
            };

            self.frame.pixels[frame_row + pixel] = rgb_555;
        }
    }

    pub fn handle_hblank_end(&mut self, interrupt_flag: &mut u16) -> ScanlineEvent {
        let mut scanline_event = ScanlineEvent {
            vblank: false,
            vcounter_match: false,
        };

        self.vcount = (self.vcount + 1) % 228;

        self.dispstat.clear_bit(DispstatBit::HblankFlag as usize);

        if self.vcounter_match() {
            self.set_interrupt(DispstatBit::VcounterInterrupt, interrupt_flag);
            self.dispstat.set_bit(DispstatBit::VcounterFlag as usize);

            scanline_event.vcounter_match = true;
        } else {
            self.dispstat.clear_bit(DispstatBit::VcounterFlag as usize);
        }

        if self.vcount == 160 {
            self.frame_ready = true;
            self.set_interrupt(DispstatBit::VblankInterrupt, interrupt_flag);

            scanline_event.vblank = true;
        }

        if (160..227).contains(&self.vcount) {
            self.dispstat.set_bit(DispstatBit::VblankFlag as usize);
        } else {
            self.dispstat.clear_bit(DispstatBit::VblankFlag as usize);
        }

        scanline_event
    }

    pub fn current_mode(&self) -> u8 {
        self.dispcnt.get_bit_range(0..3) as u8
    }

    fn vcounter_match(&self) -> bool {
        self.vcount == self.dispstat.get_bit_range(8..16) as u8
    }

    fn set_interrupt(&self, flag: DispstatBit, interrupt_flag: &mut u16) {
        match flag {
            DispstatBit::VblankInterrupt => {
                if self.dispstat.is_set(DispstatBit::VblankInterrupt as usize) {
                    interrupt_flag.set_bit(0);
                }
            }
            DispstatBit::HblankInterrupt => {
                if self.dispstat.is_set(DispstatBit::HblankInterrupt as usize) {
                    interrupt_flag.set_bit(1);
                }
            }
            DispstatBit::VcounterInterrupt => {
                if self
                    .dispstat
                    .is_set(DispstatBit::VcounterInterrupt as usize)
                {
                    interrupt_flag.set_bit(2);
                }
            }
            _ => unreachable!(),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_vblank_irq_fires_once_when_enabled() {
        let mut ppu = PPU::new();

        let mut interrupt_flag = 0u16;

        ppu.write_dispstat(1 << 3); // vblank interrupt active

        for line in 0..228 {
            ppu.handle_hblank(&mut interrupt_flag);
            ppu.handle_hblank_end(&mut interrupt_flag);

            if line == 159 {
                assert!(
                    interrupt_flag.is_set(0),
                    "vblank interrupt should occur from 159 to 160"
                );
                assert!(interrupt_flag.is_clear(1), "hblank interrupt should be off");
                assert!(
                    interrupt_flag.is_clear(2),
                    "vcounter interrupt should be off"
                );
            }
        }

        assert_eq!(ppu.vcount, 0, "should go back to 0 after a frame");
        assert!(ppu.frame_ready);
    }
}

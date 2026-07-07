/*
    https://github.com/Ashiepaws/GBEDG/blob/master/ppu/index.md
    https://blog.tigris.fr/2019/09/15/writing-an-emulator-the-first-pixel/
    https://rylev.github.io/DMG-01/public/book/graphics/tile_ram.html
    tiles are tiles: [[u8; 16]; 384], make up 384 * 16 = 6144 in vram
    byte pair for (tile N, row R) = base + N*16 + R*2, each row is 2 bytes
    Recall each byte has 8 bits, which is why they have "8 columns", each pixel
    sprite/tile has 8 rows, every row uses 2 bytes, corresponding bit
    across these two bytes provide the color of the pixel that "bit location"
    Need second row of the first tile 0 * 16 + 1 * 2;
    (0, 0); (0, 1); (1, 0); (1, 1)
    456 dots per line, only 160 spent actually drawing anything
    0xFF42 is scroll y and 0xFF43 is scroll x, used as constants to move viewport
    background_y = ly + scroll_y; R = background_y % 8
    background_x = ly + scroll_x
    N = map[(by / 8) * 32 + (bx / 8)], base-2
    by / 8 = by >> 3
    ((by >> 3) << 5), drop last 3 bits slide back up by 5
    ((by >> 3) << 5) | by >> 3 due to no carries
    byte = 8 bits -> tile row = 8 pixels -> tile = 8×8
    u8 registers -> plane = 256×256
    256 / 8 = 32 -> map = 32×32 = 1,024 cells = 1KB; the tilemaps

    8000-87FF: First part of tile set #1
    8800-8FFF: Second part of tile set #1
               First part of tile set #2
    9000-97FF: Second part of tile set #2

    0x8800 - 0x8FFF shared by two tile sets
*/
use crate::components::cpu::core::{ByteOps8, InterruptMode};

pub const SCREEN_WIDTH: usize = 160;
pub const SCREEN_HEIGHT: usize = 144;

const DOTS_PER_TCYCLE: u32 = 456;
const STARTING_TILE_DATA_ADDRESS: u16 = 0x8000;

enum Palette {
    DMG {},
    CGB {},
}

pub struct VRam {
    bank: u8,
    bank_size: u16,
    memory: Vec<u8>,
}

impl VRam {
    fn new(is_cgb: bool) -> Self {
        Self {
            bank: 0,
            bank_size: 8 * 1024,
            memory: vec![0u8; if is_cgb { 0x4000 } else { 0x2000 }],
        }
    }

    fn index_adjustment(&self, address: u16) -> usize {
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
        self.bank = value;
    }
}

struct Sprite {
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

#[derive(Clone, Copy, PartialEq)]
#[repr(u8)]
pub enum PPUMode {
    HBlank = 0,
    VBlank = 1,
    OAMSearch = 2,
    PixelTransfer = 3,
}

pub struct PPU {
    dots: u32,
    pub vram: VRam,
    pub oam: Vec<u8>,
    pub ly: u8, //scanline
    pub lyc: u8,
    pub lcdc: u8,
    pub scy: u8,
    pub scx: u8,
    pub oam_dma: u8,
    pub bgp: u8,
    pub obp0: u8,
    pub obp1: u8,
    pub wx: u8,
    pub wy: u8,
    pub stat: u8,
    pub frame_ready: bool,
    is_cgb: bool,
    pub viewport: [[u8; SCREEN_WIDTH]; SCREEN_HEIGHT],
}

impl PPU {
    pub fn new(is_cgb: bool) -> Self {
        Self {
            dots: 0,
            vram: VRam::new(is_cgb),
            oam: vec![0u8; 0x00A0],
            ly: 0,
            lyc: 0,
            lcdc: 0x00,
            scy: 0x00,
            scx: 0x00,
            oam_dma: 0x00,
            bgp: 0x00,
            obp0: 0x00,
            obp1: 0x00,
            wx: 0x00,
            wy: 0x00,
            stat: 0x00,
            frame_ready: false,
            is_cgb: is_cgb,
            viewport: [[0u8; SCREEN_WIDTH]; SCREEN_HEIGHT],
        }
    }

    pub fn tick(&mut self, t_cycles: u32, interrupt_flag: &mut u8) {
        self.dots += t_cycles;
        while self.dots >= DOTS_PER_TCYCLE {
            self.dots -= DOTS_PER_TCYCLE;

            if self.ly < 144 {
                self.render_scanline();
            }
            self.ly = (self.ly + 1) % 154;

            if self.ly == 144 {
                *interrupt_flag |= InterruptMode::VBlank.mask();
                self.frame_ready = true;
            }
        }
    }

    fn render_scanline(&mut self) {
        //"mode 2": collect this line's sprites — a local, per your own demotion
        let sprites = self.oam_search(); //Vec<Sprite>, ≤10, OAM order

        // "mode 3": for x in 0..160 — bg pixel, sprite pixel, winner → viewport[ly][x]
    }

    fn oam_search(&self) {
        //Max of 10 sprites per scanline
        //s./* covers self.ly, given sprite height */
        /*let sprites: Vec<Sprite> = self
        .oam
        .chunks(4)
        .map(Sprite::from_oam)
        .filter(|s| {})
        .take(10)
        .collect();*/
    }

    pub fn mode(&self) -> PPUMode {
        if self.ly >= 144 {
            return PPUMode::VBlank;
        }

        match self.dots {
            0..=80 => PPUMode::OAMSearch,
            81..=252 => PPUMode::PixelTransfer,
            _ => PPUMode::HBlank,
        }
    }

    pub fn write_lcdc(&self, value: u8) {}
}

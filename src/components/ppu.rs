/*
    https://github.com/Ashiepaws/GBEDG/blob/master/ppu/index.md
    https://blog.tigris.fr/2019/09/15/writing-an-emulator-the-first-pixel/
    https://rylev.github.io/DMG-01/public/book/graphics/tile_ram.html
    https://imrannazar.com/series/gameboy-emulation-in-javascript/graphics
    https://jsgroth.dev/blog/posts/game-boy-color/

    tiles are tiles: [[u8; 16]; 384], make up 384 * 16 = 6144 in vram
    byte pair for (tile N, row R) = base + N*16 + R*2, each row is 2 bytes
    Recall each byte has 8 bits, which is why they have "8 columns", each pixel
    sprite/tile has 8 rows, every row uses 2 bytes, corresponding bit
    across these two bytes provide the color of the pixel that "bit location"
    Need second row of the first tile 0 * 16 + 1 * 2;
    (0, 0); (0, 1); (1, 0); (1, 1)
    456 dots per line, only 144 spent actually drawing anything
    0xFF42 is scroll y and 0xFF43 is scroll x, used as constants to move viewport
    background_y = ly + scroll_y; R = background_y % 8
    background_x = ly + scroll_x
    N = map[(by / 8) * 32 + (bx / 8)], base-2
    32 in equation if
    by / 8 = by >> 3
    ((by >> 3) << 5), drop last 3 bits slide back up by 5
    ((by >> 3) << 5) | bx >> 3 due to no carries
    (160 / 8) = 20 tiles per scanline
    background map is 32 * 32 tiles

    8000-87FF: First part of tile set #1
    8800-8FFF: Second part of tile set #1
               First part of tile set #2
    9000-97FF: Second part of tile set #2

    0x8800 - 0x8FFF shared by two tile sets
*/

// First get the DMG working first, then extend to color.
use crate::components::cpu::core::{ByteOps8, InterruptMode};

pub const SCREEN_WIDTH: usize = 160;
pub const SCREEN_HEIGHT: usize = 144;

const DOTS_PER_TCYCLE: u32 = 456;

pub struct VRam {
    pub bank: u8,
    bank_size: u16,
    pub memory: Vec<u8>,
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
        self.bank = value & 0x01;
    }
}

struct SpriteAttribute {
    oam_index: usize,
    position_y: i16,
    position_x: i16,
    tile_index: u8,
    priority: bool,
    flip_y: bool,
    flip_x: bool,
    palette_number: u8,
}

impl SpriteAttribute {
    pub fn from_oam(oam_index: usize, bytes: &[u8], is_cgb: bool) -> Self {
        Self {
            oam_index,
            position_y: bytes[0] as i16 - 16,
            position_x: bytes[1] as i16 - 8,
            tile_index: bytes[2],
            priority: bytes[3].mask(0x80) == 0,
            flip_y: bytes[3].mask(0x40) != 0,
            flip_x: bytes[3].mask(0x20) != 0,
            palette_number: if !is_cgb {
                bytes[3].mask(0x10)
            } else {
                bytes[3].mask(0x07)
            },
        }
    }
}

struct LCDC {
    pub enable_lcd: bool,
    window_tile_map_select: u8,
    enable_window: bool,
    tile_data_select: u8,
    bg_tile_map_select: u8,
    sprite_size: u8,
    enable_sprite: bool,
    enable_bg_and_window: bool,
}

impl LCDC {
    fn from_byte(byte: u8) -> Self {
        Self {
            enable_lcd: byte.mask(0x80) != 0,
            window_tile_map_select: byte.mask(0x40),
            enable_window: byte.mask(0x20) != 0,
            tile_data_select: byte.mask(0x10).min(1),
            bg_tile_map_select: byte.mask(0x08).min(1),
            sprite_size: if byte.mask(0x04) != 0 { 16 } else { 8 },
            enable_sprite: byte.mask(0x02) != 0,
            enable_bg_and_window: byte.mask(0x01) != 0,
        }
    }

    fn get_current_tile_address(&self, tile_index: u8) -> u16 {
        if self.tile_data_select == 1 {
            let base: u16 = 0x8000;
            base.wrapping_add((tile_index as u16).wrapping_mul(16))
        } else {
            let base: u16 = 0x9000;
            base.wrapping_add((tile_index.i16() as u16).wrapping_mul(16))
        }
    }

    fn bg_map_base(&self) -> u16 {
        if self.bg_tile_map_select == 1 {
            0x9C00
        } else {
            0x9800
        }
    }

    fn window_map_base(&self) -> u16 {
        if self.window_tile_map_select != 0 {
            0x9C00
        } else {
            0x9800
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
    pub window_line: u8,
    pub stat: u8,
    pub stat_interrupt_line: bool,
    pub frame_ready: bool,
    pub bgpi: u8,
    pub mode: PPUMode,
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
            window_line: 0,
            stat: 0x00,
            stat_interrupt_line: false,
            frame_ready: false,
            bgpi: 0x00,
            mode: PPUMode::OAMSearch,
            is_cgb: is_cgb,
            viewport: [[0u8; SCREEN_WIDTH]; SCREEN_HEIGHT],
        }
    }

    pub fn tick(&mut self, t_cycles: u32, interrupt_flag: &mut u8) {
        if !LCDC::from_byte(self.lcdc).enable_lcd {
            return;
        }

        self.dots += t_cycles;

        let current_mode = self.mode();
        if current_mode != self.mode {
            self.mode = current_mode;
            // Game that heavily relies on the stat interrupt
            // is F1 race. When not done, the track is a static
            // V shape. Creates the movement effect
            self.update_stat_interrupt_line(interrupt_flag);
        }

        while self.dots >= DOTS_PER_TCYCLE {
            self.dots -= DOTS_PER_TCYCLE;

            if self.ly < 144 {
                self.render_scanline();
            }

            self.ly = (self.ly + 1) % 154;
            self.update_stat_interrupt_line(interrupt_flag);

            self.mode = self.mode();

            if self.ly == 144 {
                *interrupt_flag |= InterruptMode::VBlank.mask();
                self.window_line = 0;
                self.frame_ready = true;
            }
        }
    }

    fn render_scanline(&mut self) {
        let lcdc_struct = LCDC::from_byte(self.lcdc);
        let background_y = self.ly.wrapping_add(self.scy);
        let background_tile_row = (background_y % 8) as u16;

        let mut window_rendered = false;

        let sprite_attributes = self.oam_search(lcdc_struct.sprite_size);

        let mut bg_indices = [0u8; SCREEN_WIDTH];
        for pixel in 0..SCREEN_WIDTH {
            let background_x = self.scx.wrapping_add(pixel as u8);
            let window_origin = self.wx as i16 - 7;
            let render_window = lcdc_struct.enable_window
                && lcdc_struct.enable_bg_and_window
                && self.ly >= self.wy
                && (pixel as i16) >= window_origin;

            let color_index = if render_window {
                window_rendered = true;
                let window_column = (pixel as i16 - window_origin) as u16;
                let map_cell = (self.window_line as u16 / 8) * 32 + (window_column as u16 / 8);
                let tile_index = self.vram.read(lcdc_struct.window_map_base() + map_cell);
                let current_tile_address = lcdc_struct.get_current_tile_address(tile_index)
                    + (self.window_line as u16 % 8) * 2;
                self.compute_color_index(
                    7 - (window_column % 8),
                    current_tile_address,
                    lcdc_struct.enable_bg_and_window,
                )
            } else {
                let map_cell = (background_y as u16 / 8) * 32 + (background_x as u16 / 8);
                let tile_index = self.vram.read(lcdc_struct.bg_map_base() + map_cell);
                let current_tile_address =
                    lcdc_struct.get_current_tile_address(tile_index) + background_tile_row * 2;
                let background_column = background_x % 8;
                // Get the bit position by subtracting seven, the column goes from right to left
                // from most to least significant bit, so flip
                let bit = 7 - background_column;

                self.compute_color_index(
                    bit as u16,
                    current_tile_address,
                    lcdc_struct.enable_bg_and_window,
                )
            };

            bg_indices[pixel] = color_index;
            let shade = (self.bgp >> (color_index * 2)) & 0x03;
            self.viewport[self.ly as usize][pixel] = shade;
        }

        if window_rendered {
            self.window_line += 1;
        }

        let sprite_row = |row: u16, flip: bool, sprite_size: u8| {
            if flip {
                (sprite_size as u16 - 1) - row
            } else {
                row
            }
        };

        let base_adress = 0x8000u16;
        for sprite_attribute in &sprite_attributes {
            if lcdc_struct.enable_sprite {
                for x in sprite_attribute.position_x.max(0)
                    ..(sprite_attribute.position_x + 8).min(SCREEN_WIDTH as i16)
                {
                    let sprite_column = x - sprite_attribute.position_x;
                    let bit = if sprite_attribute.flip_x {
                        sprite_column
                    } else {
                        7 - sprite_column
                    };
                    let mut row = (self.ly as i16 - sprite_attribute.position_y) as u16;
                    let tile_index = if lcdc_struct.sprite_size == 16 {
                        sprite_attribute.tile_index & 0xFE
                    } else {
                        sprite_attribute.tile_index
                    };
                    row = sprite_row(row, sprite_attribute.flip_y, lcdc_struct.sprite_size);
                    let current_tile_address = base_adress
                        .wrapping_add((tile_index as u16).wrapping_mul(16))
                        .wrapping_add(row * 2);

                    let color_index =
                        self.compute_color_index(bit as u16, current_tile_address, true);
                    if color_index == 0 {
                        continue;
                    }

                    let shade = (self.select_object_palette(sprite_attribute.palette_number)
                        >> (color_index * 2))
                        & 0x03;

                    if sprite_attribute.priority || bg_indices[x as usize] == 0 {
                        self.viewport[self.ly as usize][x as usize] = shade
                    }
                }
            }
        }
    }

    fn oam_search(&self, sprite_size: u8) -> Vec<SpriteAttribute> {
        // Each scanline can have up to 10 sprites, first
        // identify the sprites with a y coordinate overlapping with the scanline
        let mut sprite_attributes: Vec<SpriteAttribute> = self
            .oam
            .chunks(4)
            .enumerate()
            .map(|(i, bytes)| SpriteAttribute::from_oam(i, bytes, self.is_cgb))
            .filter(|s| {
                (s.position_y..(s.position_y + sprite_size as i16)).contains(&(self.ly as i16))
            })
            .take(10)
            .collect();

        // Sprite sorting different for color
        // For DMG sort in descending order for lowest coordinate sprite to always be rendered last
        if !self.is_cgb {
            sprite_attributes.sort_by(|a, b| {
                b.position_x
                    .cmp(&a.position_x)
                    .then(b.oam_index.cmp(&a.oam_index))
            });
        } else {
            sprite_attributes.reverse();
        }

        sprite_attributes
    }

    fn select_object_palette(&self, palette_number: u8) -> u8 {
        match palette_number {
            0 => self.obp0,
            1 => self.obp1,
            _ => unreachable!(),
        }
    }

    fn compute_color_index(&self, bit: u16, current_tile_address: u16, enable: bool) -> u8 {
        let tile_data_low = self.vram.read(current_tile_address);
        let tile_data_high = self.vram.read(current_tile_address + 1);
        let color_index = if enable {
            ((tile_data_high >> bit) & 0x01) << 1 | ((tile_data_low >> bit) & 0x01)
        } else {
            0
        };

        color_index
    }

    pub fn mode(&self) -> PPUMode {
        if self.ly >= 144 {
            return PPUMode::VBlank;
        }

        match self.dots {
            0..=79 => PPUMode::OAMSearch,
            80..=251 => PPUMode::PixelTransfer,
            _ => PPUMode::HBlank,
        }
    }

    // Future reference: https://alfaexploit.com/en/posts/gameboy_dev04/
    pub fn update_stat_interrupt_line(&mut self, interrupt_flag: &mut u8) {
        let mode: PPUMode = self.mode();
        let interrupt_line = (mode == PPUMode::HBlank && self.stat.mask(0x08) != 0)
            || (mode == PPUMode::VBlank && self.stat.mask(0x10) != 0)
            || (mode == PPUMode::OAMSearch && self.stat.mask(0x20) != 0)
            || (self.ly == self.lyc && self.stat.mask(0x40) != 0);

        if interrupt_line && !self.stat_interrupt_line {
            *interrupt_flag |= InterruptMode::Stat.mask();
        }

        self.stat_interrupt_line = interrupt_line;
    }
}

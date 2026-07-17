pub mod attributes;
pub mod palette;
pub mod sprites;
pub mod vram;

use crate::components::{
    cpu::interrupts::InterruptMode,
    ppu::{
        attributes::ColorBackgroundAttributes,
        palette::{ColorPaletteRegisterType, DMG_SHADES, cram_color},
        sprites::SpriteAttribute,
        vram::VRam,
    },
    utils::ByteOps8,
};
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
    background_x = pixel + scroll_x
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

    Basic instruction:

    Lookup tilemap (1 byte): read the tile index for some grid cell from 0x9800+
    Compute tile data address: tile_data_base + index * 16 + row + 2; each tile is 16 bytes (8 rows * 2 bytes)
    Fetch the actual tile data (2 bytes): the two bitplane bytes for that row
    Combine per-pixel for the color index for that pixel: for each of the 8 pixels, one bit from each byte becomes 2-bit color index
    Take color index to pallete to get actual coolor data via the monochrome BGP register on DMG, or CRAM on GBC
    For color, each pallette is 8 bytes and a each specific color uses 2 bytes
*/

pub const SCREEN_WIDTH: usize = 160;
pub const SCREEN_HEIGHT: usize = 144;

const DOTS_PER_SCANLINE: u32 = 456;

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
            enable_lcd: (byte & 0x80) != 0,
            window_tile_map_select: byte & 0x40,
            enable_window: (byte & 0x20) != 0,
            tile_data_select: (byte & 0x10).min(1),
            bg_tile_map_select: (byte & 0x08).min(1),
            sprite_size: if (byte & 0x04) != 0 { 16 } else { 8 },
            enable_sprite: (byte & 0x02) != 0,
            enable_bg_and_window: (byte & 0x01) != 0,
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
    pub monochrome_color_ram: [u8; 2],
    pub wx: u8,
    pub wy: u8,
    pub window_line: u8,
    pub stat: u8,
    pub stat_interrupt_line: bool,
    pub frame_ready: bool,
    pub bgpi: u8,
    pub obpi: u8,
    pub opri: u8,
    pub bg_palette_ram: [u8; 64],
    pub obj_palette_ram: [u8; 64],
    pub current_mode: PPUMode,
    pub entered_hblank: bool,
    is_cgb: bool,
    pub viewport: [[u16; SCREEN_WIDTH]; SCREEN_HEIGHT],
}

impl PPU {
    pub fn new(is_cgb: bool) -> Self {
        Self {
            dots: 0,
            vram: VRam::new(is_cgb),
            oam: vec![0u8; 0x00A0],
            ly: 0,
            lyc: 0,
            lcdc: 0,
            scy: 0,
            scx: 0,
            oam_dma: 0,
            bgp: 0,
            monochrome_color_ram: [0; 2],
            wx: 0,
            wy: 0,
            window_line: 0,
            stat: 0x00,
            stat_interrupt_line: false,
            frame_ready: false,
            bgpi: 0,
            obpi: 0,
            opri: 0,
            bg_palette_ram: [0xFF; 64],
            obj_palette_ram: [0xFF; 64],
            current_mode: PPUMode::OAMSearch,
            entered_hblank: false,
            is_cgb,
            viewport: [[0u16; SCREEN_WIDTH]; SCREEN_HEIGHT],
        }
    }

    pub fn tick(&mut self, t_cycles: u32, interrupt_flag: &mut u8) {
        if !LCDC::from_byte(self.lcdc).enable_lcd {
            return;
        }

        self.dots += t_cycles;

        self.update_mode(interrupt_flag);

        while self.dots >= DOTS_PER_SCANLINE {
            self.dots -= DOTS_PER_SCANLINE;

            if self.ly < 144 {
                self.render_scanline();
            }

            self.ly = (self.ly + 1) % 154;
            self.update_stat_interrupt_line(interrupt_flag);

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
        let bg_enable = self.is_cgb || lcdc_struct.enable_bg_and_window;

        let mut bg_indices = [0u8; SCREEN_WIDTH];
        for pixel in 0..SCREEN_WIDTH {
            let background_x = self.scx.wrapping_add(pixel as u8);
            let window_origin = self.wx as i16 - 7;
            let render_window = lcdc_struct.enable_window
                && lcdc_struct.enable_bg_and_window
                && self.ly >= self.wy
                && (pixel as i16) >= window_origin;

            let (color_index, attributes) = if render_window {
                window_rendered = true;
                let window_column = (pixel as i16 - window_origin) as u16;
                let map_cell = (self.window_line as u16 / 8) * 32 + (window_column / 8);
                let map_address = lcdc_struct.window_map_base() + map_cell;

                let attributes = if self.is_cgb {
                    ColorBackgroundAttributes::from_byte(self.vram.read_banked(1, map_address))
                } else {
                    ColorBackgroundAttributes::from_byte(0)
                };

                let tile_row = if attributes.y_flip {
                    7 - (self.window_line as u16 % 8)
                } else {
                    self.window_line as u16 % 8
                };
                let bit = if attributes.x_flip {
                    window_column % 8
                } else {
                    7 - (window_column % 8)
                };

                let tile_index = self.vram.read_banked(0, map_address);
                let current_tile_address =
                    lcdc_struct.get_current_tile_address(tile_index) + tile_row * 2;
                let color_index =
                    self.compute_color_index(attributes.bank, bit, current_tile_address, bg_enable);
                (color_index, attributes)
            } else {
                let map_cell = (background_y as u16 / 8) * 32 + (background_x as u16 / 8);
                let map_address = lcdc_struct.bg_map_base() + map_cell;

                let attributes = if self.is_cgb {
                    ColorBackgroundAttributes::from_byte(self.vram.read_banked(1, map_address))
                } else {
                    ColorBackgroundAttributes::from_byte(0)
                };
                let tile_row = if attributes.y_flip {
                    7 - background_tile_row
                } else {
                    background_tile_row
                };
                let background_column = (background_x % 8) as u16;
                let bit = if attributes.x_flip {
                    background_column
                } else {
                    7 - background_column
                };

                let tile_index = self.vram.read_banked(0, map_address);
                let current_tile_address =
                    lcdc_struct.get_current_tile_address(tile_index) + tile_row * 2;
                let color_index =
                    self.compute_color_index(attributes.bank, bit, current_tile_address, bg_enable);
                (color_index, attributes)
            };

            bg_indices[pixel] = color_index;

            self.viewport[self.ly as usize][pixel] = if self.is_cgb {
                cram_color(&self.bg_palette_ram, attributes.color_palette, color_index)
            } else {
                let shade = (self.bgp >> (color_index * 2)) & 0x03;
                DMG_SHADES[shade as usize]
            };
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

                    let color_index = self.compute_color_index(
                        sprite_attribute.bank,
                        bit as u16,
                        current_tile_address,
                        true,
                    );

                    if color_index == 0 {
                        continue;
                    }

                    let color = if self.is_cgb {
                        cram_color(
                            &self.obj_palette_ram,
                            sprite_attribute.palette_number,
                            color_index,
                        )
                    } else {
                        let dmg_palette =
                            self.monochrome_object_palette(sprite_attribute.palette_number);
                        let shade = (dmg_palette >> (color_index * 2)) & 0x03;
                        DMG_SHADES[shade as usize]
                    };

                    if sprite_attribute.priority || bg_indices[x as usize] == 0 {
                        self.viewport[self.ly as usize][x as usize] = color;
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

        if !self.is_cgb || (self.opri & 0x01) != 0 {
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

    fn monochrome_object_palette(&self, palette_number: u8) -> u8 {
        match palette_number {
            0 => self.monochrome_color_ram[0],
            _ => self.monochrome_color_ram[1],
        }
    }

    fn compute_color_index(
        &self,
        bank: usize,
        bit: u16,
        current_tile_address: u16,
        enable: bool,
    ) -> u8 {
        let tile_data_low = self.vram.read_banked(bank, current_tile_address);
        let tile_data_high = self.vram.read_banked(bank, current_tile_address + 1);
        let color_index = if enable {
            ((tile_data_high >> bit) & 0x01) << 1 | ((tile_data_low >> bit) & 0x01)
        } else {
            0
        };

        color_index
    }

    pub fn current_mode(&self) -> PPUMode {
        if self.ly >= 144 {
            return PPUMode::VBlank;
        }

        match self.dots {
            0..=79 => PPUMode::OAMSearch,
            80..=251 => PPUMode::PixelTransfer,
            _ => PPUMode::HBlank,
        }
    }

    fn update_mode(&mut self, interrupt_flag: &mut u8) {
        let current_mode = self.current_mode();
        if current_mode != self.current_mode {
            if current_mode == PPUMode::HBlank && self.current_mode != PPUMode::HBlank {
                self.entered_hblank = true;
            }

            self.current_mode = current_mode;
            // Game that heavily relies on the stat interrupt
            // is F1 race. When not done, the track is a static
            // V shape. Creates the movement effect
            self.update_stat_interrupt_line(interrupt_flag);
        }
    }

    // Future reference: https://alfaexploit.com/en/posts/gameboy_dev04/
    pub fn update_stat_interrupt_line(&mut self, interrupt_flag: &mut u8) {
        let mode: PPUMode = self.current_mode();
        let interrupt_line = mode == PPUMode::HBlank && (self.stat & 0x08) != 0
            || (mode == PPUMode::VBlank && (self.stat & 0x10) != 0)
            || (mode == PPUMode::OAMSearch && (self.stat & 0x20) != 0)
            || (self.ly == self.lyc && (self.stat & 0x40) != 0);

        if interrupt_line && !self.stat_interrupt_line {
            *interrupt_flag |= InterruptMode::Stat.mask();
        }

        self.stat_interrupt_line = interrupt_line;
    }

    pub fn write_lcdc(&mut self, value: u8) {
        let lcd_was_on = self.lcdc & 0x80 != 0;
        self.lcdc = value;
        if lcd_was_on && self.lcdc & 0x80 == 0 {
            self.ly = 0;
            self.dots = 0;
            self.window_line = 0;
            self.current_mode = PPUMode::HBlank;
            self.stat_interrupt_line = false;
        }
    }

    pub fn read_color_palette_index(&self, palette: ColorPaletteRegisterType) -> u8 {
        match palette {
            ColorPaletteRegisterType::Background => self.bgpi & 0x3F,
            ColorPaletteRegisterType::Object => self.obpi & 0x3F,
        }
    }

    pub fn read_color_palette_register(&self, palette: ColorPaletteRegisterType) -> u8 {
        match palette {
            ColorPaletteRegisterType::Background => self.bgpi | 0x40,
            ColorPaletteRegisterType::Object => self.obpi | 0x40,
        }
    }

    pub fn read_color_palette_data(&self, palette: ColorPaletteRegisterType) -> u8 {
        match palette {
            ColorPaletteRegisterType::Background => {
                let index = (self.read_color_palette_index(palette)) as usize;
                self.bg_palette_ram[index]
            }
            ColorPaletteRegisterType::Object => {
                let index = (self.read_color_palette_index(palette)) as usize;
                self.obj_palette_ram[index]
            }
        }
    }

    pub fn write_color_palette_data(&mut self, value: u8, palette: ColorPaletteRegisterType) {
        match palette {
            ColorPaletteRegisterType::Background => {
                let index = (self.read_color_palette_index(palette)) as usize;
                self.bg_palette_ram[index] = value;
            }
            ColorPaletteRegisterType::Object => {
                let index = (self.read_color_palette_index(palette)) as usize;
                self.obj_palette_ram[index] = value;
            }
        }

        self.increment_color_palette_index(palette);
    }

    pub fn increment_color_palette_index(&mut self, palette: ColorPaletteRegisterType) {
        let selected_palette = match palette {
            ColorPaletteRegisterType::Background => &mut self.bgpi,
            ColorPaletteRegisterType::Object => &mut self.obpi,
        };

        if *selected_palette & 0x80 != 0 {
            let incremented_address = ((*selected_palette & 0x3F) + 1) % 64;
            *selected_palette = *selected_palette & 0x80 | incremented_address;
        }
    }
}

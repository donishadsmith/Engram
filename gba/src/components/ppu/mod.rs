mod affine;
mod special_effects;
mod sprites;

use crate::components::{
    dma::Trigger,
    utils::{BitOps, GroupedRegisters, zero_arr},
};
use affine::{AffineMatrix, AffineState};
use shared::render::Frame;
use sprites::{SpriteAttributes, SpriteMode, SpritePixel};
use std::mem::take;
// https://www.patater.com/gbaguy/gba/ch5.htm
// https://gbadev.net/tonc/
// https://github.com/gbadev-org/awesome-gbadev/blob/master/README.md#tutorials
// https://problemkaputt.de/gbatek.htm#gbalcdvideocontroller

const SCREEN_WIDTH: usize = 240;
const SCREEN_HEIGHT: usize = 160;

#[derive(Debug)]
enum DispstatBit {
    VblankFlag = 0,
    HblankFlag = 1,
    VcounterFlag = 2,
    VblankInterrupt = 3,
    HblankInterrupt = 4,
    VcounterInterrupt = 5,
}

#[derive(Clone, Copy, PartialEq, Eq)]
enum RenderType {
    Text,
    Affine,
    Bitmap,
}

pub struct ScanlineEvent {
    pub vblank: bool,
    pub vcounter_match: bool,
}

struct BitmapModeParams {
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
        width,
        height,
        bpp,
        page_flip,
    }
}

struct BgLine {
    id: usize,
    on: bool,
    priority: u8,
    palette_indices: [Option<usize>; SCREEN_WIDTH],
}

impl BgLine {
    fn new(id: usize, on: bool, priority: u8) -> Self {
        Self {
            id,
            on,
            priority,
            palette_indices: [None; SCREEN_WIDTH],
        }
    }
}

#[derive(Clone, Copy, PartialEq, Eq)]
pub enum LayerId {
    Bg0,
    Bg1,
    Bg2,
    Bg3,
    Sprite,
    Backdrop,
}

impl LayerId {
    fn from_background(bg_id: usize) -> LayerId {
        match bg_id {
            0 => LayerId::Bg0,
            1 => LayerId::Bg1,
            2 => LayerId::Bg2,
            3 => LayerId::Bg3,
            _ => unreachable!(),
        }
    }
}

#[derive(Clone, Copy)]
pub struct Pixel {
    pub id: LayerId,
    priority: u8,
    pub color: u16,
}

#[derive(Clone, Copy, PartialEq, Eq)]
enum Bpp {
    FourBpp,
    EigthBpp,
}

struct Coordinate {
    x: usize,
    y: usize,
}

pub struct PPU {
    pub vram: Box<[u8; 0x18000]>,
    pub palette_ram: Box<[u8; 0x400]>,
    pub oam: Box<[u8; 0x400]>,
    pub dispcnt: u16,
    pub dispstat: u16,
    pub bg_control: GroupedRegisters<u16>,
    pub bg_text_offset: GroupedRegisters<u16>,
    pub bg2_affine_parameters: GroupedRegisters<u16>,
    pub bg2_affine_reference: GroupedRegisters<u32>,
    pub bg2_affine_state: AffineState,
    pub bg3_affine_parameters: GroupedRegisters<u16>,
    pub bg3_affine_reference: GroupedRegisters<u32>,
    pub bg3_affine_state: AffineState,
    pub window_features: GroupedRegisters<u16>,
    pub color_special_effects: GroupedRegisters<u16>,
    pub mosaic: u16,
    pub vcount: u8,
    pub frontend: Frame,
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
            bg_control: GroupedRegisters::new(4, 0x4000008),
            bg_text_offset: GroupedRegisters::new(8, 0x4000010),
            bg2_affine_parameters: GroupedRegisters::new(4, 0x4000020),
            bg2_affine_reference: GroupedRegisters::new(2, 0x4000028),
            bg2_affine_state: AffineState::default(),
            bg3_affine_parameters: GroupedRegisters::new(4, 0x4000030),
            bg3_affine_reference: GroupedRegisters::new(2, 0x4000038),
            bg3_affine_state: AffineState::default(),
            window_features: GroupedRegisters::new(6, 0x4000040),
            mosaic: 0,
            color_special_effects: GroupedRegisters::new(3, 0x4000050),
            vcount: 0,
            frontend: Frame {
                pixels: Box::new([0; SCREEN_HEIGHT * SCREEN_WIDTH]),
                width: SCREEN_WIDTH,
                height: SCREEN_HEIGHT,
            },
            frame: Frame {
                pixels: Box::new([0; SCREEN_HEIGHT * SCREEN_WIDTH]),
                width: SCREEN_WIDTH,
                height: SCREEN_HEIGHT,
            },
            frame_ready: false,
        }
    }

    pub fn skip_boot(&mut self) {
        // https://github.com/michelhe/rustboyadvance-ng/blob/master/core/src/gpu/mod.rs
        self.bg2_affine_parameters.registers[0] = 0x100;
        self.bg2_affine_parameters.registers[3] = 0x100;
        self.bg3_affine_parameters.registers[0] = 0x100;
        self.bg3_affine_parameters.registers[3] = 0x100;
    }

    pub fn reset_registers(&mut self) {
        self.dispcnt = 0x0080; // https://problemkaputt.de/gbatek-bios-reset-functions.htm
        self.bg_control = GroupedRegisters::new(4, 0x4000008);
        self.bg_text_offset = GroupedRegisters::new(8, 0x4000010);
        self.bg2_affine_parameters = GroupedRegisters::new(4, 0x4000020);
        self.bg2_affine_reference = GroupedRegisters::new(2, 0x4000028);
        self.bg2_affine_state = AffineState::default();
        self.bg3_affine_parameters = GroupedRegisters::new(4, 0x4000030);
        self.bg3_affine_reference = GroupedRegisters::new(2, 0x4000038);
        self.bg3_affine_state = AffineState::default();
        self.window_features = GroupedRegisters::new(6, 0x4000040);
        self.mosaic = 0;
        self.color_special_effects = GroupedRegisters::new(3, 0x4000050);

        self.skip_boot();
    }

    pub fn write_dispcnt(&mut self, value: u16) {
        self.dispcnt = value;

        self.dispcnt.clear_bit(3);
    }

    pub fn write_dispstat(&mut self, mut value: u16) {
        self.dispstat.clear_bit_range(3..16);
        value.clear_bit_range(0..3);

        self.dispstat |= value;
    }

    pub fn handle_hblank(&mut self, interrupt_flag: &mut u16) -> Option<Trigger> {
        if take(&mut self.bg2_affine_state.reload) {
            self.bg2_affine_state.reload_internal_reference();
        }

        if take(&mut self.bg3_affine_state.reload) {
            self.bg3_affine_state.reload_internal_reference();
        }

        self.dispstat.set_bit(DispstatBit::HblankFlag as usize);

        self.set_interrupt(DispstatBit::HblankInterrupt, interrupt_flag);

        if self.vcount < 160 {
            self.render_scanline();

            Some(Trigger::Hblank)
        } else {
            self.bg2_affine_state.reload = true;
            self.bg3_affine_state.reload = true;

            None
        }
    }

    // ***still needs, sprite, window, and the special effects
    // probably not the cleanest implementation, refactor after ppu produces
    // visuals reasonably close to what commercial roms are supposed to look like
    fn render_scanline(&mut self) {
        let bg2_matrix = AffineMatrix::from_registers(
            self.bg2_affine_parameters.from_index(0),
            self.bg2_affine_parameters.from_index(1),
            self.bg2_affine_parameters.from_index(2),
            self.bg2_affine_parameters.from_index(3),
        );
        let bg3_matrix = AffineMatrix::from_registers(
            self.bg3_affine_parameters.from_index(0),
            self.bg3_affine_parameters.from_index(1),
            self.bg3_affine_parameters.from_index(2),
            self.bg3_affine_parameters.from_index(3),
        );

        if self.dispcnt.is_set(7) {
            let row = self.vcount as usize * SCREEN_WIDTH;
            self.frame.pixels[row..row + SCREEN_WIDTH].fill(0x7FFF);

            self.bg2_affine_state
                .increment_internal_reference(bg2_matrix);
            self.bg3_affine_state
                .increment_internal_reference(bg3_matrix);

            return;
        }

        let mut bg_lines: [BgLine; 4] = std::array::from_fn(|i| BgLine::new(i, false, 0));

        let mode = self.current_mode();
        match mode {
            0..=2 => {
                for bg_id in 0..4 {
                    let priority = self.bg_control.from_index(bg_id).get_bit_range(0..2) as u8;
                    bg_lines[bg_id] = BgLine::new(bg_id, self.dispcnt.is_set(8 + bg_id), priority);
                    if !bg_lines[bg_id].on {
                        continue;
                    }

                    if self.current_mode() == 0 {
                        self.populate_bg_array(&mut bg_lines[bg_id], RenderType::Text, None);
                    } else if self.current_mode() == 1 {
                        if bg_id > 2 {
                            continue;
                        }

                        if bg_id != 2 {
                            self.populate_bg_array(&mut bg_lines[bg_id], RenderType::Text, None);
                        } else {
                            self.populate_bg_array(
                                &mut bg_lines[bg_id],
                                RenderType::Affine,
                                Some(bg2_matrix),
                            );
                        }
                    } else {
                        if bg_id < 2 {
                            continue;
                        }

                        if bg_id == 2 {
                            self.populate_bg_array(
                                &mut bg_lines[bg_id],
                                RenderType::Affine,
                                Some(bg2_matrix),
                            );
                        } else {
                            self.populate_bg_array(
                                &mut bg_lines[bg_id],
                                RenderType::Affine,
                                Some(bg3_matrix),
                            );
                        }
                    }
                }
            }
            3..=5 => {
                let priority = self.bg_control.from_index(2).get_bit_range(0..2) as u8;
                bg_lines[2] = BgLine::new(2, self.dispcnt.is_set(10), priority);
                if bg_lines[2].on {
                    self.populate_bg_array(&mut bg_lines[2], RenderType::Bitmap, Some(bg2_matrix));
                }
            }
            _ => {}
        }

        let (sprite_line, obj_window) = if self.dispcnt.is_set(12) {
            self.render_sprite_line()
        } else {
            (std::array::from_fn(|_| None), [false; 240])
        };
        //let window_mask = self.build_window_mask();

        self.composite(&bg_lines, &sprite_line, [0xFF; 240]);

        self.bg2_affine_state
            .increment_internal_reference(bg2_matrix);
        self.bg3_affine_state
            .increment_internal_reference(bg3_matrix);
    }

    fn populate_bg_array(
        &mut self,
        bg_line: &mut BgLine,
        mode_type: RenderType,
        matrix: Option<AffineMatrix>,
    ) {
        for pixel in 0..SCREEN_WIDTH {
            let index = match mode_type {
                RenderType::Text => self.get_text_bg_palette_index(bg_line.id, pixel),
                RenderType::Affine => {
                    self.get_affine_bg_palette_index(bg_line.id, pixel, matrix.unwrap())
                }
                RenderType::Bitmap => self.get_bitmap_palette_index(
                    get_bitmap_mode_params(self.current_mode()),
                    pixel,
                    matrix.unwrap(),
                ),
            };

            bg_line.palette_indices[pixel] = index
        }
    }

    fn composite(
        &mut self,
        bg_lines: &[BgLine; 4],
        sprite_line: &[Option<SpritePixel>; 240],
        window_mask: [u8; 240],
    ) {
        let backdrop = u16::from_le_bytes([self.palette_ram[0], self.palette_ram[1]]);
        for pixel in 0..SCREEN_WIDTH {
            let mut first = Pixel {
                id: LayerId::Backdrop,
                priority: 4,
                color: backdrop,
            };
            let mut second = first;

            let mask = window_mask[pixel];

            for bg in bg_lines {
                if !bg.on || mask & (1 << bg.id) == 0 {
                    continue;
                }

                let Some(index) = bg.palette_indices[pixel] else {
                    continue;
                };
                let layer_id = LayerId::from_background(bg.id);
                let color = self.fetch_color(index, layer_id);
                let candidate = Pixel {
                    id: layer_id,
                    priority: bg.priority,
                    color,
                };

                if candidate.priority < first.priority {
                    second = first;
                    first = candidate;
                } else if candidate.priority < second.priority {
                    second = candidate;
                }
            }

            if mask & (1 << 4) != 0 {
                if let Some(sprite_pixel) = &sprite_line[pixel] {
                    let candidate = Pixel {
                        id: LayerId::Sprite,
                        priority: sprite_pixel.priority,
                        color: self.fetch_color(sprite_pixel.palette_index + 256, LayerId::Sprite),
                    };

                    if candidate.priority <= first.priority {
                        second = first;
                        first = candidate;
                    } else if candidate.priority <= second.priority {
                        second = candidate;
                    }
                }
            }

            // self.frame.pixels[self.vcount as usize * SCREEN_WIDTH + pixel]  = if mask & (1 << 5) != 0 {apply_effects(first, second, semi_transparent)} else {first.color}

            self.frame.pixels[self.vcount as usize * SCREEN_WIDTH + pixel] = first.color;
        }
    }

    fn direct_color(&self, layer_id: LayerId) -> bool {
        matches!(self.current_mode(), 3 | 5) && layer_id == LayerId::Bg2
    }

    fn get_text_bg_palette_index(&self, bg_id: usize, lcd_pixel_x: usize) -> Option<usize> {
        let control = self.bg_control.from_index(bg_id);
        let character_base_block = control.get_bit_range(2..4) as usize;
        let screen_base_block = control.get_bit_range(8..13) as usize;
        let bpp = if control.is_set(7) {
            Bpp::EigthBpp
        } else {
            Bpp::FourBpp
        };

        let lcd = Coordinate {
            x: lcd_pixel_x,
            y: self.vcount as usize,
        };
        let bg_screen_size = self.text_bg_screen_size(bg_id);
        let bg_scroll = Coordinate {
            x: self.bg_text_offset.from_index(bg_id * 2) as usize,
            y: self.bg_text_offset.from_index((bg_id * 2) + 1) as usize,
        };

        let bg_map_pixel = Coordinate {
            x: (lcd.x + bg_scroll.x) % bg_screen_size.x,
            y: (lcd.y + bg_scroll.y) % bg_screen_size.y,
        };

        let tile = Coordinate {
            x: bg_map_pixel.x / 8,
            y: bg_map_pixel.y / 8,
        };

        let mut pixel_inside_tile = Coordinate {
            x: bg_map_pixel.x % 8,
            y: bg_map_pixel.y % 8,
        };

        let blocks_per_row = (bg_screen_size.x / 8) / 32;
        let screen_block_offset = (tile.y / 32) * blocks_per_row + (tile.x / 32);
        let screen_block_vram_index = (screen_base_block + screen_block_offset) * 0x800
            + ((tile.y % 32) * 32 + (tile.x % 32)) * 2;

        let tile_attributes = u16::from_le_bytes([
            self.vram[screen_block_vram_index],
            self.vram[screen_block_vram_index + 1],
        ]);
        let tileset_offset = tile_attributes.get_bit_range(0..10) as usize;
        pixel_inside_tile.x = if tile_attributes.is_set(10) {
            7 - pixel_inside_tile.x
        } else {
            pixel_inside_tile.x
        };
        pixel_inside_tile.y = if tile_attributes.is_set(11) {
            7 - pixel_inside_tile.y
        } else {
            pixel_inside_tile.y
        };
        let palette_bank = tile_attributes.get_bit_range(12..16) as usize;

        let palette_index = self.read_tile_pixel_palette_index(
            character_base_block,
            tileset_offset,
            pixel_inside_tile,
            bpp,
            palette_bank,
        );

        if palette_index == 0 {
            None
        } else {
            Some(palette_index)
        }
    }

    fn get_affine_bg_palette_index(
        &self,
        bg_id: usize,
        lcd_pixel_x: usize,
        matrix: AffineMatrix,
    ) -> Option<usize> {
        let control = self.bg_control.from_index(bg_id);
        let character_base_block = control.get_bit_range(2..4) as usize;
        let screen_base_block = control.get_bit_range(8..13) as usize;
        let wrap = control.is_set(13);
        let screen_size = [128, 256, 512, 1024][control.get_bit_range(14..16) as usize] as i32;
        let affine_state = if bg_id == 2 {
            &self.bg2_affine_state
        } else {
            &self.bg3_affine_state
        };

        let mut affine_map_pixel_x = affine_state
            .internal_reference
            .x
            .transformed_pixel(matrix.pa, lcd_pixel_x);
        let mut affine_map_pixel_y = affine_state
            .internal_reference
            .y
            .transformed_pixel(matrix.pc, lcd_pixel_x);

        if wrap {
            affine_map_pixel_x = affine_map_pixel_x.rem_euclid(screen_size);
            affine_map_pixel_y = affine_map_pixel_y.rem_euclid(screen_size);
        } else if affine_map_pixel_x < 0
            || affine_map_pixel_x >= screen_size
            || affine_map_pixel_y < 0
            || affine_map_pixel_y >= screen_size
        {
            return None;
        }

        let (affine_map_pixel_x, affine_map_pixel_y) =
            (affine_map_pixel_x as usize, affine_map_pixel_y as usize);

        let tiles_per_side = screen_size as usize / 8;
        let tileset_offset = self.vram[screen_base_block * 0x800
            + (affine_map_pixel_y / 8) * tiles_per_side
            + affine_map_pixel_x / 8] as usize;
        let index = self.read_tile_pixel_palette_index(
            character_base_block,
            tileset_offset,
            Coordinate {
                x: affine_map_pixel_x % 8,
                y: affine_map_pixel_y % 8,
            },
            Bpp::EigthBpp,
            0,
        );

        if index == 0 { None } else { Some(index) }
    }

    fn read_tile_pixel_palette_index(
        &self,
        character_base_block: usize,
        tileset_offset: usize,
        pixel_inside_tile: Coordinate,
        bpp: Bpp,
        palette_bank: usize,
    ) -> usize {
        match bpp {
            Bpp::EigthBpp => {
                self.vram[character_base_block * 0x4000
                    + tileset_offset * 64
                    + pixel_inside_tile.y * 8
                    + pixel_inside_tile.x] as usize
            }
            Bpp::FourBpp => {
                let byte = self.vram[character_base_block * 0x4000
                    + tileset_offset * 32
                    + pixel_inside_tile.y * 4
                    + pixel_inside_tile.x / 2];
                let nibble = if pixel_inside_tile.x % 2 == 0 {
                    byte & 0x0F
                } else {
                    byte >> 4
                };
                if nibble == 0 {
                    0
                } else {
                    palette_bank * 16 + nibble as usize
                }
            }
        }
    }

    fn text_bg_screen_size(&self, bg_id: usize) -> Coordinate {
        match self.bg_control.from_index(bg_id).get_bit_range(14..16) {
            0 => Coordinate { x: 256, y: 256 },
            1 => Coordinate { x: 512, y: 256 },
            2 => Coordinate { x: 256, y: 512 },
            _ => Coordinate { x: 512, y: 512 },
        }
    }

    fn fetch_color(&self, palette_index: usize, layer_id: LayerId) -> u16 {
        if self.direct_color(layer_id) {
            u16::from_le_bytes([self.vram[palette_index], self.vram[palette_index + 1]])
        } else {
            u16::from_le_bytes([
                self.palette_ram[palette_index * 2],
                self.palette_ram[palette_index * 2 + 1],
            ])
        }
    }

    fn get_bitmap_palette_index(
        &self,
        bitmap_mode_params: BitmapModeParams,
        pixel: usize,
        matrix: AffineMatrix,
    ) -> Option<usize> {
        let affine_x = self
            .bg2_affine_state
            .internal_reference
            .x
            .transformed_pixel(matrix.pa, pixel);
        let affine_y = self
            .bg2_affine_state
            .internal_reference
            .y
            .transformed_pixel(matrix.pc, pixel);

        let width = bitmap_mode_params.width as i32;
        let height = bitmap_mode_params.height as i32;

        if affine_x < 0 || affine_x >= width || affine_y < 0 || affine_y >= height {
            return None;
        }

        let linear_index =
            affine_y as usize * bitmap_mode_params.width as usize + affine_x as usize;

        let page = self.dispcnt.get_bit(4) as usize;
        let page_offset = if bitmap_mode_params.page_flip {
            page * 0xA000
        } else {
            0
        };

        if bitmap_mode_params.bpp == 16 {
            Some(page_offset + linear_index * 2)
        } else {
            let index = page_offset + linear_index;
            let value = self.vram[index];
            if value == 0 {
                None
            } else {
                Some(value as usize)
            }
        }
    }

    // skip the sprite maximum based on size and cycle budget and put all the sprites on screen
    pub fn render_sprite_line(&self) -> ([Option<SpritePixel>; 240], [bool; 240]) {
        let mut sprite_line: [Option<SpritePixel>; 240] = std::array::from_fn(|_| None);
        let mut obj_window = [false; 240];

        for sprite_id in 0..128 {
            let sprite = SpriteAttributes::from_bytes(sprite_id, &self.oam);

            if self.current_mode() >= 3 && sprite.tile < 512 {
                continue;
            }

            let Some(row) = sprite.visible_row(self.vcount) else {
                continue;
            };

            for col in 0..sprite.bounding_box.width {
                let pixel = sprite.coordinate.x + col;
                if pixel < 0 || pixel >= SCREEN_WIDTH as i32 {
                    continue;
                }

                let (texture_x, texture_y) = match sprite.matrix {
                    Some(matrix) => {
                        let dx = col - sprite.bounding_box.width / 2;
                        let dy = row - sprite.bounding_box.height / 2;

                        let texture_x = ((matrix.pa.raw() * dx + matrix.pb.raw() * dy) >> 8)
                            + sprite.dimension.width as i32 / 2;
                        let texture_y = ((matrix.pc.raw() * dx + matrix.pd.raw() * dy) >> 8)
                            + sprite.dimension.height as i32 / 2;

                        (texture_x, texture_y)
                    }
                    None => {
                        let texture_x = if sprite.horizontal_flip {
                            sprite.dimension.width - 1 - col
                        } else {
                            col
                        };
                        let texture_y = if sprite.vertical_flip {
                            sprite.dimension.height - 1 - row
                        } else {
                            row
                        };

                        (texture_x, texture_y)
                    }
                };

                if texture_x < 0
                    || texture_x >= sprite.dimension.width as i32
                    || texture_y < 0
                    || texture_y >= sprite.dimension.height as i32
                {
                    continue;
                }

                let tile = Coordinate {
                    x: texture_x as usize / 8,
                    y: texture_y as usize / 8,
                };
                let pixel_inside_tile = Coordinate {
                    x: texture_x as usize % 8,
                    y: texture_y as usize % 8,
                };

                let tiles_wide = sprite.dimension.width as usize / 8;
                let step = if sprite.bpp == Bpp::EigthBpp { 2 } else { 1 };

                let tile_number = if self.dispcnt.is_set(6) {
                    sprite.tile + (tile.y * tiles_wide + tile.x) * step
                } else {
                    sprite.tile + tile.y * 32 + tile.x * step
                } & 0x3FF;

                let tile_number = if sprite.bpp == Bpp::EigthBpp {
                    tile_number / 2
                } else {
                    tile_number
                };

                let palette_index = self.read_tile_pixel_palette_index(
                    4,
                    tile_number,
                    pixel_inside_tile,
                    sprite.bpp,
                    sprite.palette_bank,
                );

                if palette_index == 0 {
                    continue;
                }

                if sprite.mode == SpriteMode::ObjWindow {
                    if palette_index != 0 {
                        obj_window[pixel as usize] = true;
                    }

                    continue;
                }

                if let Some(old_sprite_pixel) = &sprite_line[pixel as usize] {
                    if old_sprite_pixel.priority <= sprite.priority {
                        continue;
                    }
                }

                sprite_line[pixel as usize] = Some(SpritePixel {
                    palette_index,
                    priority: sprite.priority,
                    semi_transparent: sprite.mode == SpriteMode::Semitransparent,
                });
            }
        }

        (sprite_line, obj_window)
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
            std::mem::swap(&mut self.frame, &mut self.frontend);
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

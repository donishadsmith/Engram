use crate::components::{
    ppu::{AffineMatrix, Bpp},
    utils::BitOps,
};

const SPRITE_DIMENSIONS: [[(usize, usize); 4]; 3] = [
    [(8, 8), (16, 16), (32, 32), (64, 64)],
    [(16, 8), (32, 8), (32, 16), (64, 32)],
    [(8, 16), (8, 32), (16, 32), (32, 64)],
];

pub struct SpritePixel {
    pub palette_index: usize,
    pub priority: u8,
    pub semi_transparent: bool,
}

#[derive(Clone, Copy, PartialEq, Eq)]
pub enum SpriteMode {
    Normal,
    Semitransparent,
    ObjWindow,
    Prohibited,
}

impl SpriteMode {
    fn from_value(value: u16) -> SpriteMode {
        match value {
            0 => SpriteMode::Normal,
            1 => SpriteMode::Semitransparent,
            2 => SpriteMode::ObjWindow,
            _ => SpriteMode::Prohibited,
        }
    }
}

pub struct SpriteCoordinate {
    pub x: i32,
    pub y: i32,
}

#[derive(Clone, Copy)]
pub struct SpriteDimension {
    pub width: i32,
    pub height: i32,
}

pub struct SpriteAttributes {
    pub coordinate: SpriteCoordinate,
    pub disabled: bool,
    pub mode: SpriteMode,
    pub horizontal_flip: bool,
    pub vertical_flip: bool,
    pub bpp: Bpp,
    pub matrix: Option<AffineMatrix>,
    pub dimension: SpriteDimension,
    pub tile: usize,
    pub priority: u8,
    pub palette_bank: usize,
    pub bounding_box: SpriteDimension,
}

impl SpriteAttributes {
    pub fn from_bytes(sprite_id: usize, oam: &Box<[u8; 1024]>) -> Self {
        let (attribute0, attribute1, attribute2) = create_attribute_halfwords(sprite_id, oam);

        let affine = attribute0.is_set(8);

        let x = attribute1.get_bit_range(0..9) as i32;
        let y = attribute0.get_bit_range(0..8) as i32;
        let coordinate = SpriteCoordinate {
            x: if x >= 240 { x - 512 } else { x },
            y: if y >= 160 { y - 256 } else { y },
        };
        let double_size = affine && attribute0.is_set(9);
        let disabled = !affine && attribute0.is_set(9);
        let mode = SpriteMode::from_value(attribute0.get_bit_range(10..12));
        let horizontal_flip = !affine && attribute1.is_set(12);
        let vertical_flip = !affine && attribute1.is_set(13);
        let bpp = if attribute0.is_set(13) {
            Bpp::EigthBpp
        } else {
            Bpp::FourBpp
        };
        let shape = attribute0.get_bit_range(14..16) as usize;
        let size = attribute1.get_bit_range(14..16) as usize;
        let dimension = SPRITE_DIMENSIONS[shape][size];
        let dimension = SpriteDimension {
            width: dimension.0 as i32,
            height: dimension.1 as i32,
        };
        let tile = attribute2.get_bit_range(0..10) as usize;
        let priority = attribute2.get_bit_range(10..12) as u8;
        let palette_bank = attribute2.get_bit_range(12..16) as usize;

        let matrix = if affine {
            Some(create_sprite_affine(
                attribute1.get_bit_range(9..14) as usize,
                oam,
            ))
        } else {
            None
        };

        let bounding_box = if double_size {
            SpriteDimension {
                width: dimension.width * 2,
                height: dimension.height * 2,
            }
        } else {
            dimension
        };

        Self {
            coordinate,
            disabled,
            mode,
            horizontal_flip,
            vertical_flip,
            bpp,
            matrix,
            dimension,
            tile,
            priority,
            palette_bank,
            bounding_box,
        }
    }

    pub fn visible_row(&self, vcount: u8) -> Option<i32> {
        if self.disabled || self.mode == SpriteMode::Prohibited {
            return None;
        }

        if self.coordinate.x + self.bounding_box.width <= 0 || self.coordinate.x >= 240 {
            return None;
        }

        let row = vcount as i32 - self.coordinate.y;

        (row >= 0 && row < self.bounding_box.height).then_some(row)
    }
}

fn create_halfword(offset: usize, oam: &Box<[u8; 1024]>) -> u16 {
    u16::from_le_bytes([oam[offset], oam[offset + 1]])
}

fn create_attribute_halfwords(sprite_id: usize, oam: &Box<[u8; 1024]>) -> (u16, u16, u16) {
    let start_index = sprite_id * 8;
    let attributes = [
        create_halfword(start_index, oam),
        create_halfword(start_index + 2, oam),
        create_halfword(start_index + 4, oam),
    ];

    (attributes[0], attributes[1], attributes[2])
}

fn create_sprite_affine(group: usize, oam: &Box<[u8; 1024]>) -> AffineMatrix {
    let pa = create_halfword(group * 32 + 6, oam);
    let pb = create_halfword(group * 32 + 14, oam);
    let pc = create_halfword(group * 32 + 22, oam);
    let pd = create_halfword(group * 32 + 30, oam);

    AffineMatrix::from_registers(pa, pb, pc, pd)
}

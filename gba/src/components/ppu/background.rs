use crate::components::ppu::affine::Fixed8Fractional;
use crate::components::utils::BitOps;

// i may need to rethink some aspects of this design
#[derive(Clone, Copy, PartialEq, Eq)]
pub enum TileGridType {
    Text,
    Affine,
}

impl TileGridType {
    pub fn from_mode_and_bg(mode: u8, bg_id: u8) -> TileGridType {
        match mode {
            0 => TileGridType::Text,
            1 => match bg_id {
                0 | 1 => TileGridType::Text,
                2 => TileGridType::Affine,
                _ => unreachable!(),
            },
            2 if (2..4).contains(&bg_id) => TileGridType::Affine,
            _ => unreachable!(),
        }
    }

    pub fn to_reference_coordinate(self, reference: (u32, u32)) -> ReferenceCoordinate {
        match self {
            TileGridType::Text => ReferenceCoordinate::Text {
                x: reference.0 as u16,
                y: reference.1 as u16,
            },
            TileGridType::Affine => ReferenceCoordinate::Affine {
                x: Fixed8Fractional::from_reference(reference.0),
                y: Fixed8Fractional::from_reference(reference.1),
            },
        }
    }

    pub fn to_screen_size(self, value: u8) -> ScreenSize {
        match self {
            TileGridType::Text => ScreenSize::Text(value),
            TileGridType::Affine => ScreenSize::Affine(value),
        }
    }
}

#[derive(Clone, Copy, PartialEq, Eq)]
pub enum ScreenSize {
    Text(u8),
    Affine(u8),
}

impl ScreenSize {
    pub fn dimensions(self) -> (u16, u16) {
        match self {
            ScreenSize::Text(value) => match value {
                0 => (256, 256),
                1 => (512, 256),
                2 => (256, 512),
                3 => (512, 512),
                _ => unreachable!(),
            },
            ScreenSize::Affine(value) => match value {
                0 => (128, 128),
                1 => (256, 256),
                2 => (512, 512),
                3 => (1024, 1024),
                _ => unreachable!(),
            },
        }
    }
}

#[derive(Clone, Copy, PartialEq, Eq)]
pub struct ScreenDataFormat {
    pub tile_number: u16,
    pub horizontal_flip: bool,
    pub vertical_flip: bool,
    pub palette_number: Option<u8>,
}

impl ScreenDataFormat {
    pub fn from_halfword(value: u16, bpp: Bpp) -> Self {
        let tile_number = value.get_bit_range(0..10);
        let horizontal_flip = value.is_set(10);
        let vertical_flip = value.is_set(11);
        let palette_number = if bpp == Bpp::Eightbpp {
            None
        } else {
            Some(value.get_bit_range(12..16) as u8)
        };

        Self {
            tile_number,
            horizontal_flip,
            vertical_flip,
            palette_number,
        }
    }
}

#[derive(Clone, Copy, PartialEq, Eq)]
pub enum ReferenceCoordinate {
    Text {
        x: u16,
        y: u16,
    },
    Affine {
        x: Fixed8Fractional,
        y: Fixed8Fractional,
    },
}

#[derive(Clone, Copy, PartialEq, Eq)]
pub enum Bpp {
    Fourbpp,
    Eightbpp,
}

#[derive(Clone, Copy, PartialEq, Eq)]
pub enum Overflow {
    Transparent,
    Wraparound,
}

#[derive(Clone, Copy, PartialEq, Eq)]
pub struct BackgroundParams {
    pub id: u8,
    pub priority: u8,
    pub bpp: Bpp,
    pub character_base_block: u8,
    pub mosaic: bool,
    pub screen_base_block: u8,
    pub screen_size: ScreenSize,
    pub reference_coordinate: ReferenceCoordinate,
    pub overflow: Option<Overflow>,
}

impl BackgroundParams {
    pub fn new(
        id: u8,
        control: u16,
        screen_size: ScreenSize,
        reference_coordinate: ReferenceCoordinate,
    ) -> Self {
        let priority = control.get_bit_range(0..2) as u8;
        let character_base_block = control.get_bit_range(2..4) as u8;
        let bpp = if control.is_set(7) {
            Bpp::Eightbpp
        } else {
            Bpp::Fourbpp
        };
        let mosaic = control.is_set(6);
        let screen_base_block = control.get_bit_range(8..13) as u8;
        let overflow = if !(2..4).contains(&id) {
            None
        } else if control.is_set(13) {
            Some(Overflow::Wraparound)
        } else {
            Some(Overflow::Transparent)
        };

        Self {
            id,
            priority,
            bpp,
            character_base_block,
            mosaic,
            screen_base_block,
            screen_size,
            reference_coordinate,
            overflow,
        }
    }
}

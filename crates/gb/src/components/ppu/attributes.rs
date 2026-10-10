use shared::traits::BitOps;

pub struct ColorBackgroundAttributes {
    pub priority: bool,
    pub y_flip: bool,
    pub x_flip: bool,
    pub bank: usize,
    pub color_palette: u8,
}

impl ColorBackgroundAttributes {
    pub fn from_byte(byte: u8) -> Self {
        Self {
            priority: byte.is_set(7),
            y_flip: byte.is_set(6),
            x_flip: byte.is_set(5),
            bank: byte.get_bit(3) as usize,
            color_palette: byte.get_bit_range(0..3),
        }
    }
}

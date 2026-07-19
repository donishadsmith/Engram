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
            priority: (byte >> 7) & 0x01 == 1,
            y_flip: byte & 0x40 != 0,
            x_flip: byte & 0x20 != 0,
            bank: ((byte & 0x08) >> 3) as usize,
            color_palette: byte & 0x07,
        }
    }
}

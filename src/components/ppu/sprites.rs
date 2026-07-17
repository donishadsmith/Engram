pub struct SpriteAttribute {
    pub bank: usize,
    pub oam_index: usize,
    pub position_y: i16,
    pub position_x: i16,
    pub tile_index: u8,
    pub priority: bool,
    pub flip_y: bool,
    pub flip_x: bool,
    pub palette_number: u8,
}

impl SpriteAttribute {
    pub fn from_oam(oam_index: usize, bytes: &[u8], is_cgb: bool) -> Self {
        Self {
            bank: if is_cgb {
                ((bytes[3] & 0x08) >> 3) as usize
            } else {
                0
            },
            oam_index,
            position_y: bytes[0] as i16 - 16,
            position_x: bytes[1] as i16 - 8,
            tile_index: bytes[2],
            priority: (bytes[3] & 0x80) == 0,
            flip_y: (bytes[3] & 0x40) != 0,
            flip_x: (bytes[3] & 0x20) != 0,
            palette_number: if !is_cgb {
                bytes[3] & 0x10
            } else {
                bytes[3] & 0x07
            },
        }
    }
}

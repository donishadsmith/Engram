pub const DMG_SHADES: [u16; 4] = [0x7FFF, 0x56B5, 0x294A, 0x0000];

#[derive(Clone, Copy)]
pub enum ColorPaletteRegisterType {
    Background,
    Object,
}

pub fn cram_color(palette_ram: &[u8; 64], palette: u8, color_index: u8) -> u16 {
    let base = palette as usize * 8 + color_index as usize * 2;
    let color_data_low = palette_ram[base] as u16;
    let color_data_high = (palette_ram[base + 1] as u16) << 8;

    color_data_high | color_data_low
}

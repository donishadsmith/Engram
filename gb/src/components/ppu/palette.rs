// https://github.com/langurmonkey/playkid/blob/master/src/ppu.rs
// [224, 248, 208, 136, 192, 112, 52, 104, 86, 8, 24, 32],
// TODO: changed form black and white to the green palette to look better
// eventually attempt colorization of gb but the process looks far too cumbersome
// maybe attempt after psx
// https://gbdev.io/pandocs/Power_Up_Sequence.html#compatibility-palettes
pub const DMG_SHADES: [u32; 4] = [0xE0F8D0, 0x88C070, 0x346856, 0x081820];

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

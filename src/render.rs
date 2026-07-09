use crate::components::ppu::ppu::{PPU, SCREEN_HEIGHT, SCREEN_WIDTH};
use macroquad::prelude::*;

use macroquad::prelude::Color;

const DMG_PALETTE: [Color; 4] = [
    Color::new(1.00, 1.00, 1.00, 1.0),
    Color::new(0.83, 0.83, 0.83, 1.0),
    Color::new(0.66, 0.66, 0.66, 1.0),
    Color::new(0.00, 0.00, 0.00, 1.0),
];

fn scale() -> (f32, f32) {
    let scale_w = screen_width() / SCREEN_WIDTH as f32;
    let scale_h = screen_height() / SCREEN_HEIGHT as f32;

    (scale_w.min(scale_h), scale_h)
}

pub fn render_to_window(ppu: &PPU) {
    if !ppu.frame_ready {
        return;
    }

    let (scale_w, scale_h) = scale();

    for y in 0..SCREEN_HEIGHT {
        for x in 0..SCREEN_WIDTH {
            let color = DMG_PALETTE[ppu.viewport[y][x] as usize];
            draw_rectangle(
                x as f32 * scale_w + (screen_width() * 0.08),
                y as f32 * scale_h,
                scale_w,
                scale_h,
                color,
            )
        }
    }
}

use crate::components::ppu::ppu::{SCREEN_HEIGHT, SCREEN_WIDTH};
use macroquad::prelude::*;

fn scale() -> (f32, f32) {
    let scale_w = screen_width() / SCREEN_WIDTH as f32;
    let scale_h = screen_height() / SCREEN_HEIGHT as f32;

    (scale_w.min(scale_h), scale_h)
}

fn render_to_window() {}

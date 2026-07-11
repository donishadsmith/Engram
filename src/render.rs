use crate::components::ppu::{PPU, SCREEN_HEIGHT, SCREEN_WIDTH};
use macroquad::prelude::*;

const DMG_PALETTE: [[u8; 3]; 4] = [[255, 255, 255], [212, 212, 212], [168, 168, 168], [0, 0, 0]];

const BYTES_PER_PIXEL: usize = 4;

const BLEND_CURRENT: u16 = 6;
const BLEND_PREVIOUS: u16 = 4;
const BLEND_TOTAL: u16 = BLEND_CURRENT + BLEND_PREVIOUS;

pub struct Screen {
    texture: Texture2D,
    image: Image,
    previous_frame: [[u8; SCREEN_WIDTH]; SCREEN_HEIGHT],
}

impl Screen {
    pub fn new() -> Self {
        let image = Image {
            bytes: vec![0u8; SCREEN_WIDTH * SCREEN_HEIGHT * BYTES_PER_PIXEL],
            width: SCREEN_WIDTH as u16,
            height: SCREEN_HEIGHT as u16,
        };
        let texture = Texture2D::from_image(&image);
        texture.set_filter(FilterMode::Nearest);
        Self {
            texture,
            image,
            previous_frame: [[0; SCREEN_WIDTH]; SCREEN_HEIGHT],
        }
    }

    pub fn update(&mut self, ppu: &PPU) {
        for y in 0..SCREEN_HEIGHT {
            for x in 0..SCREEN_WIDTH {
                let current_frame_pixel = DMG_PALETTE[ppu.viewport[y][x] as usize];
                let previous_frame_pixel = DMG_PALETTE[self.previous_frame[y][x] as usize];

                let index = (y * SCREEN_WIDTH + x) * BYTES_PER_PIXEL;

                for channel in 0..3 {
                    let blended_pixel = (current_frame_pixel[channel] as u16 * BLEND_CURRENT
                        + previous_frame_pixel[channel] as u16 * BLEND_PREVIOUS)
                        / BLEND_TOTAL;
                    self.image.bytes[index + channel] = blended_pixel as u8;
                }

                self.image.bytes[index + 3] = 255;
            }
        }

        self.previous_frame = ppu.viewport;
        self.texture.update(&self.image);
    }

    pub fn draw(&self) {
        let scale = (screen_width() / SCREEN_WIDTH as f32)
            .min(screen_height() / SCREEN_HEIGHT as f32)
            .floor()
            .max(1.0);

        let (width, height) = (SCREEN_WIDTH as f32 * scale, SCREEN_HEIGHT as f32 * scale);

        draw_texture_ex(
            &self.texture,
            (screen_width() - width) / 2.0,
            (screen_height() - height) / 2.0,
            WHITE,
            DrawTextureParams {
                dest_size: Some(vec2(width, height)),
                ..Default::default()
            },
        );
    }
}

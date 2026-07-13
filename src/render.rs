use crate::components::ppu::{PPU, SCREEN_HEIGHT, SCREEN_WIDTH};
use macroquad::prelude::*;

const DEBUGGING_COLOR: [u8; 3] = [238, 75, 43];
const DMG_PALETTE: [[u8; 3]; 5] = [
    [255, 255, 255],
    [212, 212, 212],
    [168, 168, 168],
    [0, 0, 0],
    DEBUGGING_COLOR,
];

const BYTES_PER_PIXEL: usize = 4;

pub struct Screen {
    texture: Texture2D,
    image: Image,
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
        Self { texture, image }
    }

    pub fn update(&mut self, ppu: &PPU) {
        for y in 0..SCREEN_HEIGHT {
            for x in 0..SCREEN_WIDTH {
                let color = DMG_PALETTE[ppu.viewport[y][x] as usize];
                let index = (y * SCREEN_WIDTH + x) * BYTES_PER_PIXEL;
                self.image.bytes[index..index + 3].copy_from_slice(&color);
                self.image.bytes[index + 3] = 255;
            }
        }

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

fn rgb555_to_rgb888(rgb555: u16) -> [u8; 3] {
    let expand = |v: u16| -> u8 { ((v << 3) | (v >> 2)) as u8 };
    [
        expand(rgb555 & 0x1F),
        expand((rgb555 >> 5) & 0x1F),
        expand((rgb555 >> 10) & 0x1F),
    ]
}

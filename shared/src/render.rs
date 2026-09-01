use macroquad::prelude::*;

const RGBA_BYTES_PER_PIXEL: usize = 4;

pub struct Frame {
    pub pixels: Box<[u16]>,
    pub width: usize,
    pub height: usize,
}

pub struct Screen {
    texture: Texture2D,
    image: Image,
}

impl Screen {
    pub fn new(width: usize, height: usize) -> Self {
        let image = Image {
            bytes: vec![0; width * height * RGBA_BYTES_PER_PIXEL],
            width: width as u16,
            height: height as u16,
        };

        let texture = Texture2D::from_image(&image);
        texture.set_filter(FilterMode::Nearest);

        Self { texture, image }
    }

    pub fn update(&mut self, frame: &Frame) {
        for (pixel, out) in frame
            .pixels
            .iter()
            .zip(self.image.bytes.chunks_exact_mut(RGBA_BYTES_PER_PIXEL))
        {
            let [r, g, b] = rgb555_to_rgb888(*pixel);
            out.copy_from_slice(&[r, g, b, 255]);
        }

        self.texture.update(&self.image);
    }

    pub fn draw(&self, frame: &Frame) {
        let scale = (screen_width() / frame.width as f32)
            .min(screen_height() / frame.height as f32)
            .floor()
            .max(1.0);

        let (width, height) = (frame.width as f32 * scale, frame.height as f32 * scale);

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

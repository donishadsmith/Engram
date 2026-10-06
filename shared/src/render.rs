use macroquad::prelude::*;
use std::mem::swap;

use crate::traits::BitOps;

const RGBA_BYTES_PER_PIXEL: usize = 4;

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum PixelFormat {
    Rgb555,
    Rgb888,
}

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum ScalingMethod {
    Integer,
    Aspect(u8, u8),
}

#[derive(Clone)]
pub struct Frame {
    pub pixels: Box<[u32]>,
    pub width: usize,
    pub height: usize,
    pub pixel_format: PixelFormat,
    pub scaling_method: ScalingMethod,
    pub buffer_changed: bool,
    pub dimensions_changed: bool,
}

impl Frame {
    pub fn swap(&mut self, frame: &mut Frame) {
        let dims_differ = self.width != frame.width || self.height != frame.height;

        if dims_differ {
            self.dimensions_changed = true;
            swap(&mut self.width, &mut frame.width);
            swap(&mut self.height, &mut frame.height);
        }

        if dims_differ || self.pixels != frame.pixels {
            self.buffer_changed = true;
            swap(&mut self.pixels, &mut frame.pixels);
        }
    }
}

impl Default for Frame {
    fn default() -> Self {
        Self {
            pixels: Box::new([0]),
            width: 0,
            height: 0,
            pixel_format: PixelFormat::Rgb555,
            scaling_method: ScalingMethod::Integer,
            buffer_changed: false,
            dimensions_changed: false,
        }
    }
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

    pub fn update(&mut self, frame: &mut Frame) {
        if !(frame.dimensions_changed || frame.buffer_changed) {
            return;
        }

        if frame.dimensions_changed {
            self.image = Image {
                bytes: vec![0; frame.width * frame.height * RGBA_BYTES_PER_PIXEL],
                width: frame.width as u16,
                height: frame.height as u16,
            };

            self.texture = Texture2D::from_image(&self.image);
            self.texture.set_filter(FilterMode::Nearest);
        }

        if frame.buffer_changed {
            for (pixel, out) in frame
                .pixels
                .iter()
                .zip(self.image.bytes.chunks_exact_mut(RGBA_BYTES_PER_PIXEL))
            {
                let [r, g, b] = to_rbg_single(*pixel, frame.pixel_format);
                out.copy_from_slice(&[r, g, b, 255]);
            }
        }

        self.texture.update(&self.image);

        frame.dimensions_changed = false;
        frame.buffer_changed = false;
    }

    pub fn draw(&mut self, frame: &Frame) {
        let screen_width = screen_width();
        let screen_height = screen_height();

        let (width, height) = match frame.scaling_method {
            ScalingMethod::Integer => {
                let scale = (screen_width / frame.width as f32)
                    .min(screen_height / frame.height as f32)
                    .floor()
                    .max(1.0);

                (frame.width as f32 * scale, frame.height as f32 * scale)
            }
            ScalingMethod::Aspect(aspect_width, aspect_heigth) => {
                let aspect_ratio = aspect_width as f32 / aspect_heigth as f32;

                if (screen_width / screen_height) > aspect_ratio {
                    (screen_height * aspect_ratio, screen_height)
                } else {
                    (screen_width, screen_width / aspect_ratio)
                }
            }
        };

        draw_texture_ex(
            &self.texture,
            (screen_width - width) / 2.0,
            (screen_height - height) / 2.0,
            WHITE,
            DrawTextureParams {
                dest_size: Some(vec2(width, height)),
                ..Default::default()
            },
        );
    }
}

pub fn rgb555_to_rgb888(rgb555: u16) -> [u8; 3] {
    let expand = |v: u16| -> u8 { ((v << 3) | (v >> 2)) as u8 };
    [
        expand(rgb555.get_bit_range(0..5)),
        expand(rgb555.get_bit_range(5..10)),
        expand(rgb555.get_bit_range(10..15)),
    ]
}

pub fn to_rgba(frame: &Frame) -> Vec<u8> {
    let mut rgba: Vec<u8> = Vec::with_capacity(frame.height * frame.width * RGBA_BYTES_PER_PIXEL);
    for pixel in &frame.pixels {
        let rgb888 = to_rbg_single(*pixel, frame.pixel_format);
        rgba.extend(rgb888);
        rgba.push(if *pixel == (1 << 31) { 0 } else { 255 });
    }

    rgba
}

pub fn to_rbg_single(value: u32, format: PixelFormat) -> [u8; 3] {
    match format {
        PixelFormat::Rgb555 => rgb555_to_rgb888(value as u16),
        PixelFormat::Rgb888 => [(value >> 16) as u8, (value >> 8) as u8, value as u8],
    }
}

pub fn to_rgb(frame: &Frame) -> Vec<u8> {
    let mut rgb = Vec::with_capacity(frame.width * frame.height * 3);

    for pixel in &frame.pixels {
        rgb.extend(to_rbg_single(*pixel, frame.pixel_format));
    }

    rgb
}

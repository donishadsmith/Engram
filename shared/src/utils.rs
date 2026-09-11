use crate::render::to_rgb;
use chrono::Local;
use gif::{Encoder, Frame, Repeat};
use macroquad::prelude::*;
use std::{fs::File, io::Error};

pub trait Emulator {
    fn save(&self) -> Result<(), Error>;
}

pub struct GifRecorder {
    encoder: Option<Encoder<File>>,
    counter: u8,
}

impl GifRecorder {
    pub fn new() -> Self {
        Self {
            encoder: None,
            counter: 0,
        }
    }

    pub fn is_recording(&self) -> bool {
        self.encoder.is_some()
    }

    pub fn toggle(&mut self, frame: &crate::render::Frame) -> Result<bool, Error> {
        let is_recording = if self.is_recording() {
            self.stop();

            false
        } else {
            self.start(frame)?;

            true
        };

        Ok(is_recording)
    }

    pub fn keybind(&mut self, frame: &crate::render::Frame) -> Result<bool, Error> {
        if is_key_pressed(KeyCode::F7) {
            Ok(self.toggle(frame)?)
        } else {
            Ok(self.is_recording())
        }
    }

    pub fn start(&mut self, frame: &crate::render::Frame) -> Result<(), Error> {
        let file = format!("recording_{}.gif", Local::now().format("%Y%m%d_%H%M%S"));
        let mut encoder = Encoder::new(
            File::create(file)?,
            frame.width as u16,
            frame.height as u16,
            &[],
        )
        .map_err(Error::other)?;
        encoder.set_repeat(Repeat::Infinite).map_err(Error::other)?;
        self.encoder = Some(encoder);

        self.counter = 0;

        Ok(())
    }

    pub fn stop(&mut self) {
        self.encoder.take();
    }

    pub fn capture(&mut self, frame: &crate::render::Frame) {
        let Some(encoder) = self.encoder.as_mut() else {
            return;
        };

        self.counter = (self.counter + 1) % 6;
        if self.counter == 0 {
            return;
        }

        let mut gif_frame = Frame::from_rgb_speed(
            frame.width as u16,
            frame.height as u16,
            &mut to_rgb(frame),
            10,
        );

        gif_frame.delay = 2;

        let _ = encoder.write_frame(&gif_frame);
    }
}

pub fn screenshot() {
    if is_key_pressed(KeyCode::F2) {
        get_screen_data().export_png("screenshot.png");
    }
}

pub fn quit_emulator<E: Emulator>(emulator: &E) -> Result<bool, Error> {
    if is_key_pressed(KeyCode::Escape) {
        save_progress(emulator)?;

        return Ok(true);
    }

    return Ok(false);
}

pub fn save_progress<E: Emulator>(emulator: &E) -> Result<(), Error> {
    if is_key_pressed(KeyCode::F1) {
        emulator.save()?;
    }

    Ok(())
}

pub fn error_message(message: String) -> std::io::Error {
    std::io::Error::new(std::io::ErrorKind::InvalidData, message)
}

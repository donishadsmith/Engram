use crate::render::to_rgb;
use chrono::Local;
use gif::{Encoder, Frame, Repeat};
use macroquad::prelude::*;
use rfd::FileDialog;
use std::{
    fs::{File, rename},
    io::{Error, ErrorKind},
    path::PathBuf,
};

pub trait Emulator {
    fn save(&mut self) -> Result<(), Error>;
}

pub struct GifRecorder {
    encoder: Option<Encoder<File>>,
    counter: u8,
    pub delay: u16,
    pub every_n_frame: u8,
    path: Option<PathBuf>,
}

impl GifRecorder {
    pub fn new() -> Self {
        Self {
            encoder: None,
            counter: 0,
            delay: 3,
            every_n_frame: 5,
            path: None,
        }
    }

    pub fn is_recording(&self) -> bool {
        self.encoder.is_some()
    }

    pub fn toggle(&mut self, frame: &crate::render::Frame) -> Result<bool, Error> {
        let is_recording = if self.is_recording() {
            let _ = self.stop();

            false
        } else {
            self.start(frame)?;

            true
        };

        Ok(is_recording)
    }

    pub fn start(&mut self, frame: &crate::render::Frame) -> Result<(), Error> {
        let path = PathBuf::from(format!(
            "recording_{}.gif",
            Local::now().format("%Y%m%d_%H%M%S")
        ));
        let mut encoder = Encoder::new(
            File::create(&path)?,
            frame.width as u16,
            frame.height as u16,
            &[],
        )
        .map_err(Error::other)?;
        encoder.set_repeat(Repeat::Infinite).map_err(Error::other)?;
        self.encoder = Some(encoder);

        self.counter = 0;
        self.path = Some(path);

        Ok(())
    }

    pub fn stop(&mut self) -> Result<(), Error> {
        self.encoder.take();

        if let Some(source_path) = self.path.take() {
            let destination_path = FileDialog::new()
                .set_file_name(source_path.file_name().unwrap().to_string_lossy())
                .save_file();

            if let Some(destination_path) = destination_path {
                rename(&source_path, &destination_path)?;
            }
        }

        Ok(())
    }

    pub fn capture(&mut self, frame: &crate::render::Frame) {
        let Some(encoder) = self.encoder.as_mut() else {
            return;
        };

        self.counter = (self.counter + 1) % self.every_n_frame;
        if self.counter != 0 {
            return;
        }

        let mut gif_frame = Frame::from_rgb_speed(
            frame.width as u16,
            frame.height as u16,
            &mut to_rgb(frame),
            10,
        );

        gif_frame.delay = self.delay;

        let _ = encoder.write_frame(&gif_frame);
    }
}

pub fn screenshot() {
    get_screen_data().export_png(
        &format!("screenshot_{}.png", Local::now().format("%Y%m%d_%H%M%S")).to_string(),
    );
}

pub fn error_message(message: String) -> Error {
    Error::new(ErrorKind::InvalidData, message)
}

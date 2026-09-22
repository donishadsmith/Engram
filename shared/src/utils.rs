use crate::render::to_rgb;
use chrono::Local;
use gif::{Encoder, Frame, Repeat};
use macroquad::prelude::*;
use std::{
    collections::BTreeMap,
    fs::File,
    io::{Error, ErrorKind},
    path::PathBuf,
};

pub struct GifRecorder {
    encoder: Option<Encoder<File>>,
    counter: u8,
    pub delay: u8,
    pub every_n_frame: u8,
    path: Option<PathBuf>,
}

impl GifRecorder {
    pub fn new(gif_settings: &BTreeMap<String, u8>) -> Self {
        Self {
            encoder: None,
            counter: 0,
            delay: *gif_settings.get("delay").unwrap_or_else(|| &3),
            every_n_frame: *gif_settings.get("every_n_frame").unwrap_or_else(|| &5),
            path: None,
        }
    }

    pub fn is_recording(&self) -> bool {
        self.encoder.is_some()
    }

    pub fn toggle(
        &mut self,
        frame: &crate::render::Frame,
        image_dir: PathBuf,
    ) -> Result<bool, Error> {
        let is_recording = if self.is_recording() {
            let _ = self.stop();

            false
        } else {
            self.start(frame, image_dir)?;

            true
        };

        Ok(is_recording)
    }

    pub fn start(&mut self, frame: &crate::render::Frame, image_dir: PathBuf) -> Result<(), Error> {
        let path = image_dir.join(format!("clip_{}.gif", Local::now().format("%Y%m%d_%H%M%S")));
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

    pub fn stop(&mut self) {
        self.encoder.take();
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

        gif_frame.delay = self.delay as u16;

        let _ = encoder.write_frame(&gif_frame);
    }

    pub fn settings(&self) -> BTreeMap<String, u8> {
        let mut map = BTreeMap::new();
        map.insert("every_n_frame".to_owned(), self.every_n_frame);
        map.insert("delay".to_owned(), self.delay);

        map
    }
}

pub fn screenshot(image_dir: PathBuf) {
    let image_path = image_dir.join(format!(
        "screenshot_{}.png",
        Local::now().format("%Y%m%d_%H%M%S")
    ));

    match &image_path.to_str() {
        Some(image) => get_screen_data().export_png(&image),
        None => {}
    }
}

pub fn error_message(message: String) -> Error {
    Error::new(ErrorKind::InvalidData, message)
}

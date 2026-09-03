use macroquad::prelude::*;
use std::io::Error;

pub trait Emulator {
    fn save(&self) -> Result<(), Error>;
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

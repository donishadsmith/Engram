pub mod audio;
pub mod debug;
pub mod input;
pub mod render;
pub mod utils;

use egui::Context;
use macroquad::input::KeyCode;
use std::{io::Error, path::PathBuf};

use crate::{debug::DebugPage, input::GBA_KEYMAP, render::Frame};

#[derive(Clone, Copy, PartialEq, Eq)]
pub enum EmulatorId {
    Gb,
    Gba,
}

#[derive(Clone, Copy, PartialEq, Eq)]
pub enum EmulatorState {
    Quit,
    Running,
    Selection,
    Reset,
}

pub trait EmulatorSession {
    fn run(
        &mut self,
        key_bindings: &Vec<KeyCode>,
        input_blocked: bool,
    ) -> Result<EmulatorState, Error>;

    fn save_game(&mut self) -> Result<(), Error>;

    fn has_debug_ui(&self) -> bool {
        false
    }

    fn debug_visible(&self, _debug_page: DebugPage) -> bool {
        false
    }

    fn toggle_debug(&mut self, _debug_page: DebugPage) {}

    fn debug_ui(&mut self, _egui_ctx: &Context) {}

    fn has_solar(&self) -> bool {
        false
    }

    fn solar_level(&mut self, _solar_level: u8) {}

    fn reset(&mut self, rom_path: PathBuf) -> Result<(), Error>;

    fn reference_frontend(&self) -> &Frame;

    fn frame_ready(&self) -> bool;

    fn id(&self) -> EmulatorId;

    fn default_keys(&self) -> &[KeyCode] {
        match self.id() {
            EmulatorId::Gb | EmulatorId::Gba => &GBA_KEYMAP,
        }
    }
}

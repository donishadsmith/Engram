pub mod audio;
pub mod input;
pub mod render;
pub mod utils;

use egui::Context;
use std::io::Error;

#[derive(Clone, Copy, PartialEq, Eq)]
pub enum EmulatorState {
    Quit,
    Running,
    Selection,
}

pub trait EmulatorSession {
    fn run(&mut self) -> Result<EmulatorState, Error>;

    fn save_game(&mut self) -> Result<(), Error>;

    fn has_debug_ui(&self) -> bool {
        false
    }

    fn debug_visible(&self) -> bool {
        false
    }

    fn toggle_debug(&mut self) {}

    fn debug_ui(&mut self, egui_ctx: &Context) {}
}

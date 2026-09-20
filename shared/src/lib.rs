pub mod audio;
pub mod config;
pub mod debug;
pub mod editor;
pub mod keybind;
pub mod render;
pub mod script;
pub mod traits;
pub mod utils;

use egui::Context;
use macroquad::input::KeyCode;
use std::{io::Error, path::PathBuf};

use crate::{debug::DebugPage, render::Frame, script::ScriptEngine};

#[derive(Clone, Copy, PartialEq, Eq, Hash)]
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
    Launch,
}

pub trait EmulatorSession {
    fn run(
        &mut self,
        key_bindings: &Vec<KeyCode>,
        input_blocked: bool,
        volume: u8,
    ) -> Result<EmulatorState, Error>;

    fn save_game(&mut self) -> Result<(), Error>;

    fn has_debug_ui(&self) -> bool {
        false
    }

    fn debug_visible(&self, _debug_page: DebugPage) -> bool {
        false
    }

    fn toggle_debug(&mut self, _debug_page: Option<DebugPage>) {}

    fn debug_ui(&mut self, _egui_ctx: &Context) {}

    fn debug_page_available(&self, _debug_page: Option<DebugPage>) -> bool {
        false
    }

    fn has_solar(&self) -> bool {
        false
    }

    fn solar_level(&mut self, _solar_level: u8) {}

    fn reset(&mut self, rom_path: PathBuf) -> Result<(), Error>;

    fn reference_frontend(&self) -> &Frame;

    fn frame_ready(&self) -> bool;

    fn id(&self) -> EmulatorId;

    fn script_engine(&mut self) -> Option<&mut ScriptEngine> {
        None
    }
}

pub trait ScriptTarget {
    fn read_u8(&mut self, address: u32) -> u8;

    fn read_u16(&mut self, address: u32) -> u16;

    fn read_u32(&mut self, address: u32) -> u32;

    fn write_u8(&mut self, address: u32, value: u8);

    fn write_u16(&mut self, address: u32, value: u16);

    fn write_u32(&mut self, address: u32, value: u32);

    fn read_cpu_register(&self, _register_name: String) -> Option<u64>;

    // probably useless but still a fun function
    fn to_rgb(&self, value: u32) -> [u8; 3];
}

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
    Paused,
}

pub trait DebugInterface {
    fn toggle(&mut self, _debug_page: Option<DebugPage>);

    fn available_pages(&self) -> Vec<DebugPage>;

    fn show_ui(&mut self, _egui_ctx: &Context);

    fn visible(&self, _debug_page: DebugPage) -> bool;

    fn active(&self) -> bool;
}

pub trait SolarSensor {
    fn set_level(&mut self, _solar_level: u8) {}
}

pub trait EmulatorSession {
    // eventually allow emu to return running or paused based on internal state/conditions
    fn run(
        &mut self,
        key_bindings: &Vec<KeyCode>,
        input_blocked: bool,
        volume: u8,
    ) -> Result<EmulatorState, Error>;

    fn save_game(&mut self) -> Result<(), Error>;

    fn solar_sensor(&mut self) -> Option<&mut dyn SolarSensor> {
        None
    }

    fn debugger_ref(&self) -> Option<&dyn DebugInterface> {
        None
    }

    fn debugger_mut(&mut self) -> Option<&mut dyn DebugInterface> {
        None
    }

    fn reset(&mut self, rom_path: PathBuf) -> Result<(), Error>;

    fn frontend_ref(&self) -> &Frame;

    fn frame_ready(&self) -> bool;

    fn id(&self) -> EmulatorId;

    fn script_engine(&mut self) -> Option<&mut ScriptEngine> {
        None
    }

    fn pause(&mut self);
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

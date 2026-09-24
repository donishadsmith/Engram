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

use crate::{
    debug::DebugPage,
    render::Frame,
    script::{CpuError, DomainError, ScriptEngine, WatchpointArgs, WatchpointHit},
};

#[derive(Clone, Copy, PartialEq, Eq, Hash)]
pub enum EmulatorId {
    Gb,
    Gba,
}

#[derive(Clone, Copy, PartialEq, Eq)]
pub enum EmulatorState {
    Quit,
    Running,
    RomSelection,
    Reset,
    Launch,
    Paused,
    BiosSelection,
}

pub trait Emulator {
    fn save(&mut self) -> Result<(), Error>;

    fn remove_breakpoint(&mut self, address: u32);

    fn set_breakpoint(&mut self, address: u32, pause: bool) -> bool;

    fn take_breakpoint_hit(&mut self) -> Option<u32>;

    fn clear_all_breakpoints(&mut self);

    fn check_breakpoints(&self) -> Vec<(u32, EmulatorState)>;

    fn remove_watchpoint(&mut self, address: u32);

    fn set_watchpoint(&mut self, address: u32, watchpoint_args: WatchpointArgs) -> bool;

    fn take_watchpoint_hits(&mut self) -> Vec<WatchpointHit>;

    fn clear_all_watchpoints(&mut self);

    fn check_watchpoints(&self) -> Vec<(u32, WatchpointArgs)>;
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

    fn step_instruction(&mut self, volume: u8);

    fn set_resume(&mut self);
}

pub trait ScriptTarget: Emulator {
    fn read_u8(&mut self, address: u32) -> u8;

    fn read_u16(&mut self, address: u32) -> u16;

    fn read_u32(&mut self, address: u32) -> u32;

    fn write_u8(&mut self, address: u32, value: u8);

    fn write_u16(&mut self, address: u32, value: u16);

    fn write_u32(&mut self, address: u32, value: u32);

    fn read_cpu_register(&self, register_name: String) -> Result<u64, CpuError>;

    fn write_cpu_register(&mut self, register_name: String, value: u32) -> Result<(), CpuError>;

    // probably useless but still a fun function
    fn to_rgb(&self, value: u32) -> [u8; 3];

    // mgba's amazing scripting api: https://mgba.io/docs/scripting.html
    // adding these since they should help for the psx
    fn cpu_register_names(&self) -> &'static [&'static str];

    fn memory_domain_names(&self) -> &'static [&'static str];

    fn read_domain(&self, domain: &str, offset: usize) -> Result<u8, DomainError>;

    fn write_domain(&mut self, domain: &str, offset: usize, value: u8) -> Result<(), DomainError>;

    fn address_to_domain(&self, address: u32) -> Option<(&'static str, usize)>;
}

// https://www.copetti.org/writings/consoles/game-boy-advance/
// https://mgba.io/2015/06/27/cycle-counting-prefetch/
// https://github.com/ioncodes/ayyboy-advance
// https://www.gregorygaines.com/blog/decoding-the-arm7tdmi-instruction-set-game-boy-advance/
// https://ia903206.us.archive.org/34/items/NintendoGbaManualV1.1/Nintendo%20Gba%20Manual%20V1.1.pdf
// https://ww1.microchip.com/downloads/en/DeviceDoc/DDI0029G_7TDMI_R3_trm.pdf
// https://medium.com/@julio.vidaurre/making-a-gba-emulator-fbf91b85979a

#![windows_subsystem = "windows"]

mod audio_debugger;
pub mod components;
mod dump;

use crate::components::{gamepak::GamePak, gba::GBA};
use audio_debugger::AudioDebugger;
use macroquad::input::KeyCode;
use shared::{
    EmulatorId, EmulatorSession, EmulatorState,
    audio::{AUDIO_BUFFER_CAPACITY, AUDIO_TARGET_OCCUPANCY, AudioOutput},
    input::get_relevant_key_presses,
    render::Screen,
    utils::{Emulator, quit_emulator, save_progress},
};
use std::{io::Error, path::PathBuf};

const GBA_CLOCK_SPEED: u32 = 16777216;

pub struct GBASession {
    audio: AudioOutput,
    audio_debugger: AudioDebugger,
    gba: GBA,
    screen: Screen,
    frame_ready: bool,
}

impl GBASession {
    pub fn new_session(rom_path: PathBuf) -> Result<Self, Error> {
        let audio_debugger = AudioDebugger::new();
        let audio = AudioOutput::new();
        let gamepak = GamePak::load(rom_path)?;
        let apu_sample_cycles = GBA_CLOCK_SPEED / audio.sample_rate;
        let gba = GBA::boot(gamepak, apu_sample_cycles);
        let screen = Screen::new(gba.bus.ppu.frame.width, gba.bus.ppu.frame.height);

        Ok(Self {
            audio,
            audio_debugger,
            gba,
            screen,
            frame_ready: false,
        })
    }
}

impl EmulatorSession for GBASession {
    fn run(
        &mut self,
        key_bindings: &Vec<KeyCode>,
        input_blocked: bool,
    ) -> Result<EmulatorState, Error> {
        if quit_emulator(&self.gba)? {
            return Ok(EmulatorState::Quit);
        }

        if self.gba.backup_updated() {
            let _ = save_progress(&self.gba);
        }

        self.gba.keypad = get_relevant_key_presses(&key_bindings, input_blocked)
            .as_slice()
            .try_into()
            .unwrap();

        self.audio_debugger.turn_on(&mut self.gba);
        self.audio_debugger.freeze();

        while AUDIO_BUFFER_CAPACITY - self.audio.producer.slots() < AUDIO_TARGET_OCCUPANCY {
            self.gba.run();
            for sample in self.gba.bus.apu.sample_buffer.drain(..) {
                let _ = self.audio.producer.push(sample);
            }
        }

        self.frame_ready = self.gba.take_frame();
        if self.frame_ready && !self.audio_debugger.visible {
            self.screen.update(&self.gba.bus.ppu.frontend);
        }

        if !self.audio_debugger.visible {
            self.screen.draw(&self.gba.bus.ppu.frontend);
        }

        Ok(EmulatorState::Running)
    }

    fn save_game(&mut self) -> Result<(), Error> {
        self.gba.save()
    }

    fn has_debug_ui(&self) -> bool {
        true
    }

    fn debug_visible(&self) -> bool {
        self.audio_debugger.visible
    }

    fn toggle_debug(&mut self) {
        self.audio_debugger.toggle(&mut self.gba);
    }

    fn debug_ui(&mut self, egui_ctx: &egui::Context) {
        self.audio_debugger.show_ui(egui_ctx, &mut self.gba);
    }

    fn has_solar(&self) -> bool {
        self.gba.bus.gamepak.gpio.solar_sensor.is_some()
    }

    fn solar_level(&mut self, solar_level: u8) {
        if let Some(solar_sensor) = self.gba.bus.gamepak.gpio.solar_sensor.as_mut() {
            solar_sensor.set_level(solar_level);
        }
    }

    fn reset(&mut self, rom_path: PathBuf) -> Result<(), Error> {
        self.audio = AudioOutput::new();
        let gamepak = GamePak::load(rom_path)?;
        let apu_sample_cycles = GBA_CLOCK_SPEED / self.audio.sample_rate;
        self.gba = GBA::boot(gamepak, apu_sample_cycles);
        self.screen = Screen::new(self.gba.bus.ppu.frame.width, self.gba.bus.ppu.frame.height);
        self.frame_ready = false;

        Ok(())
    }

    fn reference_frontend(&self) -> &shared::render::Frame {
        &self.gba.bus.ppu.frontend
    }

    fn frame_ready(&self) -> bool {
        self.frame_ready
    }

    fn id(&self) -> EmulatorId {
        EmulatorId::Gba
    }
}

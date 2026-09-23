// https://www.copetti.org/writings/consoles/game-boy-advance/
// https://mgba.io/2015/06/27/cycle-counting-prefetch/
// https://github.com/ioncodes/ayyboy-advance
// https://www.gregorygaines.com/blog/decoding-the-arm7tdmi-instruction-set-game-boy-advance/
// https://ia903206.us.archive.org/34/items/NintendoGbaManualV1.1/Nintendo%20Gba%20Manual%20V1.1.pdf
// https://ww1.microchip.com/downloads/en/DeviceDoc/DDI0029G_7TDMI_R3_trm.pdf
// https://medium.com/@julio.vidaurre/making-a-gba-emulator-fbf91b85979a

#![windows_subsystem = "windows"]

pub mod components;
mod debug;

use crate::{
    components::{gamepak::GamePak, gba::GBA},
    debug::video::PpuDebugger,
};
use debug::audio::AudioDebugger;
use macroquad::input::KeyCode;
use shared::{
    DebugInterface, Emulator, EmulatorId, EmulatorSession, EmulatorState, SolarSensor,
    audio::{AUDIO_BUFFER_CAPACITY, AUDIO_TARGET_OCCUPANCY, AudioOutput},
    debug::DebugPage,
    keybind::get_relevant_key_presses,
    render::Screen,
    script::ScriptEngine,
};
use std::{io::Error, path::PathBuf};

const GBA_CLOCK_SPEED: u32 = 16777216;

pub struct GBASession {
    audio: AudioOutput,
    audio_debugger: AudioDebugger,
    ppu_debugger: PpuDebugger,
    gba: GBA,
    screen: Screen,
    frame_ready: bool,
    active_debug: Option<DebugPage>,
    script_engine: ScriptEngine,
}

impl GBASession {
    pub fn new_session(rom_path: PathBuf) -> Result<Self, Error> {
        let audio_debugger = AudioDebugger::new();
        let ppu_debugger = PpuDebugger::new();
        let audio = AudioOutput::new();
        let gamepak = GamePak::load(rom_path)?;
        let apu_sample_cycles = GBA_CLOCK_SPEED / audio.sample_rate;
        let gba = GBA::boot(gamepak, apu_sample_cycles);
        let screen = Screen::new(gba.bus.ppu.frame.width, gba.bus.ppu.frame.height);

        Ok(Self {
            audio,
            audio_debugger,
            ppu_debugger,
            gba,
            screen,
            frame_ready: false,
            active_debug: None,
            script_engine: ScriptEngine::new(),
        })
    }

    fn update_screen(&mut self) {
        self.frame_ready = self.gba.take_frame();
        if self.frame_ready && self.active_debug.is_none() {
            self.screen.update(&self.gba.bus.ppu.frontend);
        }

        if self.active_debug.is_none() {
            self.screen.draw(&self.gba.bus.ppu.frontend);
        }
    }

    fn drain_audio(&mut self, volume: u8) {
        for sample in self.gba.bus.apu.sample_buffer.drain(..) {
            let _ = self.audio.play(sample, volume);
        }
    }

    fn tick(&mut self, volume: u8) {
        self.gba.run();

        if self.gba.take_frame_start() {
            self.script_engine
                .execute(&mut self.gba, EmulatorId::Gba, true);
        }

        self.drain_audio(volume);

        if self.gba.cpu.breakpoint_hit.is_some() {
            self.set_resume();
        }
    }
}

impl EmulatorSession for GBASession {
    fn run(
        &mut self,
        key_bindings: &Vec<KeyCode>,
        input_blocked: bool,
        volume: u8,
    ) -> Result<EmulatorState, Error> {
        self.gba.keypad = get_relevant_key_presses(&key_bindings, input_blocked)
            .as_slice()
            .try_into()
            .unwrap();

        while AUDIO_BUFFER_CAPACITY - self.audio.producer.slots() < AUDIO_TARGET_OCCUPANCY {
            self.tick(volume);

            if self.gba.cpu.breakpoint_hit.is_some() {
                break;
            }
        }

        let state = if let Some(address) = self.gba.cpu.breakpoint_hit {
            let breakpoint_action = self
                .gba
                .cpu
                .breakpoint_action
                .get(&address)
                .unwrap()
                .clone();

            if breakpoint_action == EmulatorState::Running {
                self.script_engine
                    .execute(&mut self.gba, EmulatorId::Gba, self.frame_ready);
            }

            Ok(breakpoint_action)
        } else {
            Ok(EmulatorState::Running)
        };

        self.update_screen();

        return state;
    }

    fn pause(&mut self) {
        self.script_engine
            .execute(&mut self.gba, EmulatorId::Gba, false);

        if self.active_debug.is_none() {
            self.screen.draw(&self.gba.bus.ppu.frontend);
        }
    }

    fn save_game(&mut self) -> Result<(), Error> {
        self.gba.save()
    }

    fn solar_sensor(&mut self) -> Option<&mut dyn SolarSensor> {
        if self.gba.bus.gamepak.gpio.solar_sensor.is_some() {
            Some(self)
        } else {
            None
        }
    }

    fn reset(&mut self, rom_path: PathBuf) -> Result<(), Error> {
        self.audio = AudioOutput::new();
        let gamepak = GamePak::load(rom_path)?;
        let apu_sample_cycles = GBA_CLOCK_SPEED / self.audio.sample_rate;
        self.gba = GBA::boot(gamepak, apu_sample_cycles);
        self.screen = Screen::new(self.gba.bus.ppu.frame.width, self.gba.bus.ppu.frame.height);
        self.frame_ready = false;
        self.active_debug = None;
        self.script_engine = ScriptEngine::new();

        Ok(())
    }

    fn frontend_ref(&self) -> &shared::render::Frame {
        &self.gba.bus.ppu.frontend
    }

    fn frame_ready(&self) -> bool {
        self.frame_ready
    }

    fn id(&self) -> EmulatorId {
        EmulatorId::Gba
    }

    fn script_engine(&mut self) -> Option<&mut ScriptEngine> {
        Some(&mut self.script_engine)
    }

    fn debugger_ref(&self) -> Option<&dyn DebugInterface> {
        Some(self)
    }

    fn debugger_mut(&mut self) -> Option<&mut dyn DebugInterface> {
        Some(self)
    }

    fn step_instruction(&mut self, volume: u8) {
        self.tick(volume);
        self.update_screen();
    }

    fn set_resume(&mut self) {
        self.gba.cpu.resume_from = Some(self.gba.cpu.next_executing_address());
    }
}

impl DebugInterface for GBASession {
    fn available_pages(&self) -> Vec<DebugPage> {
        vec![DebugPage::Audio, DebugPage::Video]
    }

    fn visible(&self, debug_page: DebugPage) -> bool {
        self.active_debug == Some(debug_page)
    }

    fn toggle(&mut self, debug_page: Option<DebugPage>) {
        match self.active_debug {
            Some(DebugPage::Audio) => self.audio_debugger.close(&mut self.gba),
            Some(_) => self.ppu_debugger.close(&mut self.gba),
            None => {}
        }

        self.screen.update(&self.gba.bus.ppu.frontend);

        self.active_debug = if self.active_debug == debug_page {
            None
        } else {
            debug_page
        };
    }

    fn show_ui(&mut self, egui_ctx: &egui::Context) {
        match self.active_debug {
            Some(DebugPage::Audio) => self.audio_debugger.show_ui(egui_ctx, &mut self.gba),
            Some(DebugPage::Video) => self.ppu_debugger.show_ui(egui_ctx, &mut self.gba),
            None => {}
        }
    }

    fn active(&self) -> bool {
        self.active_debug.is_some()
    }
}

impl SolarSensor for GBASession {
    fn set_level(&mut self, solar_level: u8) {
        if let Some(solar_sensor) = self.gba.bus.gamepak.gpio.solar_sensor.as_mut() {
            solar_sensor.set_level(solar_level);
        }
    }
}

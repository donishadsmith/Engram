/* References:
   - https://gbdev.io/pandocs
   - https://aquova.net/emudev/gb
   - https://github.com/mvdnes/rboy
   - https://github.com/smparsons/retroboy
*/

#![windows_subsystem = "windows"]

pub mod components;

use crate::components::{gameboy::GameBoy, gamepak::GamePak};
use macroquad::input::KeyCode;
use shared::{
    EmulatorId, EmulatorSession, EmulatorState,
    audio::{AUDIO_BUFFER_CAPACITY, AUDIO_TARGET_OCCUPANCY, AudioOutput},
    keybind::get_relevant_key_presses,
    render::Screen,
    script::ScriptEngine,
    utils::Emulator,
};
use std::{io::Error, path::PathBuf};

const GB_CLOCK_SPEED: u32 = 4194304;

pub struct GameBoySession {
    audio: AudioOutput,
    gameboy: GameBoy,
    screen: Screen,
    apu_sample_cycles: u32,
    frame_ready: bool,
    script_engine: ScriptEngine,
}

impl GameBoySession {
    pub fn new_session(rom_path: PathBuf) -> Result<Self, Error> {
        let audio = AudioOutput::new();
        let gamepak = GamePak::load(rom_path)?;
        let apu_sample_cycles = GB_CLOCK_SPEED / audio.sample_rate;
        let gameboy = GameBoy::boot(gamepak);
        let screen = Screen::new(
            gameboy.cpu.bus.ppu.frame.width,
            gameboy.cpu.bus.ppu.frame.height,
        );

        Ok(Self {
            audio,
            gameboy,
            screen,
            apu_sample_cycles,
            frame_ready: false,
            script_engine: ScriptEngine::new(),
        })
    }
}

impl EmulatorSession for GameBoySession {
    fn run(
        &mut self,
        key_bindings: &Vec<KeyCode>,
        input_blocked: bool,
        volume: u8,
    ) -> Result<EmulatorState, Error> {
        self.gameboy.keypad = get_relevant_key_presses(&key_bindings[..8].to_vec(), input_blocked)
            .as_slice()
            .try_into()
            .unwrap();

        // https://nightshade256.github.io/2021/03/27/gb-sound-emulation.html
        while AUDIO_BUFFER_CAPACITY - self.audio.producer.slots() < AUDIO_TARGET_OCCUPANCY {
            self.gameboy.run(self.apu_sample_cycles);
            for sample in self.gameboy.cpu.bus.apu.sample_buffer.drain(..) {
                let _ = self.audio.play(sample, volume);
            }
        }

        self.frame_ready = self.gameboy.take_frame();
        if self.frame_ready {
            self.script_engine
                .execute(&mut self.gameboy, EmulatorId::Gb);
            self.screen.update(&self.gameboy.cpu.bus.ppu.frontend);
        }

        self.screen.draw(&self.gameboy.cpu.bus.ppu.frontend);

        Ok(EmulatorState::Running)
    }

    fn save_game(&mut self) -> Result<(), Error> {
        self.gameboy.save()
    }

    fn reset(&mut self, rom_path: PathBuf) -> Result<(), Error> {
        self.audio = AudioOutput::new();
        let gamepak = GamePak::load(rom_path)?;
        self.apu_sample_cycles = GB_CLOCK_SPEED / self.audio.sample_rate;
        self.gameboy = GameBoy::boot(gamepak);
        self.screen = Screen::new(
            self.gameboy.cpu.bus.ppu.frame.width,
            self.gameboy.cpu.bus.ppu.frame.height,
        );
        self.frame_ready = false;
        self.script_engine = ScriptEngine::new();

        Ok(())
    }

    fn reference_frontend(&self) -> &shared::render::Frame {
        &self.gameboy.cpu.bus.ppu.frontend
    }

    fn frame_ready(&self) -> bool {
        self.frame_ready
    }

    fn id(&self) -> EmulatorId {
        EmulatorId::Gb
    }

    fn script_engine(&mut self) -> Option<&mut ScriptEngine> {
        Some(&mut self.script_engine)
    }
}

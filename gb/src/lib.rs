/* References:
   - https://gbdev.io/pandocs
   - https://aquova.net/emudev/gb
   - https://github.com/mvdnes/rboy
   - https://github.com/smparsons/retroboy
*/

#![windows_subsystem = "windows"]

pub mod components;

use crate::components::{gameboy::GameBoy, gamepak::GamePak};
use shared::{
    EmulatorSession, EmulatorState,
    audio::{AUDIO_BUFFER_CAPACITY, AUDIO_TARGET_OCCUPANCY, AudioOutput},
    input::{GB_KEYMAP, get_relevant_key_presses},
    render::Screen,
    utils::{Emulator, quit_emulator, save_progress},
};
use std::{io::Error, path::PathBuf};

const GB_CLOCK_SPEED: u32 = 4194304;

pub struct GameBoySession {
    audio: AudioOutput,
    gameboy: GameBoy,
    screen: Screen,
    cycles_per_sample: u32,
}

impl GameBoySession {
    pub fn new_session(rom_path: PathBuf) -> Result<Self, Error> {
        let audio = AudioOutput::new();
        let gamepak = GamePak::load(rom_path)?;
        let cycles_per_sample = GB_CLOCK_SPEED / audio.sample_rate;
        let gameboy = GameBoy::boot(gamepak);
        let screen = Screen::new(
            gameboy.cpu.bus.ppu.frame.width,
            gameboy.cpu.bus.ppu.frame.height,
        );

        Ok(Self {
            audio,
            gameboy,
            screen,
            cycles_per_sample,
        })
    }
}

impl EmulatorSession for GameBoySession {
    fn run(&mut self) -> Result<EmulatorState, Error> {
        if quit_emulator(&self.gameboy)? {
            return Ok(EmulatorState::Quit);
        }

        if self.gameboy.ram_changed() {
            let _ = save_progress(&self.gameboy);
        }

        self.gameboy.keypad = get_relevant_key_presses(&GB_KEYMAP)
            .as_slice()
            .try_into()
            .unwrap();

        // https://nightshade256.github.io/2021/03/27/gb-sound-emulation.html
        while AUDIO_BUFFER_CAPACITY - self.audio.producer.slots() < AUDIO_TARGET_OCCUPANCY {
            self.gameboy.run(self.cycles_per_sample);
            for sample in self.gameboy.cpu.bus.apu.sample_buffer.drain(..) {
                let _ = self.audio.producer.push(sample);
            }
        }

        if self.gameboy.take_frame() {
            self.screen.update(&self.gameboy.cpu.bus.ppu.frame);
        }

        self.screen.draw(&self.gameboy.cpu.bus.ppu.frame);

        Ok(EmulatorState::Running)
    }

    fn save_game(&mut self) -> Result<(), Error> {
        self.gameboy.save()
    }
}

/* References:
   - https://gbdev.io/pandocs
   - https://aquova.net/emudev/gb
   - https://github.com/mvdnes/rboy
   - https://github.com/smparsons/retroboy
*/

#![windows_subsystem = "windows"]

pub mod components;

use crate::components::{
    gameboy::{GameBoy, T_CYCLES_PER_FRAME_DOUBLE},
    gamepak::GamePak,
};
use shared::{
    Emulator, EmulatorId, EmulatorSession, EmulatorState,
    audio::{AUDIO_BUFFER_CAPACITY, AUDIO_TARGET_OCCUPANCY, AudioOutput},
    render::Screen,
    script::ScriptEngine,
};
use spin_sleep::sleep_until;
use std::{
    io::Error,
    path::PathBuf,
    time::{Duration, Instant},
};

const GB_CLOCK_SPEED: u32 = 4194304;

pub struct GameBoySession {
    audio: Option<AudioOutput>,
    gameboy: GameBoy,
    screen: Screen,
    apu_sample_cycles: u32,
    frame_ready: bool,
    script_engine: ScriptEngine,
    frame_period: Duration,
    frame_deadline: Instant,
}

impl GameBoySession {
    pub fn new_session(rom_path: PathBuf) -> Result<Self, Error> {
        let audio = AudioOutput::new();
        let gamepak = GamePak::load(rom_path)?;
        let sample_rate = match &audio {
            Some(audio) => audio.sample_rate,
            None => 44100,
        };
        let apu_sample_cycles = GB_CLOCK_SPEED / sample_rate;
        let gameboy = GameBoy::boot(gamepak);
        let screen = Screen::new(
            gameboy.cpu.bus.ppu.frame.width,
            gameboy.cpu.bus.ppu.frame.height,
        );
        let frame_period = Duration::from_secs_f64(1.0 / 59.73);
        let frame_deadline = Instant::now() + frame_period;

        Ok(Self {
            audio,
            gameboy,
            screen,
            apu_sample_cycles,
            frame_ready: false,
            script_engine: ScriptEngine::new(),
            frame_period,
            frame_deadline,
        })
    }

    fn drain_audio(&mut self, volume: u8) {
        for sample in self.gameboy.cpu.bus.apu.sample_buffer.drain(..) {
            match &mut self.audio {
                Some(audio) => audio.play(sample, volume),
                None => {}
            };
        }
    }

    fn on_frame(&mut self) {
        self.frame_ready = true;
        self.gameboy.take_frame();
        self.script_engine
            .execute(&mut self.gameboy, EmulatorId::Gb, true);
        self.screen.update(&mut self.gameboy.cpu.bus.ppu.frontend);
    }

    fn draw(&mut self) {
        self.screen.draw(&self.gameboy.cpu.bus.ppu.frontend);
    }

    fn step_end(&mut self, volume: u8) {
        self.drain_audio(volume);
        self.gameboy.replenish_remaining_cycles();
        self.gameboy.take_watchpoint_pause();
        self.script_engine.step_completed();

        if self.gameboy.cpu.bus.ppu.frame_ready {
            self.on_frame();
        } else {
            self.script_engine
                .execute(&mut self.gameboy, EmulatorId::Gb, false);
        }

        self.draw();

        if self.gameboy.cpu.breakpoint_hit.is_some() {
            self.set_resume();
        }
    }
}

impl EmulatorSession for GameBoySession {
    fn run(&mut self, input: &[bool], volume: u8) -> Result<EmulatorState, Error> {
        self.frame_ready = false;
        self.gameboy.keypad = input.try_into().unwrap_or([false; 8]);

        // https://nightshade256.github.io/2021/03/27/gb-sound-emulation.html
        if self.audio.is_some() {
            while AUDIO_BUFFER_CAPACITY - self.audio.as_mut().unwrap().producer.slots()
                < AUDIO_TARGET_OCCUPANCY
            {
                self.gameboy.run(self.apu_sample_cycles);
                self.drain_audio(volume);

                if self.gameboy.cpu.bus.ppu.frame_ready {
                    self.on_frame();
                }

                if self.gameboy.cpu.breakpoint_hit.is_some() {
                    self.set_resume();
                    break;
                }

                if self.gameboy.cpu.bus.watchpoint_pause.get() {
                    break;
                }
            }
        } else {
            let mut cycles = 0;
            while cycles < T_CYCLES_PER_FRAME_DOUBLE {
                cycles += self.gameboy.step(self.apu_sample_cycles);

                if self.gameboy.cpu.bus.ppu.frame_ready {
                    self.on_frame();
                }

                if self.gameboy.cpu.breakpoint_hit.is_some() {
                    self.set_resume();
                    break;
                }

                if self.gameboy.cpu.bus.watchpoint_pause.get() {
                    break;
                }
            }
            self.gameboy.end_of_frame();
        }

        self.script_engine
            .execute(&mut self.gameboy, EmulatorId::Gb, false);

        let state = if self.gameboy.take_watchpoint_pause() {
            EmulatorState::Paused
        } else if let Some(address) = self.gameboy.cpu.breakpoint_hit {
            self.gameboy
                .cpu
                .breakpoint_queue
                .get(&address)
                .unwrap()
                .clone()
        } else {
            EmulatorState::Running
        };

        self.draw();

        if self.audio.is_none() {
            sleep_until(self.frame_deadline);
            self.frame_deadline += self.frame_period;
        }

        Ok(state)
    }

    fn pause(&mut self) {
        self.script_engine
            .execute(&mut self.gameboy, EmulatorId::Gb, false);

        self.screen.draw(&self.gameboy.cpu.bus.ppu.frontend);
    }

    fn save_game(&mut self) -> Result<(), Error> {
        self.gameboy.save()
    }

    fn reset(&mut self, rom_path: PathBuf) -> Result<(), Error> {
        self.audio = AudioOutput::new();
        let gamepak = GamePak::load(rom_path)?;
        let sample_rate = match &self.audio {
            Some(audio) => audio.sample_rate,
            None => 44100,
        };
        self.apu_sample_cycles = GB_CLOCK_SPEED / sample_rate;
        self.gameboy = GameBoy::boot(gamepak);
        self.screen = Screen::new(
            self.gameboy.cpu.bus.ppu.frame.width,
            self.gameboy.cpu.bus.ppu.frame.height,
        );
        self.frame_ready = false;
        self.script_engine = ScriptEngine::new();
        self.frame_deadline = Instant::now() + self.frame_period;

        Ok(())
    }

    fn frontend_ref(&self) -> &shared::render::Frame {
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

    fn step_instruction(&mut self, volume: u8) {
        self.gameboy.step(self.apu_sample_cycles);
        self.step_end(volume);
    }

    fn step_frame(&mut self, volume: u8) {
        self.gameboy.take_frame();

        let mut cycles = 0;
        while !self.gameboy.cpu.bus.ppu.frame_ready && cycles < T_CYCLES_PER_FRAME_DOUBLE {
            cycles += self.gameboy.step(self.apu_sample_cycles);

            if self.gameboy.cpu.breakpoint_hit.is_some()
                || self.gameboy.cpu.bus.watchpoint_pause.get()
            {
                break;
            }
        }

        self.step_end(volume);
    }

    fn set_resume(&mut self) {
        self.gameboy.cpu.resume_from = Some(
            self.gameboy
                .cpu
                .registers
                .program_counter
                .address
                .wrapping_sub(1),
        );
    }
}

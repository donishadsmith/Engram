/* References:
   - https://gbdev.io/pandocs
   - https://aquova.net/emudev/gb
   - https://github.com/mvdnes/rboy
   - https://github.com/smparsons/retroboy
*/

#![windows_subsystem = "windows"]

pub mod components;

use crate::components::{
    gameboy::{GameBoy, STATE_MAGIC_NAME, T_CYCLES_PER_FRAME_DOUBLE},
    gamepak::GamePak,
};
use shared::{
    Emulator, EmulatorId, EmulatorSession, EmulatorState,
    audio::{AUDIO_BUFFER_CAPACITY, AUDIO_TARGET_OCCUPANCY, AudioOutput},
    constants::{ALLOWED_AUDIO_PITCH_DEVIATION, MAX_FRAMES_PER_CALL},
    render::{Frame, Screen},
    script::ScriptEngine,
};
use spin_sleep::sleep_until;
use std::{
    fs::{read, rename, write},
    io::Error,
    path::PathBuf,
    time::{Duration, Instant},
};

const GB_CLOCK_SPEED: f64 = 4194304.0;

pub struct GameBoySession {
    audio: Option<AudioOutput>,
    gameboy: GameBoy,
    screen: Screen,
    frame_ready: bool,
    script_engine: ScriptEngine,
    frame_period: Duration,
    frame_deadline: Instant,
    fps_start: Instant,
    fps_frames: u64,
    fps: f64,
    menu_height: f32,
}

impl GameBoySession {
    pub fn new_session(rom_path: PathBuf) -> Result<Self, Error> {
        let audio = AudioOutput::new();
        let gamepak = GamePak::load(rom_path)?;
        let sample_rate = audio.as_ref().map_or(48000, |audio| audio.sample_rate) as f64;
        let apu_sample_period = GB_CLOCK_SPEED / sample_rate;
        let gameboy = GameBoy::boot(gamepak, apu_sample_period);
        let screen = Screen::new(
            gameboy.cpu.bus.ppu.frame.width,
            gameboy.cpu.bus.ppu.frame.height,
        );
        let frame_period = Duration::from_secs_f64(70224.0 / GB_CLOCK_SPEED);
        let frame_deadline = Instant::now() + frame_period;

        Ok(Self {
            audio,
            gameboy,
            screen,
            frame_ready: false,
            script_engine: ScriptEngine::new(),
            frame_period,
            frame_deadline,
            fps_start: Instant::now(),
            fps_frames: 0,
            fps: 0.0,
            menu_height: 0.0,
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
        self.frame_ready = self.gameboy.take_frame();
        self.compute_fps(1u64);

        self.script_engine
            .execute(&mut self.gameboy, EmulatorId::Gb, true);

        self.screen.update(&mut self.gameboy.cpu.bus.ppu.frontend);
    }

    fn draw(&mut self) {
        self.screen
            .draw(&self.gameboy.cpu.bus.ppu.frontend, self.menu_height);
    }

    fn step_end(&mut self, volume: u8) {
        self.drain_audio(volume);
        self.gameboy.replenish_remaining_cycles();
        self.gameboy.take_watchpoint_pause();
        self.script_engine.step_completed();

        if self.gameboy.cpu.bus.ppu.frame_ready
            || self.gameboy.cpu.bus.ppu.lcd_off_frame && self.gameboy.remaining_cycles == 0
        {
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

    // https://github.com/libretro/docs/blob/master/archive/ratecontrol.pdf
    fn dynamic_rate_control(&mut self) {
        let Some(audio) = &self.audio else { return };
        let current_occupancy = (AUDIO_BUFFER_CAPACITY - audio.producer.slots()) as f64;
        let difference_from_target = ((current_occupancy - (AUDIO_TARGET_OCCUPANCY as f64))
            / (AUDIO_TARGET_OCCUPANCY as f64))
            .clamp(-1.0, 1.0);
        let ideal_sample_period = GB_CLOCK_SPEED as f64 / audio.sample_rate as f64;
        self.gameboy.apu_sample_period =
            ideal_sample_period * (1.0 + ALLOWED_AUDIO_PITCH_DEVIATION * difference_from_target);
    }

    fn audio_level(&self) -> usize {
        self.audio.as_ref().map_or(AUDIO_TARGET_OCCUPANCY, |audio| {
            AUDIO_BUFFER_CAPACITY - audio.producer.slots()
        })
    }

    fn catch_up(&self) -> bool {
        Instant::now() > self.frame_deadline + self.frame_period
            || self.audio_level() < AUDIO_TARGET_OCCUPANCY / 2
    }

    fn run_frame(&mut self, volume: u8) -> bool {
        loop {
            self.gameboy.run();
            self.drain_audio(volume);

            if self.gameboy.cpu.bus.ppu.frame_ready
                || self.gameboy.cpu.bus.ppu.lcd_off_frame && self.gameboy.remaining_cycles == 0
            {
                self.on_frame();
            }

            if self.gameboy.cpu.breakpoint_hit.is_some()
                || self.gameboy.cpu.bus.watchpoint_pause.get()
            {
                return false;
            }

            if self.gameboy.remaining_cycles == 0 {
                return true;
            }
        }
    }

    fn compute_fps(&mut self, frames: u64) {
        self.fps_frames += frames;

        let elapsed_time = self.fps_start.elapsed().as_secs_f64();
        if self.fps_start.elapsed().as_secs_f64() >= 1.0 {
            self.fps = self.fps_frames as f64 / elapsed_time;
            self.fps_frames = 0;
            self.fps_start = Instant::now();
        }
    }
}

impl EmulatorSession for GameBoySession {
    fn run(&mut self, input: &[bool], volume: u8) -> Result<EmulatorState, Error> {
        self.frame_ready = false;
        self.gameboy.keypad = input.try_into().unwrap_or([false; 8]);

        // https://nightshade256.github.io/2021/03/27/gb-sound-emulation.html
        self.dynamic_rate_control();

        let mut frames = 1;
        if self.run_frame(volume) {
            while frames < MAX_FRAMES_PER_CALL && self.catch_up() && self.run_frame(volume) {
                self.frame_deadline += self.frame_period;
                frames += 1;
            }
        }

        let breakpoint_action = self
            .gameboy
            .cpu
            .breakpoint_hit
            .and_then(|address| self.gameboy.cpu.breakpoint_queue.get(&address).copied());
        self.script_engine
            .execute(&mut self.gameboy, EmulatorId::Gb, false);

        let state = if self.gameboy.take_watchpoint_pause() {
            EmulatorState::Paused
        } else {
            breakpoint_action.unwrap_or(EmulatorState::Running)
        };

        self.draw();

        let current_time = Instant::now();
        if current_time > self.frame_deadline + self.frame_period * MAX_FRAMES_PER_CALL {
            self.frame_deadline = current_time;
        }

        sleep_until(self.frame_deadline);
        self.frame_deadline += self.frame_period;

        Ok(state)
    }

    fn pause(&mut self) {
        self.script_engine
            .execute(&mut self.gameboy, EmulatorId::Gb, false);

        self.screen
            .draw(&self.gameboy.cpu.bus.ppu.frontend, self.menu_height);
    }

    fn save_game(&mut self) -> Result<(), Error> {
        self.gameboy.save()
    }

    fn reset(&mut self, rom_path: PathBuf) -> Result<(), Error> {
        self.audio = AudioOutput::new();
        let gamepak = GamePak::load(rom_path)?;
        let sample_rate = self.audio.as_ref().map_or(48000, |audio| audio.sample_rate) as f64;
        let apu_sample_period = GB_CLOCK_SPEED / sample_rate;
        self.gameboy = GameBoy::boot(gamepak, apu_sample_period);
        self.screen = Screen::new(
            self.gameboy.cpu.bus.ppu.frame.width,
            self.gameboy.cpu.bus.ppu.frame.height,
        );
        self.frame_ready = false;
        self.script_engine = ScriptEngine::new();
        self.frame_deadline = Instant::now() + self.frame_period;

        Ok(())
    }

    fn frontend_ref(&self) -> &Frame {
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
        self.gameboy.step();
        self.step_end(volume);
    }

    fn step_frame(&mut self, volume: u8) {
        self.gameboy.take_frame();

        let mut cycles = 0;
        while !self.gameboy.cpu.bus.ppu.frame_ready && cycles < T_CYCLES_PER_FRAME_DOUBLE {
            cycles += self.gameboy.step();

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
                .wrapping_sub(1) as u32,
        );
    }

    // gonna have a single state to get it working now and then later
    // do the annoying frontend and file plumbing for multiple states
    fn save_state(&mut self) -> Result<(), Error> {
        let state_path = self.gameboy.cpu.bus.gamepak.sav_path.with_extension("ss1");
        let tmp_path = state_path.with_extension("tmp");
        write(&tmp_path, self.gameboy.save_state(STATE_MAGIC_NAME)?)?;
        rename(&tmp_path, &state_path)
    }

    fn load_state(&mut self) -> Result<(), Error> {
        let state_path = self.gameboy.cpu.bus.gamepak.sav_path.with_extension("ss1");
        let bytes = read(state_path)?;
        self.gameboy.load_state(&bytes)
    }

    fn get_fps(&self) -> f64 {
        self.fps
    }

    fn reset_fps(&mut self) {
        self.fps = 0.0;
        self.fps_frames = 0;
        self.fps_start = Instant::now();
    }

    fn set_top_height(&mut self, menu_height: f32) {
        self.menu_height = menu_height;
    }
}

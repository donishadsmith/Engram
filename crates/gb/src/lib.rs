/* References:
   - https://gbdev.io/pandocs
   - https://aquova.net/emudev/gb
   - https://github.com/mvdnes/rboy
   - https://github.com/smparsons/retroboy
*/

#![windows_subsystem = "windows"]

pub mod components;

use std::{
    io::Error,
    mem::take,
    path::{Path, PathBuf},
};

use shared::{
    Audio, Cycles, EmulatorCore, EmulatorId, EmulatorSession, EmulatorState, FrameLoop, FramePacing, FrameTracking,
    Rewind, RewindControl, ScriptTarget, StateIo, Watchpoint,
    audio::AudioOutput,
    config::system_dir,
    constants::GB_REFRESH_RATE,
    render::{Frame, Screen},
    script::ScriptEngine,
    structs::{FramePacer, FrameTracker},
    video::VideoRecorder,
};

use crate::components::{
    gameboy::{GameBoy, T_CYCLES_PER_FRAME_DOUBLE},
    gamepak::GamePak,
};

const GB_CLOCK_SPEED: f64 = 4194304.0;

// TODO: After PSX, abstract similar logic among the systems
pub struct GameBoySession {
    audio: Option<AudioOutput>,
    gameboy: GameBoy,
    screen: Screen,
    frame_ready: bool,
    script_engine: ScriptEngine,
    frame_pacer: FramePacer,
    frame_tracker: FrameTracker,
    menu_height: f32,
    state_dir: PathBuf,
    recorded_audio: Vec<u8>,
    video_recorder: VideoRecorder,
}

impl GameBoySession {
    pub fn new_session(rom_path: PathBuf) -> Result<Self, Error> {
        let audio = AudioOutput::new();
        let gamepak = GamePak::load(rom_path)?;
        let sample_rate = audio.as_ref().map_or(48000, |audio| audio.sample_rate) as f64;
        let apu_sample_period = GB_CLOCK_SPEED / sample_rate;
        let gameboy = GameBoy::boot(gamepak, apu_sample_period);
        let screen = Screen::new(gameboy.cpu.bus.ppu.frame.width, gameboy.cpu.bus.ppu.frame.height);
        let state_dir = system_dir("gb", "states")?;

        Ok(Self {
            audio,
            gameboy,
            screen,
            frame_ready: false,
            script_engine: ScriptEngine::new(),
            frame_pacer: FramePacer::new(70224.0 / GB_CLOCK_SPEED),
            frame_tracker: FrameTracker::new(),
            menu_height: 0.0,
            state_dir,
            recorded_audio: Vec::new(),
            video_recorder: VideoRecorder::new(),
        })
    }

    fn on_frame(&mut self) {
        self.frame_ready = self.gameboy.take_frame();
        self.frame_tracker.compute_fps(1u64);
        self.video_capture();

        if self.gameboy.rewind_manager.is_enabled() {
            self.gameboy.capture_rewind();
        }

        self.script_engine.execute(&mut self.gameboy, EmulatorId::Gb, true);

        self.screen.update(&mut self.gameboy.cpu.bus.ppu.frontend);
    }

    fn step_end(&mut self, volume: u8) {
        self.drain_audio(volume);
        self.gameboy.replenish_remaining_cycles();
        self.gameboy.take_watchpoint_pause();
        self.gameboy.take_cycle_pause();

        if self.gameboy.cpu.bus.ppu.frame_ready
            || self.gameboy.cpu.bus.ppu.lcd_off_frame && self.gameboy.remaining_cycles == 0
        {
            self.on_frame();
        } else {
            self.script_engine.execute(&mut self.gameboy, EmulatorId::Gb, false);
        }

        self.draw();

        if self.gameboy.cpu.breakpoint_manager.hit.is_some() {
            self.set_resume();
        }
    }
}

impl EmulatorCore for GameBoySession {
    fn emulator_ref(&self) -> &dyn ScriptTarget {
        &self.gameboy
    }

    fn emulator_mut(&mut self) -> &mut dyn ScriptTarget {
        &mut self.gameboy
    }

    fn refresh_rate(&self) -> f64 {
        GB_REFRESH_RATE
    }
}

impl FrameTracking for GameBoySession {
    fn frame_tracker_ref(&self) -> &FrameTracker {
        &self.frame_tracker
    }

    fn frame_tracker_mut(&mut self) -> &mut FrameTracker {
        &mut self.frame_tracker
    }
}

impl FramePacing for GameBoySession {
    fn frame_pacer_ref(&self) -> &FramePacer {
        &self.frame_pacer
    }

    fn frame_pacer_mut(&mut self) -> &mut FramePacer {
        &mut self.frame_pacer
    }
}

impl RewindControl for GameBoySession {}

impl StateIo for GameBoySession {
    fn state_dir(&self) -> &Path {
        &self.state_dir
    }

    fn sav_path(&self) -> &Path {
        &self.gameboy.cpu.bus.gamepak.sav_path
    }
}

impl Audio for GameBoySession {
    fn audio_ref(&self) -> Option<&AudioOutput> {
        self.audio.as_ref()
    }

    fn audio_mut(&mut self) -> Option<&mut AudioOutput> {
        self.audio.as_mut()
    }

    fn sample_period(&mut self) -> &mut f64 {
        &mut self.gameboy.apu_sample_period
    }

    fn sample_remainder(&mut self) -> &mut f64 {
        &mut self.gameboy.cpu.bus.apu.sample_counter
    }

    fn sample_buffer(&mut self) -> &mut Vec<f32> {
        &mut self.gameboy.cpu.bus.apu.sample_buffer
    }

    fn recorded_audio(&mut self) -> &mut Vec<u8> {
        &mut self.recorded_audio
    }

    fn set_cycles_per_sample(&mut self, cycles_per_sample: f64) {
        self.gameboy
            .cpu
            .bus
            .apu
            .psg_output
            .set_cycles_per_sample(cycles_per_sample);
    }
}

impl FrameLoop for GameBoySession {
    fn run_frame(&mut self, volume: u8) -> bool {
        loop {
            self.gameboy.run();
            self.drain_audio(volume);

            if self.gameboy.cpu.bus.ppu.frame_ready
                || self.gameboy.cpu.bus.ppu.lcd_off_frame && self.gameboy.remaining_cycles == 0
            {
                self.on_frame();
            }

            if self.gameboy.cpu.breakpoint_manager.hit.is_some()
                || self.gameboy.cpu.bus.watchpoint_manager.pause
                || self.gameboy.pause_at_current_cycle()
            {
                return false;
            }

            if self.gameboy.remaining_cycles == 0 {
                return true;
            }
        }
    }
}

impl EmulatorSession for GameBoySession {
    fn run(&mut self, input: &[bool], volume: u8) -> Result<EmulatorState, Error> {
        self.frame_ready = false;
        self.gameboy.keypad = input.try_into().unwrap_or([false; 8]);
        self.recorded_audio = Vec::new();

        // https://nightshade256.github.io/2021/03/27/gb-sound-emulation.html
        self.dynamic_rate_control();

        self.run_frames(volume);

        let breakpoint_action = self
            .gameboy
            .cpu
            .breakpoint_manager
            .hit
            .and_then(|address| self.gameboy.cpu.breakpoint_manager.queue.get(&address).copied());
        self.script_engine.execute(&mut self.gameboy, EmulatorId::Gb, false);

        let state = if self.gameboy.take_watchpoint_pause() | self.gameboy.take_cycle_pause() {
            EmulatorState::Paused
        } else {
            breakpoint_action.unwrap_or(EmulatorState::Running)
        };

        self.draw();

        self.frame_pacer.sleep();

        Ok(state)
    }

    fn pause(&mut self) {
        self.script_engine.execute(&mut self.gameboy, EmulatorId::Gb, false);

        self.draw();
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
        self.frame_pacer.update_frame_deadline();

        self.reset_fps();

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
        self.video_capture();
    }

    fn step_frame(&mut self, volume: u8) {
        self.gameboy.take_frame();
        self.video_capture();

        let mut cycles = 0;
        while !self.gameboy.cpu.bus.ppu.frame_ready && cycles < T_CYCLES_PER_FRAME_DOUBLE {
            cycles += self.gameboy.step();

            if self.gameboy.cpu.breakpoint_manager.hit.is_some()
                || self.gameboy.cpu.bus.watchpoint_manager.pause
                || self.gameboy.pause_at_current_cycle()
            {
                break;
            }
        }

        self.step_end(volume);
    }

    fn set_resume(&mut self) {
        self.gameboy.cpu.breakpoint_manager.resume_from =
            Some(self.gameboy.cpu.registers.program_counter.address.wrapping_sub(1) as u32);
    }

    fn set_top_height(&mut self, menu_height: f32) {
        self.menu_height = menu_height;
    }

    fn force_feedback_strength(&mut self) -> f32 {
        self.gameboy.rumble_strength
    }

    fn start_video(&mut self, video_dir: PathBuf) -> Result<(), Error> {
        self.video_recorder.start(
            GB_CLOCK_SPEED as f32 / 70224.0,
            (160, 144),
            video_dir,
            self.sample_rate(),
        )?;

        Ok(())
    }

    fn is_video_recording(&self) -> bool {
        self.video_recorder.is_recording()
    }

    fn video_capture(&mut self) {
        self.video_recorder
            .capture(&self.gameboy.cpu.bus.ppu.frontend, take(&mut self.recorded_audio));
    }

    fn stop_video(&mut self) -> Result<(), Error> {
        self.video_recorder.stop()
    }

    fn draw(&mut self) {
        if take(&mut self.gameboy.force_screen_update) {
            self.screen.update(&mut self.gameboy.cpu.bus.ppu.frontend)
        }

        self.screen.draw(&self.gameboy.cpu.bus.ppu.frontend, self.menu_height);
    }
}

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
    components::{
        gamepak::GamePak,
        gba::{GBA, STATE_MAGIC_NAME},
    },
    debug::video::PpuDebugger,
};
use debug::audio::AudioDebugger;
use shared::{
    DebugInterface, Emulator, EmulatorId, EmulatorSession, EmulatorState, SolarSensor,
    audio::{AUDIO_BUFFER_CAPACITY, AUDIO_TARGET_OCCUPANCY, AudioOutput},
    debug::DebugPage,
    render::Screen,
    script::ScriptEngine,
};
use spin_sleep::sleep_until;
use std::{
    fs::{read, rename, write},
    io::Error,
    path::PathBuf,
    time::{Duration, Instant},
};

const GBA_CLOCK_SPEED: u32 = 16777216;
const ALLOWED_AUDIO_PITCH_DEVIATION: f64 = 0.005;
const MAX_FRAMES_PER_CALL: u32 = 3;

pub struct GBASession {
    audio: Option<AudioOutput>,
    audio_debugger: AudioDebugger,
    ppu_debugger: PpuDebugger,
    gba: GBA,
    screen: Screen,
    frame_ready: bool,
    active_debug: Option<DebugPage>,
    script_engine: ScriptEngine,
    frame_period: Duration,
    frame_deadline: Instant,
    fps_start: Instant,
    fps_frames: u64,
    fps: f64,
}

impl GBASession {
    pub fn new_session(rom_path: PathBuf) -> Result<Self, Error> {
        let audio_debugger = AudioDebugger::new();
        let ppu_debugger = PpuDebugger::new();
        let audio = AudioOutput::new();
        let gamepak = GamePak::load(rom_path)?;
        let sample_rate = audio.as_ref().map_or(48000, |audio| audio.sample_rate);
        let apu_sample_period = GBA_CLOCK_SPEED as f64 / sample_rate as f64;
        let gba = GBA::boot(gamepak, apu_sample_period);
        let screen = Screen::new(gba.bus.ppu.frame.width, gba.bus.ppu.frame.height);
        let frame_period = Duration::from_secs_f64(280896.0 / GBA_CLOCK_SPEED as f64);
        let frame_deadline = Instant::now() + frame_period;

        Ok(Self {
            audio,
            audio_debugger,
            ppu_debugger,
            gba,
            screen,
            frame_ready: false,
            active_debug: None,
            script_engine: ScriptEngine::new(),
            frame_deadline,
            frame_period,
            fps_start: Instant::now(),
            fps_frames: 0,
            fps: 0.0,
        })
    }

    fn on_frame(&mut self) {
        self.frame_ready = self.gba.take_frame();
        self.compute_fps(1u64);

        self.script_engine
            .execute(&mut self.gba, EmulatorId::Gba, true);

        if self.active_debug.is_none() {
            self.screen.update(&mut self.gba.bus.ppu.frontend);
        }
    }

    fn draw(&mut self) {
        if self.active_debug.is_none() {
            self.screen.draw(&self.gba.bus.ppu.frontend);
        }
    }

    fn step_end(&mut self) {
        self.gba.take_watchpoint_pause();
        self.script_engine.step_completed();

        if self.gba.bus.ppu.frame_ready {
            self.on_frame();
        } else {
            self.script_engine
                .execute(&mut self.gba, EmulatorId::Gba, false);
        }

        self.draw();
    }

    fn drain_audio(&mut self, volume: u8) {
        for sample in self.gba.bus.apu.sample_buffer.drain(..) {
            match &mut self.audio {
                Some(audio) => audio.play(sample, volume),
                None => {}
            }
        }
    }

    // https://github.com/libretro/docs/blob/master/archive/ratecontrol.pdf
    fn dynamic_rate_control(&mut self) {
        let Some(audio) = &self.audio else { return };

        let current_occupancy = (AUDIO_BUFFER_CAPACITY - audio.producer.slots()) as f64;
        let difference_from_target = ((current_occupancy - (AUDIO_TARGET_OCCUPANCY as f64))
            / (AUDIO_TARGET_OCCUPANCY as f64))
            .clamp(-1.0, 1.0);
        let ideal_sample_period = GBA_CLOCK_SPEED as f64 / audio.sample_rate as f64;
        self.gba.apu_sample_period =
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

    fn tick(&mut self, volume: u8) {
        self.gba.run();

        self.drain_audio(volume);

        if self.gba.cpu.breakpoint_hit.is_some() {
            self.set_resume();
        }
    }

    fn run_frame(&mut self, volume: u8) -> bool {
        loop {
            self.tick(volume);

            if self.gba.cpu.breakpoint_hit.is_some() || self.gba.bus.watchpoint_pause {
                return false;
            }

            if self.gba.bus.ppu.frame_ready {
                self.on_frame();

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

impl EmulatorSession for GBASession {
    fn run(&mut self, input: &[bool], volume: u8) -> Result<EmulatorState, Error> {
        self.frame_ready = false;
        self.gba.keypad = input.try_into().unwrap_or([false; 10]);

        self.dynamic_rate_control();

        let mut frames = 1;
        if self.run_frame(volume) {
            while frames < MAX_FRAMES_PER_CALL && self.catch_up() && self.run_frame(volume) {
                self.frame_deadline += self.frame_period;
                frames += 1;
            }
        }

        let breakpoint_action = self
            .gba
            .cpu
            .breakpoint_hit
            .and_then(|address| self.gba.cpu.breakpoint_queue.get(&address).copied());
        self.script_engine
            .execute(&mut self.gba, EmulatorId::Gba, false);

        let state = if self.gba.take_watchpoint_pause() {
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
        let sample_rate = self.audio.as_ref().map_or(44100, |audio| audio.sample_rate);
        let apu_sample_period = GBA_CLOCK_SPEED as f64 / sample_rate as f64;
        self.gba = GBA::boot(gamepak, apu_sample_period);
        self.screen = Screen::new(self.gba.bus.ppu.frame.width, self.gba.bus.ppu.frame.height);
        self.frame_ready = false;
        self.active_debug = None;
        self.script_engine = ScriptEngine::new();
        self.frame_deadline = Instant::now() + self.frame_period;

        self.fps_start = Instant::now();
        self.fps_frames = 0;
        self.fps = 0.0;

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
        self.step_end();
    }

    fn step_frame(&mut self, volume: u8) {
        self.gba.take_frame();
        while !self.gba.bus.ppu.frame_ready {
            self.tick(volume);

            if self.gba.cpu.breakpoint_hit.is_some() || self.gba.bus.watchpoint_pause {
                break;
            }
        }

        self.step_end();
    }

    fn set_resume(&mut self) {
        self.gba.cpu.resume_from = Some(self.gba.cpu.next_executing_address());
    }

    // probably for the playstation since it will be quite some time before save states are supported
    fn save_state(&mut self) -> Result<(), Error> {
        let state_path = self.gba.bus.gamepak.sav_path.with_extension("ss1");
        let tmp_path = state_path.with_extension("tmp");
        write(&tmp_path, self.gba.save_state(STATE_MAGIC_NAME)?)?;
        rename(&tmp_path, &state_path)
    }

    fn load_state(&mut self) -> Result<(), Error> {
        let state_path = self.gba.bus.gamepak.sav_path.with_extension("ss1");
        let bytes = read(state_path)?;
        self.gba.load_state(&bytes)
    }

    fn get_fps(&self) -> f64 {
        self.fps
    }

    fn reset_fps(&mut self) {
        self.fps = 0.0;
        self.fps_frames = 0;
        self.fps_start = Instant::now();
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

        self.screen.update(&mut self.gba.bus.ppu.frontend);

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

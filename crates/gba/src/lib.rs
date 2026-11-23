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

use std::{
    io::Error,
    mem::take,
    path::{Path, PathBuf},
};

use debug::audio::AudioDebugger;
use egui::Ui;
use shared::{
    Audio, Cycles, DebugInterface, EmulatorCore, EmulatorId, EmulatorSession, EmulatorState, FrameLoop, FramePacing,
    FrameTracking, Rewind, RewindControl, ScriptTarget, SolarSensor, StateIo, Watchpoint,
    audio::AudioOutput,
    config::system_dir,
    constants::GB_REFRESH_RATE,
    debug::DebugPage,
    render::{Frame, Screen},
    script::ScriptEngine,
    structs::{FramePacer, FrameTracker},
    video::VideoRecorder,
};

use crate::{
    components::{gamepak::GamePak, gba::GBA},
    debug::video::PpuDebugger,
};

const GBA_CLOCK_SPEED: u32 = 16777216;

// TODO: After PSX, abstract similar logic among the systems
// TODO: figure out more optimizaitons to increase fps for fastforward

pub struct GBASession {
    audio: Option<AudioOutput>,
    audio_debugger: AudioDebugger,
    ppu_debugger: PpuDebugger,
    gba: GBA,
    screen: Screen,
    frame_ready: bool,
    active_debug: Option<DebugPage>,
    script_engine: ScriptEngine,
    frame_pacer: FramePacer,
    frame_tracker: FrameTracker,
    menu_height: f32,
    state_dir: PathBuf,
    recorded_audio: Vec<u8>,
    video_recorder: VideoRecorder,
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
        let state_dir = system_dir("gba", "states")?;

        Ok(Self {
            audio,
            audio_debugger,
            ppu_debugger,
            gba,
            screen,
            frame_ready: false,
            active_debug: None,
            script_engine: ScriptEngine::new(),
            frame_pacer: FramePacer::new(280896.0 / GBA_CLOCK_SPEED as f64),
            frame_tracker: FrameTracker::new(),
            menu_height: 0.0,
            state_dir,
            recorded_audio: Vec::new(),
            video_recorder: VideoRecorder::new(),
        })
    }

    fn on_frame(&mut self) {
        self.frame_ready = self.gba.take_frame();
        self.frame_tracker.compute_fps(1u64);
        self.video_capture();

        if self.gba.rewind_manager.is_enabled() {
            self.gba.capture_rewind();
        }

        self.script_engine.execute(&mut self.gba, EmulatorId::Gba, true);

        if self.active_debug.is_none() {
            self.screen.update(&mut self.gba.bus.ppu.frontend);
        }
    }

    fn step_end(&mut self) {
        self.gba.take_watchpoint_pause();
        self.gba.take_cycle_pause();

        if self.gba.bus.ppu.frame_ready {
            self.on_frame();
        } else {
            self.script_engine.execute(&mut self.gba, EmulatorId::Gba, false);
        }

        self.draw();
    }

    fn tick(&mut self, volume: u8) {
        self.gba.run();

        self.drain_audio(volume);

        if self.gba.cpu.breakpoint_manager.hit.is_some() {
            self.set_resume();
        }
    }
}

impl EmulatorCore for GBASession {
    fn emulator_ref(&self) -> &dyn ScriptTarget {
        &self.gba
    }

    fn emulator_mut(&mut self) -> &mut dyn ScriptTarget {
        &mut self.gba
    }

    fn refresh_rate(&self) -> f64 {
        GB_REFRESH_RATE
    }
}

impl FrameTracking for GBASession {
    fn frame_tracker_ref(&self) -> &FrameTracker {
        &self.frame_tracker
    }

    fn frame_tracker_mut(&mut self) -> &mut FrameTracker {
        &mut self.frame_tracker
    }
}

// of course emerald maxes at 220 :/, hamtaro maxes at 340 ish so speed
// limit will be set to 5 even though gb technically can go faser,
// breakpoint just docks 20 fps due to the hashmap key check (before adding
// range check), refine later since it works now with very minimal changes
// anyway
impl FramePacing for GBASession {
    fn frame_pacer_ref(&self) -> &FramePacer {
        &self.frame_pacer
    }

    fn frame_pacer_mut(&mut self) -> &mut FramePacer {
        &mut self.frame_pacer
    }
}

impl RewindControl for GBASession {}

impl StateIo for GBASession {
    fn state_dir(&self) -> &Path {
        &self.state_dir
    }

    fn sav_path(&self) -> &Path {
        &self.gba.bus.gamepak.sav_path
    }
}

impl Audio for GBASession {
    fn audio_ref(&self) -> Option<&AudioOutput> {
        self.audio.as_ref()
    }

    fn audio_mut(&mut self) -> Option<&mut AudioOutput> {
        self.audio.as_mut()
    }

    fn sample_period(&mut self) -> &mut f64 {
        &mut self.gba.apu_sample_period
    }

    fn sample_remainder(&mut self) -> &mut f64 {
        &mut self.gba.apu_sample_remainder
    }

    fn sample_buffer(&mut self) -> &mut Vec<f32> {
        &mut self.gba.bus.apu.sample_buffer
    }

    fn recorded_audio(&mut self) -> &mut Vec<u8> {
        &mut self.recorded_audio
    }

    fn set_cycles_per_sample(&mut self, cycles_per_sample: f64) {
        self.gba
            .bus
            .apu
            .psg_output
            .set_cycles_per_sample(cycles_per_sample / 4.0);
    }
}

impl FrameLoop for GBASession {
    fn run_frame(&mut self, volume: u8) -> bool {
        loop {
            if self.gba.debugging_active() {
                self.tick(volume);
            } else {
                self.gba.run_until_event();
                self.drain_audio(volume);
            }

            if self.gba.cpu.breakpoint_manager.hit.is_some()
                || self.gba.bus.watchpoint_manager.pause
                || self.gba.pause_at_current_cycle()
            {
                return false;
            }

            if self.gba.bus.ppu.frame_ready {
                self.on_frame();

                return true;
            }
        }
    }
}

impl EmulatorSession for GBASession {
    fn run(&mut self, input: &[bool], volume: u8) -> Result<EmulatorState, Error> {
        self.frame_ready = false;
        self.gba.keypad = input.try_into().unwrap_or([false; 10]);

        self.dynamic_rate_control();

        self.run_frames(volume);

        let breakpoint_action = self
            .gba
            .cpu
            .breakpoint_manager
            .hit
            .and_then(|address| self.gba.cpu.breakpoint_manager.queue.get(&address).copied());
        self.script_engine.execute(&mut self.gba, EmulatorId::Gba, false);

        let state = if self.gba.take_watchpoint_pause() | self.gba.take_cycle_pause() {
            EmulatorState::Paused
        } else {
            breakpoint_action.unwrap_or(EmulatorState::Running)
        };

        self.draw();

        self.frame_pacer.sleep();

        Ok(state)
    }

    fn pause(&mut self) {
        self.script_engine.execute(&mut self.gba, EmulatorId::Gba, false);

        self.draw();
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
        let sample_rate = self.audio.as_ref().map_or(48000, |audio| audio.sample_rate);
        let apu_sample_period = GBA_CLOCK_SPEED as f64 / sample_rate as f64;
        self.gba = GBA::boot(gamepak, apu_sample_period);
        self.screen = Screen::new(self.gba.bus.ppu.frame.width, self.gba.bus.ppu.frame.height);
        self.frame_ready = false;
        self.frame_pacer.update_frame_deadline();
        self.recorded_audio = Vec::new();

        self.reset_fps();

        Ok(())
    }

    fn frontend_ref(&self) -> &Frame {
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
        // just in case i forgot something that can freeze
        for _ in 0..10 {
            self.tick(volume);

            if take(&mut self.gba.cpu.instruction_executed)
                || self.gba.cpu.breakpoint_manager.hit.is_some()
                || self.gba.bus.watchpoint_manager.pause
                || self.gba.cpu.is_halted()
                || self.gba.pause_at_current_cycle()
            {
                break;
            }
        }

        self.step_end();
    }

    fn step_frame(&mut self, volume: u8) {
        self.gba.take_frame();

        while !self.gba.bus.ppu.frame_ready {
            self.tick(volume);

            if self.gba.cpu.breakpoint_manager.hit.is_some()
                || self.gba.bus.watchpoint_manager.pause
                || self.gba.pause_at_current_cycle()
            {
                break;
            }
        }

        self.step_end();
    }

    fn set_resume(&mut self) {
        self.gba.cpu.breakpoint_manager.resume_from = Some(self.gba.cpu.next_executing_address());
    }

    fn set_top_height(&mut self, menu_height: f32) {
        self.menu_height = menu_height;
    }

    fn take_debugger_snapshot(&mut self) {
        self.gba.bus.ppu.debugger_snapshot();
    }

    fn start_video(&mut self, video_dir: PathBuf) -> Result<(), Error> {
        self.video_recorder.start(
            GBA_CLOCK_SPEED as f32 / 280896.0,
            (240, 160),
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
            .capture(&self.gba.bus.ppu.frontend, take(&mut self.recorded_audio));
    }

    fn stop_video(&mut self) -> Result<(), Error> {
        self.video_recorder.stop()
    }

    fn draw(&mut self) {
        if take(&mut self.gba.force_screen_update) {
            self.screen.update(&mut self.gba.bus.ppu.frontend)
        }

        if self.active_debug.is_none() {
            self.screen.draw(&self.gba.bus.ppu.frontend, self.menu_height);
        }
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
        self.gba.bus.ppu.frontend.buffer_changed = true;

        self.active_debug = if self.active_debug == debug_page {
            None
        } else {
            debug_page
        };
    }

    fn show_ui(&mut self, ui: &mut Ui) {
        match self.active_debug {
            Some(DebugPage::Audio) => self.audio_debugger.show_ui(ui, &mut self.gba),
            Some(DebugPage::Video) => self.ppu_debugger.show_ui(ui, &mut self.gba),
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

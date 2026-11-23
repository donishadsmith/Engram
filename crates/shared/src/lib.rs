pub mod audio;
pub mod config;
pub mod constants;
pub mod debug;
pub mod enums;
pub mod gif;
pub mod input;
pub mod lua_lab;
pub mod psg;
pub mod render;
pub mod scheduler;
pub mod script;
pub mod structs;
pub mod traits;
pub mod utils;
pub mod video;

use std::{
    fs::{read, rename, write},
    io::Error,
    mem::take,
    path::{Path, PathBuf},
    time::Instant,
};

use egui::Ui;
use serde::{Deserialize, Serialize};

use crate::{
    audio::{AUDIO_BUFFER_CAPACITY, AUDIO_TARGET_OCCUPANCY, AudioOutput},
    constants::{ALLOWED_AUDIO_PITCH_DEVIATION, MAX_FRAMES_PER_CALL, SAVE_STATE_SLOTS},
    debug::DebugPage,
    input::{AnalogSticks, enums::KeyId},
    render::Frame,
    script::{CpuError, DomainError, ScriptEngine, WatchpointArgs, WatchpointHit, WatchpointRange},
    structs::{
        BreakpointManager, CycleManager, ElapsedCycles, FramePacer, FrameTracker, RewindManager, WatchpointManager,
    },
    utils::error_message,
};

#[derive(Clone, Copy, PartialEq, Eq, Hash, Deserialize, Serialize)]
pub enum EmulatorId {
    Gb,
    Gba,
}

impl EmulatorId {
    pub fn to_key_id(self) -> KeyId {
        match self {
            EmulatorId::Gb => KeyId::Emulator(self),
            EmulatorId::Gba => KeyId::Emulator(self),
        }
    }

    pub fn supports_multidirectional_control(self) -> bool {
        match self {
            EmulatorId::Gb | EmulatorId::Gba => false,
        }
    }
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
    SwapDisc,
}

pub trait Emulator {
    fn save(&mut self) -> Result<(), Error>;

    fn frontend_mut(&mut self) -> &mut Frame;
}

pub trait DebugInterface {
    fn toggle(&mut self, _debug_page: Option<DebugPage>);

    fn available_pages(&self) -> Vec<DebugPage>;

    fn show_ui(&mut self, _ui: &mut Ui);

    fn visible(&self, _debug_page: DebugPage) -> bool;

    fn active(&self) -> bool;
}

pub trait SolarSensor {
    fn set_level(&mut self, _solar_level: u8) {}
}

pub trait EmulatorSession:
    EmulatorCore + FrameTracking + FramePacing + Audio + FrameLoop + RewindControl + StateIo
{
    // eventually allow emu to return running or paused based on internal
    // state/conditions
    fn run(&mut self, input: &[bool], volume: u8) -> Result<EmulatorState, Error>;

    fn save_game(&mut self) -> Result<(), Error> {
        self.emulator_mut().save()
    }

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

    fn step_frame(&mut self, volume: u8);

    fn set_resume(&mut self);

    fn set_top_height(&mut self, menu_height: f32);

    fn set_analog(&mut self, _analog_sticks: AnalogSticks) {}

    fn force_feedback_strength(&mut self) -> f32 {
        0.0
    }

    fn take_debugger_snapshot(&mut self) {}

    fn start_video(&mut self, video_dir: PathBuf) -> Result<(), Error>;

    fn is_video_recording(&self) -> bool;

    fn video_capture(&mut self) {}

    fn stop_video(&mut self) -> Result<(), Error>;

    fn draw(&mut self) {}
}

pub trait EmulatorCore {
    fn emulator_ref(&self) -> &dyn ScriptTarget;

    fn emulator_mut(&mut self) -> &mut dyn ScriptTarget;

    fn clock_rate(&self) -> f64 {
        self.emulator_ref().clock_rate()
    }

    fn refresh_rate(&self) -> f64;
}

pub trait FrameTracking {
    fn frame_tracker_ref(&self) -> &FrameTracker;

    fn frame_tracker_mut(&mut self) -> &mut FrameTracker;

    fn get_fps(&self) -> f64 {
        self.frame_tracker_ref().fps
    }

    fn reset_fps(&mut self) {
        self.frame_tracker_mut().reset();
    }
}

pub trait FramePacing {
    fn frame_pacer_ref(&self) -> &FramePacer;

    fn frame_pacer_mut(&mut self) -> &mut FramePacer;

    fn speed(&self) -> f32 {
        self.frame_pacer_ref().speed
    }

    fn set_speed(&mut self, speed: f32) {
        self.frame_pacer_mut().set_speed(speed);
    }

    fn update_frame_deadline(&mut self) {
        self.frame_pacer_mut().update_frame_deadline();
    }
}

pub trait Audio: EmulatorCore + FrameTracking + FramePacing {
    fn audio_ref(&self) -> Option<&AudioOutput>;

    fn audio_mut(&mut self) -> Option<&mut AudioOutput>;

    fn sample_period(&mut self) -> &mut f64;

    fn sample_remainder(&mut self) -> &mut f64;

    fn sample_buffer(&mut self) -> &mut Vec<f32>;

    fn recorded_audio(&mut self) -> &mut Vec<u8>;

    fn set_cycles_per_sample(&mut self, cycles_per_sample: f64);

    fn sample_rate(&self) -> u32 {
        self.audio_ref().map_or(48000, |audio| audio.sample_rate)
    }

    fn audio_level(&self) -> usize {
        self.audio_ref().map_or(AUDIO_TARGET_OCCUPANCY, |audio| {
            AUDIO_BUFFER_CAPACITY - audio.producer.slots()
        })
    }

    fn drain_audio(&mut self, volume: u8) {
        let mut sample_buffer = take(self.sample_buffer());

        for sample in sample_buffer.drain(..) {
            let _ = self.audio_mut().map(|audio| audio.play(sample, volume));
            self.recorded_audio().extend_from_slice(&f32::to_le_bytes(sample));
        }

        *self.sample_buffer() = sample_buffer;
    }

    // https://github.com/libretro/docs/blob/master/archive/ratecontrol.pdf
    fn dynamic_rate_control(&mut self) {
        let Some(audio) = self.audio_ref() else {
            return;
        };

        let sample_rate = audio.sample_rate as f64;
        let current_occupancy = (AUDIO_BUFFER_CAPACITY - audio.producer.slots()) as f64;
        let difference_from_target =
            ((current_occupancy - (AUDIO_TARGET_OCCUPANCY as f64)) / (AUDIO_TARGET_OCCUPANCY as f64)).clamp(-1.0, 1.0);
        let ratio = if self.speed() < 1.0 {
            self.speed() as f64
        } else {
            (self.get_fps() / self.refresh_rate()).clamp(1.0, self.speed() as f64)
        };
        let ideal_sample_period = self.clock_rate() / sample_rate * ratio;
        let sample_period = ideal_sample_period * (1.0 + ALLOWED_AUDIO_PITCH_DEVIATION * difference_from_target);

        *self.sample_period() = sample_period;
        self.set_cycles_per_sample(sample_period);
    }
}

pub trait FrameLoop: Audio {
    fn run_frame(&mut self, volume: u8) -> bool;

    fn catch_up(&self) -> bool {
        let frame_pacer = self.frame_pacer_ref();

        Instant::now() > frame_pacer.frame_deadline + frame_pacer.frame_period
            || self.audio_level() < AUDIO_TARGET_OCCUPANCY / 2
    }

    fn run_frames(&mut self, volume: u8) {
        if self.speed() <= 1.0 {
            let mut frames = 1;
            if self.run_frame(volume) {
                while frames < MAX_FRAMES_PER_CALL && self.catch_up() && self.run_frame(volume) {
                    let frame_pacer = self.frame_pacer_mut();
                    frame_pacer.frame_deadline += frame_pacer.frame_period;
                    frames += 1;
                }
            }
        } else {
            let start = Instant::now();
            for _ in 0..(self.speed() as u8) {
                if !self.run_frame(volume) || start.elapsed() >= self.frame_pacer_ref().frame_period {
                    break;
                }
            }

            let mut frames = 0;
            while frames < MAX_FRAMES_PER_CALL - 1
                && self.audio_level() < AUDIO_TARGET_OCCUPANCY / 2
                && self.run_frame(volume)
            {
                frames += 1;
            }
        }
    }
}

pub trait RewindControl: EmulatorCore {
    fn rewind(&mut self) -> Option<f64> {
        self.emulator_mut().rewind(1)
    }

    fn set_rewind_capacity(&mut self, capacity: usize) {
        let rewind_manager = self.emulator_mut().rewind_manager_mut();
        let previously_enabled = rewind_manager.is_enabled();

        rewind_manager.capacity = capacity;
        if previously_enabled && !rewind_manager.is_enabled() {
            rewind_manager.buffer.clear();
        }
    }

    fn can_rewind(&self) -> bool {
        !self.emulator_ref().rewind_manager_ref().buffer.is_empty()
    }
}

pub trait StateIo: EmulatorCore {
    fn state_dir(&self) -> &Path;

    fn sav_path(&self) -> &Path;

    fn state_path(&self, slot: usize) -> Result<PathBuf, Error> {
        let rom_stem = self
            .sav_path()
            .file_stem()
            .ok_or_else(|| Error::other("ROM path has no file name"))?;

        Ok(self
            .state_dir()
            .join(format!("{}.ss{slot}", rom_stem.to_string_lossy())))
    }

    fn save_state(&self, slot: usize) -> Result<(), Error> {
        let state_path = self.state_path(slot)?;
        let tmp_path = state_path.with_extension(format!("ss{slot}.tmp"));

        write(&tmp_path, self.emulator_ref().snapshot()?)?;
        rename(&tmp_path, &state_path)
    }

    fn load_state(&mut self, slot: usize) -> Result<(), Error> {
        let bytes = read(self.state_path(slot)?)?;

        self.emulator_mut().load_state(&bytes)
    }

    fn current_save_states(&self) -> Vec<Option<PathBuf>> {
        (1..=SAVE_STATE_SLOTS)
            .map(|slot| self.state_path(slot).ok().filter(|path| path.exists()))
            .collect()
    }

    fn get_rom_name(&self) -> String {
        // good enough
        self.sav_path().file_stem().unwrap().to_owned().into_string().unwrap()
    }
}

pub trait Breakpoint {
    fn breakpoint_manager_ref(&self) -> &BreakpointManager;

    fn breakpoint_manager_mut(&mut self) -> &mut BreakpointManager;

    fn set_breakpoint(&mut self, address: u32, pause: bool) -> bool {
        self.breakpoint_manager_mut().set(address, pause)
    }

    fn remove_breakpoint(&mut self, address: u32) -> bool {
        self.breakpoint_manager_mut().remove(address)
    }

    fn take_breakpoint_hit(&mut self) -> Option<u32> {
        take(&mut self.breakpoint_manager_mut().hit)
    }

    fn clear_all_breakpoints(&mut self) {
        self.breakpoint_manager_mut().queue.clear();
        self.breakpoint_manager_mut().range = None;
    }

    fn check_breakpoints(&self) -> Vec<(u32, EmulatorState)> {
        self.breakpoint_manager_ref().queue.clone().into_iter().collect()
    }
}

pub trait Watchpoint {
    fn watchpoint_manager_ref(&self) -> &WatchpointManager;

    fn watchpoint_manager_mut(&mut self) -> &mut WatchpointManager;

    fn check_watchpoints(&self) -> Vec<(WatchpointRange, WatchpointArgs)> {
        self.watchpoint_manager_ref().queue.clone().into_iter().collect()
    }

    fn set_watchpoint(&mut self, watchpoint_range: WatchpointRange, watchpoint_args: WatchpointArgs) -> bool {
        self.watchpoint_manager_mut()
            .queue
            .insert(watchpoint_range, watchpoint_args)
            .is_some()
    }

    fn clear_all_watchpoints(&mut self) {
        self.watchpoint_manager_mut().queue.clear();
    }

    fn remove_watchpoint(&mut self, watchpoint_range: WatchpointRange) {
        self.watchpoint_manager_mut().queue.remove(&watchpoint_range);
    }

    fn take_watchpoint_hits(&mut self) -> Vec<WatchpointHit> {
        take(&mut self.watchpoint_manager_mut().hits)
    }

    fn take_watchpoint_pause(&mut self) -> bool {
        take(&mut self.watchpoint_manager_mut().pause)
    }
}

pub trait Cycles {
    fn cycle_manager_ref(&self) -> &CycleManager;

    fn cycle_manager_mut(&mut self) -> &mut CycleManager;

    fn pause_at_cycle_reached(&self) -> bool {
        self.cycle_manager_ref()
            .pause_at_cycle
            .as_ref()
            .is_some_and(|pause| self.elapsed_cycles() >= pause.target_elapsed_cycles)
    }

    fn elapsed_cycles_pause_reached(&self) -> bool {
        self.cycle_manager_ref()
            .elapsed_cycles_pause
            .as_ref()
            .is_some_and(|pause| self.elapsed_cycles() >= pause.starting_cycle + pause.target_elapsed_cycles)
    }

    fn pause_at_current_cycle(&mut self) -> bool {
        if self.pause_at_cycle_reached() || self.elapsed_cycles_pause_reached() {
            self.cycle_manager_mut().cycle_pause = true;

            return true;
        }

        false
    }

    fn take_cycle_pause(&mut self) -> bool {
        if self.pause_at_cycle_reached() {
            self.cycle_manager_mut().pause_at_cycle = None;
        }

        if self.elapsed_cycles_pause_reached() {
            self.cycle_manager_mut().elapsed_cycles_pause = None;
        }

        take(&mut self.cycle_manager_mut().cycle_pause)
    }

    fn elapsed_cycles(&self) -> u64;

    fn clock_rate(&self) -> f64;

    fn set_cycle_pause(&mut self, cycle: u64) {
        self.cycle_manager_mut().pause_at_cycle = Some(ElapsedCycles {
            starting_cycle: 0,
            target_elapsed_cycles: cycle,
        });
    }

    fn set_elapsed_cycles_pause(&mut self, elapsed_cycle: u64) {
        let starting_cycle = self.elapsed_cycles();

        self.cycle_manager_mut().elapsed_cycles_pause = Some(ElapsedCycles {
            starting_cycle,
            target_elapsed_cycles: elapsed_cycle,
        });
    }

    fn cycle_paused_text(&self) -> Vec<String> {
        let mut vec = Vec::with_capacity(2);

        if let Some(pause) = self.cycle_manager_ref().pause_at_cycle.as_ref()
            && self.pause_at_cycle_reached()
        {
            vec.push(format!(
                "pause due to specific cycle being reached - Current cycle {}; Target cycle {}",
                self.elapsed_cycles(),
                pause.target_elapsed_cycles
            ));
        }

        if let Some(pause) = self.cycle_manager_ref().elapsed_cycles_pause.as_ref()
            && self.elapsed_cycles_pause_reached()
        {
            vec.push(format!(
                "pause due to specific elapsed cycles being reached - Starting cycle {}; Current cycle {}; Target \
                 cycle {}; Requested elapsed cycles {}",
                pause.starting_cycle,
                self.elapsed_cycles(),
                pause.starting_cycle + pause.target_elapsed_cycles,
                pause.target_elapsed_cycles
            ));
        }

        vec
    }
}

pub trait Rewind: State + Cycles {
    fn rewind_manager_ref(&self) -> &RewindManager;

    fn rewind_manager_mut(&mut self) -> &mut RewindManager;

    fn snapshot(&self) -> Result<Vec<u8>, Error>;

    fn rewind_cycles(&self) -> u64 {
        self.elapsed_cycles()
    }

    fn restore(&mut self, buffer: Vec<u8>) -> Result<(), Error> {
        self.load_state(&buffer)
    }

    fn rewind_occupancy(&self) -> (usize, usize, usize) {
        self.rewind_manager_ref().occupancy()
    }

    fn capture_rewind(&mut self) {
        let cycles = self.rewind_cycles();
        if self.rewind_manager_ref().should_capture(cycles)
            && let Ok(state) = self.snapshot()
        {
            self.rewind_manager_mut().push(cycles, state);
        }
    }

    fn rewind(&mut self, count: usize) -> Option<f64> {
        let current = self.rewind_cycles();
        let (state, snapshot_cycles) = self.rewind_manager_mut().pop_n(count)?;

        self.load_state(&state).ok()?;

        // a slot load or lua restore can result in newer snapshots in the
        // buffer than the current cycle, prevent underflow
        let rewound_cycles = current.saturating_sub(snapshot_cycles as u64);

        Some(rewound_cycles as f64 / self.clock_rate())
    }
}

pub trait State {
    fn state_magic_name(&self) -> &[u8];

    fn save_state(&self) -> Result<Vec<u8>, Error>
    where
        Self: Serialize + Deserialize<'static>,
    {
        let serialized_data =
            postcard::to_allocvec(self).map_err(|err| error_message(format!("Failed to create save state: {err}")))?;
        let compressed_data = lz4_flex::compress_prepend_size(&serialized_data);

        let mut output = Vec::with_capacity(self.state_magic_name().len() + compressed_data.len());
        output.extend_from_slice(self.state_magic_name());
        output.extend_from_slice(&compressed_data);

        Ok(output)
    }

    fn load_state(&mut self, bytes: &[u8]) -> Result<(), Error>;
}

pub trait ScriptedInput {
    fn scripted_keypad_ref(&self) -> &Option<Vec<bool>>;

    fn scripted_keypad_mut(&mut self) -> &mut Option<Vec<bool>>;

    fn set_scripted_inputs(&mut self, keys: Option<Vec<bool>>) {
        *self.scripted_keypad_mut() = keys.filter(|keys| keys.contains(&true));
    }

    fn check_scripted_inputs(&self) -> Option<&[bool]> {
        self.scripted_keypad_ref().as_deref()
    }
}

pub trait ScriptTarget: Emulator + Watchpoint + Breakpoint + Cycles + Rewind + ScriptedInput {
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

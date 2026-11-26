use std::{
    collections::{HashMap, VecDeque},
    path::PathBuf,
    sync::mpsc::{self, Receiver, TryRecvError},
    thread::spawn,
    time::{Duration, Instant},
};

use rfd::FileDialog;
use serde::{Deserialize, Serialize};
use spin_sleep::sleep_until;

use crate::{
    EmulatorState,
    constants::MAX_FRAMES_PER_CALL,
    enums::Width,
    script::{MemoryAccessor, WatchpointArgs, WatchpointHit, WatchpointRange, WatchpointType},
    traits::UnsignedInt,
    utils::{get_halfword_shift, get_word_mask, zero_arr},
};

#[derive(Deserialize, Serialize)]
pub struct GroupedRegisters<T: UnsignedInt> {
    pub registers: Box<[T]>,
    pub base_address: usize,
}

impl<T: UnsignedInt> GroupedRegisters<T> {
    pub fn new(capacity: usize, base_address: u32) -> Self {
        Self {
            registers: zero_arr::<T>(capacity),
            base_address: base_address as usize,
        }
    }

    pub fn index(&self, address: u32) -> usize {
        (address as usize - self.base_address) / size_of::<T>()
    }
}

impl GroupedRegisters<u16> {
    pub fn read_u16(&self, address: u32) -> u16 {
        self.registers[self.index(address & !1)]
    }

    pub fn write_u16(&mut self, address: u32, value: u16) {
        let index = self.index(address & !1);
        self.registers[index] = value;
    }

    pub fn from_index(&self, index: usize) -> u16 {
        self.registers[index]
    }
}

impl GroupedRegisters<u32> {
    pub fn read_u32(&self, address: u32) -> u32 {
        self.registers[self.index(address & !3)]
    }

    pub fn write_u32(&mut self, address: u32, value: u32) {
        self.registers[self.index(address & !3)] = value;
    }

    pub fn read_u16(&self, address: u32) -> u16 {
        let address = address & !3;
        let index = self.index(address);

        let mask = get_word_mask(address);
        let shift = get_halfword_shift(address);

        ((self.registers[index] & mask) >> shift) as u16
    }

    pub fn write_u16(&mut self, address: u32, value: u16) {
        let index = self.index(address & !3);
        let shift = get_halfword_shift(address);
        let register = &mut self.registers[index];

        *register = (*register & get_word_mask(address)) | ((value as u32) << shift);
    }

    pub fn from_index(&self, index: usize) -> u32 {
        self.registers[index]
    }
}

pub struct ElapsedCycles {
    pub starting_cycle: u64,
    pub target_elapsed_cycles: u64,
}

pub struct PendingDialog {
    pub receiver: Receiver<Option<PathBuf>>,
}

impl PendingDialog {
    pub fn new(dialog: impl FnOnce() -> Option<PathBuf> + Send + 'static) -> Self {
        let (sender, receiver) = mpsc::channel();

        spawn(move || {
            let _ = sender.send(dialog());
        });

        Self { receiver }
    }

    pub fn open_rom() -> Self {
        Self::new(|| {
            FileDialog::new()
                .set_title("Select a ROM")
                .add_filter("Compatible ROMs", &["gb", "gbc", "gba"])
                .pick_file()
        })
    }

    pub fn open_lua_script() -> Self {
        Self::new(|| {
            FileDialog::new()
                .set_title("Select Lua Script")
                .add_filter("Lua", &["lua", "txt"])
                .pick_file()
        })
    }

    pub fn select_folder(start_dir: PathBuf) -> Self {
        Self::new(|| FileDialog::new().set_directory(start_dir).pick_folder())
    }

    pub fn poll(&self) -> Option<Option<PathBuf>> {
        match self.receiver.try_recv() {
            Err(TryRecvError::Empty) => None,
            Ok(path) => Some(path),
            Err(TryRecvError::Disconnected) => Some(None),
        }
    }
}

// TODO: attempt to do something similar with watchpoints to clean up code when
// i feel like it
pub struct BreakpointManager {
    pub range: Option<(u32, u32)>,
    pub hit: Option<u32>,
    pub queue: HashMap<u32, EmulatorState>,
    pub resume_from: Option<u32>,
}

impl BreakpointManager {
    pub fn in_range(&self, address: u32) -> bool {
        if let Some((min, max)) = self.range {
            return address >= min && address <= max;
        }

        false
    }

    pub fn set(&mut self, address: u32, pause: bool) -> bool {
        let action = if pause {
            EmulatorState::Paused
        } else {
            EmulatorState::Running
        };

        if self.range.is_none() {
            self.range = Some((address, address));
        } else {
            let (min, max) = self.range.unwrap();
            self.range = Some((min.min(address), max.max(address)));
        }

        self.queue.insert(address, action).is_none()
    }

    pub fn remove(&mut self, address: u32) -> bool {
        let evicted = self.queue.remove(&address);
        let keys = self.queue.keys().copied();
        self.range = keys.clone().min().zip(keys.max());

        evicted.is_some()
    }
}

impl Default for BreakpointManager {
    fn default() -> Self {
        Self {
            range: None,
            hit: None,
            queue: HashMap::new(),
            resume_from: None,
        }
    }
}

// another manager to prevent the drift that occurs when i forget to update
// something
pub struct WatchpointManager {
    pub queue: HashMap<WatchpointRange, WatchpointArgs>,
    pub hits: Vec<WatchpointHit>,
    pub pause: bool,
}

impl WatchpointManager {
    #[inline]
    pub fn check_read(&mut self, address: u32, accessor: MemoryAccessor, value: u32, cycle: u64, width: Width) {
        if matches!(accessor, MemoryAccessor::Lua | MemoryAccessor::CpuReadWriteModify) || self.queue.is_empty() {
            return;
        }

        let address = width.align(address);

        let Self { queue, hits, pause } = self;

        for (range, args) in queue.iter() {
            if args.width != width || !range.overlaps(address, width) || !args.accessor.allows(accessor) {
                continue;
            }

            if args.fires_on_read(value) {
                push_hit(
                    hits,
                    pause,
                    WatchpointHit {
                        address,
                        value,
                        pause: args.pause,
                        width,
                        on: WatchpointType::Read,
                        pc: 0,
                        fetch_source: None,
                        cycle,
                        accessor,
                        target: args.target,
                        compare: args.compare,
                    },
                );
            }
        }
    }

    #[inline]
    pub fn check_write(&mut self, address: u32, accessor: MemoryAccessor, value: u32, cycle: u64, width: Width) {
        if accessor == MemoryAccessor::Lua || self.queue.is_empty() {
            return;
        }

        let address = width.align(address);

        let Self { queue, hits, pause } = self;

        for (range, args) in queue.iter_mut() {
            if args.width != width || !range.overlaps(address, width) || !args.accessor.allows(accessor) {
                continue;
            }

            let on = if args.on == WatchpointType::Change {
                WatchpointType::Change
            } else {
                WatchpointType::Write
            };

            if args.should_fire_on_write(address, value) {
                push_hit(
                    hits,
                    pause,
                    WatchpointHit {
                        address,
                        value,
                        pause: args.pause,
                        width,
                        on,
                        pc: 0,
                        fetch_source: None,
                        cycle,
                        accessor,
                        target: args.target,
                        compare: args.compare,
                    },
                );
            }
        }
    }
}

fn push_hit(hits: &mut Vec<WatchpointHit>, pause: &mut bool, hit: WatchpointHit) {
    if hit.pause {
        *pause = true;
    }

    if hits.len() < WatchpointHit::MAX_PENDING {
        hits.push(hit);
    }
}

impl Default for WatchpointManager {
    fn default() -> Self {
        Self {
            queue: HashMap::new(),
            hits: Vec::new(),
            pause: false,
        }
    }
}

pub struct RewindManager {
    pub buffer: VecDeque<(u64, Vec<u8>)>,
    pub capacity: usize,
    interval: u64,
    last_cycle: u64,
}

impl RewindManager {
    pub fn new(interval: u64) -> Self {
        Self {
            buffer: VecDeque::with_capacity(300),
            capacity: 0,
            interval,
            last_cycle: 0,
        }
    }

    pub fn should_capture(&self, cycles: u64) -> bool {
        cycles < self.last_cycle || cycles >= self.last_cycle + self.interval
    }

    pub fn push(&mut self, cycles: u64, state: Vec<u8>) {
        if self.buffer.len() == self.capacity {
            self.buffer.pop_front();
        }

        self.buffer.push_back((cycles, state));
        self.last_cycle = cycles;
    }

    pub fn pop_n(&mut self, count: usize) -> Option<(Vec<u8>, u64)> {
        let mut last_state = None;

        for _ in 0..count {
            match self.buffer.pop_back() {
                Some(pair) => last_state = Some(pair),
                None => break,
            }
        }

        let (cycles, state) = last_state?;
        self.last_cycle = cycles;

        Some((state, cycles))
    }

    pub fn occupancy(&self) -> (usize, usize, usize) {
        let bytes = self.buffer.iter().map(|(_, state)| state.len()).sum();

        (self.buffer.len(), self.capacity, bytes)
    }

    pub fn is_enabled(&self) -> bool {
        self.capacity != 0
    }
}

impl Default for RewindManager {
    fn default() -> Self {
        RewindManager::new(u64::MAX)
    }
}

pub struct FrameTracker {
    fps_start: Instant,
    fps_frames: u64,
    pub fps: f64,
}

impl FrameTracker {
    pub fn new() -> Self {
        Self {
            fps_start: Instant::now(),
            fps_frames: 0,
            fps: 0.0,
        }
    }

    pub fn compute_fps(&mut self, frames: u64) {
        self.fps_frames += frames;

        let elapsed_time = self.fps_start.elapsed().as_secs_f64();
        if elapsed_time >= 1.0 {
            self.fps = self.fps_frames as f64 / elapsed_time;
            self.fps_frames = 0;
            self.fps_start = Instant::now();
        }
    }

    pub fn reset(&mut self) {
        *self = Self::new();
    }
}

pub struct FramePacer {
    pub frame_period: Duration,
    pub frame_deadline: Instant,
    pub speed: f32,
    base_period: f64,
}

impl FramePacer {
    pub fn new(base_period: f64) -> Self {
        let frame_period = Duration::from_secs_f64(base_period);

        Self {
            frame_period,
            frame_deadline: Instant::now() + frame_period,
            speed: 1.0,
            base_period,
        }
    }

    pub fn update_frame_deadline(&mut self) {
        self.frame_deadline = Instant::now() + self.frame_period
    }

    pub fn set_speed(&mut self, speed: f32) {
        self.speed = speed;

        self.frame_period = Duration::from_secs_f64(if speed < 1.0 {
            self.base_period / speed as f64
        } else {
            self.base_period
        });
    }

    pub fn sleep(&mut self) {
        let current_time = Instant::now();
        if current_time > self.frame_deadline + self.frame_period * MAX_FRAMES_PER_CALL {
            self.frame_deadline = current_time;
        }

        sleep_until(self.frame_deadline);
        self.frame_deadline += self.frame_period;
    }
}

pub struct CycleManager {
    pub elapsed_cycles_pause: Option<ElapsedCycles>,
    pub pause_at_cycle: Option<ElapsedCycles>,
    pub cycle_pause: bool,
}

impl Default for CycleManager {
    fn default() -> Self {
        CycleManager {
            elapsed_cycles_pause: None,
            pause_at_cycle: None,
            cycle_pause: false,
        }
    }
}

// https://psx-spx.consoledev.net/ps1/system/iomap/
// https://nsec.sjtu.edu.cn/data/MK.Computer.Organization.and.Design.4th.Edition.Oct.2011.pdf
use serde::{Deserialize, Serialize};
use std::{collections::HashMap, env::var, fmt::Arguments, fs::File, io::BufWriter};

use shared::{
    debug::Trace,
    enums::{FetchSource, Width},
    scheduler::EventScheduler,
    script::{WatchpointArgs, WatchpointHit},
    traits::BitOps,
};

use crate::components::psx::Event;
// gonna experiment with generic this time - https://github.com/simias/rustation-ng/blob/main/src/psx.rs

pub enum Requester {
    Cpu,
    Dma,
}

pub trait BusAccess: BitOps {
    const ACCESS_WIDTH: Width;

    fn to_u32(self) -> u32;

    fn from_u32(value: u32) -> Self;

    fn read_le_bytes(bytes: &[u8], index: usize) -> Self;

    fn write_le_bytes(bytes: &mut [u8], index: usize, value: Self);
}

impl BusAccess for u8 {
    const ACCESS_WIDTH: Width = Width::Byte;

    fn from_u32(value: u32) -> Self {
        value as u8
    }

    fn to_u32(self) -> u32 {
        self as u32
    }

    fn read_le_bytes(bytes: &[u8], index: usize) -> Self {
        bytes[index]
    }

    fn write_le_bytes(bytes: &mut [u8], index: usize, value: Self) {
        bytes[index] = value;
    }
}

impl BusAccess for u16 {
    const ACCESS_WIDTH: Width = Width::Halfword;

    fn from_u32(value: u32) -> Self {
        value as u16
    }

    fn to_u32(self) -> u32 {
        self as u32
    }

    fn read_le_bytes(bytes: &[u8], index: usize) -> Self {
        u16::from_le_bytes(bytes[index..index + 2].try_into().unwrap())
    }

    fn write_le_bytes(bytes: &mut [u8], index: usize, value: Self) {
        bytes[index..index + 2].copy_from_slice(&value.to_le_bytes());
    }
}

impl BusAccess for u32 {
    const ACCESS_WIDTH: Width = Width::Word;

    fn from_u32(value: u32) -> Self {
        value
    }

    fn to_u32(self) -> u32 {
        self
    }

    fn read_le_bytes(bytes: &[u8], index: usize) -> Self {
        u32::from_le_bytes(bytes[index..index + 4].try_into().unwrap())
    }

    fn write_le_bytes(bytes: &mut [u8], index: usize, value: Self) {
        bytes[index..index + 4].copy_from_slice(&value.to_le_bytes());
    }
}

#[derive(Serialize, Deserialize)]
pub struct Bus {
    pub scheduler: EventScheduler<Event>,
    #[serde(skip)]
    trace: Option<Trace>,
    #[serde(skip)]
    pub watchpoint_queue: HashMap<u32, WatchpointArgs>,
    #[serde(skip)]
    pub watchpoint_hits: Vec<WatchpointHit>,
    pub watchpoint_pause: bool,
}

impl Bus {
    pub fn new() -> Self {
        let trace = var("PSXTRACE")
            .ok()
            .and_then(|str| str.parse().ok())
            .map(|limit| Trace {
                file: BufWriter::with_capacity(1 << 20, File::create("psx_trace.txt").unwrap()),
                limit,
                count: 0,
            });

        let scheduler = EventScheduler::<Event>::new();

        Self {
            scheduler,
            trace,
            watchpoint_queue: HashMap::new(),
            watchpoint_hits: Vec::new(),
            watchpoint_pause: false,
        }
    }

    pub fn read<T: BusAccess>(&mut self, address: u32, requester: Requester) -> T {
        self.determine_cycle_cost(address, T::ACCESS_WIDTH, requester);
        unimplemented!()
    }

    pub fn peek<T: BusAccess>(&self, address: u32) -> T {
        unimplemented!()
    }

    pub fn write<T: BusAccess>(&mut self, address: u32, value: T, requester: Requester) {
        self.determine_cycle_cost(address, T::ACCESS_WIDTH, requester);
    }

    pub fn poke<T: BusAccess>(&mut self, address: u32, value: T) {}

    pub fn dump(&mut self, arguments: Arguments) {
        if let Some(trace) = &mut self.trace {
            trace.dump(arguments);
        }
    }

    pub fn notify_fetch(&mut self, address: u32, fetch_source: FetchSource) {}

    pub fn determine_cycle_cost(&mut self, address: u32, width: Width, requester: Requester) {}

    pub fn add_cycles(&mut self, cycles: u64) {
        self.scheduler.current += cycles;
    }
}

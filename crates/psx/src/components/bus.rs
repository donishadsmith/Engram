// https://psx-spx.consoledev.net/ps1/system/iomap/
// https://nsec.sjtu.edu.cn/data/MK.Computer.Organization.and.Design.4th.Edition.Oct.2011.pdf

use serde::{Deserialize, Serialize};
use std::{collections::HashMap, env::var, fmt::Arguments, fs::File, io::BufWriter};

use shared::{
    debug::Trace,
    enums::Width,
    scheduler::EventScheduler,
    script::{
        Accessor, MemoryAccessor, WatchpointArgs, WatchpointHit, WatchpointRange, WatchpointType,
        find_watchpoint,
    },
    traits::BitOps,
    utils::{kilobytes, zero_arr},
};

use crate::components::psx::Event;

// gonna experiment with generic this time - https://github.com/simias/rustation-ng/blob/main/src/psx.rs
// https://github.com/kaezrr/starpsx/blob/main/core/src/mem.rs
/*
pub enum FetchType {
    Instruction,
    Data,
}
*/
pub trait BusAccess: BitOps {
    const ACCESS_WIDTH: Width;

    fn to_u32(self) -> u32;

    fn from_u32(value: u32) -> Self;

    fn read_le_bytes(bytes: &[u8], index: usize) -> Self;

    fn write_le_bytes(bytes: &mut [u8], index: usize, value: Self);

    fn address_offset(address: u32) -> u32 {
        address - Self::ACCESS_WIDTH.align(address)
    }

    fn read(input: u32, address: u32) -> Self {
        Self::from_u32(input >> (8 * address.get_bit_range(0..2)))
    }

    fn write(input: &mut u32, address: u32, value: Self) {
        match Self::ACCESS_WIDTH {
            Width::Byte => {
                let shift = 8 * address.get_bit_range(0..2) as usize;
                input.clear_bit_range(shift..(shift + 8));
                *input |= Self::to_u32(value) << shift
            }
            Width::Halfword => {
                if address.is_clear(0) {
                    input.clear_bit_range(0..16);
                    *input |= Self::to_u32(value)
                } else {
                    input.clear_bit_range(16..32);
                    *input |= Self::to_u32(value) << 16
                }
            }
            Width::Word => *input = Self::to_u32(value),
        };
    }
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

#[inline]
pub fn translate_address(address: u32) -> u32 {
    match address >> 29 {
        0b100 | 0b101 => address.get_bit_range(0..29), // kseg0 + kseg1
        _ => address,                                  // kseg2 + kuseg
    }
}

// https://psx-spx.consoledev.net/memorymap/
// https://psx-spx.consoledev.net/iomap/
#[derive(Serialize, Deserialize)]
pub struct Bus {
    pub scheduler: EventScheduler<Event>,
    bios: Box<[u8]>,
    dram: Box<[u8]>,
    scratchpad: Box<[u8]>,
    bcc: u32, //FFFE0130h BCC  BIU/Cache Control
    /*
        1F801070h  2  I_STAT / I_REG  Interrupt status register
        1F801074h  2  I_MASK          Interrupt mask register
    */
    istat: u32,
    imask: u32,
    #[serde(skip)]
    trace: Option<Trace>,
    #[serde(skip)]
    pub watchpoint_queue: HashMap<WatchpointRange, WatchpointArgs>,
    #[serde(skip)]
    pub watchpoint_hits: Vec<WatchpointHit>,
    #[serde(skip)]
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
            bios: zero_arr(kilobytes(512)),
            dram: zero_arr(kilobytes(2048)),
            scratchpad: zero_arr(kilobytes(1)),
            bcc: 0,
            istat: 0,
            imask: 0,
            trace,
            watchpoint_queue: HashMap::new(),
            watchpoint_hits: Vec::new(),
            watchpoint_pause: false,
        }
    }

    pub fn read<T: BusAccess>(&mut self, address: u32, accessor: MemoryAccessor) -> T {
        self.determine_cycle_cost(address, T::ACCESS_WIDTH, accessor);

        let address = translate_address(address);
        let value = match address {
            0x00000000..=0x001FFFFF => T::read_le_bytes(&self.dram, address as usize),
            0x1F800000..=0x1F8003FF => {
                T::read_le_bytes(&self.scratchpad, (address - 0x1F800000) as usize)
            }
            0x1F801000..=0x1F801FFF => self.read_register::<T>(address, accessor),
            0x1FC00000..=0x1FC7FFFF => {
                T::read_le_bytes(&self.bios, (address - 0x1FC00000) as usize)
            }
            0xFFFE0130 => T::read(self.bcc, address),
            _ => T::read(0xFFFFFFFF, address),
        };

        self.check_read_watchpoint(address, T::ACCESS_WIDTH, T::to_u32(value), accessor);

        value
    }

    pub fn read_register<T: BusAccess>(&mut self, address: u32, accessor: MemoryAccessor) -> T {
        match address {
            0x1F801070..0x1F801074 => T::read(self.istat, address),
            0x1F801074..0x1F801078 => T::read(self.imask, address),
            _ => todo!(),
        }
    }

    pub fn write<T: BusAccess>(&mut self, address: u32, value: T, accessor: MemoryAccessor) {
        self.determine_cycle_cost(address, T::ACCESS_WIDTH, accessor);

        let address = translate_address(address);
        match address {
            0x00000000..=0x001FFFFF => T::write_le_bytes(&mut self.dram, address as usize, value),
            0x1F800000..=0x1F8003FF => {
                T::write_le_bytes(&mut self.scratchpad, (address - 0x1F800000) as usize, value)
            }
            0x1FC00000..=0x1FC7FFFF if accessor == MemoryAccessor::Lua => {
                T::write_le_bytes(&mut self.bios, (address - 0x1FC00000) as usize, value)
            }
            0x1F801000..=0x1F801FFF => self.write_register::<T>(address, value),
            0xFFFE0130 => T::write(&mut self.bcc, address, value),
            _ => {}
        }

        self.check_write_watchpoint(address, T::ACCESS_WIDTH, T::to_u32(value), accessor);
    }

    pub fn write_register<T: BusAccess>(&mut self, address: u32, value: T) {
        match address {
            0x1F801070..0x1F801074 => {
                let mut interrupt_mask = 0xFFFFFFFF;

                T::write(&mut interrupt_mask, address, value);

                self.istat &= interrupt_mask;
            }
            0x1F801074..0x1F801078 => T::write(&mut self.imask, address, value),
            _ => todo!(),
        }
    }

    pub fn dump(&mut self, arguments: Arguments) {
        if let Some(trace) = &mut self.trace {
            trace.dump(arguments);
        }
    }

    //pub fn notify_fetch(&mut self, address: u32, fetch_source: FetchSource) {}

    pub fn determine_cycle_cost(&mut self, address: u32, width: Width, accessor: MemoryAccessor) {}

    pub fn add_cycles(&mut self, cycles: u64) {
        self.scheduler.current += cycles;
    }

    // maybe consolidate logic later when works, just copy and pasted from the gba with minimal revisions
    // nice to do but i probably will never do it tbh
    #[inline]
    fn check_read_watchpoint(
        &mut self,
        address: u32,
        width: Width,
        value: u32,
        accessor: MemoryAccessor,
    ) {
        if accessor == MemoryAccessor::Lua || self.watchpoint_queue.is_empty() {
            return;
        }

        let Some(range) = find_watchpoint(address, &self.watchpoint_queue) else {
            return;
        };

        let args = &self.watchpoint_queue[&range];

        if args.accessor != Accessor::All
            && args.accessor.to_string().to_lowercase() != accessor.to_string().to_lowercase()
        {
            return;
        }

        if args.width == width && args.fires_on_read(value) {
            self.push_watchpoint_hit(
                address,
                value,
                args.pause,
                width,
                WatchpointType::Read,
                accessor,
            );
        }
    }

    #[inline]
    fn check_write_watchpoint(
        &mut self,
        address: u32,
        width: Width,
        value: u32,
        accessor: MemoryAccessor,
    ) {
        if accessor == MemoryAccessor::Lua || self.watchpoint_queue.is_empty() {
            return;
        }

        let Some(range) = find_watchpoint(address, &self.watchpoint_queue) else {
            return;
        };

        let args = self.watchpoint_queue.get_mut(&range).unwrap();

        if args.accessor != Accessor::All
            && args.accessor.to_string().to_lowercase() != accessor.to_string().to_lowercase()
        {
            return;
        }

        if args.width != width {
            return;
        }

        let pause = args.pause;
        let on = if args.on == WatchpointType::Change {
            WatchpointType::Change
        } else {
            WatchpointType::Write
        };

        if args.should_fire_on_write(address, value) {
            self.push_watchpoint_hit(address, value, pause, width, on, accessor);
        }
    }

    #[inline]
    fn push_watchpoint_hit(
        &mut self,
        address: u32,
        value: u32,
        pause: bool,
        width: Width,
        on: WatchpointType,
        accessor: MemoryAccessor,
    ) {
        if pause {
            self.watchpoint_pause = true;
        }

        if self.watchpoint_hits.len() >= WatchpointHit::MAX_PENDING {
            return;
        }

        self.watchpoint_hits.push(WatchpointHit {
            address,
            value,
            pause,
            width,
            on,
            pc: 0,
            fetch_source: None,
            cycle: self.scheduler.current,
            accessor,
        });
    }

    pub fn set_watchpoint(&mut self, range: WatchpointRange, args: WatchpointArgs) -> bool {
        self.watchpoint_queue
            .insert(translate_watchpoint(range), args)
            .is_some()
    }

    pub fn remove_watchpoint(&mut self, range: WatchpointRange) {
        self.watchpoint_queue.remove(&translate_watchpoint(range));
    }
}

fn translate_watchpoint(range: WatchpointRange) -> WatchpointRange {
    WatchpointRange {
        start_address: translate_address(range.start_address),
        end_address: range.end_address.map(translate_address),
    }
}

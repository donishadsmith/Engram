// https://mgba.io/2015/06/27/cycle-counting-prefetch/
// https://developer.arm.com/documentation/ddi0084/f/memory-interface/bus-cycle-types/sequential-cycles
// https://corrupt.wiki/systems/gameboy-advance/bizhawk-memory-domains
// https://medium.com/@michelheily/hello-gba-journey-of-making-an-emulator-part-1-8793000e8606
// https://www.cs.rit.edu/~tjh8300/CowBite/CowBiteSpec.htm#Memory%20Map
// https://www.nesdev.org/wiki/Open_bus_behavior
// https://www.cs.rit.edu/~tjh8300/CowBite/CowBiteSpec.htm#Memory%20Map

// Just do an afterboot startup

// https://problemkaputt.de/gbatek.htm#GBAUnpredictableThings
use crate::components::scheduler::Scheduler;

#[derive(Clone, Copy, PartialEq, Eq)]
pub enum AccessType {
    Sequential, // Memory address related to previous address, incremented by + 2 (half word) or +4 (word)
    Nonsequential, // Memory address is fetched and has nothing to do with the previous instruction
}

// Note, there is an addition GBA cycle type: Internal, no memory access, performing a complex internal operation like a multiply, only 1 cycle

mod sealed {
    pub trait Sealed {}

    impl Sealed for u8 {}

    impl Sealed for u16 {}

    impl Sealed for u32 {}
}

pub trait BusValue: sealed::Sealed + Sized + Copy {
    fn read(bus: &mut Bus, address: u32, access_type: AccessType) -> Self;

    fn write(bus: &mut Bus, address: u32, value: Self, access_type: AccessType);
}

pub struct Bus {
    pub scheduler: Scheduler,
    iwram: Box<[u8; 0x8000]>,
    ewram: Box<[u8; 0x4000]>,
    last_read: u32,
    last_bios_fetch: u32 // According to medium article, MMBN6 has an email bug due to null pointer dereference in the BIOS
    // region [00DCh+8] in bios is 0xE12FFF1E; https://problemkaputt.de/gbatek.htm#GBAUnpredictableThings
}

impl Bus {
    pub fn new() -> Self {
        Self {
            scheduler: Scheduler::new(),
            iwram: Box::new([0u8; 0x8000]),
            ewram: Box::new([0u8; 0x4000]),
            last_read: 0,
            last_bios_fetch: 0xE12FFF1E
        }
    }

    pub fn read<T: BusValue>(&mut self, address: u32, access_type: AccessType) -> T {
        T::read(self, address, access_type)
    }

    pub fn write<T: BusValue>(&mut self, address: u32, value: T, access_type: AccessType) {
        T::write(self, address, value, access_type)
    }

    pub fn idle(&mut self, cycles: u64) {
        self.scheduler.current += cycles;
    }

    pub fn cost(&mut self, address: u32, width: u32, access_type: AccessType) {
        self.scheduler.current += 1; // TODO: wait table
    }

    fn read_register_16(&mut self, address: u32) -> u16 {
        0
    }

    fn write_register_16(&mut self, address: u32, value: u16) {}
}

// impl BusValue for u16

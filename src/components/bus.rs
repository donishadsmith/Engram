// https://mgba.io/2015/06/27/cycle-counting-prefetch/
// https://developer.arm.com/documentation/ddi0084/f/memory-interface/bus-cycle-types/sequential-cycles

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


pub struct Bus {
    pub sheduler: Scheduler,
}

pub trait BusValue: sealed::Sealed + Sized + Copy {
    fn read(bus: &mut Bus, address: u32, access_type: AccessType) -> Self;

    fn write(bus: &mut Bus, address: u32, value: Self, access_type: AccessType);
}

impl Bus {
    pub fn read<T: BusValue>(&mut self, address: u32, access_type: AccessType) -> T {
        T::read(self, address, access)
    }

    pub fn write<T: BusValue>(&mut self, address: u32, value: T, access_type: AccessType) {
        T::write(self, address, value, access)
    }
}

// impl BusValue for u16

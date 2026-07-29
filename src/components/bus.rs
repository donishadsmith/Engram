// https://mgba.io/2015/06/27/cycle-counting-prefetch/
// https://developer.arm.com/documentation/ddi0084/f/memory-interface/bus-cycle-types/sequential-cycles
// https://corrupt.wiki/systems/gameboy-advance/bizhawk-memory-domains
// https://medium.com/@michelheily/hello-gba-journey-of-making-an-emulator-part-1-8793000e8606
// https://www.cs.rit.edu/~tjh8300/CowBite/CowBiteSpec.htm#Memory%20Map
// https://www.nesdev.org/wiki/Open_bus_behavior
// https://www.cs.rit.edu/~tjh8300/CowBite/CowBiteSpec.htm#Memory%20Map

// https://blog.asie.pl/2025/09/wonderful-update-september-2025/

// Just do an afterboot startup

// https://problemkaputt.de/gbatek.htm#GBAUnpredictableThings
use crate::components::{
    apu::APU,
    gamepak::GamePak,
    ppu::PPU,
    scheduler::Scheduler,
    utils::zero_arr,
};

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
    bios: Box<[u8; 0x4000]>,
    iwram: Box<[u8; 0x8000]>,
    ewram: Box<[u8; 0x40000]>,
    last_read: u32,
    last_bios_fetch: u32, // According to medium article, MMBN6 has an email bug due to null pointer dereference in the BIOS
    // region [00DCh+8] in bios is 0xE129F000; https://problemkaputt.de/gbatek.htm#GBAUnpredictableThings
    apu: APU,
    ppu: PPU,
    gamepak: GamePak,
}

impl Bus {
    pub fn new(gamepak: GamePak) -> Self {
        Self {
            scheduler: Scheduler::new(),
            bios: zero_arr(),
            iwram: zero_arr(),
            ewram: zero_arr(),
            last_read: 0,
            last_bios_fetch: 0xE129F000,
            ppu: PPU::new(),
            gamepak,
            apu: APU::new(),
        }
    }

    #[inline]
    fn ewram_index(address: u32) -> usize {
        (address & 0x3FFFF) as usize
    }

    #[inline]
    fn iwram_index(address: u32) -> usize {
        (address & 0x7FFF) as usize
    }

    #[inline]
    pub fn palette_index(address: u32) -> usize {
        (address & 0x3FF) as usize
    }

    #[inline]
    pub fn oam_index(address: u32) -> usize {
        (address & 0x3FF) as usize
    }

    #[inline]
    pub fn vram_index(address: u32) -> usize {
        let index = (address & 0x1FFFE) as usize;
        let index = if index >= 0x18000 {
            index - 0x8000
        } else {
            index
        };

        index
    }

    #[inline]
    pub fn backup_byte(&self, address: u32) -> u8 {
        if self.gamepak.backup_memory.is_empty() {
            return 0;
        }

        let base_address = 0x0E000000;
        let mask = (self.gamepak.backup_memory.len() - 1) as u32;
        let index = ((address - base_address) & mask) as usize;

        self.gamepak.backup_memory[index]
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

    fn open(&self) -> u32 {
        0
    }
}

impl BusValue for u16 {
    fn read(bus: &mut Bus, address: u32, access_type: AccessType) -> Self {
        bus.cost(address, 16, access_type);
        let address = address & !1; // ensure even
        let little_endian =
            |arr: &[u8], index: usize| u16::from_le_bytes([arr[index], arr[index + 1]]);

        match address >> 24 {
            0x00 => little_endian(&*bus.bios, (address & 0x3FFE) as usize),
            0x02 => little_endian(&*bus.ewram, Bus::ewram_index(address)),
            0x03 => little_endian(&*bus.iwram, Bus::iwram_index(address)),
            0x04 => bus.read_register_16(address),
            0x05 => little_endian(&*bus.ppu.palette_ram, Bus::palette_index(address)),
            0x06 => little_endian(&*bus.ppu.vram, Bus::vram_index(address)),
            0x07 => little_endian(&*bus.ppu.oam, Bus::oam_index(address)),
            0x08..=0x0D => bus.gamepak.read_rom_region(address) as u16, 
            0x0E | 0x0F => {
                let byte = bus.backup_byte(address) as u16;
                byte | (byte << 8)
            }
            _ => bus.open() as u16,
        }
    }

    fn write(bus: &mut Bus, address: u32, value: Self, access_type: AccessType) {}
}

// impl BusValue for u8

// impl BusValue as u32

// https://psx-spx.consoledev.net/ps1/system/iomap/

use shared::traits::BitOps;
// gonna experiment with generic this time - https://github.com/simias/rustation-ng/blob/main/src/psx.rs

#[derive(Clone, Copy, PartialEq, Eq)]
pub enum MemoryAccess {
    Byte,
    Halfword,
    Word,
}

pub trait BusAccess: BitOps {
    const ACCESS_WIDTH: MemoryAccess;

    fn to_u32(self) -> u32;

    fn from_u32(value: u32) -> Self;

    fn read_le_bytes(bytes: &[u8], index: usize) -> Self;

    fn write_le_bytes(bytes: &mut [u8], index: usize, value: Self);
}

impl BusAccess for u8 {
    const ACCESS_WIDTH: MemoryAccess = MemoryAccess::Byte;

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
    const ACCESS_WIDTH: MemoryAccess = MemoryAccess::Halfword;

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
    const ACCESS_WIDTH: MemoryAccess = MemoryAccess::Word;

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

pub struct Bus {}

impl Bus {
    pub fn read<T: BusAccess>(&mut self, address: u32) -> T {
        unimplemented!()
    }

    pub fn lua_read<T: BusAccess>(&self, address: u32) -> T {
        unimplemented!()
    }

    pub fn write<T: BusAccess>(&mut self, address: u32, value: T) {}
}

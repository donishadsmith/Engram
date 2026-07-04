use crate::components::{
    bootloader::{CGB_BOOT, DMG_BOOTIX},
    cartridge::{CGBFlag, Cartridge},
    cpu::core::AddressBus,
    memory::{BootStatus, Memory},
};

//http://gameboy.mongenel.com/dmg/asmmemmap.html

pub struct Bus {
    pub memory: Memory,
}

impl Bus {
    pub fn new(cartridge: Cartridge) -> Self {
        Self {
            memory: Memory::new(cartridge),
        }
    }

    fn boot_rom_read(&self, address: u16) -> Option<u8> {
        if self.memory.boot_status == BootStatus::Complete {
            return None;
        }

        match self.memory.cartridge.header.cgb_flag {
            CGBFlag::DMG => match address {
                0x0000..=0x00FF => Some(DMG_BOOTIX[address as usize]),
                _ => None,
            },
            // https://gbdev.gg8.se/wiki/articles/Gameboy_Bootstrap_ROM
            // The rom dump includes the 256 byte rom (0x0000-0x00FF) and the 1792 byte rom (0x0200-0x08FF)
            CGBFlag::CBG => match address {
                0x0000..=0x00FF | 0x0200..=0x08FF => Some(CGB_BOOT[address as usize]),
                _ => None,
            },
        }
    }
}

impl AddressBus for Bus {
    fn read(&self, address: u16) -> u8 {
        if let Some(byte) = self.boot_rom_read(address) {
            return byte;
        }

        match address {
            0x0000..=0x7FFF | 0xA000..=0xBFFF => self.memory.cartridge.mbc.read(address),
            0x8000..=0x9FFF => self.memory.vram[(address - 0x8000) as usize],
            0xC000..=0xDFFF => self.memory.wram[(address - 0xC000) as usize],
            0xE000..=0xFDFF => self.memory.wram[(address - 0xE000) as usize],
            0xFE00..=0xFE9F => self.memory.oam[(address - 0xFE00) as usize],
            0xFF44 => 0x90, // TODO: LY - Fixed read for now until PPU exists; self.ppu.ly
            0xFF80..=0xFFFE => self.memory.hram[(address - 0xFF80) as usize],
            0xFEA0..=0xFEFF | 0xFF00..=0xFF7F | 0xFFFF => 0xFF,
        }
    }

    fn write(&mut self, address: u16, value: u8) {
        match address {
            0x0000..=0x7FFF | 0xA000..=0xBFFF => self.memory.cartridge.mbc.write(address, value),
            0x8000..=0x9FFF => self.memory.vram[(address - 0x8000) as usize] = value,
            0xC000..=0xDFFF => self.memory.wram[(address - 0xC000) as usize] = value,
            0xE000..=0xFDFF => self.memory.wram[(address - 0xE000) as usize] = value,
            0xFE00..=0xFE9F => self.memory.oam[(address - 0xFE00) as usize] = value,
            0xFF80..=0xFFFE => self.memory.hram[(address - 0xFF80) as usize] = value,
            0xFEA0..=0xFEFF | 0xFF00..=0xFF7F | 0xFFFF => {}
        }
    }
}

use crate::components::{
    bootloader::{CGB_BOOT, DMG_BOOTIX},
    cpu::core::ByteOps8,
    memory::memory::Memory,
    rom::cartridge::{CGBFlag, Cartridge},
};

#[derive(Clone, Copy, PartialEq)]
pub enum BootStatus {
    Complete,
    Incomplete,
}

impl BootStatus {
    fn to_str(self) -> &'static str {
        match self {
            BootStatus::Complete => "Boot Complete",
            BootStatus::Incomplete => "Boot Incomplete",
        }
    }
}

pub trait AddressBus {
    fn read(&self, address: u16) -> u8;

    fn write(&mut self, address: u16, value: u8);

    fn pending_interrupt(&self) -> u8;
}

//http://gameboy.mongenel.com/dmg/asmmemmap.html
pub struct Bus {
    pub memory: Memory,
    boot_status: BootStatus,
}

impl Bus {
    pub fn new(cartridge: Cartridge) -> Self {
        Self {
            memory: Memory::new(cartridge),
            boot_status: BootStatus::Incomplete,
        }
    }

    fn boot_rom_read(&self, address: u16) -> Option<u8> {
        if self.boot_status == BootStatus::Complete {
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
            0x8000..=0x9FFF => self.memory.ppu.vram[(address - 0x8000) as usize],
            0xC000..=0xDFFF => self.memory.wram[(address - 0xC000) as usize],
            0xE000..=0xFDFF => self.memory.wram[(address - 0xE000) as usize],
            0xFE00..=0xFE9F => self.memory.ppu.oam[(address - 0xFE00) as usize],
            0xFF0F => self.memory.interrupt_flag | 0xE0,
            0xFF30..=0xFF3F => self.memory.apu.wave_ram[(address - 0xFF30) as usize],
            0xFF44 => self.memory.ppu.ly,
            0xFF80..=0xFFFE => self.memory.hram[(address - 0xFF80) as usize],
            0xFEA0..=0xFEFF | 0xFF00..=0xFF7F => 0xFF,
            0xFFFF => self.memory.interrupt_enable,
        }
    }

    fn write(&mut self, address: u16, value: u8) {
        match address {
            0x0000..=0x7FFF | 0xA000..=0xBFFF => self.memory.cartridge.mbc.write(address, value),
            0x8000..=0x9FFF => self.memory.ppu.vram[(address - 0x8000) as usize] = value,
            0xC000..=0xDFFF => self.memory.wram[(address - 0xC000) as usize] = value,
            0xE000..=0xFDFF => self.memory.wram[(address - 0xE000) as usize] = value,
            0xFF01 => self.memory.serial_data = value,
            0xFF02 => {
                if value == 0x81 {
                    print!("{}", self.memory.serial_data as char);
                    self.memory.interrupt_flag |= 0x08;
                }
            }
            0xFE00..=0xFE9F => self.memory.ppu.oam[(address - 0xFE00) as usize] = value,
            0xFF0F => self.memory.interrupt_flag = value & 0x1F,
            0xFF30..=0xFF3F => self.memory.apu.wave_ram[(address - 0xFF30) as usize] = value,
            0xFF50 => {
                if value.mask(0x01) != 0 {
                    self.boot_status = BootStatus::Complete;
                }
            }
            0xFF80..=0xFFFE => self.memory.hram[(address - 0xFF80) as usize] = value,
            0xFEA0..=0xFEFF | 0xFF00..=0xFF7F => {}
            0xFFFF => self.memory.interrupt_enable = value,
        }
    }

    fn pending_interrupt(&self) -> u8 {
        self.read(0xFF0F).mask(self.read(0xFFFF)).mask(0x1F)
    }
}

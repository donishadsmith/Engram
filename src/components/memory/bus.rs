// https://gbdev.io/pandocs/Memory_Map.html
// https://gekkio.fi/files/gb-docs/gbctr.pdf
use crate::components::{
    bootloader::{CGB_BOOT, DMG_BOOTIX},
    cpu::core::{ByteOps8, InterruptMode},
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

    fn pending_interrupt(&self) -> u8 {
        0
    }

    fn perform_speed_switch(&mut self) -> bool {
        false
    }
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

    fn is_cgb(&self) -> bool {
        self.memory.cartridge.header.cgb_flag == CGBFlag::CBG
    }

    fn get_wram_index(&self, address: u16) -> usize {
        // Echo Ram is a mirror of work ram 0xC000–0xDDFF
        let adjusted_address = if (0xE000..=0xFDFF).contains(&address) {
            address - (0xFDFF - 0xDDFF)
        } else {
            address
        };

        if (0xC000..=0xCFFF).contains(&adjusted_address) {
            return (adjusted_address - 0xC000) as usize;
        }

        // 0xD000-0xDFFF; 4kb on monochrome; bankable on color using svbk register
        let offset = (adjusted_address - 0xD000) as usize;
        if !self.is_cgb() {
            offset + (0xD000 - 0xC000)
        } else {
            // https://gbdev.io/pandocs/CGB_Registers.html?highlight=cgb%20mode
            let bank = (self.memory.svbk_register & 0x07).max(1) as usize;
            bank * (0xD000 - 0xC000) + offset
        }
    }

    // transfer data from rom or ram
    fn dma_transfer(&mut self, value: u8) {
        let base_address = (value as u16) << 8;

        for i in 0..0xA0u16 {
            let byte = self.read(base_address + i);
            self.memory.ppu.oam[i as usize] = byte;
        }

        self.memory.ppu.oam_dma = value;
    }
}

impl AddressBus for Bus {
    fn read(&self, address: u16) -> u8 {
        if let Some(byte) = self.boot_rom_read(address) {
            return byte;
        }

        match address {
            0x0000..=0x7FFF | 0xA000..=0xBFFF => self.memory.cartridge.mbc.read(address),
            0x8000..=0x9FFF => self.memory.ppu.vram.read(address),
            0xC000..=0xCFFF | 0xD000..=0xDFFF | 0xE000..=0xFDFF => {
                self.memory.wram[self.get_wram_index(address)]
            }
            0xFF00 => self.memory.joypad.read(),
            0xFF01 => self.memory.serial_data,
            0xFF02 => self.memory.serial_control | 0x7E,
            0xFF04 => (self.memory.timer.div >> 8) as u8,
            0xFF05 => self.memory.timer.tima,
            0xFF06 => self.memory.timer.tma,
            0xFF07 => self.memory.timer.tac | 0xF8,
            0xFE00..=0xFE9F => self.memory.ppu.oam[(address - 0xFE00) as usize],
            0xFEA0..=0xFEFF => 0xFF,
            0xFF0F => self.memory.interrupt_flag | 0xE0,
            //0xFF10..=0xFF26 => self.memory.apu.read((address - 0xFF10) as usize),
            0xFF30..=0xFF3F => self.memory.apu.wave_ram[(address - 0xFF30) as usize],
            0xFF40 => self.memory.ppu.lcdc,
            0xFF41 => {
                let equal = ((self.memory.ppu.ly == self.memory.ppu.lyc) as u8) << 2;
                0x80 | equal | (self.memory.ppu.mode() as u8)
            }
            0xFF42 => self.memory.ppu.scy,
            0xFF43 => self.memory.ppu.scx,
            0xFF44 => self.memory.ppu.ly,
            0xFF45 => self.memory.ppu.lyc,
            0xFF46 => self.memory.ppu.oam_dma,
            0xFF47 => self.memory.ppu.bgp,
            0xFF48 => self.memory.ppu.obp0,
            0xFF49 => self.memory.ppu.obp1,
            0xFF4A => self.memory.ppu.wy,
            0xFF4B => self.memory.ppu.wx,
            0xFF4D if self.is_cgb() => self.memory.key_register,
            0xFF4F if self.is_cgb() => self.memory.ppu.vram.bank | 0xFE,
            0xFF55 if self.is_cgb() => 0xFF,
            0xFF68 if self.is_cgb() => self.memory.ppu.bgpi | 0x40,
            //0xFF69 if self.is_cgb() => {
            //self.memory.ppu.bpcd[(self.memory.ppu.bgpi & 0x3F) as usize]
            //}
            //0xFF70 if self.is_cgb() => self.memory.svbk = value & 0x07,
            //0xFF6A if self.is_cgb() => self.memory.ppu.ocps | 0x40,
            //0xFF6B if self.is_cgb() => {
            //self.memory.ppu.ocpd[(self.memory.ppu.ocps & 0x3F) as usize]
            //}
            //0xFF6C if self.is_cgb() => self.memory.ppu.opri | 0xFE,
            //0xFF70 if self.is_cgb() => self.memory.svbk | 0xF8,
            0xFF76 if self.is_cgb() => 0xFF, // pcm12
            0xFF77 if self.is_cgb() => 0xFF, // pcm34
            0xFF80..=0xFFFE => self.memory.hram[(address - 0xFF80) as usize],
            0xFFFF => self.memory.interrupt_enable,
            _ => {
                eprintln!("The following address is not readable: {:04x}", address);
                0xFF
            }
        }
    }

    fn write(&mut self, address: u16, value: u8) {
        match address {
            0x0000..=0x7FFF | 0xA000..=0xBFFF => self.memory.cartridge.mbc.write(address, value),
            0x8000..=0x9FFF => self.memory.ppu.vram.write(address, value),
            0xC000..=0xDFFF | 0xE000..=0xFDFF => {
                let index = self.get_wram_index(address);
                self.memory.wram[index] = value
            }
            0xFF00 => self.memory.joypad.select = (value | 0xC0) & 0x30,
            0xFF01 => self.memory.serial_data = value,
            0xFF02 => {
                if value == 0x81 {
                    print!("{}", self.memory.serial_data as char);
                    self.memory.interrupt_flag |= InterruptMode::Serial.mask();
                }
            }
            0xFF04 => self.memory.timer.div = 0,
            0xFF05 => self.memory.timer.tima = value,
            0xFF06 => self.memory.timer.tma = value,
            0xFF07 => self.memory.timer.tac = value,
            0xFE00..=0xFE9F => self.memory.ppu.oam[(address - 0xFE00) as usize] = value,
            0xFF0F => self.memory.interrupt_flag = value & 0x1F,
            0xFF10..=0xFF26 => self.memory.apu.write(address, value),
            0xFF30..=0xFF3F => self.memory.apu.wave_ram[(address - 0xFF30) as usize] = value,
            0xFF40 => self.memory.ppu.write_lcdc(value),
            0xFF41 => self.memory.ppu.stat = value & 0x78, // ignore the coincidence flag
            0xFF42 => self.memory.ppu.scy = value,
            0xFF43 => self.memory.ppu.scx = value,
            0xFF44 => {}
            0xFF45 => self.memory.ppu.lyc = value,
            0xFF46 => self.dma_transfer(value),
            0xFF47 => self.memory.ppu.bgp = value,
            0xFF48 => self.memory.ppu.obp0 = value,
            0xFF49 => self.memory.ppu.obp1 = value,
            0xFF4A => self.memory.ppu.wy = value,
            0xFF4B => self.memory.ppu.wx = value,
            0xFF4D if self.is_cgb() => {
                self.memory.key_register = (self.memory.key_register & 0x80) | (value & 0x01);
            }
            0xFF4F if self.is_cgb() => self.memory.ppu.vram.bank_swap(value),
            0xFF50 => {
                if value.mask(0x01) != 0 {
                    self.boot_status = BootStatus::Complete;
                }
            }
            0xFF70 if self.is_cgb() => {
                self.memory.svbk_register = value;
            }
            0xFF80..=0xFFFE => self.memory.hram[(address - 0xFF80) as usize] = value,
            0xFFFF => self.memory.interrupt_enable = value,
            _ => eprintln!("The following address is not writable: {:04x}", address),
        }
    }

    fn pending_interrupt(&self) -> u8 {
        self.read(0xFF0F).mask(self.read(0xFFFF)).mask(0x1F)
    }

    fn perform_speed_switch(&mut self) -> bool {
        if self.is_cgb() && self.memory.key_register & 0x01 != 0 {
            self.memory.key_register ^= 0x80;
            self.memory.key_register &= !0x01;
            true
        } else {
            false
        }
    }
}

use crate::components::cartridge::{CGBFlag, Cartridge};

pub enum BootStatus {
    Complete,
    Incomplete,
}

/*
https://rylev.github.io/DMG-01/public/book/memory_map.html
https://gbdev.io/pandocs/Specifications.html
http://gameboy.mongenel.com/dmg/asmmemmap.html

MemoryMap

- 8 kb work ram for DMG/ 32 kb for CGB - read/write
- 8 kb video ram/ 16 kb for gameboy color
- 127 bytes of high ram - LD instructions
- 160 bytes of oam - sprites

*/

pub struct Memory {
    boot_status: BootStatus,
    cartridge: Cartridge,
    wram: Vec<u8>,
    vram: Vec<u8>,
    hram: Vec<u8>,
    oam: Vec<u8>,
}

impl Memory {
    pub fn new(cartridge: Cartridge) -> Self {
        let (wram_size, vram_size) = if cartridge.header.cgb_flag == CGBFlag::CBG {
            (0x8000, 0x4000)
        } else {
            (0x2000, 0x2000)
        };

        Self {
            boot_status: BootStatus::Incomplete,
            cartridge,
            wram: vec![0u8; wram_size],
            vram: vec![0u8; vram_size],
            hram: vec![0u8; 0x007F],
            oam: vec![0u8; 0x00A0],
        }
    }

    pub fn boot_complete(&mut self) {
        self.boot_status = BootStatus::Complete;
    }
}

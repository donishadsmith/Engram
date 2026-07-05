use crate::components::cartridge::{CGBFlag, Cartridge};

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
    pub cartridge: Cartridge,
    pub wram: Vec<u8>,
    pub vram: Vec<u8>,
    pub hram: Vec<u8>,
    pub oam: Vec<u8>,
    pub interrupt_flag: u8,
    pub interrupt_enable: u8,
}

impl Memory {
    pub fn new(cartridge: Cartridge) -> Self {
        let (wram_size, vram_size) = if cartridge.header.cgb_flag == CGBFlag::CBG {
            (0x8000, 0x4000)
        } else {
            (0x2000, 0x2000)
        };

        Self {
            cartridge,
            wram: vec![0u8; wram_size],
            vram: vec![0u8; vram_size],
            hram: vec![0u8; 0x007F],
            oam: vec![0u8; 0x00A0],
            interrupt_enable: 0x00,
            interrupt_flag: 0x00,
        }
    }
}

use crate::components::{
    apu::APU,
    joypad::Joypad,
    ppu::PPU,
    rom::cartridge::{CGBFlag, Cartridge},
    timer::Timer,
};

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
    pub ppu: PPU,
    pub apu: APU,
    pub timer: Timer,
    pub joypad: Joypad,
    pub wram: Vec<u8>,
    pub hram: Vec<u8>,
    pub interrupt_flag: u8,
    pub interrupt_enable: u8,
    pub serial_data: u8,
    pub serial_control: u8,
    pub key_register: u8,
    pub svbk_register: u8,
    pub hdma_registers: [u8; 5],
}

impl Memory {
    pub fn new(cartridge: Cartridge) -> Self {
        let cgb_flag = cartridge.header.cgb_flag;
        let wram_size = if cgb_flag == CGBFlag::CGB {
            0x8000
        } else {
            0x2000
        };

        Self {
            cartridge,
            wram: vec![0u8; wram_size],
            ppu: PPU::new(cgb_flag == CGBFlag::CGB),
            apu: APU::new(),
            timer: Timer::new(),
            joypad: Joypad::new(),
            hram: vec![0u8; 0x007F],
            interrupt_enable: 0x00,
            interrupt_flag: 0x00,
            serial_data: 0x00,
            serial_control: 0,
            key_register: 0,
            svbk_register: 0,
            hdma_registers: [0; 5],
        }
    }
}

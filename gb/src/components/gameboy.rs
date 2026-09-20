use crate::components::{
    bus::{AddressBus, Bus},
    cpu::{
        CPU,
        registers::{Register8Bits, Register16Bits},
    },
    gamepak::GamePak,
};
use shared::{
    ScriptTarget,
    render::{PixelFormat, to_rbg_single},
    utils::Emulator,
};
use std::{io::Error, mem::take};

const T_CYCLES_PER_FRAME_DOUBLE: u32 = 140448;

// http://marc.rawer.de/Gameboy/Docs/GBCPUman.pdf
// https://gekkio.fi/files/gb-docs/gbctr.pdf
// https://www.zilog.com/docs/z80/um0080.pdf

pub struct GameBoy {
    pub cpu: CPU<Bus>,
    pub keypad: [bool; 8],
}

impl GameBoy {
    pub fn boot(gamepak: GamePak) -> Self {
        let checksum = gamepak.header.checksum;
        let cgb_flag = gamepak.header.cgb_flag;
        let bus = Bus::new(gamepak);

        Self {
            cpu: CPU::start(cgb_flag, checksum, bus),
            keypad: [false; 8],
        }
    }

    pub fn run(&mut self, apu_sample_cycles: u32) {
        let mut remaining_cycles = T_CYCLES_PER_FRAME_DOUBLE;

        while remaining_cycles > 0 {
            let machine_cycles = self.cpu.cycle() as u32;
            let timer_t_cycles = machine_cycles * 4;

            for _ in 0..machine_cycles {
                self.cpu.bus.oam_dma_step();
            }

            let double_speed = self.cpu.bus.key_register & 0x80 != 0;
            let cpu_t_cycles = if double_speed {
                timer_t_cycles
            } else {
                timer_t_cycles * 2
            };

            let ppu_t_cycles = cpu_t_cycles / 2;

            self.cpu
                .bus
                .ppu
                .tick(ppu_t_cycles, &mut self.cpu.bus.interrupt_flag);

            self.cpu.bus.hblank_dma_step();

            self.cpu.bus.timer.tick(
                timer_t_cycles,
                &mut self.cpu.bus.interrupt_flag,
                double_speed,
            );

            let increase_apu_div_counter = self.cpu.bus.timer.increase_div_apu_counter;
            self.cpu
                .bus
                .apu
                .tick(ppu_t_cycles, apu_sample_cycles, increase_apu_div_counter);

            remaining_cycles = remaining_cycles.saturating_sub(cpu_t_cycles);
        }

        self.cpu
            .bus
            .joypad
            .poll(self.keypad, &mut self.cpu.bus.interrupt_flag);

        self.cpu.bus.gamepak.mbc.tick();
    }

    pub fn ppu_debug_dump(&self) {
        let ppu = &self.cpu.bus.ppu;
        let write_cram = |cram: &[u8]| {
            cram.chunks(8)
                .enumerate()
                .map(|(i, pal)| {
                    let colors: Vec<String> = pal
                        .chunks(2)
                        .map(|c| format!("{:04X}", u16::from_le_bytes([c[0], c[1]])))
                        .collect();
                    format!("palette{}: {}", i, colors.join(" "))
                })
                .collect::<Vec<_>>()
                .join("\n")
        };

        std::fs::write(
            "ppu_registers.txt",
            format!(
                "lcdc={:02X} stat={:02X} scy={} scx={} wy={} wx={} bgp={:02X} obp0={:02X} obp1={:02X} ly={} bgpi={:02X} obpi={:02X}\nBG CRAM:\n{}\nOBJ CRAM:\n{}",
                ppu.lcdc,
                ppu.stat,
                ppu.scy,
                ppu.scx,
                ppu.wy,
                ppu.wx,
                ppu.bgp,
                ppu.monochrome_color_ram[0],
                ppu.monochrome_color_ram[1],
                ppu.ly,
                ppu.bgpi,
                ppu.obpi,
                write_cram(&ppu.bg_palette_ram),
                write_cram(&ppu.obj_palette_ram),
            ),
        )
        .unwrap();
    }

    pub fn take_frame(&mut self) -> bool {
        take(&mut self.cpu.bus.ppu.frame_ready)
    }
}

impl Emulator for GameBoy {
    fn save(&mut self) -> Result<(), Error> {
        self.cpu.bus.gamepak.write_sav()?;

        Ok(())
    }
}

impl Drop for GameBoy {
    fn drop(&mut self) {
        let _ = self.save();
    }
}

impl ScriptTarget for GameBoy {
    fn read_u8(&mut self, address: u32) -> u8 {
        match u16::try_from(address) {
            Ok(address) => self.cpu.bus.read(address),
            Err(_) => 0xFF,
        }
    }

    fn read_u16(&mut self, address: u32) -> u16 {
        u16::from_le_bytes([self.read_u8(address), self.read_u8(address + 1)])
    }

    fn read_u32(&mut self, address: u32) -> u32 {
        u32::from_le_bytes([
            self.read_u8(address),
            self.read_u8(address + 1),
            self.read_u8(address + 2),
            self.read_u8(address + 3),
        ])
    }

    fn write_u8(&mut self, address: u32, value: u8) {
        match u16::try_from(address) {
            Ok(address) => self.cpu.bus.write(address, value),
            Err(_) => {}
        }
    }

    fn write_u16(&mut self, address: u32, value: u16) {
        let bytes = u16::to_le_bytes(value);

        self.write_u8(address, bytes[0]);
        self.write_u8(address + 1, bytes[1]);
    }

    fn write_u32(&mut self, address: u32, value: u32) {
        let bytes = u32::to_le_bytes(value);

        self.write_u8(address, bytes[0]);
        self.write_u8(address + 1, bytes[1]);
        self.write_u8(address + 2, bytes[2]);
        self.write_u8(address + 3, bytes[3]);
    }

    fn to_rgb(&self, value: u32) -> [u8; 3] {
        to_rbg_single(value, PixelFormat::Rgb555)
    }

    fn read_cpu_register(&self, register_name: String) -> Option<u64> {
        match register_name.as_str() {
            "a" => Some(self.cpu.registers.get_8bit(Register8Bits::A) as u64),
            "f" => Some(self.cpu.registers.get_8bit(Register8Bits::F) as u64),
            "b" => Some(self.cpu.registers.get_8bit(Register8Bits::B) as u64),
            "c" => Some(self.cpu.registers.get_8bit(Register8Bits::C) as u64),
            "d" => Some(self.cpu.registers.get_8bit(Register8Bits::D) as u64),
            "e" => Some(self.cpu.registers.get_8bit(Register8Bits::E) as u64),
            "h" => Some(self.cpu.registers.get_8bit(Register8Bits::H) as u64),
            "l" => Some(self.cpu.registers.get_8bit(Register8Bits::L) as u64),
            "ir" => Some(self.cpu.registers.instruction_register.unwrap_or_else(|| 0) as u64),
            "pc" => Some(self.cpu.registers.program_counter.address as u64),
            "sp" => Some(self.cpu.registers.stack_pointer as u64),
            "af" => Some(self.cpu.registers.get_16bit(Register16Bits::AF) as u64),
            "bc" => Some(self.cpu.registers.get_16bit(Register16Bits::BC) as u64),
            "de" => Some(self.cpu.registers.get_16bit(Register16Bits::DE) as u64),
            "hl" => Some(self.cpu.registers.get_16bit(Register16Bits::HL) as u64),
            _ => None,
        }
    }
}

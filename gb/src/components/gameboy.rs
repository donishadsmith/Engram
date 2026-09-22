use crate::components::{
    bus::{AddressBus, Bus},
    cpu::{
        CPU,
        registers::{Register8Bits, Register16Bits},
    },
    gamepak::GamePak,
};
use shared::{
    Emulator, ScriptTarget,
    render::{PixelFormat, to_rbg_single},
    script::{CpuError, DomainError},
};
use std::{io::Error, mem::take};

const T_CYCLES_PER_FRAME_DOUBLE: u32 = 140448;

// http://marc.rawer.de/Gameboy/Docs/GBCPUman.pdf
// https://gekkio.fi/files/gb-docs/gbctr.pdf
// https://www.zilog.com/docs/z80/um0080.pdf

pub struct GameBoy {
    pub cpu: CPU<Bus>,
    pub keypad: [bool; 8],
    pub remaining_cycles: u32,
}

impl GameBoy {
    pub fn boot(gamepak: GamePak) -> Self {
        let checksum = gamepak.header.checksum;
        let cgb_flag = gamepak.header.cgb_flag;
        let bus = Bus::new(gamepak);

        Self {
            cpu: CPU::start(cgb_flag, checksum, bus),
            keypad: [false; 8],
            remaining_cycles: 0,
        }
    }

    pub fn step(&mut self, apu_sample_cycles: u32) -> u32 {
        let machine_cycles = self.cpu.cycle() as u32;
        if self.cpu.breakpoint_hit.is_some() {
            return 0;
        }

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
        let div_apu = self.cpu.bus.timer.increase_div_apu_counter;
        self.cpu
            .bus
            .apu
            .tick(ppu_t_cycles, apu_sample_cycles, div_apu);

        cpu_t_cycles
    }

    pub fn run(&mut self, apu_sample_cycles: u32) {
        self.replenish_remaining_cycles();
        while self.remaining_cycles > 0 {
            self.remaining_cycles = self
                .remaining_cycles
                .saturating_sub(self.step(apu_sample_cycles));

            if self.cpu.breakpoint_hit.is_some() {
                return;
            }
        }

        self.cpu
            .bus
            .joypad
            .poll(self.keypad, &mut self.cpu.bus.interrupt_flag);
        self.cpu.bus.gamepak.mbc.tick();
    }

    pub fn replenish_remaining_cycles(&mut self) {
        if self.remaining_cycles == 0 {
            self.remaining_cycles = T_CYCLES_PER_FRAME_DOUBLE
        }
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

    fn remove_breakpoint(&mut self, address: u32) {
        self.cpu.remove_breakpoint(address as u16);
    }

    fn set_breakpoint(&mut self, address: u32) {
        self.cpu.breakpoint_queue.push(address as u16);
    }

    fn take_breakpoint_hit(&mut self) -> Option<u32> {
        take(&mut self.cpu.breakpoint_hit)
    }

    fn clear_all_breakpoints(&mut self) {
        self.cpu.breakpoint_queue.clear();
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

    fn read_cpu_register(&self, register_name: String) -> Result<u64, CpuError> {
        match register_name.as_str() {
            "a" => Ok(self.cpu.registers.get_8bit(Register8Bits::A) as u64),
            "f" => Ok(self.cpu.registers.get_8bit(Register8Bits::F) as u64),
            "b" => Ok(self.cpu.registers.get_8bit(Register8Bits::B) as u64),
            "c" => Ok(self.cpu.registers.get_8bit(Register8Bits::C) as u64),
            "d" => Ok(self.cpu.registers.get_8bit(Register8Bits::D) as u64),
            "e" => Ok(self.cpu.registers.get_8bit(Register8Bits::E) as u64),
            "h" => Ok(self.cpu.registers.get_8bit(Register8Bits::H) as u64),
            "l" => Ok(self.cpu.registers.get_8bit(Register8Bits::L) as u64),
            "ir" => Ok(self.cpu.registers.instruction_register.unwrap_or_else(|| 0) as u64),
            "pc" => Ok(self.cpu.registers.program_counter.address as u64),
            "sp" => Ok(self.cpu.registers.stack_pointer as u64),
            "af" => Ok(self.cpu.registers.get_16bit(Register16Bits::AF) as u64),
            "bc" => Ok(self.cpu.registers.get_16bit(Register16Bits::BC) as u64),
            "de" => Ok(self.cpu.registers.get_16bit(Register16Bits::DE) as u64),
            "hl" => Ok(self.cpu.registers.get_16bit(Register16Bits::HL) as u64),
            _ => Err(CpuError::UnknownRegister),
        }
    }

    fn write_cpu_register(&mut self, register_name: String, value: u32) -> Result<(), CpuError> {
        match register_name.as_str() {
            "a" => self.cpu.registers.set_8bit(Register8Bits::A, value as u8),
            "f" => self.cpu.registers.set_8bit(Register8Bits::F, value as u8),
            "b" => self.cpu.registers.set_8bit(Register8Bits::B, value as u8),
            "c" => self.cpu.registers.set_8bit(Register8Bits::C, value as u8),
            "d" => self.cpu.registers.set_8bit(Register8Bits::D, value as u8),
            "e" => self.cpu.registers.set_8bit(Register8Bits::E, value as u8),
            "h" => self.cpu.registers.set_8bit(Register8Bits::H, value as u8),
            "l" => self.cpu.registers.set_8bit(Register8Bits::L, value as u8),
            "ir" => return Err(CpuError::ReadOnly),
            "pc" => self.cpu.registers.program_counter.address = value as u16,
            "sp" => self.cpu.registers.stack_pointer = value as u16,
            "af" => self
                .cpu
                .registers
                .set_16bit(Register16Bits::AF, value as u16),
            "bc" => self
                .cpu
                .registers
                .set_16bit(Register16Bits::BC, value as u16),
            "de" => self
                .cpu
                .registers
                .set_16bit(Register16Bits::DE, value as u16),
            "hl" => self
                .cpu
                .registers
                .set_16bit(Register16Bits::HL, value as u16),
            _ => return Err(CpuError::UnknownRegister),
        }

        Ok(())
    }

    fn cpu_register_names(&self) -> &'static [&'static str] {
        &[
            "r0", "r1", "r2", "r3", "r4", "r5", "r6", "r7", "r8", "r9", "r10", "r11", "r12", "r13",
            "sp", "r14", "lr", "r15", "pc", "cpsr",
        ]
    }

    fn memory_domain_names(&self) -> &'static [&'static str] {
        &[
            "rom",
            "oam",
            "hram",
            "vram",
            "sram",
            "wram",
            "bg_palette",
            "obj_palette",
        ]
    }

    fn read_domain(&self, domain: &str, offset: usize) -> Result<u8, DomainError> {
        let region: &[u8] = match domain {
            "rom" => self.cpu.bus.gamepak.mbc.get_rom(),
            "vram" => &self.cpu.bus.ppu.vram.memory,
            "oam" => &self.cpu.bus.ppu.oam,
            "hram" => &self.cpu.bus.hram,
            "sram" => &self.cpu.bus.gamepak.mbc.get_ram(),
            "bg_palette" => &self.cpu.bus.ppu.bg_palette_ram,
            "obj_palette" => &self.cpu.bus.ppu.obj_palette_ram,
            "wram" => &self.cpu.bus.wram,
            _ => return Err(DomainError::UnknownDomain),
        };

        region
            .get(offset)
            .copied()
            .ok_or(DomainError::OutOfRange { size: region.len() })
    }

    fn write_domain(&mut self, domain: &str, offset: usize, value: u8) -> Result<(), DomainError> {
        let region: &mut [u8] = match domain {
            "rom" => self.cpu.bus.gamepak.mbc.get_rom_mut(),
            "vram" => &mut self.cpu.bus.ppu.vram.memory,
            "oam" => &mut self.cpu.bus.ppu.oam,
            "hram" => &mut self.cpu.bus.hram,
            "sram" => self.cpu.bus.gamepak.mbc.get_ram_mut(),
            "bg_palette" => &mut self.cpu.bus.ppu.bg_palette_ram,
            "obj_palette" => &mut self.cpu.bus.ppu.obj_palette_ram,
            "wram" => &mut self.cpu.bus.wram,
            _ => return Err(DomainError::UnknownDomain),
        };

        let old_value = region.get_mut(offset);

        match old_value {
            Some(v) => {
                *v = value;
                Ok(())
            }
            None => Err(DomainError::OutOfRange { size: region.len() }),
        }
    }
}

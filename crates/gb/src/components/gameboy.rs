use std::{
    io::Error,
    mem::{swap, take},
};

use serde::{Deserialize, Serialize};
use shared::{
    Breakpoint, Cycles, Emulator, Rewind, ScriptTarget, ScriptedInput, State, Watchpoint,
    render::{Frame, PixelFormat, to_rbg_single},
    script::{CpuError, DomainError, MemoryAccessor},
    structs::{BreakpointManager, CycleManager, RewindManager, WatchpointManager},
    traits::BitOps,
    utils::error_message,
};

use crate::{
    GB_CLOCK_SPEED,
    components::{
        bus::{AddressBus, Bus},
        cpu::{
            SharpSm83,
            registers::{Register8Bits, Register16Bits},
        },
        gamepak::{GamePak, mbc::prelude::Mbc},
    },
};

pub const STATE_MAGIC_NAME: &[u8] = b"ENGRAMGB6";
pub const T_CYCLES_PER_FRAME_DOUBLE: u32 = 140448;

// http://marc.rawer.de/Gameboy/Docs/GBCPUman.pdf
// https://gekkio.fi/files/gb-docs/gbctr.pdf
// https://www.zilog.com/docs/z80/um0080.pdf

#[derive(Serialize, Deserialize)]
pub struct GameBoy {
    pub cpu: SharpSm83<Bus>,
    #[serde(skip)]
    pub scripted_keypad: Option<Vec<bool>>,
    pub keypad: [bool; 8],
    pub remaining_cycles: u32,
    pub apu_sample_period: f64,
    pub rumble_strength: f32,
    #[serde(skip)]
    cycle_manager: CycleManager,
    #[serde(skip)]
    pub force_screen_update: bool,
    #[serde(skip)]
    pub rewind_manager: RewindManager,
}

impl GameBoy {
    pub fn boot(gamepak: GamePak, apu_sample_period: f64) -> Self {
        let checksum = gamepak.header.checksum;
        let cgb_flag = gamepak.header.cgb_flag;
        let bus = Bus::new(gamepak, apu_sample_period);

        Self {
            cpu: SharpSm83::start(cgb_flag, checksum, bus),
            scripted_keypad: None,
            keypad: [false; 8],
            remaining_cycles: 0,
            apu_sample_period,
            rumble_strength: 0.0,
            cycle_manager: CycleManager::default(),
            force_screen_update: false,
            rewind_manager: RewindManager::new(GB_CLOCK_SPEED as u64),
        }
    }

    pub fn step(&mut self) -> u32 {
        let pc = self.cpu.registers.program_counter.address.wrapping_sub(1) as u32;
        let hits_before = self.cpu.bus.watchpoint_manager.hits.len();

        let machine_cycles = self.cpu.cycle() as u32;
        if self.cpu.breakpoint_manager.hit.is_some() {
            return 0;
        }

        let t_cycles = machine_cycles * 4;
        self.cpu.bus.t_cycles += t_cycles as u64;

        for _ in 0..machine_cycles {
            self.cpu.bus.oam_dma_step();
        }

        let double_speed = self.cpu.bus.key_register.is_set(7);
        let cpu_t_cycles = if double_speed { t_cycles } else { t_cycles * 2 };
        self.cpu.bus.gamepak.mbc.increment_force_feedback_counter(cpu_t_cycles);
        let ppu_t_cycles = cpu_t_cycles / 2;

        self.cpu.bus.ppu.tick(ppu_t_cycles, &mut self.cpu.bus.interrupt_flag);
        self.cpu.bus.hblank_dma_step();
        self.cpu
            .bus
            .timer
            .tick(t_cycles, &mut self.cpu.bus.interrupt_flag, double_speed);
        let div_apu = self.cpu.bus.timer.increase_div_apu_counter;
        self.cpu.bus.apu.tick(ppu_t_cycles, self.apu_sample_period, div_apu);

        for hit in &mut self.cpu.bus.watchpoint_manager.hits[hits_before..] {
            hit.pc = pc;
        }

        cpu_t_cycles
    }

    pub fn run(&mut self) {
        self.replenish_remaining_cycles();
        while self.remaining_cycles > 0 {
            self.remaining_cycles = self.remaining_cycles.saturating_sub(self.step());

            if self.cpu.bus.ppu.frame_ready || self.cpu.bus.ppu.lcd_off_frame && self.remaining_cycles <= 0 {
                let rumble_count = self.cpu.bus.gamepak.mbc.force_feedback_counter() as f32;
                self.rumble_strength = (rumble_count / T_CYCLES_PER_FRAME_DOUBLE as f32).min(1.0);
                self.cpu.bus.gamepak.mbc.reset_rumble_counter();
            }

            if self.cpu.breakpoint_manager.hit.is_some()
                || self.cpu.bus.watchpoint_manager.pause
                || self.cpu.bus.ppu.frame_ready
                || self.cpu.bus.ppu.lcd_off_frame && self.remaining_cycles <= 0
                || self.pause_at_current_cycle()
            {
                return;
            }
        }

        self.end_of_frame();
    }

    pub fn end_of_frame(&mut self) {
        self.cpu.bus.joypad.poll(
            self.scripted_keypad
                .as_deref()
                .and_then(|keys| keys.try_into().ok())
                .unwrap_or(self.keypad),
            &mut self.cpu.bus.interrupt_flag,
        );

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
                "lcdc={:02X} stat={:02X} scy={} scx={} wy={} wx={} bgp={:02X} obp0={:02X} obp1={:02X} ly={} \
                 bgpi={:02X} obpi={:02X}\nBG CRAM:\n{}\nOBJ CRAM:\n{}",
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
        take(&mut self.cpu.bus.ppu.frame_ready) || take(&mut self.cpu.bus.ppu.lcd_off_frame)
    }
}

impl Emulator for GameBoy {
    fn save(&mut self) -> Result<(), Error> {
        self.cpu.bus.gamepak.write_sav()?;

        Ok(())
    }

    fn frontend_mut(&mut self) -> &mut Frame {
        &mut self.cpu.bus.ppu.frontend
    }
}

impl Breakpoint for GameBoy {
    fn breakpoint_manager_ref(&self) -> &BreakpointManager {
        &self.cpu.breakpoint_manager
    }

    fn breakpoint_manager_mut(&mut self) -> &mut BreakpointManager {
        &mut self.cpu.breakpoint_manager
    }
}

impl Watchpoint for GameBoy {
    fn watchpoint_manager_ref(&self) -> &WatchpointManager {
        &self.cpu.bus.watchpoint_manager
    }

    fn watchpoint_manager_mut(&mut self) -> &mut WatchpointManager {
        &mut self.cpu.bus.watchpoint_manager
    }
}

impl Cycles for GameBoy {
    fn cycle_manager_ref(&self) -> &CycleManager {
        &self.cycle_manager
    }

    fn cycle_manager_mut(&mut self) -> &mut CycleManager {
        &mut self.cycle_manager
    }

    fn clock_rate(&self) -> f64 {
        GB_CLOCK_SPEED
    }

    fn elapsed_cycles(&self) -> u64 {
        self.cpu.bus.t_cycles
    }
}

impl State for GameBoy {
    fn state_magic_name(&self) -> &[u8] {
        STATE_MAGIC_NAME
    }

    fn load_state(&mut self, bytes: &[u8]) -> Result<(), Error> {
        let magic_len = STATE_MAGIC_NAME.len();
        if bytes.len() < magic_len || &bytes[..magic_len] != STATE_MAGIC_NAME {
            return Err(error_message("Not a Game Boy save state".to_string()));
        }

        let decompressed_data = lz4_flex::decompress_size_prepended(&bytes[magic_len..])
            .map_err(|err| error_message(format!("Invalid save state: {err}")))?;
        let mut data: GameBoy = postcard::from_bytes(&decompressed_data)
            .map_err(|err| error_message(format!("Invalid save state: {err}")))?;

        let (old, new) = (&self.cpu.bus.gamepak.header, &data.cpu.bus.gamepak.header);
        if old.title != new.title || old.checksum != new.checksum {
            return Err(error_message("Save state is for a different game".to_string()));
        }

        self.save()?;

        // note to self: since rtc is based on sysytem clock be aware that the
        // time will automatically be updated to current time
        swap(
            &mut data.cpu.bus.watchpoint_manager,
            &mut self.cpu.bus.watchpoint_manager,
        );

        swap(&mut data.cpu.breakpoint_manager, &mut self.cpu.breakpoint_manager);

        data.apu_sample_period = self.apu_sample_period;
        data.force_screen_update = true;
        data.frontend_mut().buffer_changed = true;

        data.cpu
            .bus
            .gamepak
            .mbc
            .set_rom(self.cpu.bus.gamepak.mbc.get_rom().into());
        swap(&mut data.cpu.bus.gamepak.sav_path, &mut self.cpu.bus.gamepak.sav_path);
        swap(&mut data.scripted_keypad, &mut self.scripted_keypad);

        swap(&mut data.cpu.bus.apu.psg_output, &mut self.cpu.bus.apu.psg_output);
        data.cpu.bus.apu.mixer.changed = true;

        swap(&mut data.cycle_manager, &mut self.cycle_manager);

        swap(&mut data.rewind_manager, &mut self.rewind_manager);

        *self = data;

        Ok(())
    }
}

impl Rewind for GameBoy {
    fn rewind_manager_ref(&self) -> &RewindManager {
        &self.rewind_manager
    }

    fn rewind_manager_mut(&mut self) -> &mut RewindManager {
        &mut self.rewind_manager
    }

    fn snapshot(&self) -> Result<Vec<u8>, Error> {
        self.save_state()
    }

    fn rewind_cycles(&self) -> u64 {
        let double_speed = self.cpu.bus.key_register.is_set(7);

        if double_speed {
            self.cpu.bus.t_cycles / 2
        } else {
            self.cpu.bus.t_cycles
        }
    }
}

impl Drop for GameBoy {
    fn drop(&mut self) {
        let _ = self.save();
    }
}

impl ScriptedInput for GameBoy {
    fn scripted_keypad_ref(&self) -> &Option<Vec<bool>> {
        &self.scripted_keypad
    }

    fn scripted_keypad_mut(&mut self) -> &mut Option<Vec<bool>> {
        &mut self.scripted_keypad
    }
}

impl ScriptTarget for GameBoy {
    fn read_u8(&mut self, address: u32) -> u8 {
        match u16::try_from(address) {
            Ok(address) => self.cpu.bus.read(address, MemoryAccessor::Lua),
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
            Ok(address) => self.cpu.bus.write(address, value, MemoryAccessor::Lua),
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
            "af" => self.cpu.registers.set_16bit(Register16Bits::AF, value as u16),
            "bc" => self.cpu.registers.set_16bit(Register16Bits::BC, value as u16),
            "de" => self.cpu.registers.set_16bit(Register16Bits::DE, value as u16),
            "hl" => self.cpu.registers.set_16bit(Register16Bits::HL, value as u16),
            _ => return Err(CpuError::UnknownRegister),
        }

        Ok(())
    }

    fn cpu_register_names(&self) -> &'static [&'static str] {
        &[
            "a", "f", "b", "c", "d", "e", "h", "l", "ir", "pc", "sp", "af", "bc", "de", "hl",
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

    fn address_to_domain(&self, address: u32) -> Option<(&'static str, usize)> {
        let address = u16::try_from(address).ok()?;

        let (domain, base) = match address {
            0x0000..=0x3FFF => ("rom", address as usize),
            0x4000..=0x7FFF => (
                "rom",
                self.cpu.bus.gamepak.mbc.rom_bank() * 0x4000 + (address - 0x4000) as usize,
            ),
            0x8000..=0x9FFF => (
                "vram",
                (self.cpu.bus.ppu.vram.bank as usize) * 0x2000 + (address - 0x8000) as usize,
            ),
            0xA000..=0xBFFF => (
                "sram",
                self.cpu.bus.gamepak.mbc.ram_bank() * 0x2000 + (address - 0xA000) as usize,
            ),
            0xC000..=0xCFFF => ("wram", (address - 0xC000) as usize),
            0xD000..=0xDFFF => ("wram", self.cpu.bus.get_wram_index(address)),
            0xFE00..=0xFE9F => ("oam", (address - 0xFE00) as usize),
            0xFF80..=0xFFFE => ("hram", (address - 0xFF80) as usize),
            _ => return None,
        };

        Some((domain, base))
    }
}

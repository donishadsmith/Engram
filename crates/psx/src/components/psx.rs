use std::{io::Error, mem::swap};

use serde::{Deserialize, Serialize};
use shared::{
    Breakpoint, Cycles, Emulator, Rewind, ScriptTarget, ScriptedInput, State, Watchpoint,
    render::Frame,
    script::{CpuError, DomainError, MemoryAccessor, WatchpointArgs, WatchpointRange},
    structs::{BreakpointManager, CycleManager, RewindManager, WatchpointManager},
    utils::error_message,
};

use crate::components::{
    bus::Bus,
    cpu::{R3000a, RegisterAlias, cop0::Cop0Registers},
};

pub const PSX_CLOCK_SPEED: f64 = 33868800.0;
pub const STATE_MAGIC_NAME: &[u8] = b"ENGRAMPSX0-ALPHA";

#[derive(Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Deserialize, Serialize)]
pub enum CdromEvent {}

#[derive(Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Deserialize, Serialize)]
pub enum Event {
    Hblank,
    Vblank,
    Timer(u8),
    Dma(u8),
    SpuSample,
    Sio,
    Cdrom(CdromEvent),
}

// the easist way to get things working sooner is probably to copy verbatum most of the gba code for
// the traits for now then refactor everything around the time i start the fourth emu
#[derive(Serialize, Deserialize)]
pub struct Psx {
    pub bus: Bus,
    pub cpu: R3000a,
    #[serde(skip)]
    pub scripted_keypad: Option<Vec<bool>>,
    pub keypad: [bool; 14],
    spu_sample_period: f64,
    pub spu_sample_remainder: f64,
    #[serde(skip)]
    cycle_manager: CycleManager,
    #[serde(skip)]
    pub force_buffer_change: bool, // for debugger
    #[serde(skip)]
    pub force_screen_update: bool,
    #[serde(skip)]
    pub rewind_manager: RewindManager,
}

// put cd rom as parameter eventually
impl Psx {
    pub fn boot(spu_sample_period: f64) -> Self {
        let mut bus = Bus::new(spu_sample_period);
        let mut cpu = R3000a::new();

        Self {
            bus,
            cpu,
            scripted_keypad: None,
            keypad: [false; 14],
            spu_sample_period,
            spu_sample_remainder: 0.0,
            cycle_manager: CycleManager::default(),
            force_buffer_change: false,
            force_screen_update: false,
            rewind_manager: RewindManager::new(PSX_CLOCK_SPEED as u64),
        }
    }

    pub fn run(&mut self) {
        let pc = self.cpu.registers.current_pc;
        let hits_before = self.bus.watchpoint_manager.hits.len();

        self.cpu.step(&mut self.bus);

        if self.cpu.breakpoint_manager.hit.is_some() {
            return;
        }

        self.handle_events();

        let (istat, imask) = (self.bus.istat, self.bus.imask);

        self.bus.dump(format_args!(
            "interrupt_status={:032b} interrupt_mask={:032b}",
            istat, imask
        ));

        // technically serviced by the time it shows on ui + plus will just be a
        // flash on ui when enabled but looks cool
        self.bus.copy_interrupt_info();

        for hit in &mut self.bus.watchpoint_manager.hits[hits_before..] {
            hit.pc = pc;
        }
    }

    pub fn debugging_active(&self) -> bool {
        !self.cpu.breakpoint_manager.queue.is_empty()
            || !self.bus.watchpoint_manager.queue.is_empty()
            || self.bus.trace.is_some()
            || self.cycle_manager.pause_at_cycle.is_some()
            || self.cycle_manager.elapsed_cycles_pause.is_some()
    }

    pub fn run_until_event(&mut self) {
        loop {
            self.cpu.step(&mut self.bus);

            if self.bus.scheduler.current >= self.bus.scheduler.next() {
                break;
            }
        }

        self.handle_events();
        self.bus.copy_interrupt_info();
    }

    fn handle_events(&mut self) {
        while let Some((deadline, event)) = self.bus.scheduler.pop() {
            match event {
                _ => {}
            }
        }
    }

    pub fn take_frame(&mut self) -> bool {
        todo!()
    }
}

impl Emulator for Psx {
    fn save(&mut self) -> Result<(), Error> {
        Ok(())
    }

    fn frontend_mut(&mut self) -> &mut Frame {
        todo!()
    }
}

impl Breakpoint for Psx {
    fn breakpoint_manager_ref(&self) -> &BreakpointManager {
        &self.cpu.breakpoint_manager
    }

    fn breakpoint_manager_mut(&mut self) -> &mut BreakpointManager {
        &mut self.cpu.breakpoint_manager
    }
}

impl Watchpoint for Psx {
    fn watchpoint_manager_ref(&self) -> &WatchpointManager {
        &self.bus.watchpoint_manager
    }

    fn watchpoint_manager_mut(&mut self) -> &mut WatchpointManager {
        &mut self.bus.watchpoint_manager
    }

    fn set_watchpoint(&mut self, watchpoint_range: WatchpointRange, watchpoint_args: WatchpointArgs) -> bool {
        self.bus.set_watchpoint(watchpoint_range, watchpoint_args)
    }

    fn remove_watchpoint(&mut self, watchpoint_range: WatchpointRange) {
        self.bus.remove_watchpoint(watchpoint_range);
    }
}

impl Cycles for Psx {
    fn cycle_manager_ref(&self) -> &CycleManager {
        &self.cycle_manager
    }

    fn cycle_manager_mut(&mut self) -> &mut CycleManager {
        &mut self.cycle_manager
    }

    fn clock_rate(&self) -> f64 {
        PSX_CLOCK_SPEED
    }

    fn elapsed_cycles(&self) -> u64 {
        self.bus.scheduler.current
    }
}

impl State for Psx {
    fn state_magic_name(&self) -> &[u8] {
        STATE_MAGIC_NAME
    }

    fn load_state(&mut self, bytes: &[u8]) -> Result<(), Error> {
        let magic_len = STATE_MAGIC_NAME.len();
        if bytes.len() < magic_len || &bytes[..magic_len] != STATE_MAGIC_NAME {
            return Err(error_message("Not a Playstation 1 save state".to_string()));
        }

        let decompressed_data = lz4_flex::decompress_size_prepended(&bytes[magic_len..])
            .map_err(|err| error_message(format!("Invalid save state: {err}")))?;
        let mut data: Psx = postcard::from_bytes(&decompressed_data)
            .map_err(|err| error_message(format!("Invalid save state: {err}")))?;

        // add header rom stuff later
        /*
        if self.bus.cdrom.header != data.bus.cdrom.header {
            return Err(error_message(
                "Save state is for a different game".to_string(),
            ));
        }
        */

        self.save()?;

        swap(&mut data.bus.watchpoint_manager, &mut self.bus.watchpoint_manager);

        swap(&mut data.cpu.breakpoint_manager, &mut self.cpu.breakpoint_manager);

        swap(&mut data.scripted_keypad, &mut self.scripted_keypad);

        data.spu_sample_period = self.spu_sample_period;
        swap(&mut data.bus.trace, &mut self.bus.trace);
        data.spu_sample_remainder = self.spu_sample_remainder;

        data.force_buffer_change = true;

        data.force_screen_update = true;

        swap(&mut data.cycle_manager, &mut self.cycle_manager);

        swap(&mut data.rewind_manager, &mut self.rewind_manager);

        data.bus.istat_copy = self.bus.istat_copy;
        data.bus.imask_copy = self.bus.imask_copy;

        *self = data;

        Ok(())
    }
}

impl Rewind for Psx {
    fn rewind_manager_ref(&self) -> &RewindManager {
        &self.rewind_manager
    }

    fn rewind_manager_mut(&mut self) -> &mut RewindManager {
        &mut self.rewind_manager
    }

    fn snapshot(&self) -> Result<Vec<u8>, Error> {
        self.save_state()
    }
}

impl Drop for Psx {
    fn drop(&mut self) {
        let _ = self.save();
    }
}

impl ScriptedInput for Psx {
    fn scripted_keypad_ref(&self) -> &Option<Vec<bool>> {
        &self.scripted_keypad
    }

    fn scripted_keypad_mut(&mut self) -> &mut Option<Vec<bool>> {
        &mut self.scripted_keypad
    }
}

impl ScriptTarget for Psx {
    fn read_u8(&mut self, address: u32) -> u8 {
        self.bus.peek::<u8>(address)
    }

    fn read_u16(&mut self, address: u32) -> u16 {
        self.bus.peek::<u16>(address)
    }

    fn read_u32(&mut self, address: u32) -> u32 {
        self.bus.peek::<u32>(address)
    }

    fn write_u8(&mut self, address: u32, value: u8) {
        self.bus.write::<u8>(address, value, MemoryAccessor::Lua);
    }

    fn write_u16(&mut self, address: u32, value: u16) {
        self.bus.write::<u16>(address, value, MemoryAccessor::Lua);
    }

    fn write_u32(&mut self, address: u32, value: u32) {
        self.bus.write::<u32>(address, value, MemoryAccessor::Lua);
    }

    fn read_cpu_register(&self, register_name: String) -> Result<u64, CpuError> {
        let name = register_name.as_str();

        if let Some(alias) = RegisterAlias::from_name(name) {
            return Ok(self.cpu.registers.lua_read_cpu_register(alias) as u64);
        }

        if let Some(register) = Cop0Registers::from_name(name) {
            return Ok(self.cpu.cop0.move_from(register) as u64);
        }

        Err(CpuError::UnknownRegister)
    }

    fn write_cpu_register(&mut self, register_name: String, value: u32) -> Result<(), CpuError> {
        let name = register_name.as_str();

        if let Some(alias) = RegisterAlias::from_name(name) {
            self.cpu.registers.lua_write_cpu_register(alias, value);

            return Ok(());
        }

        match Cop0Registers::from_name(name) {
            Some(Cop0Registers::Prid) => Err(CpuError::ReadOnly),
            Some(register) => {
                self.cpu.cop0.move_to(register, value);

                Ok(())
            }
            None => Err(CpuError::UnknownRegister),
        }
    }

    fn to_rgb(&self, value: u32) -> [u8; 3] {
        todo!()
    }

    fn cpu_register_names(&self) -> &'static [&'static str] {
        &[
            "zero",
            "at",
            "v0",
            "v1",
            "a0",
            "a1",
            "a2",
            "a3",
            "t0",
            "t1",
            "t2",
            "t3",
            "t4",
            "t5",
            "t6",
            "t7",
            "s0",
            "s1",
            "s2",
            "s3",
            "s4",
            "s5",
            "s6",
            "s7",
            "t8",
            "t9",
            "k0",
            "k1",
            "gp",
            "sp",
            "fp",
            "ra",
            "pc",
            "hi",
            "lo",
            "cop0_bpc",
            "cop0_bda",
            "cop0_tar",
            "cop0_dcic",
            "cop0_bada",
            "cop0_bdam",
            "cop0_bpcm",
            "cop0_sr",
            "cop0_cause",
            "cop0_epc",
            "cop0_prid",
        ]
    }

    fn memory_domain_names(&self) -> &'static [&'static str] {
        /* &["rom", "oam", "palette", "vram", "iwram", "ewram"] */
        todo!()
    }

    fn read_domain(&self, domain: &str, offset: usize) -> Result<u8, DomainError> {
        /*
        let region: &[u8] = match domain {
            "rom" => &self.bus.gamepak.rom,
            "vram" => &*self.bus.ppu.vram,
            "oam" => &*self.bus.ppu.oam,
            "iwram" => &*self.bus.iwram,
            "ewram" => &*self.bus.ewram,
            "palette" => &*self.bus.ppu.palette_ram,
            _ => return Err(DomainError::UnknownDomain),
        };

        region
            .get(offset)
            .copied()
            .ok_or(DomainError::OutOfRange { size: region.len() })
        */

        todo!()
    }

    fn write_domain(&mut self, domain: &str, offset: usize, value: u8) -> Result<(), DomainError> {
        /*let region: &mut [u8] = match domain {
            "rom" => &mut *self.bus.gamepak.rom,
            "vram" => &mut *self.bus.ppu.vram,
            "oam" => &mut *self.bus.ppu.oam,
            "iwram" => &mut *self.bus.iwram,
            "ewram" => &mut *self.bus.ewram,
            "palette" => &mut *self.bus.ppu.palette_ram,
            _ => return Err(DomainError::UnknownDomain),
        };

        let old_value = region.get_mut(offset);

        match old_value {
            Some(v) => {
                *v = value;
                Ok(())
            }
            None => Err(DomainError::OutOfRange { size: region.len() }),
        }*/

        todo!()
    }

    fn address_to_domain(&self, address: u32) -> Option<(&'static str, usize)> {
        /*let (domain, offset) = match address >> 24 {
            0x02 => ("ewram", Bus::ewram_index(address)),
            0x03 => ("iwram", Bus::iwram_index(address)),
            0x05 => ("palette", Bus::palette_index(address)),
            0x06 => ("vram", Bus::vram_index(address)),
            0x07 => ("oam", Bus::oam_index(address)),
            0x08..=0x0D => ("rom", address.get_bit_range(0..25) as usize),
            _ => return None,
        };

        Some((domain, offset))*/

        todo!()
    }
}

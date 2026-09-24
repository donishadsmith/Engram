// https://archive.org/details/NintendoGbaManualV1.1/page/n59/mode/1up
use crate::components::{
    bus::{AccessType, Bus},
    cpu::{Arm7tdmi, HaltState},
    dma::Trigger,
    gamepak::GamePak,
    scheduler::Event,
};
use shared::{
    Emulator, EmulatorState, ScriptTarget,
    render::to_rbg_single,
    script::{CpuError, DomainError, WatchpointArgs, WatchpointHit},
    traits::BitOps,
};
use std::{io::Error, mem::take};

pub struct GBA {
    pub bus: Bus,
    pub cpu: Arm7tdmi,
    pub keypad: [bool; 10],
}

impl GBA {
    pub fn boot(gamepak: GamePak, apu_sample_period: u32) -> Self {
        let mut bus = Bus::new(gamepak, apu_sample_period);
        bus.skip_boot();

        bus.scheduler.initialize_events();

        let mut cpu = Arm7tdmi::new();
        cpu.skip_boot();

        Self {
            bus,
            cpu,
            keypad: [false; 10],
        }
    }

    pub fn run(&mut self) {
        let bus = &mut self.bus;

        let pc = self.cpu.next_executing_address();
        let hits_before = bus.watchpoint_hits.len();

        if self.cpu.is_halted() {
            bus.scheduler.skip_to_next_event();
        } else {
            self.cpu.step(bus);

            if self.cpu.breakpoint_hit.is_some() {
                return;
            }
        }

        if let Some(_) = self.bus.take_halt_request() {
            self.cpu.halt_state = HaltState::Halted;
        }

        self.handle_events();

        let (ie, iflag, dispstat, ime) = (
            self.bus.interrupt_enable,
            self.bus.interrupt_flag,
            self.bus.ppu.dispstat,
            self.bus.ime_enabled(),
        );

        self.bus.dump(format_args!("interrupt_enable={:016b} interrupt_flag={:016b} dispstat={:016b} interrupt_master_enable={}", ie, iflag, dispstat, ime));

        // technically serviced by the time it shows on ui + plus will just be a flash on ui when enabled but looks cool
        self.bus.copy_interrupt_info();
        self.check_interrupts();

        for hit in &mut self.bus.watchpoint_hits[hits_before..] {
            hit.pc = pc;
        }
    }

    fn handle_events(&mut self) {
        while let Some((deadline, event)) = self.bus.scheduler.pop() {
            match event {
                Event::Hblank | Event::HblankEnd | Event::ApuSample | Event::ApuSequencer => {
                    match event {
                        Event::Hblank => {
                            let trigger = self.bus.ppu.handle_hblank(&mut self.bus.interrupt_flag);
                            self.trigger_dma(trigger);
                        }
                        Event::HblankEnd => {
                            let scanline_event =
                                self.bus.ppu.handle_hblank_end(&mut self.bus.interrupt_flag);

                            if scanline_event.vblank {
                                self.bus
                                    .keypad
                                    .poll(self.keypad, &mut self.bus.interrupt_flag);
                            }

                            if scanline_event.vblank {
                                self.trigger_dma(Some(Trigger::Vblank));
                            }

                            if (2..162).contains(&self.bus.ppu.vcount) {
                                self.trigger_dma(Some(Trigger::Vcount));
                            }
                        }
                        Event::ApuSample => {
                            self.bus.apu.advance_psg(deadline);
                            self.bus.apu.produce_sample()
                        }
                        Event::ApuSequencer => {
                            self.bus.apu.advance_psg(deadline);
                            self.bus.apu.frame_sequencer_step();
                        }
                        _ => unreachable!(),
                    }

                    self.bus.scheduler.reschedule(event, deadline);
                }
                Event::TimerOverflow(timer_id) => {
                    let overflow_mask = self.bus.timers.handle_overflow(
                        timer_id,
                        deadline,
                        &mut self.bus.scheduler,
                        &mut self.bus.interrupt_flag,
                    );

                    for timer_id in 0..2 {
                        if overflow_mask.is_set(timer_id) {
                            self.bus.pop_fifo(timer_id as u8)
                        }
                    }
                }
            }
        }
    }

    fn check_interrupts(&mut self) {
        if self.bus.pending_interrupt() != 0 {
            self.cpu.awake();
            if self.bus.ime_enabled() && self.cpu.registers.irq_enabled() {
                self.cpu.raise_irq(&mut self.bus)
            }
        }
    }

    pub fn take_frame(&mut self) -> bool {
        take(&mut self.bus.ppu.frame_ready)
    }

    pub fn take_frame_start(&mut self) -> bool {
        take(&mut self.bus.ppu.frame_start)
    }

    pub fn trigger_dma(&mut self, trigger: Option<Trigger>) {
        if let Some(trigger) = trigger {
            for channel in 0..4 {
                self.bus.run_dma(channel, Some(trigger));
            }
        }
    }

    pub fn take_watchpoint_pause(&mut self) -> bool {
        take(&mut self.bus.watchpoint_pause)
    }
}

impl Emulator for GBA {
    fn save(&mut self) -> Result<(), Error> {
        self.bus.gamepak.write_sav()?;

        Ok(())
    }

    fn set_breakpoint(&mut self, address: u32, pause: bool) -> bool {
        self.cpu.set_breakpoint(address, pause)
    }

    fn remove_breakpoint(&mut self, address: u32) {
        self.cpu.remove_breakpoint(address);
    }

    fn take_breakpoint_hit(&mut self) -> Option<u32> {
        take(&mut self.cpu.breakpoint_hit)
    }

    fn clear_all_breakpoints(&mut self) {
        self.cpu.breakpoint_queue.clear();
    }

    fn check_breakpoints(&self) -> Vec<(u32, EmulatorState)> {
        self.cpu.breakpoint_action.clone().into_iter().collect()
    }

    fn check_watchpoints(&self) -> Vec<(u32, WatchpointArgs)> {
        self.bus.watchpoint_queue.clone().into_iter().collect()
    }

    fn set_watchpoint(&mut self, address: u32, watchpoint_args: WatchpointArgs) -> bool {
        if self.bus.watchpoint_queue.contains_key(&address) {
            return false;
        }

        self.bus.watchpoint_queue.insert(address, watchpoint_args);
        true
    }

    fn clear_all_watchpoints(&mut self) {
        self.bus.watchpoint_queue.clear();
    }

    fn remove_watchpoint(&mut self, address: u32) {
        self.bus.watchpoint_queue.remove(&address);
    }

    fn take_watchpoint_hits(&mut self) -> Vec<WatchpointHit> {
        take(&mut self.bus.watchpoint_hits)
    }
}

impl Drop for GBA {
    fn drop(&mut self) {
        let _ = self.save();
    }
}

impl ScriptTarget for GBA {
    fn read_u8(&mut self, address: u32) -> u8 {
        self.bus.read_u8(address, AccessType::Lua)
    }

    fn read_u16(&mut self, address: u32) -> u16 {
        self.bus.read_u16(address, AccessType::Lua)
    }

    fn read_u32(&mut self, address: u32) -> u32 {
        self.bus.read_u32(address, AccessType::Lua)
    }

    fn write_u8(&mut self, address: u32, value: u8) {
        self.bus.write_u8(address, value, AccessType::Lua);
    }

    fn write_u16(&mut self, address: u32, value: u16) {
        self.bus.write_u16(address, value, AccessType::Lua);
    }

    fn write_u32(&mut self, address: u32, value: u32) {
        self.bus.write_u32(address, value, AccessType::Lua);
    }

    fn read_cpu_register(&self, register_name: String) -> Result<u64, CpuError> {
        match register_name.as_str() {
            "cpsr" => Ok(self.cpu.registers.cpsr as u64),
            "r0" => Ok(self.cpu.registers.r[0] as u64),
            "r1" => Ok(self.cpu.registers.r[1] as u64),
            "r2" => Ok(self.cpu.registers.r[2] as u64),
            "r3" => Ok(self.cpu.registers.r[3] as u64),
            "r4" => Ok(self.cpu.registers.r[4] as u64),
            "r5" => Ok(self.cpu.registers.r[5] as u64),
            "r6" => Ok(self.cpu.registers.r[6] as u64),
            "r7" => Ok(self.cpu.registers.r[7] as u64),
            "r8" => Ok(self.cpu.registers.r[8] as u64),
            "r9" => Ok(self.cpu.registers.r[9] as u64),
            "r10" => Ok(self.cpu.registers.r[10] as u64),
            "r11" => Ok(self.cpu.registers.r[11] as u64),
            "r12" => Ok(self.cpu.registers.r[12] as u64),
            "r13" | "sp" => Ok(self.cpu.registers.r[13] as u64),
            "r14" | "lr" => Ok(self.cpu.registers.r[14] as u64),
            "r15" | "pc" => Ok(self.cpu.next_executing_address() as u64),
            _ => Err(CpuError::UnknownRegister),
        }
    }

    fn write_cpu_register(&mut self, register_name: String, value: u32) -> Result<(), CpuError> {
        match register_name.as_str() {
            "cpsr" => self.cpu.set_cpsr(value),
            "r0" => self.cpu.registers.r[0] = value,
            "r1" => self.cpu.registers.r[1] = value,
            "r2" => self.cpu.registers.r[2] = value,
            "r3" => self.cpu.registers.r[3] = value,
            "r4" => self.cpu.registers.r[4] = value,
            "r5" => self.cpu.registers.r[5] = value,
            "r6" => self.cpu.registers.r[6] = value,
            "r7" => self.cpu.registers.r[7] = value,
            "r8" => self.cpu.registers.r[8] = value,
            "r9" => self.cpu.registers.r[9] = value,
            "r10" => self.cpu.registers.r[10] = value,
            "r11" => self.cpu.registers.r[11] = value,
            "r12" => self.cpu.registers.r[12] = value,
            "r13" | "sp" => self.cpu.registers.r[13] = value,
            "r14" | "lr" => self.cpu.registers.r[14] = value,
            "r15" | "pc" => self.cpu.branch_to(value),
            _ => return Err(CpuError::UnknownRegister),
        }

        Ok(())
    }

    fn to_rgb(&self, value: u32) -> [u8; 3] {
        to_rbg_single(value, self.bus.ppu.frontend.pixel_format)
    }

    fn cpu_register_names(&self) -> &'static [&'static str] {
        &[
            "r0", "r1", "r2", "r3", "r4", "r5", "r6", "r7", "r8", "r9", "r10", "r11", "r12", "r13",
            "sp", "r14", "lr", "r15", "pc", "cpsr",
        ]
    }

    fn memory_domain_names(&self) -> &'static [&'static str] {
        &["rom", "oam", "palette", "vram", "iwram", "ewram"]
    }

    fn read_domain(&self, domain: &str, offset: usize) -> Result<u8, DomainError> {
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
    }

    fn write_domain(&mut self, domain: &str, offset: usize, value: u8) -> Result<(), DomainError> {
        let region: &mut [u8] = match domain {
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
        }
    }

    fn address_to_domain(&self, address: u32) -> Option<(&'static str, usize)> {
        let (domain, base) = match address >> 24 {
            0x02 => ("ewram", 0x02000000),
            0x03 => ("iwram", 0x03000000),
            0x05 => ("palette", 0x05000000),
            0x06 => ("vram", 0x06000000),
            0x07 => ("oam", 0x07000000),
            0x08..=0x0D => ("rom", 0x08000000),
            _ => return None,
        };

        Some((domain, base))
    }
}

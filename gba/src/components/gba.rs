// https://archive.org/details/NintendoGbaManualV1.1/page/n59/mode/1up
use crate::components::{
    bus::Bus,
    cpu::{Arm7tdmi, HaltState},
    dma::Trigger,
    gamepak::{BackupChip, GamePak},
    scheduler::Event,
    utils::BitOps,
};
use shared::utils::Emulator;

pub struct GBA {
    pub bus: Bus,
    pub cpu: Arm7tdmi,
    pub keypad: [bool; 10],
}

impl GBA {
    pub fn boot(gamepak: GamePak) -> Self {
        let mut bus = Bus::new(gamepak);
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

        if self.cpu.is_halted() {
            bus.scheduler.skip_to_next_event();
        } else {
            self.cpu.step(bus)
        };

        if let Some(_) = self.bus.take_halt_request() {
            self.cpu.halt_state = HaltState::Halted;
        }

        self.handle_events();

        self.check_interrupts();
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
                            let trigger =
                                self.bus.ppu.handle_hblank_end(&mut self.bus.interrupt_flag);
                            if let Some(trigger) = trigger {
                                if trigger == Trigger::Vblank || trigger == Trigger::Vcount(160) {
                                    self.bus
                                        .keypad
                                        .poll(self.keypad, &mut self.bus.interrupt_flag);
                                }
                            }

                            self.trigger_dma(trigger);
                        }
                        Event::ApuSample => {}
                        Event::ApuSequencer => {}
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
                            self.bus.sound_fifo(timer_id as u8);
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

    pub fn backup_updated(&mut self) -> bool {
        let updated = match &mut self.bus.gamepak.backup_chip {
            BackupChip::Eeprom(eeprom) => &mut eeprom.updated,
            BackupChip::Sram(sram) => &mut sram.updated,
            BackupChip::Flash(flash) => &mut flash.updated,
            BackupChip::None => &mut false,
        };

        let was_updated = *updated;
        *updated = false;

        was_updated
    }

    pub fn take_frame(&mut self) -> bool {
        std::mem::take(&mut self.bus.ppu.frame_ready)
    }

    pub fn trigger_dma(&mut self, trigger: Option<Trigger>) {
        if let Some(trigger) = trigger {
            for channel in 0..4 {
                self.bus.run_dma(channel, Some(trigger));
            }
        }
    }
}

impl Emulator for GBA {
    fn save(&self) -> Result<(), std::io::Error> {
        self.bus.gamepak.write_sav()?;

        Ok(())
    }
}

impl Drop for GBA {
    fn drop(&mut self) {
        let _ = self.save();
    }
}

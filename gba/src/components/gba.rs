// https://archive.org/details/NintendoGbaManualV1.1/page/n59/mode/1up
use crate::components::{
    bus::Bus,
    cpu::{Arm7tdmi, HaltState},
    gamepak::{BackupChip, GamePak},
    scheduler::Event,
};

pub struct GBA {
    pub bus: Bus,
    pub cpu: Arm7tdmi,
}

impl GBA {
    pub fn boot(gamepak: GamePak) -> Self {
        let mut bus = Bus::new(gamepak);
        bus.skip_boot();
        bus.scheduler.initialize_events();

        let mut cpu = Arm7tdmi::new();
        cpu.skip_boot();

        Self { bus, cpu }
    }

    pub fn run(&mut self, keypad: [bool; 10]) {
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

        self.bus.keypad.poll(keypad, &mut self.bus.interrupt_flag);
    }

    fn handle_events(&mut self) {
        while let Some((deadline, event)) = self.bus.scheduler.pop() {
            match event {
                Event::Hblank | Event::HblankEnd | Event::ApuSample | Event::ApuSequencer => {
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
                        if overflow_mask & (1 << timer_id) != 0 {
                            self.bus.sound_fifo(timer_id);
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

    pub fn backup_save(&self) -> Result<(), std::io::Error> {
        self.bus.gamepak.write_sav()?;

        Ok(())
    }

    pub fn backup_ram_updated(&mut self) -> bool {
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
}

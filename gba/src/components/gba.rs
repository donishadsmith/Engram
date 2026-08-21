// https://archive.org/details/NintendoGbaManualV1.1/page/n59/mode/1up
use crate::components::{
    bus::Bus,
    cpu::{Arm7tdmi, HaltState},
    gamepak::GamePak,
    scheduler::Event,
};

pub struct GBA {
    pub bus: Bus,
    pub cpu: Arm7tdmi,
}

impl GBA {
    pub fn boot(gamepak: GamePak) -> Self {
        Self {
            bus: Bus::new(gamepak),
            cpu: Arm7tdmi::new(),
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
                    self.bus.scheduler.reschedule(event, deadline);
                }
                Event::TimerOverflow(timer_id) => {
                    let overflow_mask = self.bus.timers.handle_overflow(
                        timer_id,
                        deadline,
                        &mut self.bus.scheduler,
                        &mut self.bus.interrupt_flag,
                    );

                    for timer in 0..2 {
                        if overflow_mask & (1 << timer) != 0 {
                            self.bus.apu.on_timer_overflow(timer);
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
}

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
            bus.scheduler.go_to_next_event();
        } else {
            self.cpu.step(bus)
        };

        if let Some(_) = self.bus.take_halt_request() {
            self.cpu.halt_state = HaltState::Halted;
        }

        while let Some(event) = self.bus.scheduler.pop() {
            self.handle_event(event);
        }

        self.check_interrupts();
    }

    fn handle_event(&mut self, event: Event) {
        match event {
            Event::Hblank => {}
            Event::Vblank => {}
            Event::TimerOverflow(n) => {}
            Event::ApuSample => {}
        }
    }

    fn check_interrupts(&mut self) {
        if self.bus.pending_interrupt() != 0 {
            self.cpu.awake();
            if self.bus.ime_enabled() && self.cpu.registers.irq_enabled() {
                self.cpu.raise_irq()
            }
        }
    }
}

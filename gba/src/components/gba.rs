use crate::components::{
    bus::Bus,
    cpu::{Arm7tdmi, CpuRequest},
    gamepak::GamePak,
    scheduler::Event,
};

pub struct GBA {
    bus: Bus,
    cpu: Arm7tdmi,
}

impl GBA {
    pub fn start(gamepak: GamePak) -> Self {
        Self {
            bus: Bus::new(gamepak),
            cpu: Arm7tdmi::new(),
        }
    }

    pub fn advance(&mut self) {
        let bus = &mut self.bus;

        if self.cpu.is_halted() {
            bus.scheduler.go_to_next_event();
        } else if let Some(cpu_request) = self.cpu.step(bus) {
            match cpu_request {
                CpuRequest::Swi(function) => self.handle_swi(function),
            }
        };

        while let Some(event) = self.bus.scheduler.pop() {
            self.handle_event(event);
        }
    }

    fn handle_event(&mut self, event: Event) {
        match event {
            Event::Hblank => {}
            Event::Vblank => {}
            Event::TimerOverflow(n) => {}
            Event::ApuSample => {}
        }

        self.check_interrupts();
    }

    fn check_interrupts(&mut self) {
        let pending = self.bus.pending_interrupt();
        if pending != 0 {
            self.cpu.awake(pending);
            if self.bus.ime_enabled() && self.cpu.registers.irqs_enabled() {
                self.cpu.raise_irq(&mut self.bus)
            }
        }
    }

    fn handle_swi(&mut self, function: u32) {
        // https://github.com/mgba-emu/mgba/blob/b54fc45b4ddab1c493122f6644f6d290dce319ce/src/gba/hle-bios.s#L69
        match function {
            0x00 => {} // SoftReset
            0x02 => {} //
            _ => {
                eprintln!(
                    "The following SWI function not implemented: {:#04X}",
                    function
                );
            }
        }
    }
}

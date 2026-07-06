use crate::components::{cpu::core::CPU, memory::bus::Bus, rom::cartridge::Cartridge};

const MACHINE_CYCLES_PER_FRAME: u16 = 17556;

// http://marc.rawer.de/Gameboy/Docs/GBCPUman.pdf
// https://gekkio.fi/files/gb-docs/gbctr.pdf
// https://www.zilog.com/docs/z80/um0080.pdf

pub struct GameBoy {
    cpu: CPU<Bus>,
}

impl GameBoy {
    pub fn boot(cartridge: Cartridge) -> Self {
        let checksum = cartridge.header.checksum;
        let cgb_flag = cartridge.header.cgb_flag;
        let bus = Bus::new(cartridge);

        Self {
            cpu: CPU::<Bus>::start(cgb_flag, checksum, bus),
        }
    }

    pub fn run(&mut self) {
        let mut remaining_cycles = MACHINE_CYCLES_PER_FRAME;

        while remaining_cycles > 0 {
            let machine_cycles = self.cpu.cycle() as u16;

            self.cpu
                .bus
                .memory
                .ppu
                .tick(machine_cycles * 4, &mut self.cpu.bus.memory.interrupt_flag);
            remaining_cycles = remaining_cycles.saturating_sub(machine_cycles);
        }
    }
}

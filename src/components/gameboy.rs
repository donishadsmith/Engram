use crate::components::{cpu::core::CPU, memory::bus::Bus, rom::cartridge::Cartridge};

const T_CYCLES_PER_FRAME_DOUBLE: u32 = 140448;

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
        let mut remaining_cycles = T_CYCLES_PER_FRAME_DOUBLE;

        while remaining_cycles > 0 {
            let mut cpu_t_cycles = (self.cpu.cycle() as u32) * 4;

            if self.cpu.bus.memory.key_register & 0x80 == 0 {
                cpu_t_cycles *= 2;
            }

            let ppu_t_cycles = cpu_t_cycles / 2;

            self.cpu
                .bus
                .memory
                .ppu
                .tick(ppu_t_cycles, &mut self.cpu.bus.memory.interrupt_flag);
            remaining_cycles = remaining_cycles.saturating_sub(cpu_t_cycles);
        }
    }
}

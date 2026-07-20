use crate::components::{cpu::CPU, memory::bus::Bus, rom::cartridge::Cartridge};

const T_CYCLES_PER_FRAME_DOUBLE: u32 = 140448;

// http://marc.rawer.de/Gameboy/Docs/GBCPUman.pdf
// https://gekkio.fi/files/gb-docs/gbctr.pdf
// https://www.zilog.com/docs/z80/um0080.pdf

pub struct GameBoy {
    pub cpu: CPU<Bus>,
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

    pub fn run(&mut self, pressed_key: [bool; 8], cycles_per_sample: u32) {
        let mut remaining_cycles = T_CYCLES_PER_FRAME_DOUBLE;

        while remaining_cycles > 0 {
            let machine_cycles = self.cpu.cycle() as u32;
            let timer_t_cycles = machine_cycles * 4;

            for _ in 0..machine_cycles {
                self.cpu.bus.oam_dma_step();
            }

            let double_speed = self.cpu.bus.memory.key_register & 0x80 != 0;
            let cpu_t_cycles = if double_speed {
                timer_t_cycles
            } else {
                timer_t_cycles * 2
            };

            let ppu_t_cycles = cpu_t_cycles / 2;

            self.cpu
                .bus
                .memory
                .ppu
                .tick(ppu_t_cycles, &mut self.cpu.bus.memory.interrupt_flag);

            self.cpu.bus.hblank_dma_step();

            self.cpu.bus.memory.timer.tick(
                timer_t_cycles,
                &mut self.cpu.bus.memory.interrupt_flag,
                double_speed,
            );

            let increase_apu_div_counter = self.cpu.bus.memory.timer.increase_div_apu_counter;
            self.cpu
                .bus
                .memory
                .apu
                .tick(ppu_t_cycles, cycles_per_sample, increase_apu_div_counter);

            remaining_cycles = remaining_cycles.saturating_sub(cpu_t_cycles);
        }

        self.cpu
            .bus
            .memory
            .joypad
            .poll(pressed_key, &mut self.cpu.bus.memory.interrupt_flag);

        self.cpu.bus.memory.cartridge.mbc.tick();
    }

    pub fn battery_save(&self) -> Result<(), std::io::Error> {
        self.cpu.bus.memory.cartridge.write_sav()?;

        Ok(())
    }

    pub fn ram_changed(&mut self) -> bool {
        let updated_ram = self.cpu.bus.memory.cartridge.mbc.ram_changed().clone();
        *self.cpu.bus.memory.cartridge.mbc.ram_changed() = false;

        updated_ram
    }

    pub fn take_frame(&mut self) -> bool {
        std::mem::take(&mut self.cpu.bus.memory.ppu.frame_ready)
    }
}

impl Drop for GameBoy {
    fn drop(&mut self) {
        let _ = self.battery_save();
    }
}

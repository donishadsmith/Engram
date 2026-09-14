// Very clear instructions: https://github.com/Ashiepaws/GBEDG/blob/master/timers/index.md

use crate::components::cpu::interrupts::InterruptMode;
use shared::traits::BitOps;

pub struct Timer {
    div: u16,
    tima: u8,
    tma: u8,
    tac: u8,
    pub increase_div_apu_counter: bool,
}

impl Timer {
    pub fn new() -> Self {
        Self {
            div: 0,
            tima: 0,
            tma: 0,
            tac: 0,
            increase_div_apu_counter: false,
        }
    }

    fn target_div_bit(&self) -> usize {
        match self.tac.get_bit_range(0..2) {
            0b00 => 9,
            0b01 => 3,
            0b10 => 5,
            _ => 7,
        }
    }

    pub fn tick(&mut self, t_cycles: u32, interrupt_flag: &mut u8, double_speed: bool) {
        let timer_enabled = self.tac.is_set(2);
        let target_bit = self.target_div_bit();
        let apu_bit = if double_speed { 13 } else { 12 };

        for _ in 0..t_cycles {
            let previous_div = self.div;
            self.div = self.div.wrapping_add(1);

            // Tick on falling edge of the div bit when the timer is enabled
            if timer_enabled && previous_div.is_set(target_bit) && self.div.is_clear(target_bit) {
                let (result, overflowed) = self.tima.overflowing_add(1);

                if overflowed {
                    // When Tima overflows it is set to tma and an interrupt is requested
                    self.tima = self.tma;
                    *interrupt_flag |= InterruptMode::Timer.mask();
                } else {
                    self.tima = result;
                }
            }

            self.increase_div_apu_counter =
                previous_div.is_set(apu_bit) && self.div.is_clear(apu_bit);
        }
    }

    pub fn read_register(&self, address: u16) -> u8 {
        match address {
            0xFF04 => (self.div >> 8) as u8,
            0xFF05 => self.tima,
            0xFF06 => self.tma,
            0xFF07 => self.tac,
            _ => 0xFF,
        }
    }

    pub fn write_register(&mut self, address: u16, value: u8) {
        match address {
            0xFF04 => self.div = 0,
            0xFF05 => self.tima = value,
            0xFF06 => self.tma = value,
            0xFF07 => self.tac = value.get_bit_range(0..3),
            _ => {}
        }
    }
}

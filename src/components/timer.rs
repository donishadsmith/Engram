// Very clear instructions: https://github.com/Ashiepaws/GBEDG/blob/master/timers/index.md

use crate::components::cpu::core::InterruptMode;

pub struct Timer {
    pub div: u16,
    pub tima: u8,
    pub tma: u8,
    pub tac: u8,
}

impl Timer {
    pub fn new() -> Self {
        Self {
            div: 0,
            tima: 0,
            tma: 0,
            tac: 0,
        }
    }

    fn target_div_bit(&self) -> u16 {
        match self.tac & 0x03 {
            0b00 => 1 << 9,
            0b01 => 1 << 3,
            0b10 => 1 << 5,
            _ => 1 << 7,
        }
    }

    pub fn tick(&mut self, t_cycles: u32, interrupt_flag: &mut u8) {
        let timer_enabled = self.tac & 0x04 != 0;
        let target_bit = self.target_div_bit();

        for _ in 0..t_cycles {
            let previous_div = self.div;
            self.div = self.div.wrapping_add(1);

            // Falling edge occurs if the timer enabled, the previous div and target bit is 1
            // and current div and target bit is 0
            if timer_enabled && (previous_div & target_bit != 0) && (self.div & target_bit == 0) {
                let (result, overflowed) = self.tima.overflowing_add(1);

                if overflowed {
                    // When Tima overflows it is set to tma and an interrupt is requested
                    self.tima = self.tma;
                    *interrupt_flag |= InterruptMode::Timer.mask();
                } else {
                    self.tima = result;
                }
            }
        }
    }
}

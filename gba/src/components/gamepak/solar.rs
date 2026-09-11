// https://problemkaputt.de/gbatek-gba-cart-solar-sensor.htm
use crate::components::utils::BitOps;

// Shamelessly take mgba's GBA_LUX_LEVELS array
// https://github.com/mgba-emu/mgba/blob/master/src/gba/cart/gpio.c#L17
const SOLAR_LEVELS: [u8; 10] = [5, 11, 18, 27, 42, 62, 84, 109, 139, 183];

pub struct SolarSensor {
    pub level: u8,
    sample: u8,
    counter: u8,
    clock: bool,
}

impl SolarSensor {
    pub fn new() -> Self {
        Self {
            level: 0,
            sample: 0xFF,
            counter: 0,
            clock: true,
        }
    }

    pub fn set_pins(&mut self, pins: u8) {
        let clock_previously_low = !self.clock;
        self.clock = pins.is_set(0);

        if pins.is_set(2) {
            return;
        }

        if pins.is_set(1) {
            self.counter = 0;
            self.latch_sample();
        }

        if clock_previously_low && self.clock {
            self.counter = self.counter.saturating_add(1);
        }
    }

    pub fn flag(&self) -> bool {
        self.counter >= self.sample
    }

    pub fn set_level(&mut self, solar_level: u8) {
        self.level = solar_level
    }

    pub fn latch_sample(&mut self) {
        let mut value = 0x16;

        if self.level > 0 {
            value += SOLAR_LEVELS[self.level as usize - 1];
        }

        self.sample = 0xFF - value;
    }
}

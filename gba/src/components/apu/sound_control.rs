use crate::components::utils::BitOps;

#[derive(Clone, Copy)]
#[repr(u8)]
pub enum EnvelopeDirection {
    Decrement = 0b00000000,
    Increment = 0b00001000,
}

impl EnvelopeDirection {
    pub fn from_register(value: u16) -> EnvelopeDirection {
        match value.get_bit(11) == 0 {
            true => EnvelopeDirection::Decrement,
            false => EnvelopeDirection::Increment,
        }
    }

    pub fn update_volume(self, current_volume: &mut u16) {
        match self {
            EnvelopeDirection::Decrement if *current_volume > 0 => *current_volume -= 1,
            EnvelopeDirection::Increment if *current_volume < 15 => *current_volume += 1,
            _ => {}
        }
    }
}

pub struct Envelope {
    pub initial_volume: u16,
    pub direction: EnvelopeDirection,
    pub current_volume: u16,
    pub step_time: u16,
    pub timer: u16,
}

impl Envelope {
    pub fn new() -> Self {
        Self {
            initial_volume: 0,
            direction: EnvelopeDirection::Decrement,
            current_volume: 0,
            step_time: 0,
            timer: 0,
        }
    }

    pub fn read(&self) -> u16 {
        (self.initial_volume << 12) | (self.direction as u16) << 8 | self.step_time << 8
    }

    pub fn set(&mut self, value: u16) {
        self.step_time = value.get_bit_range(8..11);
        self.direction = EnvelopeDirection::from_register(value);
        self.initial_volume = value.get_bit_range(12..16);
    }

    pub fn tick(&mut self) {
        if self.step_time == 0 {
            return;
        }

        if self.timer > 0 {
            self.timer -= 1;
        }

        if self.timer == 0 {
            self.timer = self.step_time;
            self.direction.update_volume(&mut self.current_volume);
        }
    }
}

pub struct Length {
    pub timer: u16,
    pub enabled: bool,
}

impl Length {
    pub fn new() -> Self {
        Self {
            timer: 0,
            enabled: false,
        }
    }

    pub fn tick(&mut self) -> bool {
        if !(self.enabled && self.timer > 0) {
            return false;
        }

        self.timer -= 1;
        self.timer == 0
    }

    pub fn read(&self) -> u16 {
        (self.enabled as u16) << 14
    }

    pub fn set_timer(&mut self, value: u16) {
        self.timer = 64 - value.get_bit_range(0..6);
    }
}

#[derive(Clone, Copy)]
#[repr(u8)]
pub enum EnvelopeDirection {
    Decrement = 0b00000000,
    Increment = 0b00001000,
}

impl EnvelopeDirection {
    pub fn from_register(value: u8) -> EnvelopeDirection {
        match (value >> 3) & 0x01 {
            0 => EnvelopeDirection::Decrement,
            1 => EnvelopeDirection::Increment,
            _ => unreachable!(),
        }
    }

    pub fn update_volume(self, current_volume: &mut u8) {
        match self {
            EnvelopeDirection::Decrement if *current_volume > 0 => *current_volume -= 1,
            EnvelopeDirection::Increment if *current_volume < 15 => *current_volume += 1,
            _ => {}
        }
    }
}

pub struct Envelope {
    pub initial_volume: u8,
    pub direction: EnvelopeDirection,
    pub current_volume: u8,
    pub period: u8,
    pub timer: u8,
}

impl Envelope {
    pub fn new() -> Self {
        Self {
            initial_volume: 0,
            direction: EnvelopeDirection::Decrement,
            current_volume: 0,
            period: 0,
            timer: 0,
        }
    }

    pub fn read(&self) -> u8 {
        (self.initial_volume << 4) | self.direction as u8 | self.period
    }

    pub fn set(&mut self, value: u8) {
        self.initial_volume = value >> 4;
        self.direction = EnvelopeDirection::from_register(value);
        self.period = value & 0x07;
    }

    pub fn tick(&mut self, frame_sequencer_step_envelope: bool) {
        if !frame_sequencer_step_envelope || self.period == 0 {
            return;
        }

        if self.timer > 0 {
            self.timer -= 1;
        }

        if self.timer == 0 {
            self.timer = self.period;
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

    pub fn tick(&mut self, frame_sequencer_step_length: bool) {
        if !frame_sequencer_step_length {
            return;
        }

        if !(self.enabled && self.timer > 0) {
            return;
        }

        self.timer -= 1;
        if self.timer == 0 {
            self.enabled = false
        }
    }

    pub fn read(&self) -> u8 {
        if self.enabled { 0xFF } else { 0xBF }
    }

    pub fn write(&mut self, value: u8) {
        self.timer = 64 - (value & 0x3F) as u16;
    }
}

#[derive(Clone, Copy)]
#[repr(u8)]
enum DutyCycle {
    Duty12 = 0b00000000,
    Duty25 = 0b01000000,
    Duty50 = 0b10000000,
    Duty75 = 0b11000000,
}

impl DutyCycle {
    fn from_register(value: u8) -> DutyCycle {
        match value >> 6 {
            0b00 => DutyCycle::Duty12,
            0b01 => DutyCycle::Duty25,
            0b10 => DutyCycle::Duty50,
            0b11 => DutyCycle::Duty75,
            _ => unreachable!(),
        }
    }

    fn multiplier(self, current_phase: u8) -> u8 {
        let waveform = match self {
            DutyCycle::Duty12 => [0, 0, 0, 0, 0, 0, 0, 1],
            DutyCycle::Duty25 => [1, 0, 0, 0, 0, 0, 0, 1],
            DutyCycle::Duty50 => [1, 0, 0, 0, 0, 1, 1, 1],
            DutyCycle::Duty75 => [0, 1, 1, 1, 1, 1, 1, 0],
        };

        waveform[current_phase as usize]
    }
}

#[derive(Clone, Copy)]
#[repr(u8)]
enum EnvelopeDirection {
    Decrement = 0b00000000,
    Increment = 0b00001000,
}

impl EnvelopeDirection {
    fn from_register(value: u8) -> EnvelopeDirection {
        match (value >> 3) & 0x01 {
            0 => EnvelopeDirection::Decrement,
            1 => EnvelopeDirection::Increment,
            _ => unreachable!(),
        }
    }

    fn update_volume(self, current_volume: &mut u8) {
        match self {
            EnvelopeDirection::Decrement if *current_volume > 0 => *current_volume -= 1,
            EnvelopeDirection::Increment if *current_volume < 15 => *current_volume += 1,
            _ => {}
        }
    }
}

struct Envelope {
    initial_volume: u8,
    direction: EnvelopeDirection,
    current_volume: u8,
    period: u8,
    timer: u8,
}

struct Sweep {
    pace: u8,
}

pub struct PulseChannel {
    enabled: bool,
    duty: DutyCycle,
    duty_position: u8,
    frequency_timer: u16,
    length_timer: u8,
    length_enabled: bool,
    frequency_period: u8,
    envelope: Envelope,
    sweep: Option<Sweep>,
}

impl PulseChannel {
    pub fn read_nrx1(&self) -> u8 {
        (self.duty as u8) | 0x3F
    }

    pub fn write_nrx1(&mut self, value: u8) {
        self.length_timer = 64 - (value & 0x3F);
        self.duty = DutyCycle::from_register(value);
    }

    pub fn read_nrx2(&self) -> u8 {
        let mut value = self.envelope.initial_volume << 4;
        value |= self.envelope.direction as u8;
        value |= self.envelope.period;

        value
    }

    pub fn write_nrx2(&mut self, value: u8) {
        self.envelope.initial_volume = value >> 4;
        self.envelope.direction = EnvelopeDirection::from_register(value);
        self.envelope.period = value & 0x07;

        if value & 0xF8 == 0 {
            self.enabled = false;
        }
    }
}

// https://gbdev.io/pandocs/Power_Up_Sequence.html

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

impl Envelope {
    fn new() -> Self {
        Self {
            initial_volume: 0,
            direction: EnvelopeDirection::Decrement,
            current_volume: 0,
            period: 0,
            timer: 0,
        }
    }
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
    frequency_period: u16,
    envelope: Envelope,
    sweep: Option<Sweep>,
}

impl PulseChannel {
    fn base() -> Self {
        Self {
            enabled: false,
            duty: DutyCycle::Duty12,
            duty_position: 0,
            frequency_timer: 0,
            length_timer: 0,
            length_enabled: false,
            frequency_period: 0,
            envelope: Envelope::new(),
            sweep: None,
        }
    }

    pub fn new_channel1() -> Self {
        let mut channel = Self::base();
        channel.sweep = Some(Sweep { pace: 0 });
        channel.write_nrx1(0xBF);
        channel.write_nrx2(0xF3);

        channel
    }

    pub fn new_channel2() -> Self {
        let mut channel = Self::base();
        channel.write_nrx1(0x3F);
        channel.write_nrx2(0x00);

        channel
    }

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

    pub fn read_nrx3(&self) -> u8 {
        0xFF
    }

    pub fn write_nrx3(&mut self, value: u8) {
        self.frequency_period = (self.frequency_period & 0x0700) | value as u16;
    }

    pub fn read_nrx4(&self) -> u8 {
        if self.length_enabled { 0xFF } else { 0xBF }
    }

    pub fn write_nrx4(&mut self, value: u8) {
        self.frequency_period = (self.frequency_period & 0x00FF) | (((value & 0x07) as u16) << 8);
        self.length_enabled = (value & 0x40) != 0;

        if (value >> 7) & 0x01 == 1 {
            self.enabled = self.dac_enabled();

            if self.length_timer == 0 {
                self.length_timer = 64;
            }

            self.frequency_timer = (2048 - self.frequency_period) * 4;
            self.envelope.timer = self.envelope.period;
            self.envelope.current_volume = self.envelope.initial_volume;
        }
    }

    fn dac_enabled(&self) -> bool {
        self.envelope.initial_volume != 0
            || matches!(self.envelope.direction, EnvelopeDirection::Increment)
    }
}

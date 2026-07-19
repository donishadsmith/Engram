// https://gbdev.io/pandocs/Power_Up_Sequence.html
// https://www.reddit.com/r/EmuDev/comments/5gkwi5/gb_apu_sound_emulation/
// https://gbdev.gg8.se/wiki/articles/Gameboy_sound_hardware
// https://gbdev.gg8.se/wiki/articles/Sound_Controller#FF10_-_NR10_-_Channel_1_Sweep_register_.28R.2FW.29

use crate::components::apu::sound_control::{Envelope, EnvelopeDirection, Length};

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
enum SweepDirection {
    Addition = 0,
    Subtraction = 1,
}

impl SweepDirection {
    fn from_register(value: u8) -> SweepDirection {
        match (value >> 3) & 0x01 {
            0 => SweepDirection::Addition,
            1 => SweepDirection::Subtraction,
            _ => unreachable!(),
        }
    }
}

struct Sweep {
    pace: u8,
    shift: u8,
    direction: SweepDirection,
    timer: u8,
    shadow_frequency: u16,
    enabled: bool,
}

impl Sweep {
    fn new() -> Self {
        Self {
            pace: 0,
            shift: 0,
            direction: SweepDirection::Addition,
            timer: 0,
            shadow_frequency: 0,
            enabled: false,
        }
    }

    fn calculate_frequency(&self) -> u16 {
        let delta = self.shadow_frequency >> self.shift;
        match self.direction {
            SweepDirection::Addition => self.shadow_frequency + delta,
            SweepDirection::Subtraction => self.shadow_frequency.wrapping_sub(delta),
        }
    }
}

pub struct PulseChannel {
    pub enabled: bool,
    duty: DutyCycle,
    duty_position: u8,
    frequency_timer: u16,
    pub length: Length,
    frequency_period: u16,
    pub envelope: Envelope,
    sweep: Option<Sweep>,
}

impl PulseChannel {
    fn base() -> Self {
        Self {
            enabled: false,
            duty: DutyCycle::Duty12,
            duty_position: 0,
            frequency_timer: 0,
            length: Length::new(),
            frequency_period: 0,
            envelope: Envelope::new(),
            sweep: None,
        }
    }

    pub fn new_channel1() -> Self {
        let mut channel = Self::base();
        channel.sweep = Some(Sweep::new());
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
        self.length.write(value);
        self.duty = DutyCycle::from_register(value);
    }

    pub fn read_nrx2(&self) -> u8 {
        (self.envelope.initial_volume << 4) | self.envelope.direction as u8 | self.envelope.period
    }

    pub fn write_nrx2(&mut self, value: u8) {
        self.envelope.set(value);

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
        self.length.read()
    }

    pub fn write_nrx4(&mut self, value: u8) {
        self.frequency_period = (self.frequency_period & 0x00FF) | (((value & 0x07) as u16) << 8);
        self.length.enabled = (value & 0x40) != 0;

        if (value >> 7) & 0x01 == 1 {
            self.enabled = self.dac_enabled();

            if self.length.timer == 0 {
                self.length.timer = 64;
            }

            self.frequency_timer = (2048 - self.frequency_period) * 4;
            self.envelope.timer = self.envelope.period;
            self.envelope.current_volume = self.envelope.initial_volume;

            if let Some(sweep) = self.sweep.as_mut() {
                sweep.shadow_frequency = self.frequency_period;
                sweep.timer = if sweep.pace == 0 { 8 } else { sweep.pace };
                sweep.enabled = sweep.pace != 0 || sweep.shift != 0;

                if sweep.shift != 0 && sweep.calculate_frequency() > 2047 {
                    self.enabled = false;
                }
            }
        }
    }

    // Should only be used for channel 1 which should be set
    pub fn read_nr10(&self) -> u8 {
        let sweep = self.sweep.as_ref().unwrap();
        0x80 | (sweep.pace << 4) | ((sweep.direction as u8) << 3) | sweep.shift
    }

    // Same here
    pub fn write_nr10(&mut self, value: u8) {
        let sweep = self.sweep.as_mut().unwrap();
        sweep.pace = (value >> 4) & 0x07;
        sweep.direction = SweepDirection::from_register(value);
        sweep.shift = value & 0x07;
    }

    fn dac_enabled(&self) -> bool {
        self.envelope.initial_volume != 0
            || matches!(self.envelope.direction, EnvelopeDirection::Increment)
    }

    pub fn tick(&mut self) {
        if self.frequency_timer > 0 {
            self.frequency_timer -= 1;
        }

        if self.frequency_timer == 0 {
            self.frequency_timer = (2048 - self.frequency_period) * 4;
            self.duty_position = (self.duty_position + 1) & 0x07;
        }
    }

    pub fn tick_sweep(&mut self, frame_sequencer_step_sweep: bool) {
        if !frame_sequencer_step_sweep {
            return;
        }

        let Some(sweep) = self.sweep.as_mut() else {
            return;
        };

        if sweep.timer > 0 {
            sweep.timer -= 1;
        }

        if sweep.timer > 0 {
            return;
        }

        sweep.timer = if sweep.pace == 0 { 8 } else { sweep.pace };
        if !sweep.enabled || sweep.pace == 0 {
            return;
        }

        let new_frequency = sweep.calculate_frequency();
        if new_frequency > 2047 {
            self.enabled = false;
            return;
        }

        if sweep.shift != 0 {
            sweep.shadow_frequency = new_frequency;
            self.frequency_period = new_frequency;

            if sweep.calculate_frequency() > 2047 {
                self.enabled = false;
            }
        }
    }

    pub fn sample(&self) -> u8 {
        if self.enabled {
            self.duty.multiplier(self.duty_position) * self.envelope.current_volume
        } else {
            0
        }
    }
}

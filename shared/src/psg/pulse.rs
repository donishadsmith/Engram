// https://gbdev.io/pandocs/Power_Up_Sequence.html
// https://www.reddit.com/r/EmuDev/comments/5gkwi5/gb_apu_sound_emulation/
// https://gbdev.gg8.se/wiki/articles/Gameboy_sound_hardware
// https://gbdev.gg8.se/wiki/articles/Sound_Controller#FF10_-_NR10_-_Channel_1_Sweep_register_.28R.2FW.29
// https://gbdev.gg8.se/wiki/articles/Power_Up_Sequence?utm_source
use crate::psg::sound_control::{Envelope, Length};
use crate::traits::BitOps;

#[derive(Clone, Copy, PartialEq, Eq)]
pub enum PulseChannelId {
    Channel1,
    Channel2,
}

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
        match value.get_bit_range(6..8) {
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
        if value.is_set(3) {
            SweepDirection::Subtraction
        } else {
            SweepDirection::Addition
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
    fn base(channel_id: PulseChannelId) -> Self {
        Self {
            enabled: false,
            duty: DutyCycle::Duty12,
            duty_position: 0,
            frequency_timer: 0,
            length: Length::new(),
            frequency_period: 0,
            envelope: Envelope::new(),
            sweep: match channel_id {
                PulseChannelId::Channel1 => Some(Sweep::new()),
                PulseChannelId::Channel2 => None,
            },
        }
    }

    pub fn new_channel1() -> Self {
        Self::base(PulseChannelId::Channel1)
    }

    pub fn new_channel2() -> Self {
        Self::base(PulseChannelId::Channel2)
    }

    pub fn read_nrx0(&self) -> u8 {
        let sweep = self.sweep.as_ref().unwrap();
        0x80 | (sweep.pace << 4) | ((sweep.direction as u8) << 3) | sweep.shift
    }

    pub fn write_nrx0(&mut self, value: u8) {
        let sweep = self.sweep.as_mut().unwrap();
        sweep.pace = value.get_bit_range(4..7);
        sweep.direction = SweepDirection::from_register(value);
        sweep.shift = value.get_bit_range(0..3);
    }

    pub fn read_nrx1(&self) -> u8 {
        (self.duty as u8) | 0x3F
    }

    pub fn write_nrx1(&mut self, value: u8) {
        self.length.write(value);
        self.duty = DutyCycle::from_register(value);
    }

    pub fn read_nrx2(&self) -> u8 {
        self.envelope.read()
    }

    pub fn write_nrx2(&mut self, value: u8) {
        self.envelope.set(value);

        if value.get_bit_range(3..8) == 0 {
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
        self.frequency_period =
            self.frequency_period.get_bit_range(0..8) | ((value.get_bit_range(0..3) as u16) << 8);
        self.length.enabled = value.is_set(6);

        if value.is_set(7) {
            self.enabled = self.envelope.dac_enabled();

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

    pub fn tick(&mut self) {
        if self.frequency_timer > 0 {
            self.frequency_timer -= 1;
        }

        if self.frequency_timer == 0 {
            self.frequency_timer = (2048 - self.frequency_period) * 4;
            self.duty_position = (self.duty_position + 1).get_bit_range(0..3);
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

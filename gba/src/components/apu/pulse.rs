// https://problemkaputt.de/gbatek.htm#gbasoundchannel1tonesweep
// https://gbdev.io/pandocs/Audio_Registers.html
// https://gbdev.gg8.se/wiki/articles/Gameboy_sound_hardware
// https://gbdev.gg8.se/wiki/articles/Sound_Controller#FF10_-_NR10_-_Channel_1_Sweep_register_.28R.2FW.29

use crate::components::{
    apu::sound_control::{Envelope, EnvelopeDirection, Length},
    utils::{BitOps, GroupedRegisters},
};

#[derive(Clone, Copy)]
enum PulseChannelId {
    Channel1,
    Channel2,
}

impl PulseChannelId {
    fn base_address(self) -> u32 {
        match self {
            PulseChannelId::Channel1 => 0x4000060,
            PulseChannelId::Channel2 => 0x4000068,
        }
    }

    fn duty_register_index(self) -> usize {
        match self {
            PulseChannelId::Channel1 => 1,
            PulseChannelId::Channel2 => 0,
        }
    }
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
    fn from_register(value: u16) -> DutyCycle {
        match value.get_bit_range(6..8) {
            0 => DutyCycle::Duty12,
            1 => DutyCycle::Duty25,
            2 => DutyCycle::Duty50,
            3 => DutyCycle::Duty75,
            _ => unreachable!(),
        }
    }

    fn multiplier(self, current_phase: u16) -> u16 {
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
    Increase = 0,
    Decrease = 1,
}

impl SweepDirection {
    fn from_register(value: u16) -> SweepDirection {
        match value.is_set(3) {
            true => SweepDirection::Decrease,
            false => SweepDirection::Increase,
        }
    }
}

struct Sweep {
    pace: u16,
    shift: u16,
    direction: SweepDirection,
    timer: u16,
    shadow_frequency: u16,
    enabled: bool,
}

impl Sweep {
    fn new() -> Self {
        Self {
            pace: 0,
            shift: 0,
            direction: SweepDirection::Increase,
            timer: 0,
            shadow_frequency: 0,
            enabled: false,
        }
    }

    fn calculate_frequency(&self) -> u16 {
        let delta = self.shadow_frequency >> self.shift;
        match self.direction {
            SweepDirection::Increase => self.shadow_frequency + delta,
            SweepDirection::Decrease => self.shadow_frequency.wrapping_sub(delta),
        }
    }

    fn current_state(&self) -> u16 {
        self.pace << 4 | (self.direction as u16) << 3 | self.shift
    }

    fn update_from_register(&mut self, value: u16) {
        self.shift = value.get_bit_range(0..3);
        self.direction = SweepDirection::from_register(value);
        self.pace = value.get_bit_range(4..7)
    }
}

pub struct PulseChannel {
    pub enabled: bool,
    duty: DutyCycle,
    duty_position: u16,
    frequency_timer: u16,
    pub length: Length,
    frequency_period: u16,
    pub envelope: Envelope,
    sweep: Option<Sweep>,
    channel_id: PulseChannelId,
    pub soundcnt: GroupedRegisters<u16>,
    pub history: Vec<u8>,
    pub mute: bool,
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
            channel_id,
            soundcnt: GroupedRegisters::new(3, channel_id.base_address()),
            history: Vec::with_capacity(2048),
            mute: false,
        }
    }

    pub fn new_channel1() -> Self {
        Self::base(PulseChannelId::Channel1)
    }

    pub fn new_channel2() -> Self {
        Self::base(PulseChannelId::Channel2)
    }

    fn pitch_adjustment(&mut self) {
        self.frequency_timer = (2048 - self.frequency_period) * 16;
    }

    pub fn update_from_register(&mut self, address: u32) {
        match address {
            0x4000060 => {
                if let Some(sweep) = self.sweep.as_mut() {
                    sweep.update_from_register(self.soundcnt.from_index(0));
                }
            }
            0x4000062 | 0x4000068 => {
                let index = self.channel_id.duty_register_index();
                let value = self.soundcnt.from_index(index);
                self.length.set_timer(value);
                self.duty = DutyCycle::from_register(value);
                self.envelope.set(value);
                if value.get_bit_range(11..16) == 0 {
                    self.enabled = false;
                }
            }
            0x4000064 | 0x400006C => {
                let value = self.soundcnt.from_index(2);
                self.length.enabled = value.is_set(14);
                self.frequency_period = self.soundcnt.from_index(2).get_bit_range(0..11);
                self.soundcnt.write_u16(address, value & !0x8000);
                if value.is_set(15) {
                    self.trigger_reset_event();
                }
            }
            _ => {}
        }
    }

    pub fn read_from_register(&self, address: u32) -> u16 {
        match address {
            0x4000060 => {
                if let Some(sweep) = self.sweep.as_ref() {
                    sweep.current_state()
                } else {
                    0
                }
            }
            0x4000062 | 0x4000068 => self.envelope.read() | self.duty as u16,
            0x4000064 | 0x400006C => (self.length.enabled as u16) << 14,
            _ => 0,
        }
    }

    pub fn tick(&mut self) {
        if self.frequency_timer > 0 {
            self.frequency_timer -= 1;
        }

        if self.frequency_timer == 0 {
            self.pitch_adjustment();
            self.duty_position = (self.duty_position + 1).get_bit_range(0..3);
        }
    }

    pub fn tick_sweep(&mut self) {
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

    fn dac_enabled(&self) -> bool {
        self.envelope.initial_volume != 0
            || matches!(self.envelope.direction, EnvelopeDirection::Increment)
    }

    fn trigger_reset_event(&mut self) {
        self.enabled = self.dac_enabled();

        if self.length.timer == 0 {
            self.length.timer = 64;
        }

        self.pitch_adjustment();
        self.envelope.timer = self.envelope.step_time;
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

    pub fn get_sample(&mut self) -> u8 {
        let sample = if self.enabled {
            self.duty.multiplier(self.duty_position) * self.envelope.current_volume
        } else {
            0
        };

        self.history.push(sample as u8);

        sample as u8
    }
}

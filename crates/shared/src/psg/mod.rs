use crate::EmulatorId;
use crate::psg::{noise::NoiseChannel, pulse::PulseChannel, wave::WaveChannel};
use crate::traits::BitOps;
use blip_buf::BlipBuf;
use serde::{Deserialize, Serialize};

// added to shared painful refactor after lazily copying and pasting the psg channels from the gb to the gba
// shouldve done from the start
pub mod noise;
pub mod pulse;
pub mod sound_control;
pub mod wave;

const PSG_SCALE: f32 = (i16::MAX / 60) as f32;

#[derive(Clone, Copy, PartialEq, Eq, Deserialize, Serialize)]
pub enum PsgChannelRegister {
    Nrx0,
    Nrx1,
    Nrx2,
    Nrx3,
    Nrx4,
}

#[derive(Clone, Copy, PartialEq, Eq)]
pub enum PsgMixerRegister {
    Nr50,
    Nr51,
    Nr52,
}

#[derive(Deserialize, Serialize)]
struct SequencerStep {
    length: bool,
    sweep: bool,
    envelope: bool,
}

#[derive(Deserialize, Serialize)]
pub struct Sequencer {
    step: u8,
}

impl Sequencer {
    fn new() -> Self {
        Self { step: 0 }
    }

    fn tick(&mut self) -> SequencerStep {
        let step = self.step;
        self.step = (self.step + 1).get_bit_range(0..3);

        SequencerStep {
            length: step.is_clear(0),
            sweep: step == 0x02 || step == 0x06,
            envelope: step == 0x07,
        }
    }
}

#[derive(Deserialize, Serialize)]
pub struct PsgChannel {
    pub channel1: PulseChannel,
    pub channel2: PulseChannel,
    pub channel3: WaveChannel,
    pub channel4: NoiseChannel,
    pub sequencer: Sequencer,
}

impl PsgChannel {
    pub fn new(emulator_id: EmulatorId) -> Self {
        Self {
            channel1: PulseChannel::new_channel1(),
            channel2: PulseChannel::new_channel2(),
            channel3: WaveChannel::new(emulator_id),
            channel4: NoiseChannel::new(),
            sequencer: Sequencer::new(),
        }
    }

    pub fn tick(&mut self) {
        self.channel1.tick();
        self.channel2.tick();
        self.channel3.tick();
        self.channel4.tick();
    }

    pub fn sequencer(&mut self) {
        let step = self.sequencer.tick();

        if self.channel1.length.tick(step.length) {
            self.channel1.enabled = false;
        }
        if self.channel2.length.tick(step.length) {
            self.channel2.enabled = false;
        }
        if self.channel3.length.tick(step.length) {
            self.channel3.enabled = false;
        }
        if self.channel4.length.tick(step.length) {
            self.channel4.enabled = false;
        }

        self.channel1.envelope.tick(step.envelope);
        self.channel2.envelope.tick(step.envelope);
        self.channel4.envelope.tick(step.envelope);

        self.channel1.tick_sweep(step.sweep);
    }

    pub fn samples(&self) -> [u8; 4] {
        [
            self.channel1.sample(),
            self.channel2.sample(),
            self.channel3.sample(),
            self.channel4.sample(),
        ]
    }

    pub fn enabled(&self) -> [bool; 4] {
        [
            self.channel1.enabled,
            self.channel2.enabled,
            self.channel3.enabled,
            self.channel4.enabled,
        ]
    }
}

#[derive(Deserialize, Serialize)]
pub struct PsgMixer {
    pub nr50: u8,
    pub nr51: u8,
    pub on: bool,
    pub changed: bool,
}

impl PsgMixer {
    pub fn new() -> Self {
        Self {
            nr50: 0,
            nr51: 0,
            on: false,
            changed: true,
        }
    }

    pub fn write(&mut self, register: PsgMixerRegister, value: u8) {
        match register {
            PsgMixerRegister::Nr50 => self.nr50 = value,
            PsgMixerRegister::Nr51 => self.nr51 = value,
            PsgMixerRegister::Nr52 => self.on = value.is_set(7),
        }

        self.changed = true;
    }

    pub fn status(&self, enabled: [bool; 4]) -> u8 {
        let mut value = (self.on as u8) << 7;
        for (index, enabled) in enabled.into_iter().enumerate() {
            value |= (enabled as u8) << index;
        }

        value
    }

    pub fn mix(&self, samples: [u8; 4]) -> (f32, f32) {
        let mut left = 0.0f32;
        let mut right = 0.0f32;
        for (index, sample) in samples.into_iter().enumerate() {
            if self.nr51.is_set(4 + index) {
                left += sample as f32;
            }

            if self.nr51.is_set(index) {
                right += sample as f32;
            }
        }

        let volume_left = self.nr50.get_bit_range(4..7) as f32;
        let volume_right = self.nr50.get_bit_range(0..3) as f32;

        (
            left * (volume_left + 1.0) / 8.0,
            right * (volume_right + 1.0) / 8.0,
        )
    }

    pub fn panned_left(&self, channel: usize) -> bool {
        self.nr51.is_set(4 + channel)
    }
    pub fn panned_right(&self, channel: usize) -> bool {
        self.nr51.is_set(channel)
    }
}

pub struct PsgOutput {
    left: BlipBuf,
    right: BlipBuf,
    previous_left_sample: i32,
    previous_right_sample: i32,
    pub cycle: u32,
    sample_rate: f64,
}

impl PsgOutput {
    pub fn new(sample_rate: f64) -> Self {
        let mut left = BlipBuf::new(4096);
        let mut right = BlipBuf::new(4096);

        let _ = left.set_rates(4194304.0, sample_rate);
        let _ = right.set_rates(4194304.0, sample_rate);

        Self {
            left,
            right,
            previous_left_sample: 0,
            previous_right_sample: 0,
            cycle: 0,
            sample_rate,
        }
    }

    pub fn add_samples(&mut self, samples: (f32, f32)) {
        let left = (samples.0 * PSG_SCALE) as i32;
        let right = (samples.1 * PSG_SCALE) as i32;
        if left != self.previous_left_sample {
            let _ = self
                .left
                .add_delta(self.cycle, left - self.previous_left_sample);
            self.previous_left_sample = left;
        }

        if right != self.previous_right_sample {
            let _ = self
                .right
                .add_delta(self.cycle, right - self.previous_right_sample);
            self.previous_right_sample = right;
        }
    }

    pub fn read_samples(&mut self, sample_buffer: &mut Vec<f32>) {
        let _ = self.left.end_frame(self.cycle);
        let _ = self.right.end_frame(self.cycle);
        self.cycle = 0;

        let mut left = [0i16; 8];
        let mut right = [0i16; 8];

        let n_samples = self.left.read_samples(&mut left, false);
        self.right.read_samples(&mut right, false);

        for index in 0..n_samples {
            sample_buffer.push(left[index] as f32 / 32768.0);
            sample_buffer.push(right[index] as f32 / 32768.0);
        }
    }

    pub fn set_cycles_per_sample(&mut self, cycles_per_sample: f64) {
        let clock_rate = cycles_per_sample * self.sample_rate;
        let _ = self.left.set_rates(clock_rate, self.sample_rate);
        let _ = self.right.set_rates(clock_rate, self.sample_rate);
    }
}

impl Default for PsgOutput {
    fn default() -> Self {
        PsgOutput::new(48000.0)
    }
}

use crate::EmulatorId;
use crate::psg::{noise::NoiseChannel, pulse::PulseChannel, wave::WaveChannel};
use crate::traits::BitOps;

// added to shared painful refactor after lazily copying and pasting the psg channels from the gb to the gba
// shouldve done from the start
pub mod convolve;
pub mod noise;
pub mod pulse;
pub mod sound_control;
pub mod wave;

#[derive(Clone, Copy, PartialEq, Eq)]
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

struct FrameSequencerStep {
    length: bool,
    sweep: bool,
    envelope: bool,
}

pub struct FrameSequencer {
    step: u8,
}

impl FrameSequencer {
    fn new() -> Self {
        Self { step: 0 }
    }

    fn tick(&mut self) -> FrameSequencerStep {
        let step = self.step;
        self.step = (self.step + 1).get_bit_range(0..3);

        FrameSequencerStep {
            length: step.is_clear(0),
            sweep: step == 0x02 || step == 0x06,
            envelope: step == 0x07,
        }
    }
}

pub struct PsgChannel {
    pub channel1: PulseChannel,
    pub channel2: PulseChannel,
    pub channel3: WaveChannel,
    pub channel4: NoiseChannel,
    pub frame_sequencer: FrameSequencer,
}

impl PsgChannel {
    pub fn new(emulator_id: EmulatorId) -> Self {
        Self {
            channel1: PulseChannel::new_channel1(),
            channel2: PulseChannel::new_channel2(),
            channel3: WaveChannel::new(emulator_id),
            channel4: NoiseChannel::new(),
            frame_sequencer: FrameSequencer::new(),
        }
    }

    pub fn tick(&mut self) {
        self.channel1.tick();
        self.channel2.tick();
        self.channel3.tick();
        self.channel4.tick();
    }

    pub fn clock_sequencer(&mut self) {
        let step = self.frame_sequencer.tick();

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

pub struct PsgMixer {
    pub nr50: u8,
    pub nr51: u8,
    pub on: bool,
}

impl PsgMixer {
    pub fn new() -> Self {
        Self {
            nr50: 0,
            nr51: 0,
            on: false,
        }
    }

    pub fn write(&mut self, register: PsgMixerRegister, value: u8) {
        match register {
            PsgMixerRegister::Nr50 => self.nr50 = value,
            PsgMixerRegister::Nr51 => self.nr51 = value,
            PsgMixerRegister::Nr52 => self.on = value.is_set(7),
        }
    }

    pub fn status(&self, enabled: [bool; 4]) -> u8 {
        let mut value = (self.on as u8) << 7;
        for (index, enabled) in enabled.into_iter().enumerate() {
            value |= (enabled as u8) << index;
        }

        value
    }

    pub fn mix(&self, samples: [u8; 4]) -> (f32, f32) {
        let mut left = 0.0;
        let mut right = 0.0;
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
}

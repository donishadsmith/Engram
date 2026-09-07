// https://gbadev.net/gbadoc/audio/introduction.html
mod fifo;
pub mod global_control;
mod noise;
mod pulse;
mod sound_control;

use crate::components::{
    apu::{global_control::AudioChannel, noise::NoiseChannel, pulse::PulseChannel},
    dma::FifoChannel,
    utils::BitOps,
};
use fifo::Fifo;
use global_control::GlobalControl;

struct SequencerStep {
    length: bool,
    sweep: bool,
    envelope: bool,
}

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

pub struct APU {
    pub global_control: GlobalControl,
    pub channel1: PulseChannel,
    pub channel2: PulseChannel,
    pub channel4: NoiseChannel,
    pub fifo_a: Fifo,
    pub fifo_b: Fifo,
    pub sample_buffer: Vec<f32>,
    last_psg_update: u64,
    sequencer: Sequencer,
}

impl APU {
    pub fn new() -> Self {
        Self {
            global_control: GlobalControl::new(),
            channel1: PulseChannel::new_channel1(),
            channel2: PulseChannel::new_channel2(),
            channel4: NoiseChannel::new(),
            fifo_a: Fifo::new(FifoChannel::A),
            fifo_b: Fifo::new(FifoChannel::B),
            sample_buffer: Vec::new(),
            last_psg_update: 0,
            sequencer: Sequencer::new(),
        }
    }

    pub fn enable_channels(&mut self) {
        if self.global_control.soundcnt_x.is_set(7) {
            self.fifo_a.enabled = true;
            self.fifo_b.enabled = true;
        } else {
            self.fifo_a.enabled = false;
            self.fifo_b.enabled = false;
        }
    }

    pub fn advance_psg(&mut self, timestamp: u64) {
        let elapsed_cycles = timestamp - self.last_psg_update;

        for _ in 0..elapsed_cycles {
            self.channel1.tick();
            self.channel2.tick();
            self.channel4.tick();
        }

        self.last_psg_update = timestamp;
    }

    pub fn produce_sample(&mut self) {
        if !self.global_control.master_enabled() {
            self.sample_buffer.push(0.0);
            self.sample_buffer.push(0.0);

            return;
        }

        let fifo_a_volume = self.global_control.volume_control(AudioChannel::FifoA);
        let fifo_b_volume = self.global_control.volume_control(AudioChannel::FifoB);
        let psg_volume = self.global_control.volume_control(AudioChannel::Channel1);

        let psg1 = if self.channel1.mute {
            0.0
        } else {
            f32::from(self.channel1.get_sample()) * 8.0 * psg_volume
        };

        let psg2 = if self.channel2.mute {
            0.0
        } else {
            f32::from(self.channel2.get_sample()) * 8.0 * psg_volume
        };

        let psg4 = if self.channel4.mute {
            0.0
        } else {
            f32::from(self.channel4.get_sample()) * 8.0 * psg_volume
        };

        let fifo_a = if self.fifo_a.mute {
            0.0
        } else {
            f32::from(self.fifo_a.latched as i8)
        } * 4.0
            * fifo_a_volume;

        let fifo_b = if self.fifo_a.mute {
            0.0
        } else {
            f32::from(self.fifo_b.latched as i8)
        } * 4.0
            * fifo_b_volume;

        let mixed_left = self.global_control.panned_left(AudioChannel::FifoA, fifo_a)
            + self.global_control.panned_left(AudioChannel::FifoB, fifo_b)
            + self
                .global_control
                .panned_left(AudioChannel::Channel1, psg1)
            + self
                .global_control
                .panned_left(AudioChannel::Channel2, psg2)
            + self
                .global_control
                .panned_left(AudioChannel::Channel4, psg4);

        let mixed_right = self
            .global_control
            .panned_right(AudioChannel::FifoA, fifo_a)
            + self
                .global_control
                .panned_right(AudioChannel::FifoB, fifo_b)
            + self
                .global_control
                .panned_right(AudioChannel::Channel1, psg1)
            + self
                .global_control
                .panned_right(AudioChannel::Channel2, psg2)
            + self
                .global_control
                .panned_right(AudioChannel::Channel4, psg4);

        self.sample_buffer
            .push(mixed_left.clamp(-512.0, 511.0) / 512.0);
        self.sample_buffer
            .push(mixed_right.clamp(-512.0, 511.0) / 512.0);
    }

    pub fn frame_sequencer_step(&mut self) {
        let sequencer_step = self.sequencer.tick();

        if sequencer_step.length {
            if self.channel1.length.tick() {
                self.channel1.enabled = false;
            }

            if self.channel2.length.tick() {
                self.channel2.enabled = false;
            }

            if self.channel4.length.tick() {
                self.channel4.enabled = false;
            }
        }

        if sequencer_step.envelope {
            self.channel1.envelope.tick();
            self.channel2.envelope.tick();
            self.channel4.envelope.tick();
        }

        if sequencer_step.sweep {
            self.channel1.tick_sweep();
        }
    }

    pub fn reset_sound_registers(&mut self) {
        self.channel1 = PulseChannel::new_channel1();
        self.channel2 = PulseChannel::new_channel2();
        self.channel4 = NoiseChannel::new();
        self.global_control.reset();
        self.fifo_a.reset();
        self.fifo_b.reset();
    }
}

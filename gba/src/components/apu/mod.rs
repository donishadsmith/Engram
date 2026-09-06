// TODO: psg folder contains structs from the gb, update structs to make them approapriate
// for the gba
// https://gbadev.net/gbadoc/audio/introduction.html
mod fifo;
mod global_control;
mod pulse;
mod sound_control;

use crate::components::{apu::pulse::PulseChannel, dma::FifoChannel, utils::BitOps};
use fifo::Fifo;
use global_control::GlobalControl;
use shared::audio::LowPassFilter;

const FIR_KERNEL: [f64; 46] = [0.0; 46]; // temp

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
    pub fifo_a: Fifo,
    pub fifo_b: Fifo,
    pub sample_buffer: Vec<f32>,
    last_psg_update: u64,
    sequencer: Sequencer,
    low_pass_left: LowPassFilter,
    low_pass_right: LowPassFilter,
}

impl APU {
    pub fn new() -> Self {
        Self {
            global_control: GlobalControl::new(),
            channel1: PulseChannel::new_channel1(),
            channel2: PulseChannel::new_channel2(),
            fifo_a: Fifo::new(FifoChannel::A),
            fifo_b: Fifo::new(FifoChannel::B),
            sample_buffer: Vec::new(),
            last_psg_update: 0,
            sequencer: Sequencer::new(),
            low_pass_left: LowPassFilter::new(FIR_KERNEL),
            low_pass_right: LowPassFilter::new(FIR_KERNEL),
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
        }

        self.last_psg_update = timestamp;
    }

    pub fn produce_sample(&mut self) {
        // https://github.com/michelhe/rustboyadvance-ng/blob/master/core/src/sound/mod.rs
        let psg1 = i16::from(!self.channel1.mute as u8 * self.channel1.get_sample()) * 8;
        let psg2 = i16::from(!self.channel2.mute as u8 * self.channel2.get_sample()) * 8;
        let a = ((!self.fifo_a.mute as u8 * self.fifo_a.latched) as i8) as i16;
        let b = ((!self.fifo_b.mute as u8 * self.fifo_b.latched) as i8) as i16;
        let mixed = (a << 2) + (b << 2) + psg1 + psg2;
        let sample = mixed.clamp(-512, 511) as f32 / 512.0;

        self.sample_buffer.push(sample);
        self.sample_buffer.push(sample);
    }

    pub fn frame_sequencer_step(&mut self) {
        let sequencer_step = self.sequencer.tick();

        if sequencer_step.length {
            if self.channel1.length.tick() {
                self.channel1.enabled = false;
            }

            if self.channel2.length.tick() {
                self.channel1.enabled = false;
            }
        }

        if sequencer_step.envelope {
            self.channel1.envelope.tick();
            self.channel2.envelope.tick();
        }

        if sequencer_step.sweep {
            self.channel1.tick_sweep();
        }
    }

    pub fn reset_sound_registers(&mut self) {
        self.channel1 = PulseChannel::new_channel1();
        self.channel2 = PulseChannel::new_channel2();
        self.global_control.reset();
        self.fifo_a.reset();
        self.fifo_b.reset();
    }
}

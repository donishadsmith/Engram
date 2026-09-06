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

#[derive(Clone, Copy)]
enum Volume {
    Quarter,
    Full,
    Half,
    Prohibited,
}

impl Volume {
    fn for_dma(full: bool) -> Volume {
        match full {
            true => Volume::Full,
            false => Volume::Half,
        }
    }

    fn for_psg(value: u16) -> Volume {
        match value.get_bit_range(0..2) {
            0 => Volume::Quarter,
            1 => Volume::Half,
            2 => Volume::Full,
            _ => Volume::Prohibited,
        }
    }

    fn to_float(self) -> f32 {
        match self {
            Volume::Quarter => 0.25,
            Volume::Full => 1.0,
            Volume::Half => 0.5,
            Volume::Prohibited => 0.0,
        }
    }
}

#[derive(Clone, Copy, PartialEq, Eq)]
pub enum AudioChannel {
    Channel1 = 0,
    Channel2 = 1,
    Channel3 = 2,
    Channel4 = 3,
    FifoA = 4,
    FifoB = 5,
}

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
        let fifo_a_volume = self.volume_control(AudioChannel::FifoA);
        let fifo_b_volume = self.volume_control(AudioChannel::FifoB);
        let psg_volume = self.volume_control(AudioChannel::Channel1);
        let psg1 = i16::from(!self.channel1.mute as u8 * self.channel1.get_sample()) * 8;
        let psg2 = i16::from(!self.channel2.mute as u8 * self.channel2.get_sample()) * 8;
        let a = ((!self.fifo_a.mute as u8 * self.fifo_a.latched) as i8) as i16;
        let b = ((!self.fifo_b.mute as u8 * self.fifo_b.latched) as i8) as i16;
        let mixed = (((a << 2) as f32) * fifo_a_volume)
            + (((b << 2) as f32) * fifo_b_volume)
            + (psg1 as f32) * psg_volume
            + (psg2 as f32) * psg_volume;
        let sample = mixed.clamp(-512.0, 511.0) / 512.0;

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

    pub fn volume_control(&self, channel: AudioChannel) -> f32 {
        if channel == AudioChannel::FifoA {
            Volume::for_dma(self.global_control.soundcnt_h.is_set(2)).to_float()
        } else if channel == AudioChannel::FifoB {
            Volume::for_dma(self.global_control.soundcnt_h.is_set(3)).to_float()
        } else {
            Volume::for_psg(self.global_control.soundcnt_h.get_bit_range(0..2)).to_float()
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

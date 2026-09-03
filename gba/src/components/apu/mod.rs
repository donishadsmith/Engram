// TODO: psg folder contains structs from the gb, update structs to make them approapriate
// for the gba
// https://gbadev.net/gbadoc/audio/introduction.html
mod fifo;
mod global_control;

use crate::components::{dma::FifoChannel, utils::BitOps};
use fifo::Fifo;
use global_control::GlobalControl;
use shared::audio::LowPassFilter;

const FIR_KERNEL: [f64; 46] = [0.0; 46]; // temp

pub struct APU {
    pub global_control: GlobalControl,
    pub fifo_a: Fifo,
    pub fifo_b: Fifo,
    pub sample_buffer: Vec<f32>,
    low_pass_left: LowPassFilter,
    low_pass_right: LowPassFilter,
}

impl APU {
    pub fn new() -> Self {
        Self {
            global_control: GlobalControl::new(),
            fifo_a: Fifo::new(FifoChannel::A),
            fifo_b: Fifo::new(FifoChannel::B),
            sample_buffer: Vec::new(),
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

    pub fn produce_sample(&mut self) {
        // https://github.com/michelhe/rustboyadvance-ng/blob/master/core/src/sound/mod.rs
        let a = ((!self.fifo_a.mute as u8 * self.fifo_a.latched) as i8) as i16;
        let b = ((!self.fifo_b.mute as u8 * self.fifo_b.latched) as i8) as i16;
        let mixed = (a << 2) + (b << 2);
        let clamped = mixed.clamp(-512, 511);
        let sample = ((clamped - 512) as f32) / 512.0;

        self.sample_buffer.push(sample);
        self.sample_buffer.push(sample);
    }

    pub fn reset_sound_registers(&mut self) {
        self.global_control.reset();
        self.fifo_a.reset();
        self.fifo_b.reset();
    }
}

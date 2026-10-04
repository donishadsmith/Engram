// https://gbadev.net/gbadoc/audio/introduction.html
mod fifo;
pub mod global_control;

use std::{array::from_fn, collections::VecDeque};

use crate::components::dma::FifoChannel;
use fifo::Fifo;
use global_control::GlobalControl;
use shared::{
    EmulatorId,
    psg::{PsgChannel, PsgMixer, convolve::LowPassFilter},
    traits::{BitOps, GroupedRegisters},
};

pub struct Apu {
    pub global_control: GlobalControl,
    pub psg_mixer: PsgMixer,
    pub psg: PsgChannel,
    pub fifo_a: Fifo,
    pub fifo_b: Fifo,
    pub psg_history: [VecDeque<u8>; 4],
    pub psg_registers: GroupedRegisters<u16>,
    pub sample_buffer: Vec<f32>,
    last_psg_update: u64,
    psg_prescaler: u8,
    pub psg_mute: [bool; 4],
    low_pass_left: LowPassFilter,
    low_pass_right: LowPassFilter,
    pub debugger_active: bool,
}

impl Apu {
    pub fn new() -> Self {
        Self {
            global_control: GlobalControl::new(),
            psg: PsgChannel::new(EmulatorId::Gba),
            fifo_a: Fifo::new(FifoChannel::A),
            fifo_b: Fifo::new(FifoChannel::B),
            sample_buffer: Vec::new(),
            last_psg_update: 0,
            psg_history: from_fn(|_| VecDeque::with_capacity(2048)),
            psg_mixer: PsgMixer::new(),
            psg_registers: GroupedRegisters::new(16, 0x4000060),
            psg_mute: from_fn(|_| false),
            psg_prescaler: 0,
            low_pass_left: LowPassFilter::new(),
            low_pass_right: LowPassFilter::new(),
            debugger_active: false,
        }
    }

    // just gonna mask instead of recreate
    pub fn read_psg_halfword(&self, address: u32) -> u16 {
        let mask = match address & !1 {
            0x4000060 => 0x007F,
            0x4000062 | 0x4000068 => 0xFFC0,
            0x4000064 | 0x400006C | 0x4000074 => 0x4000,
            0x4000070 => 0x00E0,
            0x4000072 => 0xE000,
            0x4000078 => 0xFF00,
            0x400007C => 0x40FF,
            _ => 0,
        };

        self.psg_registers.read_u16(address) & mask
    }

    pub fn write_psg_halfword(&mut self, address: u32, value: u16) {
        self.psg_registers.write_u16(address, value);

        let bytes = value.to_le_bytes();
        self.write_psg_byte(address, bytes[0]);
        self.write_psg_byte(address + 1, bytes[1]);
    }

    pub fn write_psg_byte(&mut self, address: u32, value: u8) {
        match address {
            0x4000060 => self.psg.channel1.write_nrx0(value),
            0x4000062 => self.psg.channel1.write_nrx1(value),
            0x4000063 => self.psg.channel1.write_nrx2(value),
            0x4000064 => self.psg.channel1.write_nrx3(value),
            0x4000065 => self.psg.channel1.write_nrx4(value),
            0x4000068 => self.psg.channel2.write_nrx1(value),
            0x4000069 => self.psg.channel2.write_nrx2(value),
            0x400006C => self.psg.channel2.write_nrx3(value),
            0x400006D => self.psg.channel2.write_nrx4(value),
            0x4000070 => self.psg.channel3.write_nrx0(value),
            0x4000072 => self.psg.channel3.write_nrx1(value),
            0x4000073 => self.psg.channel3.write_nrx2(value),
            0x4000074 => self.psg.channel3.write_nrx3(value),
            0x4000075 => self.psg.channel3.write_nrx4(value),
            0x4000078 => self.psg.channel4.write_nrx1(value),
            0x4000079 => self.psg.channel4.write_nrx2(value),
            0x400007C => self.psg.channel4.write_nrx3(value),
            0x400007D => self.psg.channel4.write_nrx4(value),
            _ => {}
        }
    }

    pub fn enable_channels(&mut self) {
        self.fifo_a.enabled = self.psg_mixer.on;
        self.fifo_b.enabled = self.psg_mixer.on;
    }

    pub fn advance_psg(&mut self, timestamp: u64) {
        let elapsed_cycles = timestamp - self.last_psg_update;

        for _ in 0..elapsed_cycles {
            self.psg_prescaler = (self.psg_prescaler + 1).get_bit_range(0..2);
            if self.psg_prescaler == 0 {
                self.psg.tick();

                let mut psg_samples = self.psg.samples();
                for (index, muted) in self.psg_mute.iter().enumerate() {
                    if *muted {
                        psg_samples[index] = 0
                    }
                }

                let (left_sample, right_sample) = self.psg_mixer.mix(psg_samples);
                self.low_pass_left.collect_sample(left_sample as f64);
                self.low_pass_right.collect_sample(right_sample as f64);
            }
        }

        self.last_psg_update = timestamp;
    }

    pub fn produce_sample(&mut self) {
        if !self.psg_mixer.on {
            self.sample_buffer.push(0.0);
            self.sample_buffer.push(0.0);

            return;
        }

        let mut psg_samples = self.psg.samples();
        for (index, sample) in psg_samples.iter_mut().enumerate() {
            if self.debugger_active {
                if self.psg_history[index].len() == 2048 {
                    self.psg_history[index].pop_front();
                }

                self.psg_history[index].push_back(*sample);
            }
        }

        let psg_scale = 8.0 * self.global_control.psg_volume();
        let psg_left = self.low_pass_left.convolve() as f32;
        let psg_right = self.low_pass_right.convolve() as f32;
        let mut left = psg_left * psg_scale;
        let mut right = psg_right * psg_scale;

        for (fifo, channel) in [
            (&self.fifo_a, FifoChannel::A),
            (&self.fifo_b, FifoChannel::B),
        ] {
            let sample = if fifo.mute {
                0.0
            } else {
                f32::from(fifo.latched as i8)
                    * 4.0
                    * self.global_control.volume_control_fifo(channel)
            };

            if self.global_control.panned_left_fifo(channel) {
                left += sample;
            }
            if self.global_control.panned_right_fifo(channel) {
                right += sample;
            }
        }

        self.sample_buffer.push(left.clamp(-512.0, 511.0) / 512.0);
        self.sample_buffer.push(right.clamp(-512.0, 511.0) / 512.0);
    }

    // huge oopsie, i believe not gating this resulted in many vectors growing without bounds
    // now just make the sample collectors ring buffers - why didnt i do that to begin with
    pub fn debugger_status(&mut self, on: bool) {
        self.debugger_active = on;
        self.fifo_a.debugger_active = on;
        self.fifo_b.debugger_active = on;
    }

    pub fn reset_sound_registers(&mut self) {
        self.psg = PsgChannel::new(EmulatorId::Gba);
        self.psg_mixer = PsgMixer::new();
        self.psg_registers = GroupedRegisters::new(16, 0x4000060);
        self.psg_prescaler = 0;
        self.global_control = GlobalControl::new();
        self.fifo_a.reset();
        self.fifo_b.reset();
    }
}

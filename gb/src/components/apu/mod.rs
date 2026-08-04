pub mod fir;
pub mod noise;
pub mod pulse;
pub mod sound_control;
pub mod wave;

use {
    crate::components::apu::noise::NoiseChannel, fir::LowPassFilter, pulse::PulseChannel,
    wave::WaveChannel,
};

/*
    Crystal

    Pulse 1: Lead melody, sound effects (i.e., chimes, dings, thuds)
    Pulse 2: Lower melody
    Pulse 1 + 2: Cries, interestingly removing 1 channel barely changes cries
    Wave: Bass
    Noise: White noise effects (attack hits)
*/

// https://jsgroth.dev/blog/posts/gb-rewrite-apu/
// https://nightshade256.github.io/2021/03/27/gb-sound-emulation.html
// https://gbdev.gg8.se/wiki/articles/Gameboy_sound_hardware
// https://gbdev.gg8.se/wiki/articles/Power_Up_Sequence
#[derive(Clone, Copy, Eq, Hash, PartialEq)]
enum AudioChannel {
    Channel1,
    Channel2,
    Channel3,
    Channel4,
}

struct StereoVolume {
    left: u8,
    right: u8,
}

pub struct GlobalControl {
    pub nr50: u8,
    pub nr51: u8,
    pub nr52: u8,
}

impl GlobalControl {
    fn new() -> Self {
        Self {
            nr50: 0x77,
            nr51: 0xF3,
            nr52: 0xF1,
        }
    }

    fn audio_on(&self) -> bool {
        (self.nr52 & 0x80) != 0
    }

    fn panned_left(&self, channel: AudioChannel) -> bool {
        let bit = match channel {
            AudioChannel::Channel1 => 4,
            AudioChannel::Channel2 => 5,
            AudioChannel::Channel3 => 6,
            AudioChannel::Channel4 => 7,
        };

        (self.nr51 >> bit) & 1 != 0
    }

    fn panned_right(&self, channel: AudioChannel) -> bool {
        let bit = match channel {
            AudioChannel::Channel1 => 0,
            AudioChannel::Channel2 => 1,
            AudioChannel::Channel3 => 2,
            AudioChannel::Channel4 => 3,
        };

        (self.nr51 >> bit) & 1 != 0
    }

    // Just gonna ignore VIN for now
    fn volume(&self) -> StereoVolume {
        let left = (self.nr50 >> 4) & 0x07;
        let right = self.nr50 & 0x07;

        StereoVolume {
            left: if left == 0 { 1 } else { left },
            right: if right == 0 { 1 } else { right },
        }
    }
}

struct FrameSequencerStep {
    length: bool,
    sweep: bool,
    envelope: bool,
}

// tick at 512 hz when div bit for goes from 1 -> 0
pub struct FrameSequencer {
    step: u8,
}

impl FrameSequencer {
    fn new() -> Self {
        Self { step: 0 }
    }

    fn tick(&mut self) -> FrameSequencerStep {
        let step = self.step;
        self.step = (self.step + 1) & 0x07;

        FrameSequencerStep {
            length: step & 0x01 == 0,
            sweep: step == 0x02 || step == 0x06,
            envelope: step == 0x07,
        }
    }
}

pub struct APU {
    pub global_control: GlobalControl,
    pub channel1: PulseChannel,
    pub channel2: PulseChannel,
    pub channel3: WaveChannel,
    pub channel4: NoiseChannel,
    pub frame_sequencer: FrameSequencer,
    sample_counter: u32,
    pub sample_buffer: Vec<f32>,
    low_pass_left: LowPassFilter,
    low_pass_right: LowPassFilter,
}

impl APU {
    pub fn new() -> Self {
        Self {
            global_control: GlobalControl::new(),
            channel1: PulseChannel::new_channel1(),
            channel2: PulseChannel::new_channel2(),
            channel3: WaveChannel::new(),
            channel4: NoiseChannel::new(),
            frame_sequencer: FrameSequencer::new(),
            sample_counter: 0,
            sample_buffer: Vec::new(),
            low_pass_left: LowPassFilter::new(),
            low_pass_right: LowPassFilter::new(),
        }
    }

    pub fn read_wram(&self, address: u16) -> u8 {
        self.channel3.ram[(address - 0xFF30) as usize]
    }

    pub fn write_wram(&mut self, address: u16, value: u8) {
        self.channel3.ram[(address - 0xFF30) as usize] = value
    }

    pub fn tick(&mut self, t_cycles: u32, cycles_per_sample: u32, increase_apu_div_counter: bool) {
        let frame_sequencer_step = if increase_apu_div_counter {
            Some(self.frame_sequencer.tick())
        } else {
            None
        };

        if let Some(frame_sequencer_step) = frame_sequencer_step {
            self.channel1.length.tick(frame_sequencer_step.length);
            self.channel2.length.tick(frame_sequencer_step.length);
            self.channel3.length.tick(frame_sequencer_step.length);
            self.channel4.length.tick(frame_sequencer_step.length);

            self.channel1.envelope.tick(frame_sequencer_step.envelope);
            self.channel2.envelope.tick(frame_sequencer_step.envelope);
            self.channel4.envelope.tick(frame_sequencer_step.envelope);

            self.channel1.tick_sweep(frame_sequencer_step.sweep);
        }

        for _ in 0..t_cycles {
            self.channel1.tick();
            self.channel2.tick();
            self.channel3.tick();
            self.channel4.tick();

            let channel1_sample = self.channel1.sample() as f64;
            let channel2_sample = self.channel2.sample() as f64;
            let channel3_sample = self.channel3.sample() as f64;
            let channel4_sample = self.channel4.sample() as f64;

            use AudioChannel::*;
            let mut sample_left = 0.0;
            let mut sample_right = 0.0;
            for (channel, sample) in [
                (Channel1, channel1_sample),
                (Channel2, channel2_sample),
                (Channel3, channel3_sample),
                (Channel4, channel4_sample),
            ] {
                if self.global_control.panned_left(channel) {
                    sample_left += sample;
                }
                if self.global_control.panned_right(channel) {
                    sample_right += sample;
                }
            }

            let stereo_volume = self.global_control.volume();
            sample_left *= (stereo_volume.left + 1) as f64 / 8.0;
            sample_right *= (stereo_volume.right + 1) as f64 / 8.0;

            self.low_pass_left.collect_sample(sample_left / 60.0);
            self.low_pass_right.collect_sample(sample_right / 60.0);

            self.sample_counter += 1;
            if self.sample_counter >= cycles_per_sample {
                self.sample_counter = 0;

                self.sample_buffer
                    .push(self.low_pass_left.convolve() as f32);
                self.sample_buffer
                    .push(self.low_pass_right.convolve() as f32);
            }
        }
    }

    pub fn read_register(&self, address: u16) -> u8 {
        match address {
            0xFF10 => self.channel1.read_nr10(),
            0xFF11 => self.channel1.read_nrx1(),
            0xFF12 => self.channel1.read_nrx2(),
            0xFF13 => self.channel1.read_nrx3(),
            0xFF14 => self.channel1.read_nrx4(),
            0xFF16 => self.channel2.read_nrx1(),
            0xFF17 => self.channel2.read_nrx2(),
            0xFF18 => self.channel2.read_nrx3(),
            0xFF19 => self.channel2.read_nrx4(),
            0xFF20 => self.channel4.read_nr41(),
            0xFF21 => self.channel4.read_nr42(),
            0xFF22 => self.channel4.read_nr43(),
            0xFF23 => self.channel4.read_nr44(),
            0xFF1A => self.channel3.read_nr30(),
            0xFF1B => self.channel3.read_nr31(),
            0xFF1C => self.channel3.read_nr32(),
            0xFF1D => self.channel3.read_nr33(),
            0xFF1E => self.channel3.read_nr34(),
            0xFF24 => self.read_nr50(),
            0xFF25 => self.read_nr51(),
            0xFF26 => self.read_nr52(),
            _ => 0xFF,
        }
    }

    pub fn write_register(&mut self, address: u16, value: u8) {
        if !self.global_control.audio_on() && address != 0xFF26 {
            return;
        }

        match address {
            0xFF10 => self.channel1.write_nr10(value),
            0xFF11 => self.channel1.write_nrx1(value),
            0xFF12 => self.channel1.write_nrx2(value),
            0xFF13 => self.channel1.write_nrx3(value),
            0xFF14 => self.channel1.write_nrx4(value),
            0xFF15 => {}
            0xFF16 => self.channel2.write_nrx1(value),
            0xFF17 => self.channel2.write_nrx2(value),
            0xFF18 => self.channel2.write_nrx3(value),
            0xFF19 => self.channel2.write_nrx4(value),
            0xFF20 => self.channel4.write_nr41(value),
            0xFF21 => self.channel4.write_nr42(value),
            0xFF22 => self.channel4.write_nr43(value),
            0xFF23 => self.channel4.write_nr44(value),
            0xFF1A => self.channel3.write_nr30(value),
            0xFF1B => self.channel3.write_nr31(value),
            0xFF1C => self.channel3.write_nr32(value),
            0xFF1D => self.channel3.write_nr33(value),
            0xFF1E => self.channel3.write_nr34(value),
            0xFF24 => self.write_nr50(value),
            0xFF25 => self.write_nr51(value),
            0xFF26 => self.write_nr52(value),
            _ => {}
        }
    }

    fn read_nr50(&self) -> u8 {
        self.global_control.nr50
    }

    fn write_nr50(&mut self, value: u8) {
        self.global_control.nr50 = value;
    }

    fn read_nr51(&self) -> u8 {
        self.global_control.nr51
    }

    fn write_nr51(&mut self, value: u8) {
        self.global_control.nr51 = value;
    }

    fn read_nr52(&self) -> u8 {
        let mut value = 0x70;
        if self.global_control.audio_on() {
            value |= 0x80;
        }

        if self.channel1.enabled {
            value |= 0x01;
        }

        if self.channel2.enabled {
            value |= 0x02;
        }

        if self.channel3.enabled {
            value |= 0x04;
        }

        if self.channel4.enabled {
            value |= 0x08;
        }

        value
    }

    fn write_nr52(&mut self, value: u8) {
        self.global_control.nr52 = value & 0x80;
    }
}

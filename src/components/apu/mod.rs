pub mod noise;
pub mod pulse;
pub mod wave;

use {pulse::PulseChannel, wave::WaveChannel};

// https://jsgroth.dev/blog/posts/gb-rewrite-apu/
// https://nightshade256.github.io/2021/03/27/gb-sound-emulation.html
// https://gbdev.gg8.se/wiki/articles/Gameboy_sound_hardware
// https://gbdev.gg8.se/wiki/articles/Power_Up_Sequence
pub enum AudioPan {
    Left,
    Right,
}

#[derive(Clone, Copy)]
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

    fn channel_on(&self, channel: AudioChannel) -> bool {
        let mask = match channel {
            AudioChannel::Channel1 => 0x01,
            AudioChannel::Channel2 => 0x02,
            AudioChannel::Channel3 => 0x04,
            AudioChannel::Channel4 => 0x08,
        };

        self.audio_on() && (self.nr52 & mask) != 0
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

    // Just gonna ignore VIN
    fn volume(&self) -> StereoVolume {
        StereoVolume {
            left: (self.nr50 >> 4) & 0x07,
            right: self.nr50 & 0x07,
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
    pub frame_sequencer: FrameSequencer,
    sample_counter: u32,
    pub sample_buffer: Vec<f32>,
}

impl APU {
    pub fn new() -> Self {
        Self {
            global_control: GlobalControl::new(),
            channel1: PulseChannel::new_channel1(),
            channel2: PulseChannel::new_channel2(),
            channel3: WaveChannel::new(),
            frame_sequencer: FrameSequencer::new(),
            sample_counter: 0,
            sample_buffer: Vec::new(),
        }
    }

    pub fn read_wram(&self, address: u16) -> u8 {
        self.channel3.ram[(address - 0xFF30) as usize]
    }

    pub fn write_wram(&mut self, address: u16, value: u8) {
        self.channel3.ram[(address - 0xFF30) as usize] = value
    }

    pub fn tick(&mut self, t_cycles: u32, increase_apu_div_counter: bool) {
        let frame_sequencer_step = if increase_apu_div_counter {
            Some(self.frame_sequencer.tick())
        } else {
            None
        };

        if let Some(frame_sequencer_step) = frame_sequencer_step {
            self.channel1.tick_length(frame_sequencer_step.length);
            self.channel2.tick_length(frame_sequencer_step.length);
            self.channel1.tick_envelope(frame_sequencer_step.envelope);
            self.channel2.tick_envelope(frame_sequencer_step.envelope);
            self.channel1.tick_sweep(frame_sequencer_step.sweep);
        }

        for _ in 0..t_cycles {
            self.channel1.tick();
            self.channel2.tick();
            self.sample_counter += 1;
            if self.sample_counter >= 87 {
                self.sample_counter = 0;
                let sample = self.channel2.sample() as f32 / 15.0;
                self.sample_buffer.push(sample);
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
            0xFF24 => self.global_control.nr50,
            0xFF25 => self.global_control.nr51,
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
            0xFF26 => self.write_nr52(value),
            _ => {}
        }
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

        value
    }

    fn write_nr52(&mut self, value: u8) {
        self.global_control.nr52 = value & 0x80;
    }
}

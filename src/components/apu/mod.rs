pub mod noise;
pub mod pulse;
pub mod wave;

use crate::components::apu::wave::WaveChannel;

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
struct FrameSequencer {
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
    pub channel3: WaveChannel,
    pub frame_sequencer: FrameSequencer,
}

impl APU {
    pub fn new() -> Self {
        Self {
            global_control: GlobalControl::new(),
            channel3: WaveChannel::new(),
            frame_sequencer: FrameSequencer::new(),
        }
    }

    pub fn read_wram(&self, address: u16) -> u8 {
        self.channel3.ram[(address - 0xFF30) as usize]
    }

    pub fn write_wram(&mut self, address: u16, value: u8) {
        self.channel3.ram[(address - 0xFF30) as usize] = value
    }

    pub fn tick(&mut self, increase_apu_div_counter: bool) {
        let frame_sequencer_step = if increase_apu_div_counter {
            Some(self.frame_sequencer.tick())
        } else {
            None
        };
    }
}

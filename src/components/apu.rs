// https://jsgroth.dev/blog/posts/gb-rewrite-apu/
// https://nightshade256.github.io/2021/03/27/gb-sound-emulation.html
// https://gbdev.gg8.se/wiki/articles/Gameboy_sound_hardware

pub enum AudioPan {
    Left,
    Right,
}

#[derive(Clone, Copy)]
pub enum AudioChannel {
    Channel1,
    Channel2,
    Channel3,
    Channel4,
}

pub struct StereoVolume {
    pub left: u8,
    pub right: u8,
}

pub struct GlobalControl {
    pub nr50: u8,
    pub nr51: u8,
    pub nr52: u8,
}

impl GlobalControl {
    pub fn audio_on(&self) -> bool {
        (self.nr52 & 0x80) != 0
    }

    pub fn channel_on(&self, channel: AudioChannel) -> bool {
        let mask = match channel {
            AudioChannel::Channel1 => 0x01,
            AudioChannel::Channel2 => 0x02,
            AudioChannel::Channel3 => 0x04,
            AudioChannel::Channel4 => 0x08,
        };

        self.audio_on() && (self.nr52 & mask) != 0
    }

    pub fn panned_left(&self, channel: AudioChannel) -> bool {
        let bit = match channel {
            AudioChannel::Channel1 => 4,
            AudioChannel::Channel2 => 5,
            AudioChannel::Channel3 => 6,
            AudioChannel::Channel4 => 7,
        };

        (self.nr51 >> bit) & 1 != 0
    }

    pub fn panned_right(&self, channel: AudioChannel) -> bool {
        let bit = match channel {
            AudioChannel::Channel1 => 0,
            AudioChannel::Channel2 => 1,
            AudioChannel::Channel3 => 2,
            AudioChannel::Channel4 => 3,
        };

        (self.nr51 >> bit) & 1 != 0
    }

    // Just gonna ignore VIN
    pub fn volume(&self) -> StereoVolume {
        StereoVolume {
            left: (self.nr50 >> 4) & 0x07,
            right: self.nr50 & 0x07,
        }
    }
}

pub struct FrameSequencerStep {
    length_counter: bool,
    sweep: bool,
    envelope: bool,
}

// tick at 512 hz when div bit for goes from 1 -> 0
pub struct FrameSequencer {
    step: u8,
}

impl FrameSequencer {
    fn tick(&mut self) -> FrameSequencerStep {
        let step = self.step;
        self.step = (self.step + 1) & 0x07;

        FrameSequencerStep {
            length_counter: step & 0x01 == 0,
            sweep: step == 0x02 || step == 0x06,
            envelope: step == 0x07,
        }
    }
}

pub struct PulseChannel {}

pub struct APU {
    pub wave_ram: Vec<u8>,
}

impl APU {
    pub fn new() -> Self {
        Self {
            wave_ram: vec![0u8; 16],
        }
    }

    pub fn read(&self, address: u16) {}

    pub fn write(&mut self, address: u16, value: u8) {}
}

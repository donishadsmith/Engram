pub enum AudioChannel {
    Channel1,
    Channel2,
    Channel3,
    Channel4,
}

pub struct PulseChannel {}

pub struct GlobalControl {
    nr50: u8,
    nr51: u8,
    nr52: u8,
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
}

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

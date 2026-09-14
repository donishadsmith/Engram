use crate::components::dma::FifoChannel;
use shared::traits::BitOps;

#[derive(Clone, Copy, PartialEq, Eq)]
pub enum AudioChannel {
    Channel1 = 0,
    Channel2 = 1,
    Channel3 = 2,
    Channel4 = 3,
    FifoA = 4,
    FifoB = 5,
}

#[derive(Clone, Copy)]
pub enum PanDirection {
    Left,
    Right,
}

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

pub struct GlobalControl {
    pub soundcnt_l: u16,
    pub soundcnt_h: u16,
    pub soundcnt_x: u16,
    pub soundbias: u16,
}

impl GlobalControl {
    pub fn new() -> Self {
        Self {
            soundcnt_l: 0,
            soundcnt_h: 0,
            soundcnt_x: 0,
            soundbias: 0x200,
        }
    }

    pub fn timer_select(&self, channel_id: FifoChannel) -> u8 {
        match channel_id {
            FifoChannel::A => self.soundcnt_h.get_bit(10) as u8,
            FifoChannel::B => self.soundcnt_h.get_bit(14) as u8,
        }
    }

    pub fn reset_fifo(&self, channel_id: FifoChannel) -> bool {
        match channel_id {
            FifoChannel::A => self.soundcnt_h.is_set(11),
            FifoChannel::B => self.soundcnt_h.is_set(15),
        }
    }

    pub fn sound_on(&self, channel_id: AudioChannel, panned: PanDirection) -> bool {
        match channel_id {
            AudioChannel::Channel1
            | AudioChannel::Channel2
            | AudioChannel::Channel3
            | AudioChannel::Channel4 => {
                let shift = match panned {
                    PanDirection::Right => 8,
                    PanDirection::Left => 12,
                };

                self.soundcnt_l.is_set(shift + channel_id as usize)
            }
            AudioChannel::FifoA => match panned {
                PanDirection::Right => self.soundcnt_h.is_set(8),
                _ => self.soundcnt_h.is_set(9),
            },
            AudioChannel::FifoB => match panned {
                PanDirection::Right => self.soundcnt_h.is_set(12),
                _ => self.soundcnt_h.is_set(13),
            },
        }
    }

    pub fn panned_left(&self, channel_id: AudioChannel, sample: f32) -> f32 {
        self.sound_on(channel_id, PanDirection::Left) as u8 as f32 * sample
    }

    pub fn panned_right(&self, channel_id: AudioChannel, sample: f32) -> f32 {
        self.sound_on(channel_id, PanDirection::Right) as u8 as f32 * sample
    }

    pub fn master_enabled(&self) -> bool {
        self.soundcnt_x.is_set(7)
    }

    pub fn volume_control(&self, channel: AudioChannel) -> f32 {
        if channel == AudioChannel::FifoA {
            Volume::for_dma(self.soundcnt_h.is_set(2)).to_float()
        } else if channel == AudioChannel::FifoB {
            Volume::for_dma(self.soundcnt_h.is_set(3)).to_float()
        } else {
            Volume::for_psg(self.soundcnt_h.get_bit_range(0..2)).to_float()
        }
    }
    pub fn reset(&mut self) {
        self.soundcnt_l = 0;
        self.soundcnt_h = 0;
        self.soundcnt_x = 0;
        self.soundbias = 0x200;
    }
}

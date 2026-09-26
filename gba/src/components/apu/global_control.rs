use crate::components::dma::FifoChannel;
use shared::traits::BitOps;

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
    pub soundcnt_h: u16,
    pub soundbias: u16,
}

impl GlobalControl {
    pub fn new() -> Self {
        Self {
            soundcnt_h: 0,
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

    pub fn panned_left_fifo(&self, channel_id: FifoChannel) -> bool {
        match channel_id {
            FifoChannel::A => self.soundcnt_h.is_set(9),
            FifoChannel::B => self.soundcnt_h.is_set(13),
        }
    }

    pub fn panned_right_fifo(&self, channel_id: FifoChannel) -> bool {
        match channel_id {
            FifoChannel::A => self.soundcnt_h.is_set(8),
            FifoChannel::B => self.soundcnt_h.is_set(12),
        }
    }

    pub fn volume_control_fifo(&self, channel_id: FifoChannel) -> f32 {
        match channel_id {
            FifoChannel::A => Volume::for_dma(self.soundcnt_h.is_set(2)).to_float(),
            FifoChannel::B => Volume::for_dma(self.soundcnt_h.is_set(3)).to_float(),
        }
    }

    pub fn psg_volume(&self) -> f32 {
        match self.soundcnt_h.get_bit_range(0..2) {
            0 => Volume::Quarter,
            1 => Volume::Half,
            2 => Volume::Full,
            _ => Volume::Prohibited,
        }
        .to_float()
    }
}

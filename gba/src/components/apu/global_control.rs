use crate::components::{dma::FifoChannel, utils::BitOps};

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
            soundbias: 0,
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

    pub fn reset(&mut self) {
        self.soundcnt_l = 0;
        self.soundcnt_h = 0;
        self.soundcnt_x = 0;
        self.soundbias = 0;
    }
}

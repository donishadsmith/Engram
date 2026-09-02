use crate::components::{
    dma::{FifoChannel, Trigger},
    utils::BitOps,
};
use std::collections::VecDeque;

pub struct Fifo {
    queue: VecDeque<u8>,
    channel_id: FifoChannel,
    pub latched: u8,
    pub enabled: bool,
}

impl Fifo {
    pub fn new(channel_id: FifoChannel) -> Self {
        Self {
            queue: VecDeque::with_capacity(32),
            channel_id,
            latched: 0,
            enabled: false,
        }
    }

    pub fn push_samples(&mut self, value: u16) {
        if self.queue_full() {
            return;
        }

        for i in 0..2 {
            let start_index = (i * 8) as usize;
            let data = value.get_bit_range(start_index..(start_index + 8)) as u8;
            self.queue.push_back(data);
            if self.queue_full() {
                break;
            }
        }
    }

    pub fn queue_full(&self) -> bool {
        self.queue.len() == 32
    }

    pub fn update_latched_sample(&mut self) {
        if !self.enabled {
            return;
        }

        if let Some(data) = self.queue.pop_front() {
            self.latched = data;
        }
    }

    pub fn transfer_request(&self) -> Option<Trigger> {
        if self.queue.len() <= 16 {
            Some(Trigger::SoundFifo(self.channel_id))
        } else {
            None
        }
    }

    pub fn reset(&mut self) {
        self.latched = 0;
        self.enabled = false;
        self.queue.clear();
    }
}

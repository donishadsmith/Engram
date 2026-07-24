// https://www.gregorygaines.com/blog/emulator-polling-vs-scheduler-game-loop/
// https://brilliant.org/wiki/binary-heap/

// Lets schedule events instead of polling

use std::{collections::BinaryHeap, cmp::Reverse};

pub enum Event {
    Hblank,
    Vblank,
    TimerOverflow(u8),
    ApuSample
}

pub struct Scheduler {
    pub current: u64,
    queue: BinaryHeap<Reverse<(u64, Event)>>
}

impl Scheduler {
    pub fn new() -> Self {
        Self {current: 0, queue: BinaryHeap::new()}
    }

    pub fn schedule(&mut self, event: Event, cycles: u64) {
        self.queue.push(Reverse((self.current + cycles, event)));
    }
}
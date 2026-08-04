// https://www.gregorygaines.com/blog/emulator-polling-vs-scheduler-game-loop/
// https://brilliant.org/wiki/binary-heap/

// Lets schedule events instead of polling
// We can use a match statement in the gba main loop instead of constantly calling
// component functions on each iteration

use std::{cmp::Reverse, collections::BinaryHeap};

#[derive(Eq, Ord, PartialEq, PartialOrd)]
pub enum Event {
    Hblank,
    Vblank,
    TimerOverflow(u8),
    ApuSample,
}

pub struct Scheduler {
    pub current: u64,
    queue: BinaryHeap<Reverse<(u64, Event)>>,
}

impl Scheduler {
    pub fn new() -> Self {
        Self {
            current: 0,
            queue: BinaryHeap::new(),
        }
    }

    pub fn schedule(&mut self, event: Event, cycles: u64) {
        self.queue.push(Reverse((self.current + cycles, event)));
    }

    pub fn next(&self) -> u64 {
        self.queue.peek().map_or(u64::MAX, |Reverse((t, _))| *t)
    }

    pub fn pop(&mut self) -> Option<Event> {
        if self.next() <= self.current {
            self.queue.pop().map(|Reverse((_, kind))| kind)
        } else {
            None
        }
    }
}

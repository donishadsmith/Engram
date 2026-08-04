// https://www.gregorygaines.com/blog/emulator-polling-vs-scheduler-game-loop/
// https://brilliant.org/wiki/binary-heap/
// https://github.com/michelhe/rustboyadvance-ng/blob/master/core/src/sched.rs
// https://github.com/elipsitz/gba-emulator/blob/main/gba_core/src/scheduler.rs

// Lets schedule events instead of polling
// We can use a match statement in the gba main loop instead of constantly calling
// component functions on each iteration

use std::{cmp::Reverse, collections::BinaryHeap};

#[derive(Eq, Ord, PartialEq, PartialOrd, Debug)]
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

    pub fn add(&mut self, event: Event, cycles: u64) {
        self.queue.push(Reverse((self.current + cycles, event)));
    }

    pub fn next(&self) -> u64 {
        self.queue.peek().map_or(u64::MAX, |Reverse((t, _))| *t)
    }

    pub fn go_to_next_event(&mut self) {
        let current = self.current;
        let next = self.next();

        if next != u64::MAX && next > current {
            self.current += next - current
        }
    }

    pub fn pop(&mut self) -> Option<Event> {
        if self.next() <= self.current {
            self.queue.pop().map(|Reverse((_, kind))| kind)
        } else {
            None
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_scheduler_with_schedule() {
        let mut scheduler = Scheduler::new();

        scheduler.add(Event::Hblank, 5);

        assert_eq!(scheduler.next(), 5);
        assert_eq!(scheduler.pop(), None);

        scheduler.current = 6;

        assert_eq!(scheduler.pop(), Some(Event::Hblank));
    }

    #[test]
    fn test_scheduler_with_no_schedule() {
        let mut scheduler = Scheduler::new();

        assert_eq!(scheduler.next(), u64::MAX);
        assert_eq!(scheduler.pop(), None);
    }
}

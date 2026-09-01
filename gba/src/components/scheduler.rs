// https://www.gregorygaines.com/blog/emulator-polling-vs-scheduler-game-loop/
// https://brilliant.org/wiki/binary-heap/
// https://github.com/michelhe/rustboyadvance-ng/blob/master/core/src/sched.rs
// https://github.com/elipsitz/gba-emulator/blob/main/gba_core/src/scheduler.rs

use std::{cmp::Reverse, collections::BinaryHeap};

pub const HBLANK_OFFSET: u64 = 1006;
pub const CYCLES_PER_SCANLINE: u64 = 1232;
pub const APU_SAMPLE: u64 = 512;
pub const APU_SEQUENCER: u64 = 32768;

#[derive(Clone, Copy, Eq, Ord, PartialEq, PartialOrd, Debug)]
pub enum Event {
    Hblank,
    HblankEnd,
    TimerOverflow(u8),
    ApuSample,
    ApuSequencer,
}

pub struct EventScheduler {
    pub current: u64,
    queue: BinaryHeap<Reverse<(u64, Event)>>,
}

impl EventScheduler {
    pub fn new() -> Self {
        Self {
            current: 0,
            queue: BinaryHeap::new(),
        }
    }

    pub fn push(&mut self, event: Event, time: u64) {
        self.queue.push(Reverse((time, event)));
    }

    pub fn next(&self) -> u64 {
        self.queue
            .peek()
            .map_or(u64::MAX, |Reverse((time, _))| *time)
    }

    pub fn skip_to_next_event(&mut self) {
        let current = self.current;
        let next = self.next();

        if next != u64::MAX && next > current {
            self.current += next - current
        }
    }

    pub fn pop(&mut self) -> Option<(u64, Event)> {
        if self.next() <= self.current {
            self.queue
                .pop()
                .map(|Reverse((deadline, kind))| (deadline, kind))
        } else {
            None
        }
    }

    pub fn reschedule(&mut self, event: Event, deadline: u64) {
        match event {
            Event::Hblank | Event::HblankEnd => self.push(event, deadline + CYCLES_PER_SCANLINE),
            Event::ApuSample => self.push(event, deadline + APU_SAMPLE),
            Event::ApuSequencer => self.push(event, deadline + APU_SEQUENCER),
            _ => unreachable!(),
        }
    }

    pub fn initialize_events(&mut self) {
        self.push(Event::Hblank, HBLANK_OFFSET);
        self.push(Event::HblankEnd, CYCLES_PER_SCANLINE);
        self.push(Event::ApuSample, APU_SAMPLE);
        self.push(Event::ApuSequencer, APU_SEQUENCER);
    }

    pub fn cancel(&mut self, event: Event) {
        self.queue.retain(|Reverse((_, e))| *e != event);
    }

    pub fn is_scheduled(&mut self, event: Event) -> bool {
        self.queue.iter().any(|Reverse((_, e))| *e == event)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_scheduler_with_schedule() {
        let mut scheduler = EventScheduler::new();

        scheduler.push(Event::Hblank, 5);

        assert_eq!(scheduler.next(), 5);
        assert_eq!(scheduler.pop(), None);

        scheduler.current = 6;

        assert_eq!(scheduler.pop(), Some((5, Event::Hblank)));
    }

    #[test]
    fn test_scheduler_with_no_schedule() {
        let mut scheduler = EventScheduler::new();

        assert_eq!(scheduler.next(), u64::MAX);
        assert_eq!(scheduler.pop(), None);
    }

    #[test]
    fn test_scheduler_order() {
        let mut scheduler = EventScheduler::new();

        scheduler.push(Event::Hblank, 10);
        scheduler.push(Event::ApuSample, 4);

        scheduler.current = 5;

        assert_eq!(scheduler.next(), 4);
        assert_eq!(scheduler.pop(), Some((4, Event::ApuSample)));

        scheduler.current = 10;

        assert_eq!(scheduler.pop(), Some((10, Event::Hblank)));
    }

    #[test]
    fn test_cancel() {
        let mut scheduler = EventScheduler::new();
        scheduler.push(Event::Hblank, 5);
        scheduler.cancel(Event::Hblank);

        assert_eq!(scheduler.next(), u64::MAX);
        assert_eq!(scheduler.pop(), None);
    }

    #[test]
    fn test_event_in_queue() {
        let mut scheduler = EventScheduler::new();
        scheduler.push(Event::Hblank, 5);

        assert!(scheduler.is_scheduled(Event::Hblank));
    }
}

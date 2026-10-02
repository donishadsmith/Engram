// https://www.gregorygaines.com/blog/emulator-polling-vs-scheduler-game-loop/
// https://brilliant.org/wiki/binary-heap/
// https://github.com/michelhe/rustboyadvance-ng/blob/master/core/src/sched.rs
// https://github.com/elipsitz/gba-emulator/blob/main/gba_core/src/scheduler.rs

use std::{cmp::Reverse, collections::BinaryHeap};

pub trait ScheduledEvent: PartialEq + Clone + Copy + Ord {}

impl<T: Copy + Ord> ScheduledEvent for T {}

pub struct EventScheduler<T: ScheduledEvent> {
    pub current: u64,
    queue: BinaryHeap<Reverse<(u64, T)>>,
}

impl<T: ScheduledEvent> EventScheduler<T> {
    pub fn new() -> Self {
        Self {
            current: 0,
            queue: BinaryHeap::new(),
        }
    }

    pub fn push(&mut self, event: T, time: u64) {
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

    pub fn pop(&mut self) -> Option<(u64, T)> {
        if self.next() <= self.current {
            self.queue
                .pop()
                .map(|Reverse((deadline, kind))| (deadline, kind))
        } else {
            None
        }
    }

    pub fn cancel(&mut self, event: T) {
        self.queue.retain(|Reverse((_, e))| *e != event);
    }

    pub fn is_scheduled(&self, event: T) -> bool {
        self.queue.iter().any(|Reverse((_, e))| *e == event)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[derive(Clone, Copy, Eq, Ord, PartialEq, PartialOrd, Debug)]
    pub enum Event {
        Hblank,
        ApuSample,
    }

    #[test]
    fn test_scheduler_with_schedule() {
        let mut scheduler = EventScheduler::<Event>::new();

        scheduler.push(Event::Hblank, 5);

        assert_eq!(scheduler.next(), 5);
        assert_eq!(scheduler.pop(), None);

        scheduler.current = 6;

        assert_eq!(scheduler.pop(), Some((5, Event::Hblank)));
    }

    #[test]
    fn test_scheduler_with_no_schedule() {
        let mut scheduler = EventScheduler::<Event>::new();

        assert_eq!(scheduler.next(), u64::MAX);
        assert_eq!(scheduler.pop(), None);
    }

    #[test]
    fn test_scheduler_order() {
        let mut scheduler = EventScheduler::<Event>::new();

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
        let mut scheduler = EventScheduler::<Event>::new();
        scheduler.push(Event::Hblank, 5);
        scheduler.cancel(Event::Hblank);

        assert_eq!(scheduler.next(), u64::MAX);
        assert_eq!(scheduler.pop(), None);
    }

    #[test]
    fn test_event_in_queue() {
        let mut scheduler = EventScheduler::<Event>::new();
        scheduler.push(Event::Hblank, 5);

        assert!(scheduler.is_scheduled(Event::Hblank));
    }
}

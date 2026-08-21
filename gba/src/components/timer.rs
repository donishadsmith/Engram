use crate::components::{
    scheduler::{Event, EventScheduler},
    utils::BitOps,
};

#[derive(PartialEq, Eq, Clone, Copy, Debug)]
pub enum IncrementMode {
    Cascade,
    Prescaler(u16),
}

pub struct Timer {
    pub id: u8,
    pub counter: u16,
    pub increment_mode: IncrementMode,
    pub control_register: u16,
    pub counter_register: u16,
    pub overflow_interrupt_enabled: bool,
    pub on: bool,
    anchor: (u64, u16),
    expected_overflow: u64,
}

impl Timer {
    fn new(id: u8) -> Self {
        Self {
            id,
            counter: 0,
            control_register: 0,
            counter_register: 0,
            increment_mode: IncrementMode::Prescaler(1),
            overflow_interrupt_enabled: false,
            on: false,
            anchor: (0, 0),
            expected_overflow: u64::MAX,
        }
    }

    fn increment_mode_from(&self) -> IncrementMode {
        if self.id != 0 && self.control_register.is_set(2) {
            IncrementMode::Cascade
        } else {
            match self.control_register.get_bit_range(0..2) {
                0b00 => IncrementMode::Prescaler(1),
                0b01 => IncrementMode::Prescaler(64),
                0b10 => IncrementMode::Prescaler(256),
                0b11 => IncrementMode::Prescaler(1024),
                _ => unreachable!(),
            }
        }
    }

    pub fn current_counter(&self, timestamp: u64) -> u16 {
        if !self.on {
            return self.counter;
        }

        match self.increment_mode {
            IncrementMode::Cascade => self.counter,
            IncrementMode::Prescaler(prescaler) => {
                let prescaler = prescaler as u64;
                let (old_time, counter_at_old_time) = self.anchor;
                let elapsed_ticks = (timestamp / prescaler) - (old_time / prescaler);
                let current_ticks = elapsed_ticks + counter_at_old_time as u64;

                current_ticks.min(0xFFFF) as u16
            }
        }
    }

    pub fn write_counter_register(&mut self, value: u16) {
        self.counter_register = value
    }

    pub fn read_control_register(&self) -> u16 {
        self.control_register
    }

    pub fn write_control_register(&mut self, value: u16, scheduler: &mut EventScheduler) {
        if self.on {
            self.counter = self.current_counter(scheduler.current);
        }

        let timer_previously_off = !self.on;
        self.control_register = value;
        self.on = self.control_register.is_set(7);
        self.overflow_interrupt_enabled = value.is_set(6);

        if timer_previously_off && self.on {
            self.counter = self.counter_register;
        }

        self.increment_mode = self.increment_mode_from();

        let event = Event::TimerOverflow(self.id);
        scheduler.cancel(event);

        if self.on && !matches!(self.increment_mode, IncrementMode::Cascade) {
            self.schedule_overflow(scheduler.current, scheduler)
        }
    }

    fn reload_counter(&mut self) {
        self.counter = self.counter_register;
    }

    fn set_counter(&mut self, new_value: u16, overflow: bool) {
        if overflow {
            self.reload_counter();
        } else {
            self.counter = new_value;
        }
    }

    pub fn schedule_overflow(&mut self, timestamp: u64, scheduler: &mut EventScheduler) {
        let IncrementMode::Prescaler(prescaler) = self.increment_mode else {
            return;
        };

        let prescaler = prescaler as u64;

        let first_tick = (timestamp / prescaler + 1) * prescaler;
        let remaining_ticks = 0x10000 - self.counter as u64;
        let deadline = first_tick + (remaining_ticks - 1) * prescaler;

        self.anchor = (timestamp, self.counter);
        self.expected_overflow = deadline;
        scheduler.push(Event::TimerOverflow(self.id), deadline);
    }
}

pub struct Timers {
    pub timers: [Timer; 4],
}

impl Timers {
    pub fn new() -> Self {
        Self {
            timers: [Timer::new(0), Timer::new(1), Timer::new(2), Timer::new(3)],
        }
    }

    pub fn handle_overflow(
        &mut self,
        timer_id: u8,
        deadline: u64,
        scheduler: &mut EventScheduler,
        interrupt_flag: &mut u16,
    ) -> u8 {
        let timer = &self.timers[timer_id as usize];
        if !timer.on || deadline != timer.expected_overflow {
            return 0;
        }

        self.timers[timer_id as usize].reload_counter();
        self.set_interrupt_request_flag(timer_id as usize, true, interrupt_flag);

        let mut overflowed_mask = 1 << timer_id;
        if timer_id < 3 {
            overflowed_mask |= self.perform_cascade(timer_id as usize, true, interrupt_flag)
        }

        self.timers[timer_id as usize].schedule_overflow(deadline, scheduler);

        overflowed_mask
    }

    fn perform_cascade(
        &mut self,
        timer_id: usize,
        mut previous_counter_overflowed: bool,
        interrupt_flag: &mut u16,
    ) -> u8 {
        let mut overflowed_mask = 0u8;
        for current_timer_id in (timer_id + 1)..4 {
            let next_timer = &mut self.timers[current_timer_id];
            if next_timer.increment_mode != IncrementMode::Cascade
                || !next_timer.on
                || !previous_counter_overflowed
            {
                break;
            }

            let (new_value, overflow) = next_timer.counter.overflowing_add(1);
            next_timer.set_counter(new_value, overflow);

            if overflow {
                overflowed_mask |= 1 << current_timer_id;
            }

            previous_counter_overflowed = overflow;
            self.set_interrupt_request_flag(current_timer_id, overflow, interrupt_flag);
        }

        overflowed_mask
    }

    fn set_interrupt_request_flag(
        &self,
        timer_id: usize,
        overflow: bool,
        interrupt_flag: &mut u16,
    ) {
        if !(overflow && self.timers[timer_id].overflow_interrupt_enabled) {
            return;
        }

        interrupt_flag.set_bit(3 + timer_id);
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::components::{bus::AccessType, scheduler::Event::TimerOverflow, utils::create_bus};

    // counter=200; prescaler=256; first tick is 256 and deadline = 256+(0x10000 - 200 - 1)*256 = 256 + 65335* 256 = 16726016
    const FIRST_DEADLINE: u64 = 16726016;

    fn setup_timer1(bus: &mut crate::components::bus::Bus) {
        bus.write_u16(0x4000104, 200, AccessType::Nonsequential);
        bus.write_u16(0x4000106, 0b0000_0000_1100_0010, AccessType::Sequential);
    }

    #[test]
    fn test_timer_setup() {
        let mut bus = create_bus();
        setup_timer1(&mut bus);

        assert_eq!(bus.timers.timers[1].counter, 200);
        assert!(matches!(
            bus.timers.timers[1].increment_mode,
            IncrementMode::Prescaler(256)
        ));
        assert_eq!(bus.scheduler.next(), FIRST_DEADLINE);

        bus.idle(FIRST_DEADLINE + 10 - bus.scheduler.current);
        let (deadline, event) = bus.scheduler.pop().unwrap();
        assert_eq!((deadline, event), (FIRST_DEADLINE, TimerOverflow(1)));

        bus.timers
            .handle_overflow(1, deadline, &mut bus.scheduler, &mut bus.interrupt_flag);
        assert_eq!(bus.timers.timers[1].counter, 200);
        assert!(bus.interrupt_flag.is_set(4));
        assert_eq!(bus.scheduler.next(), FIRST_DEADLINE + (0x10000 - 200) * 256);
    }

    #[test]
    fn test_current_counter() {
        let mut bus = create_bus();
        setup_timer1(&mut bus);

        // 1k ticks past fist tick; counter=200+1000
        bus.idle(1000 * 256 - bus.scheduler.current);
        assert_eq!(
            bus.read_u16(0x4000104, AccessType::Nonsequential),
            200 + 1000
        );
    }

    #[test]
    fn test_cascade() {
        let mut bus = create_bus();
        setup_timer1(&mut bus);

        bus.write_u16(0x4000108, 200, AccessType::Sequential);
        bus.write_u16(0x400010A, 0b0000000011000110, AccessType::Sequential);

        assert_eq!(bus.timers.timers[2].increment_mode, IncrementMode::Cascade);
        assert!(!bus.scheduler.is_scheduled(TimerOverflow(2)));

        // ONE event handles the entire climb from 200 to overflow
        bus.idle(FIRST_DEADLINE + 10 - bus.scheduler.current);
        let (deadline, event) = bus.scheduler.pop().unwrap();
        assert_eq!(event, TimerOverflow(1));

        bus.timers
            .handle_overflow(1, deadline, &mut bus.scheduler, &mut bus.interrupt_flag);
        assert_eq!(bus.timers.timers[1].counter, 200);
        assert_eq!(bus.timers.timers[2].counter, 201);
        assert!(bus.interrupt_flag.is_set(4));
        assert!(!bus.interrupt_flag.is_set(5));
    }

    #[test]
    fn test_deadlines() {
        let mut bus = create_bus();
        setup_timer1(&mut bus);

        let mut last = 0;
        for _ in 0..4 {
            let next = bus.scheduler.next();
            assert!(next > last, "deadline didnt increase; {next} <= {last}");

            bus.idle(next + 1 - bus.scheduler.current);
            let (deadline, TimerOverflow(n)) = bus.scheduler.pop().unwrap() else {
                panic!("timer shouldve overflowed");
            };

            last = deadline;
            bus.timers
                .handle_overflow(n, deadline, &mut bus.scheduler, &mut bus.interrupt_flag);
        }
    }

    #[test]
    fn test_disable_cancels() {
        let mut bus = create_bus();
        setup_timer1(&mut bus);
        assert!(bus.scheduler.is_scheduled(TimerOverflow(1)));

        bus.write_u16(0x4000106, 0b0000000001000010, AccessType::Sequential);
        assert!(!bus.scheduler.is_scheduled(TimerOverflow(1)));
    }

    #[test]
    fn test_rewrite_preserves_elapsed() {
        let mut bus = create_bus();
        setup_timer1(&mut bus);

        bus.idle(100 * 256 - bus.scheduler.current);
        bus.write_u16(0x4000106, 0b0000000011000010, AccessType::Sequential);

        let counter = bus.read_u16(0x4000104, AccessType::Nonsequential);
        assert!(counter >= 300, "rewrite didnt preserve elapse: {counter}");
    }
}

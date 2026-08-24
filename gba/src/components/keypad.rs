// https://problemkaputt.de/gbatek-gba-keypad-input.htm

use crate::components::utils::BitOps;

#[derive(Clone, Copy)]
enum KeypadButton {
    A = 0,
    B = 1,
    Select = 2,
    Start = 3,
    Right = 4,
    Left = 5,
    Up = 6,
    Down = 7,
    R = 8,
    L = 9,
}

const BUTTONS: [KeypadButton; 10] = [
    KeypadButton::Up,
    KeypadButton::Left,
    KeypadButton::Down,
    KeypadButton::Right,
    KeypadButton::A,
    KeypadButton::B,
    KeypadButton::Start,
    KeypadButton::Select,
    KeypadButton::R,
    KeypadButton::L,
];

pub struct Keypad {
    pub keyinput: u16,
    pub keycnt: u16,
    pub previously_triggered: bool,
}

impl Keypad {
    pub fn new() -> Self {
        Self {
            keyinput: 0x3FF,
            keycnt: 0,
            previously_triggered: false,
        }
    }

    pub fn poll(&mut self, keypad: [bool; 10], interrupt_flag: &mut u16) {
        let mut key_bits = 0u16;
        for (key, is_down) in BUTTONS.iter().zip(keypad) {
            if is_down {
                key_bits.set_bit(*key as usize);
            }
        }

        self.keyinput = 0x3FF & !key_bits;
        let mask = self.keycnt.get_bit_range(0..10);
        let condition_met = if self.keycnt.is_set(15) {
            mask != 0 && key_bits & mask == mask
        } else {
            key_bits & mask != 0
        };

        let previously_triggered = self.previously_triggered;
        if self.keycnt.is_set(14) && condition_met && !previously_triggered {
            interrupt_flag.set_bit(12);
        }

        self.previously_triggered = condition_met
    }

    // perhaps good idea to reset these in soft reset when bit 7 is set?
    pub fn reset(&mut self) {
        self.keyinput = 0x3FF;
        self.keycnt = 0;
    }
}

#[cfg(test)]
mod tests {
    use crate::components::{
        bus::AccessType,
        gamepak::BackupType,
        utils::{BitOps, create_bus},
    };

    const BOOL_ARR1: [bool; 10] = [
        true, false, true, false, false, false, false, false, false, false,
    ];

    const BOOL_ARR2: [bool; 10] = [
        false, false, false, false, false, false, false, false, false, true,
    ];

    const BOOL_ARR3: [bool; 10] = [
        true, false, true, false, false, false, false, false, false, true,
    ];

    #[test]
    fn test_button_press_or() {
        let mut bus = create_bus(BackupType::Flash);

        bus.write_u16(0x4000132, 0b0100000000000101, AccessType::Nonsequential);

        bus.keypad.poll(BOOL_ARR1, &mut bus.interrupt_flag);

        assert!(
            bus.keypad.keyinput == 0b1100111111,
            "the actual key input: {:0b}",
            bus.keypad.keyinput
        );
        assert!(bus.interrupt_flag.is_clear(12));

        bus.keypad.poll(BOOL_ARR2, &mut bus.interrupt_flag);

        assert!(
            bus.keypad.keyinput == 0b0111111111,
            "the actual key input: {:0b}",
            bus.keypad.keyinput
        );

        assert!(bus.interrupt_flag.is_clear(12));
    }

    #[test]
    fn test_button_press_and() {
        let mut bus = create_bus(BackupType::Flash);

        bus.write_u16(0x4000132, 0b1100000000000101, AccessType::Nonsequential);

        bus.keypad.poll(BOOL_ARR1, &mut bus.interrupt_flag);
        assert!(
            bus.keypad.keyinput == 0b1100111111,
            "the actual key input: {:0b}",
            bus.keypad.keyinput
        );
        assert!(bus.interrupt_flag.is_clear(12));

        bus.write_u16(0x4000132, 0b1100000011000000, AccessType::Nonsequential);

        bus.keypad.poll(BOOL_ARR1, &mut bus.interrupt_flag);
        assert!(
            bus.keypad.keyinput == 0b1100111111,
            "the actual key input: {:0b}",
            bus.keypad.keyinput
        );
        assert!(bus.interrupt_flag.is_set(12));
    }

    #[test]
    fn test_and_fire_superset() {
        let mut bus = create_bus(BackupType::Flash);

        bus.write_u16(0x4000132, 0b1100000011000000, AccessType::Nonsequential);

        bus.keypad.poll(BOOL_ARR3, &mut bus.interrupt_flag);
        assert!(bus.interrupt_flag.is_set(12));
    }

    #[test]
    fn test_no_refiring_when_held() {
        let mut bus = create_bus(BackupType::Flash);

        bus.write_u16(0x4000132, 0b0100000001000000, AccessType::Nonsequential);

        bus.keypad.poll(BOOL_ARR1, &mut bus.interrupt_flag);
        assert!(bus.interrupt_flag.is_set(12));

        bus.interrupt_flag = 0;

        bus.keypad.poll(BOOL_ARR1, &mut bus.interrupt_flag);
        assert!(bus.interrupt_flag.is_clear(12));

        bus.keypad.poll([false; 10], &mut bus.interrupt_flag);
        bus.keypad.poll(BOOL_ARR1, &mut bus.interrupt_flag);
        assert!(bus.interrupt_flag.is_set(12));
    }
}

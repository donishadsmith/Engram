// https://gbdev.io/pandocs/Joypad_Input.html

use crate::components::cpu::interrupts::InterruptMode;

const BUTTONS: [JoypadButton; 8] = [
    JoypadButton::DPad(DPadButton::Up),
    JoypadButton::DPad(DPadButton::Left),
    JoypadButton::DPad(DPadButton::Down),
    JoypadButton::DPad(DPadButton::Right),
    JoypadButton::Action(ActionButton::A),
    JoypadButton::Action(ActionButton::B),
    JoypadButton::Action(ActionButton::Start),
    JoypadButton::Action(ActionButton::Select),
];

#[derive(Clone, Copy)]
pub enum ActionButton {
    A,
    B,
    Start,
    Select,
}

#[derive(Clone, Copy)]
pub enum DPadButton {
    Up,
    Down,
    Left,
    Right,
}

#[derive(Clone, Copy)]
pub enum JoypadButton {
    Action(ActionButton),
    DPad(DPadButton),
}

impl JoypadButton {
    fn bit(self) -> u8 {
        match self {
            JoypadButton::DPad(DPadButton::Right) | JoypadButton::Action(ActionButton::A) => 0,
            JoypadButton::DPad(DPadButton::Left) | JoypadButton::Action(ActionButton::B) => 1,
            JoypadButton::DPad(DPadButton::Up) | JoypadButton::Action(ActionButton::Select) => 2,
            JoypadButton::DPad(DPadButton::Down) | JoypadButton::Action(ActionButton::Start) => 3,
        }
    }
}

pub struct Joypad {
    pub select: u8,
    dpad: u8,
    buttons: u8,
}

impl Joypad {
    pub fn new() -> Self {
        Self {
            select: 0x30,
            dpad: 0x0F,
            buttons: 0x0F,
        }
    }

    pub fn read(&self) -> u8 {
        let mut matrix = 0xC0 | (self.select & 0x30) | 0x0F;

        if self.select & 0x10 == 0 {
            matrix &= 0xF0 | self.dpad;
        }

        if self.select & 0x20 == 0 {
            matrix &= 0xF0 | self.buttons;
        }

        matrix
    }

    pub fn poll(&mut self, pressed_key: [bool; 8], interrupt_flag: &mut u8) {
        for (index, &button) in BUTTONS.iter().enumerate() {
            let selected = match button {
                JoypadButton::DPad(_) => self.select & 0x10 == 0,
                JoypadButton::Action(_) => self.select & 0x20 == 0,
            };

            let group = match button {
                JoypadButton::DPad(_) => &mut self.dpad,
                JoypadButton::Action(_) => &mut self.buttons,
            };

            let bit = 1 << button.bit();
            if pressed_key[index] {
                if *group & bit != 0 && selected {
                    *interrupt_flag |= InterruptMode::Joypad.mask();
                }
                *group &= !bit;
            } else {
                *group |= bit;
            }
        }
    }
}

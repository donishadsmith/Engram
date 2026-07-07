// https://gbdev.io/pandocs/Joypad_Input.html

use crate::components::cpu::core::InterruptMode;
use macroquad::input::{KeyCode, is_key_down};

const KEYMAP: [(KeyCode, JoypadButton); 8] = [
    (KeyCode::W, JoypadButton::DPad(DPadButton::Up)),
    (KeyCode::A, JoypadButton::DPad(DPadButton::Left)),
    (KeyCode::S, JoypadButton::DPad(DPadButton::Down)),
    (KeyCode::D, JoypadButton::DPad(DPadButton::Right)),
    (KeyCode::K, JoypadButton::Action(ActionButton::A)),
    (KeyCode::L, JoypadButton::Action(ActionButton::B)),
    (KeyCode::I, JoypadButton::Action(ActionButton::Start)),
    (KeyCode::P, JoypadButton::Action(ActionButton::Select)),
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
        let mut result = 0xC0 | self.select | 0x0F;

        if self.select & 0x10 == 0 {
            result &= 0xF0 | self.dpad;
        }
        if self.select & 0x20 == 0 {
            result &= 0xF0 | self.buttons;
        }

        result
    }

    pub fn listen(&mut self, interrupt_flag: &mut u8) {
        for (key, button) in KEYMAP {
            let group = match button {
                JoypadButton::DPad(_) => &mut self.dpad,
                JoypadButton::Action(_) => &mut self.buttons,
            };

            let bit = 1 << button.bit();
            if is_key_down(key) {
                if *group & bit != 0 {
                    *interrupt_flag |= InterruptMode::Joypad.mask();
                }

                *group &= !bit;
            } else {
                *group |= bit;
            }
        }
    }
}

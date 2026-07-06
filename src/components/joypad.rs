use macroquad::input::KeyCode;

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
enum ActionButton {
    A,
    B,
    Start,
    Select,
}

#[derive(Clone, Copy)]
enum DPadButton {
    Up,
    Down,
    Left,
    Right,
}

#[derive(Clone, Copy)]
enum JoypadButton {
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

struct Joypad {
    select: u8,
    dpad: u8,
    buttons: u8,
}

impl Joypad {}

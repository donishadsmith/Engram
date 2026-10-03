use gilrs::Button;
use macroquad::input::KeyCode;

use crate::EmulatorId;

#[derive(Clone, Copy, PartialEq, Eq)]
pub enum KeyId {
    Emulator(EmulatorId),
    Hotkeys,
}

impl KeyId {
    pub fn map_to_shared_key_id(self) -> KeyId {
        match self {
            KeyId::Emulator(EmulatorId::Gb) => KeyId::Emulator(EmulatorId::Gba),
            _ => self,
        }
    }
}

pub enum Hotkeys {
    Screenshot,
    Debugger,
    Gif,
    Lua,
    Fullscreen,
}

#[derive(Clone, Copy, PartialEq, Eq)]
pub enum Input {
    Key(KeyCode),
    Gamepad(Button),
}

#[derive(Clone, Copy, PartialEq, Eq)]
pub enum InputType {
    Keyboard,
    Gamepad,
}

use gilrs::Button;
use macroquad::input::KeyCode;

use crate::{
    EmulatorId,
    input::utils::{button_to_string, keycode_to_string},
};

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

impl Input {
    pub fn to_string(self) -> String {
        match self {
            Input::Key(key) => keycode_to_string(key),
            Input::Gamepad(button) => button_to_string(button),
        }
    }
}

#[derive(Clone, Copy, PartialEq, Eq)]
pub enum InputType {
    Keyboard,
    Gamepad,
}

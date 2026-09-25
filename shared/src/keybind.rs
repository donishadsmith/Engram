use macroquad::input::{KeyCode, get_keys_down};
use std::{
    collections::{BTreeMap, HashSet},
    io::Error,
};

// TODO: figure out how to map Pico button board presses to keybindings

use crate::{EmulatorId, config::Config};

// taken straight from miniquad and used regex because no way could i type this all out
// https://github.com/not-fl3/miniquad/blob/master/src/native/wasm/keycodes.rs
pub const BINDABLE_KEYS: [KeyCode; 117] = [
    KeyCode::Space,
    KeyCode::Apostrophe,
    KeyCode::Comma,
    KeyCode::Minus,
    KeyCode::Period,
    KeyCode::Slash,
    KeyCode::Key0,
    KeyCode::Key1,
    KeyCode::Key2,
    KeyCode::Key3,
    KeyCode::Key4,
    KeyCode::Key5,
    KeyCode::Key6,
    KeyCode::Key7,
    KeyCode::Key8,
    KeyCode::Key9,
    KeyCode::Semicolon,
    KeyCode::Equal,
    KeyCode::A,
    KeyCode::B,
    KeyCode::C,
    KeyCode::D,
    KeyCode::E,
    KeyCode::F,
    KeyCode::G,
    KeyCode::H,
    KeyCode::I,
    KeyCode::J,
    KeyCode::K,
    KeyCode::L,
    KeyCode::M,
    KeyCode::N,
    KeyCode::O,
    KeyCode::P,
    KeyCode::Q,
    KeyCode::R,
    KeyCode::S,
    KeyCode::T,
    KeyCode::U,
    KeyCode::V,
    KeyCode::W,
    KeyCode::X,
    KeyCode::Y,
    KeyCode::Z,
    KeyCode::LeftBracket,
    KeyCode::Backslash,
    KeyCode::RightBracket,
    KeyCode::Apostrophe,
    KeyCode::Escape,
    KeyCode::Enter,
    KeyCode::Tab,
    KeyCode::Backspace,
    KeyCode::Insert,
    KeyCode::Delete,
    KeyCode::Right,
    KeyCode::Left,
    KeyCode::Down,
    KeyCode::Up,
    KeyCode::PageUp,
    KeyCode::PageDown,
    KeyCode::Home,
    KeyCode::End,
    KeyCode::CapsLock,
    KeyCode::ScrollLock,
    KeyCode::NumLock,
    KeyCode::PrintScreen,
    KeyCode::Pause,
    KeyCode::F1,
    KeyCode::F2,
    KeyCode::F3,
    KeyCode::F4,
    KeyCode::F5,
    KeyCode::F6,
    KeyCode::F7,
    KeyCode::F8,
    KeyCode::F9,
    KeyCode::F10,
    KeyCode::F11,
    KeyCode::F12,
    KeyCode::F13, // wait do keyboards with f keys greater than 12 actually exist
    KeyCode::F14,
    KeyCode::F15,
    KeyCode::F16,
    KeyCode::F17,
    KeyCode::F18,
    KeyCode::F19,
    KeyCode::F20,
    KeyCode::F21,
    KeyCode::F22,
    KeyCode::F23,
    KeyCode::F24,
    KeyCode::Kp0,
    KeyCode::Kp1,
    KeyCode::Kp2,
    KeyCode::Kp3,
    KeyCode::Kp4,
    KeyCode::Kp5,
    KeyCode::Kp6,
    KeyCode::Kp7,
    KeyCode::Kp8,
    KeyCode::Kp9,
    KeyCode::KpDecimal,
    KeyCode::KpDivide,
    KeyCode::KpMultiply,
    KeyCode::KpSubtract,
    KeyCode::KpAdd,
    KeyCode::KpEnter,
    KeyCode::KpEqual,
    KeyCode::LeftShift,
    KeyCode::LeftControl,
    KeyCode::LeftAlt,
    KeyCode::LeftSuper,
    KeyCode::RightShift,
    KeyCode::RightControl,
    KeyCode::RightAlt,
    KeyCode::RightSuper,
    KeyCode::Menu,
];

pub struct SaveKeys {
    pub gbakeys: BTreeMap<String, String>,
    pub hotkeys: BTreeMap<String, String>,
}

#[derive(Clone, Copy)]
pub struct Bindings {
    label: &'static str,
    key: KeyCode,
}

pub const DEFAULT_HOT_KEYS: [Bindings; 4] = [
    Bindings {
        label: "Screenshot",
        key: KeyCode::F7,
    },
    Bindings {
        label: "Lua",
        key: KeyCode::F8,
    },
    Bindings {
        label: "Debugger",
        key: KeyCode::F11,
    },
    Bindings {
        label: "Gif",
        key: KeyCode::F12,
    },
];

pub const DEFAULT_GBA_KEYS: [Bindings; 10] = [
    Bindings {
        label: "Up",
        key: KeyCode::W,
    },
    Bindings {
        label: "Left",
        key: KeyCode::A,
    },
    Bindings {
        label: "Down",
        key: KeyCode::S,
    },
    Bindings {
        label: "Right",
        key: KeyCode::D,
    },
    Bindings {
        label: "A",
        key: KeyCode::L,
    },
    Bindings {
        label: "B",
        key: KeyCode::K,
    },
    Bindings {
        label: "Start",
        key: KeyCode::Enter,
    },
    Bindings {
        label: "Select",
        key: KeyCode::RightShift,
    },
    Bindings {
        label: "R",
        key: KeyCode::I,
    },
    Bindings {
        label: "L",
        key: KeyCode::O,
    },
];

pub fn get_relevant_key_presses(keymap: &[KeyCode], input_blocked: bool) -> Vec<bool> {
    if input_blocked {
        return vec![false; keymap.len()];
    }

    let down_keys: HashSet<KeyCode> = get_keys_down();

    keymap.iter().map(|k| down_keys.contains(k)).collect()
}

pub fn keycode_to_string(key: KeyCode) -> String {
    format!("{:?}", key)
}

fn string_to_keycode(str: &str) -> Option<KeyCode> {
    BINDABLE_KEYS
        .iter()
        .copied()
        .find(|key| format!("{key:?}") == str)
}

#[derive(Clone, Copy, PartialEq, Eq)]
pub enum KeyId {
    Gba,
    Hotkeys,
}

impl KeyId {
    // maybe later use _ and include the emu id as a parameter
    pub fn reserved(self) -> KeyId {
        match self {
            KeyId::Gba => KeyId::Hotkeys,
            KeyId::Hotkeys => KeyId::Gba,
        }
    }
}

pub enum Hotkeys {
    Screenshot,
    Debugger,
    Gif,
    Lua,
}

// TODO: probably should clean this up later
pub struct KeyBindings {
    gb: Vec<Bindings>,
    gba: Vec<Bindings>,
    hotkeys: Vec<Bindings>,
}

impl KeyBindings {
    pub fn new() -> Self {
        Self {
            gb: Vec::with_capacity(8),
            gba: Vec::with_capacity(10),
            hotkeys: Vec::with_capacity(2),
        }
    }

    pub fn keys(&self, key_id: KeyId) -> Vec<KeyCode> {
        let bindings = self.get_bindings(key_id);

        bindings.iter().map(|b| b.key).collect()
    }

    pub fn labels(&self, key_id: KeyId) -> Vec<&str> {
        let bindings = self.get_bindings(key_id);

        bindings.iter().map(|b| b.label).collect()
    }

    fn get_bindings(&self, key_id: KeyId) -> Vec<Bindings> {
        match key_id {
            KeyId::Gba => self.gba.clone(),
            KeyId::Hotkeys => self.hotkeys.clone(),
        }
    }

    pub fn load_keys(mut self, config: &Config) -> Self {
        let collect_keys = |default_array: &[Bindings], treemap: &BTreeMap<String, String>| {
            default_array
                .iter()
                .map(|b| Bindings {
                    label: b.label,
                    key: treemap
                        .get(b.label)
                        .and_then(|s| string_to_keycode(s))
                        .unwrap_or(b.key),
                })
                .collect::<Vec<Bindings>>()
        };

        self.gba = collect_keys(&DEFAULT_GBA_KEYS, &config.gbakeys);
        self.gb = self.gba.clone()[..8].to_vec();
        self.hotkeys = collect_keys(&DEFAULT_HOT_KEYS, &config.hotkeys);

        self
    }

    pub fn save_keys(&self) -> Result<SaveKeys, Error> {
        let collect_keys = |vec: &Vec<Bindings>| {
            vec.into_iter()
                .map(|b| {
                    (
                        String::from(b.label),
                        String::from(keycode_to_string(b.key)),
                    )
                })
                .collect::<BTreeMap<String, String>>()
        };

        let gbakeys = collect_keys(&self.gba);
        let hotkeys = collect_keys(&self.hotkeys);

        Ok(SaveKeys { gbakeys, hotkeys })
    }

    pub fn rebind(&mut self, key_id: KeyId, index: usize, key: KeyCode) {
        let key_bindings = match key_id {
            KeyId::Gba => &mut self.gba,
            KeyId::Hotkeys => &mut self.hotkeys,
        };

        key_bindings[index].key = key;
    }

    pub fn restore_defaults(&mut self, key_id: KeyId, emulator_id: EmulatorId) {
        match key_id {
            KeyId::Hotkeys => self.hotkeys = DEFAULT_HOT_KEYS.to_vec(),
            _ => match (emulator_id, key_id) {
                (EmulatorId::Gb, KeyId::Gba) | (EmulatorId::Gba, KeyId::Gba) => {
                    self.gba = DEFAULT_GBA_KEYS.to_vec()
                }
                _ => {}
            },
        }
    }

    pub fn get_hotkey_bind(&self, hotkey: Hotkeys) -> KeyCode {
        let text = match hotkey {
            Hotkeys::Screenshot => "Screenshot",
            Hotkeys::Debugger => "Debugger",
            Hotkeys::Gif => "Gif",
            Hotkeys::Lua => "Lua",
        };

        self.hotkeys
            .iter()
            .find(|k| k.label == text)
            .and_then(|k| Some(k.key))
            .unwrap()
    }
}

pub mod constants;
pub mod enums;
pub mod utils;

use crate::input::{
    constants::{DEFAULT_GBA_KEYS, DEFAULT_HOT_KEYS},
    enums::{Hotkeys, Input, InputType, KeyId},
    utils::{button_to_string, keycode_to_string, string_to_button, string_to_keycode},
};
use crate::{EmulatorId, config::Config};
use gilrs::Button;
use macroquad::input::KeyCode;
use std::{collections::BTreeMap, io::Error};

pub struct SaveKeys {
    pub gba_keyboard: BTreeMap<String, String>,
    pub gba_gamepad: BTreeMap<String, String>,
    pub hotkeys: BTreeMap<String, String>,
}

#[derive(Clone, Copy)]
pub struct Bindings {
    pub label: &'static str,
    pub key: KeyCode,
    pub button: Option<Button>,
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

    pub fn keys(&self, key_id: KeyId) -> Vec<Input> {
        self.get_bindings_ref(key_id)
            .iter()
            .map(|b| Input::Key(b.key))
            .collect()
    }

    pub fn buttons(&self, key_id: KeyId) -> Vec<Input> {
        self.get_bindings_ref(key_id)
            .iter()
            .map(|b| Input::Gamepad(b.button.unwrap()))
            .collect()
    }

    pub fn labels(&self, key_id: KeyId) -> Vec<&str> {
        self.get_bindings_ref(key_id)
            .iter()
            .map(|b| b.label)
            .collect()
    }

    fn get_bindings_ref(&self, key_id: KeyId) -> &Vec<Bindings> {
        match key_id {
            KeyId::Emulator(EmulatorId::Gb) | KeyId::Emulator(EmulatorId::Gba) => &self.gba,
            KeyId::Hotkeys => &self.hotkeys,
        }
    }

    fn get_bindings_mut(&mut self, key_id: KeyId) -> &mut Vec<Bindings> {
        match key_id {
            KeyId::Emulator(EmulatorId::Gb) | KeyId::Emulator(EmulatorId::Gba) => &mut self.gba,
            KeyId::Hotkeys => &mut self.hotkeys,
        }
    }

    pub fn load_keys(mut self, config: &Config) -> Self {
        let collect_keys = |default_array: &[Bindings],
                            keyboard_treemap: &BTreeMap<String, String>,
                            gamepad_treemap: Option<&BTreeMap<String, String>>,
                            key_id: KeyId| {
            default_array
                .iter()
                .map(|b| Bindings {
                    label: b.label,
                    key: keyboard_treemap
                        .get(b.label)
                        .and_then(|s| string_to_keycode(s))
                        .unwrap_or(b.key),
                    button: if key_id == KeyId::Hotkeys {
                        None
                    } else {
                        gamepad_treemap
                            .and_then(|map| map.get(b.label))
                            .and_then(|s| string_to_button(s))
                            .or(b.button)
                    },
                })
                .collect::<Vec<Bindings>>()
        };

        self.gba = collect_keys(
            &DEFAULT_GBA_KEYS,
            &config.gba_keyboard,
            Some(&config.gba_gamepad),
            KeyId::Emulator(EmulatorId::Gba),
        );
        self.gb = self.gba.clone()[..8].to_vec();
        self.hotkeys = collect_keys(&DEFAULT_HOT_KEYS, &config.hotkeys, None, KeyId::Hotkeys);

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

        // think of way to consolidate logic later, copy and paste now
        let collect_buttons = |vec: &Vec<Bindings>| {
            vec.into_iter()
                .map(|b| {
                    (
                        String::from(b.label),
                        String::from(button_to_string(b.button.unwrap())),
                    )
                })
                .collect::<BTreeMap<String, String>>()
        };

        let gba_keyboard = collect_keys(&self.gba);
        let gba_gamepad = collect_buttons(&self.gba);
        let hotkeys = collect_keys(&self.hotkeys);

        Ok(SaveKeys {
            gba_keyboard,
            gba_gamepad,
            hotkeys,
        })
    }

    pub fn rebind(&mut self, key_id: KeyId, index: usize, input: Input) {
        let key_bindings = match key_id {
            KeyId::Emulator(EmulatorId::Gba) | KeyId::Emulator(EmulatorId::Gb) => &mut self.gba,
            KeyId::Hotkeys => &mut self.hotkeys,
        };

        match input {
            Input::Key(key) => key_bindings[index].key = key,
            Input::Gamepad(button) => key_bindings[index].button = Some(button),
        }
    }

    pub fn restore_defaults(&mut self, key_id: KeyId, input_type: InputType) {
        let current_bindings = self.get_bindings_mut(key_id);

        let default_bindings = get_default_keys(key_id);

        for input in current_bindings.iter_mut() {
            let default_input = default_bindings
                .iter()
                .find(|b| b.label == input.label)
                .unwrap();
            match input_type {
                InputType::Gamepad => input.button = default_input.button,
                InputType::Keyboard => input.key = default_input.key,
            }
        }
    }

    pub fn reserved(&self, key_id: KeyId, input: Input) -> bool {
        match key_id {
            KeyId::Hotkeys => self.keys(KeyId::Emulator(EmulatorId::Gba)).contains(&input), /*eventually use .extend*/
            KeyId::Emulator(_) => self.keys(KeyId::Hotkeys).contains(&input),
        }
    }

    pub fn get_hotkey_bind(&self, hotkey: Hotkeys) -> KeyCode {
        let text = match hotkey {
            Hotkeys::Screenshot => "Screenshot",
            Hotkeys::Debugger => "Debugger",
            Hotkeys::Gif => "Gif",
            Hotkeys::Lua => "Lua",
            Hotkeys::Fullscreen => "Fullscreen",
            Hotkeys::LoadState => "LoadState",
            Hotkeys::SaveState => "SaveState",
        };

        self.hotkeys
            .iter()
            .find(|k| k.label == text)
            .and_then(|k| Some(k.key))
            .unwrap()
    }

    pub fn index_of(&self, key_id: KeyId, input: Input) -> Option<usize> {
        match input {
            Input::Key(_) => self.keys(key_id).iter().position(|&i| i == input),
            Input::Gamepad(_) => self.buttons(key_id).iter().position(|&i| i == input),
        }
    }
}

fn get_default_keys(key_id: KeyId) -> &'static [Bindings] {
    match key_id {
        KeyId::Hotkeys => &DEFAULT_HOT_KEYS,
        KeyId::Emulator(EmulatorId::Gb) | KeyId::Emulator(EmulatorId::Gba) => &DEFAULT_GBA_KEYS,
    }
}

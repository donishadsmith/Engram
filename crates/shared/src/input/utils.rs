use crate::input::{
    constants::{BINDABLE_BUTTONS, BINDABLE_KEYS},
    enums::Input,
};
use gilrs::{Button, Gamepad};
use macroquad::input::{KeyCode, get_keys_down};

pub fn get_relevant_key_presses(
    keymap: &[Input],
    gamepad: Option<Gamepad<'_>>,
    input_blocked: bool,
) -> Vec<bool> {
    if input_blocked {
        return vec![false; keymap.len()];
    }

    // just always assume the keymap is the same payload type
    let mut boolean_vector = Vec::with_capacity(14);
    for input in keymap.iter() {
        boolean_vector.push(match input {
            Input::Gamepad(button) => gamepad.as_ref().is_some_and(|p| p.is_pressed(*button)),
            Input::Key(keycode) => get_keys_down().contains(keycode),
        })
    }

    boolean_vector
}

pub fn keycode_to_string(key: KeyCode) -> String {
    format!("{:?}", key)
}

pub fn string_to_keycode(str: &str) -> Option<KeyCode> {
    BINDABLE_KEYS
        .iter()
        .copied()
        .find(|key| format!("{key:?}") == str)
}

pub fn button_to_string(button: Button) -> String {
    format!("{:?}", button)
}

pub fn string_to_button(str: &str) -> Option<Button> {
    BINDABLE_BUTTONS
        .iter()
        .copied()
        .find(|button| format!("{button:?}") == str)
}

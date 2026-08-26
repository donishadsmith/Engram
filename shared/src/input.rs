use macroquad::input::{KeyCode, get_keys_down};
use std::collections::HashSet;

pub const GB_KEYMAP: [KeyCode; 8] = [
    KeyCode::W,
    KeyCode::A,
    KeyCode::S,
    KeyCode::D,
    KeyCode::L,
    KeyCode::K,
    KeyCode::Enter,
    KeyCode::RightShift,
];

pub const GBA_KEYMAP: [KeyCode; 10] = [
    KeyCode::W,
    KeyCode::A,
    KeyCode::S,
    KeyCode::D,
    KeyCode::L,
    KeyCode::K,
    KeyCode::Enter,
    KeyCode::RightShift,
    KeyCode::I,
    KeyCode::O,
];

pub fn get_relevant_key_presses(keymap: &[KeyCode]) -> Vec<bool> {
    let down_keys: HashSet<KeyCode> = get_keys_down();

    keymap.iter().map(|k| down_keys.contains(k)).collect()
}

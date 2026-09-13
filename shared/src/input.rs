use macroquad::input::{KeyCode, get_keys_down};
use std::collections::HashSet;

pub const RESERVED_KEYS: [KeyCode; 2] = [KeyCode::F7, KeyCode::F12];

pub const GBA_LABELS: [&str; 10] = [
    "Up", "Left", "Down", "Right", "A", "B", "Start", "Select", "R", "L",
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
pub fn get_relevant_key_presses(keymap: &[KeyCode], input_blocked: bool) -> Vec<bool> {
    if input_blocked {
        return vec![false; keymap.len()];
    }

    let down_keys: HashSet<KeyCode> = get_keys_down();

    keymap.iter().map(|k| down_keys.contains(k)).collect()
}

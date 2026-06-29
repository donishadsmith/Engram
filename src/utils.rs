use macroquad::input::{KeyCode, get_keys_down, get_keys_pressed};
use rfd::FileDialog;
use std::path::PathBuf;

pub enum EmulatorState {
    Active,
    Quit,
    Start,
}

pub fn file_dialog() -> Option<PathBuf> {
    FileDialog::new()
        .set_title("Select a GameBoy ROM file")
        .add_filter("GameBoy Roms", &["gb"])
        .pick_file()
}

fn change_emulator_state(emulator_state: &mut EmulatorState, state: EmulatorState) {
    *emulator_state = state;
}

fn quit_emulator(emulator_state: &mut EmulatorState) {
    if let Some(key) = get_key()
        && key == KeyCode::Escape
    {
        change_emulator_state(emulator_state, EmulatorState::Quit);
    }
}

pub fn get_key() -> Option<KeyCode> {
    let mut key_press = get_keys_pressed().iter().next().cloned();
    if key_press.is_none() {
        key_press = get_keys_down().iter().next().cloned();
    }

    key_press
}

use chrono::{DateTime, Local};
use macroquad::input::{KeyCode, get_keys_pressed, is_key_down};
use rfd::FileDialog;
use shared::{
    EmulatorId,
    debug::DebugPage,
    input::{
        KeyBindings,
        enums::{Input, InputType, KeyId},
        utils::{button_to_string, keycode_to_string},
    },
};
use std::{collections::HashMap, fs::metadata, path::PathBuf};

pub const STATE_KEYS: [KeyCode; 5] = [
    KeyCode::F1,
    KeyCode::F2,
    KeyCode::F3,
    KeyCode::F4,
    KeyCode::F5,
];

pub fn initialize_debug_hashmap() -> HashMap<EmulatorId, DebugPage> {
    let mut map = HashMap::new();

    map.insert(EmulatorId::Gba, DebugPage::Video);

    map
}

pub fn file_dialog() -> Option<PathBuf> {
    FileDialog::new()
        .set_title("Select ROM")
        .add_filter("ROMs", &["gb", "gbc", "gba"])
        .pick_file()
}

pub fn bindings_grid(
    ui: &mut egui::Ui,
    grid_id: &str,
    key_bindings: &KeyBindings,
    key_id: KeyId,
    input_type: InputType,
    key_rebinding: &mut Option<usize>,
    target_key_id: &mut Option<KeyId>,
    restore_default_bindings: &mut bool,
) {
    egui::Grid::new(grid_id).num_columns(2).show(ui, |ui| {
        for (index, (label, input)) in key_bindings
            .labels(key_id)
            .iter()
            .zip(match input_type {
                InputType::Gamepad if matches!(key_id, KeyId::Emulator(_)) => {
                    key_bindings.buttons(key_id)
                }
                _ => key_bindings.keys(key_id),
            })
            .enumerate()
        {
            ui.label(&label.to_string().replace("State", " State"));

            let text = if *key_rebinding == Some(index) && *target_key_id == Some(key_id) {
                "".to_string()
            } else {
                match input {
                    Input::Key(keycode) => keycode_to_string(keycode),
                    Input::Gamepad(button) => button_to_string(button),
                }
            };

            if ui.button(text).clicked() {
                *key_rebinding = Some(index);
                *target_key_id = Some(key_id);
            }

            ui.end_row();
        }

        if ui.button("Restore Defaults").clicked() {
            *target_key_id = Some(key_id);
            *restore_default_bindings = true;
        }
    });
}

pub fn shift_key_down() -> bool {
    is_key_down(KeyCode::LeftShift) || is_key_down(KeyCode::RightShift)
}

pub fn detect_state_key() -> Option<usize> {
    let mut boolean_arr = [false; 5];
    for (index, key) in STATE_KEYS.iter().enumerate() {
        boolean_arr[index] = get_keys_pressed().contains(key)
    }

    boolean_arr.iter().rposition(|&boolean| boolean)
}

pub fn slot_label(path: &Option<PathBuf>, slot: usize, control_key: &str) -> (String, bool) {
    let Some(path) = path else {
        return (
            format!(
                "Slot {slot}:  empty ({control_key}+{})",
                keycode_to_string(STATE_KEYS[slot - 1])
            ),
            false,
        );
    };

    match metadata(path).and_then(|metadata| metadata.modified()) {
        Ok(modified) => {
            let time: DateTime<Local> = modified.into();
            (
                format!(
                    "Slot {slot}:  {} ({control_key}+{})",
                    time.format("%b %d %H:%M"),
                    keycode_to_string(STATE_KEYS[slot - 1])
                ),
                true,
            )
        }

        Err(_) => (
            format!(
                "Slot {slot}:  empty ({control_key}+{})",
                keycode_to_string(STATE_KEYS[slot - 1])
            ),
            false,
        ),
    }
}

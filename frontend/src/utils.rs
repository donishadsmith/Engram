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
use std::{collections::HashMap, path::PathBuf};

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

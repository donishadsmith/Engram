use chrono::{DateTime, Local};
use egui::FontDefinitions;
use egui_notify::Toasts;
use macroquad::input::{KeyCode, get_keys_pressed, is_key_down};
use rfd::FileDialog;
use shared::{
    EmulatorId, EmulatorSession,
    constants::SAVE_STATE_SLOTS,
    debug::DebugPage,
    input::{
        KeyBindings,
        enums::{Input, InputType, KeyId},
        utils::{button_to_string, keycode_to_string},
    },
};
use std::{
    collections::HashMap,
    fs::{metadata, remove_file},
    io::Error,
    path::PathBuf,
    time::{Duration, Instant},
};

// increasing from 5 to 12 because why not
pub const STATE_KEYS: [KeyCode; 12] = [
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
];

#[derive(Clone, Copy)]
pub enum DestructiveAction {
    Overwrite(usize),
    Delete(usize),
    DeleteAll,
    Reset,
}

#[derive(Clone, Copy, PartialEq, Eq)]
pub enum KeybindingTab {
    Keyboard,
    Gamepad,
}

pub struct UiState {
    pub key_rebinding: Option<usize>,
    pub target_key_id: Option<KeyId>,
    pub restore_default_bindings: bool,
    pub force_clear_inputs: bool,
    pub controller_keybinding_tab: KeybindingTab,
    pub destructive_action: Option<DestructiveAction>,
    pub fonts: FontDefinitions,
    pub open_gif_settings: bool,
    pub set_dirs: bool,
    pub start_gif_recording: bool,
    pub debug_toggled: bool,
    // for controller hotkey
    pub last_save_state_time: Instant,
    pub last_load_state_time: Instant,
    pub last_gif_time: Instant,
    pub last_screenshot_time: Instant,
    pub last_video_time: Instant,
    pub controller_hotkey_delay: Duration,
}

impl UiState {
    pub fn new() -> Self {
        Self {
            key_rebinding: None,
            target_key_id: None,
            restore_default_bindings: false,
            force_clear_inputs: false,
            controller_keybinding_tab: KeybindingTab::Keyboard,
            destructive_action: None,
            fonts: FontDefinitions::default(),
            open_gif_settings: false,
            set_dirs: false,
            start_gif_recording: false,
            debug_toggled: false,
            last_save_state_time: Instant::now(),
            last_load_state_time: Instant::now(),
            last_gif_time: Instant::now(),
            last_screenshot_time: Instant::now(),
            last_video_time: Instant::now(),
            controller_hotkey_delay: Duration::from_millis(500),
        }
    }
}

pub fn initialize_debug_hashmap() -> HashMap<EmulatorId, DebugPage> {
    let mut map = HashMap::new();

    map.insert(EmulatorId::Gba, DebugPage::Video);

    map
}

pub fn file_dialog() -> Option<PathBuf> {
    FileDialog::new()
        .set_title("Select a ROM")
        .add_filter("Compatible ROMs", &["gb", "gbc", "gba"])
        .pick_file()
}

pub fn bindings_grid(
    ui: &mut egui::Ui,
    grid_id: &str,
    key_bindings: &KeyBindings,
    input_type: InputType,
    key_rebinding: &mut Option<usize>,
    target_key_id: Option<KeyId>,
    restore_default_bindings: &mut bool,
) {
    egui::Grid::new(grid_id).num_columns(2).show(ui, |ui| {
        let Some(key_id) = target_key_id else {
            return;
        };

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

            let text = if *key_rebinding == Some(index) {
                match input {
                    Input::Key(_) => "Press a key".to_string(),
                    Input::Gamepad(_) => "Press a button".to_string(),
                }
            } else {
                match input {
                    Input::Key(keycode) => keycode_to_string(keycode),
                    Input::Gamepad(button) => button_to_string(button),
                }
            };

            if ui.button(text).clicked() {
                *key_rebinding = Some(index);
            }

            ui.end_row();
        }

        if ui.button("Restore Defaults").clicked() {
            *restore_default_bindings = true;
        }
    });
}

pub fn shift_key_down() -> bool {
    is_key_down(KeyCode::LeftShift) || is_key_down(KeyCode::RightShift)
}

pub fn detect_state_key() -> Option<usize> {
    let mut boolean_arr = [false; SAVE_STATE_SLOTS];
    for (index, key) in STATE_KEYS.iter().enumerate() {
        boolean_arr[index] = get_keys_pressed().contains(key)
    }

    boolean_arr
        .iter()
        .rposition(|&boolean| boolean)
        .and_then(|slot| Some(slot + 1))
}

pub fn slot_label(
    emulator: &mut Box<dyn EmulatorSession>,
    slot: usize,
    control_key: &str,
) -> (String, bool) {
    let Some(path) = get_state(emulator, slot) else {
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

pub fn get_state(emulator: &mut Box<dyn EmulatorSession>, slot: usize) -> Option<PathBuf> {
    emulator.current_save_states().remove(slot - 1)
}

pub fn state_exists(emulator: &mut Box<dyn EmulatorSession>, slot: usize) -> bool {
    get_state(emulator, slot).is_some()
}

pub fn save_slots_with_toast(
    emulator: &mut Box<dyn EmulatorSession>,
    toasts: &mut Toasts,
    slot: usize,
) {
    let (message, success) = save_slots_lua(emulator, slot, false);
    match success {
        true => toasts.success(message),
        false => toasts.error(message),
    };
}

pub fn save_slots_lua(
    emulator: &mut Box<dyn EmulatorSession>,
    slot: usize,
    lowercase: bool,
) -> (String, bool) {
    let first_letter = if lowercase { "s" } else { "S" };
    let status_text = if get_state(emulator, slot).is_some() {
        "overwritten"
    } else {
        "saved"
    };

    match emulator.save_state(slot) {
        Ok(()) => (
            format!("{first_letter}ave state {slot} {}", status_text),
            true,
        ),
        Err(err) => (
            format!("{first_letter}ave state {slot} failed: {err}"),
            false,
        ),
    }
}

pub fn load_slots_with_toast(
    emulator: &mut Box<dyn EmulatorSession>,
    toasts: &mut Toasts,
    slot: usize,
) {
    let (message, success) = load_slots_lua(emulator, slot, false);
    match success {
        true => toasts.success(message),
        false => toasts.error(message),
    };
}

pub fn load_slots_lua(
    emulator: &mut Box<dyn EmulatorSession>,
    slot: usize,
    lowercase: bool,
) -> (String, bool) {
    let first_letter = if lowercase { "s" } else { "S" };
    match emulator.load_state(slot) {
        Ok(()) => (format!("{first_letter}ave state {slot} loaded"), true),
        Err(err) => (
            format!("{first_letter}ave state {slot} failed to load: {err}"),
            false,
        ),
    }
}

pub fn delete_slots_with_toast(
    emulator: &mut Box<dyn EmulatorSession>,
    toasts: &mut Toasts,
    slot: usize,
) {
    let (message, success) = delete_slots_lua(emulator, slot, false);
    match success {
        true => toasts.success(message),
        false => toasts.error(message),
    };
}

pub fn delete_slots_lua(
    emulator: &mut Box<dyn EmulatorSession>,
    slot: usize,
    lowercase: bool,
) -> (String, bool) {
    let first_letter = if lowercase { "s" } else { "S" };
    match get_state(emulator, slot) {
        Some(state) => match remove_file(state) {
            Ok(_) => (format!("{first_letter}ave state {slot} deleted"), true),
            Err(err) => (
                format!("{first_letter}ave state {slot} failed to delete: {err}"),
                false,
            ),
        },
        None => (
            format!("{first_letter}ave state {slot} does not exist"),
            false,
        ),
    }
}

pub fn delete_all_slots_with_toast(emulator: &mut Box<dyn EmulatorSession>, toasts: &mut Toasts) {
    let mut n_deleted = 0;
    for (index, state) in emulator.current_save_states().iter().enumerate() {
        let slot = index + 1;
        let mut error: Option<Error> = None;
        if let Some(path) = state {
            match remove_file(path) {
                Ok(_) => n_deleted += 1,
                Err(err) => error = Some(err),
            }

            if let Some(err) = error {
                toasts.error(format!("Save state {slot} failed to delete: {err}"));
            }
        }
    }

    match n_deleted {
        0 => toasts.error(format!("No save states could be deleted")),
        _ => toasts.success(format!("Number of save states deleted: {n_deleted}")),
    };
}

pub fn thin_separator(ui: &mut egui::Ui) {
    let (rect, _) =
        ui.allocate_exact_size(egui::vec2(ui.available_width(), 6.0), egui::Sense::hover());

    ui.painter().hline(
        rect.x_range(),
        rect.center().y,
        egui::Stroke::new(0.5, ui.visuals().weak_text_color()),
    );
}

pub fn update_controller_hotkey_text(
    state_text: String,
    slot: usize,
    is_save_state: bool,
) -> String {
    let arrow = match slot {
        1 => "DPadUp",
        2 => "DPadRight",
        3 => "DPadDown",
        4 => "DPadLeft",
        _ => unreachable!(),
    };

    let trigger = if is_save_state {
        "LeftTrigger"
    } else {
        "RightTrigger"
    };

    state_text.replace(")", format!("/{trigger}+Start+{arrow})").as_str())
}

use macroquad::input::{KeyCode, get_last_key_pressed, is_key_down, is_key_pressed};
use shared::{
    editor::super_key_down,
    input::enums::{Hotkeys, Input, InputType, KeyId},
    utils::screenshot,
};

use crate::{
    session::{GamepadUpdate, Session},
    utils::{
        DestructiveAction, KeybindingTab, UiState, delete_slots_with_toast, detect_state_key,
        load_slots_with_toast, save_slots_with_toast, shift_key_down, state_exists,
    },
};

pub fn detect_hot_keys(session: &mut Session, ui_state: &mut UiState) {
    if !session.show_hotkeys {
        if !(shift_key_down() || super_key_down() || is_key_down(KeyCode::Delete))
            && is_key_pressed(session.key_bindings.get_hotkey_bind(Hotkeys::Fullscreen))
        {
            session.toggle_fullscreen()
        }

        if !(shift_key_down() || super_key_down() || is_key_down(KeyCode::Delete))
            && is_key_pressed(session.key_bindings.get_hotkey_bind(Hotkeys::Screenshot))
        {
            session.toasts.success("Screenshot saved");
            screenshot(session.get_image_path());
        }

        if !(shift_key_down() || super_key_down() || is_key_down(KeyCode::Delete))
            && is_key_pressed(session.key_bindings.get_hotkey_bind(Hotkeys::Debugger))
        {
            ui_state.debug_toggled = true;
        }

        if session.emulator.is_some() {
            if !(shift_key_down() || super_key_down() || is_key_down(KeyCode::Delete))
                && is_key_pressed(session.key_bindings.get_hotkey_bind(Hotkeys::Lua))
            {
                session.lua_editor.opened = !session.lua_editor.opened;
            }

            if !(shift_key_down() || super_key_down() || is_key_down(KeyCode::Delete))
                && is_key_pressed(session.key_bindings.get_hotkey_bind(Hotkeys::Gif))
            {
                if session.gif.is_recording() {
                    session.gif.stop();

                    session.toasts.success("GIF saved");
                } else {
                    ui_state.open_gif_settings = !ui_state.open_gif_settings;
                }
            }

            if shift_key_down()
                && let Some(slot) = detect_state_key()
            {
                if let Some(emulator) = &mut session.emulator {
                    if session.destructive_action_popup && state_exists(emulator, slot) {
                        ui_state.destructive_action = Some(DestructiveAction::Overwrite(slot));
                    } else {
                        save_slots_with_toast(emulator, &mut session.toasts, slot);
                    }
                }
            }

            if super_key_down()
                && let Some(slot) = detect_state_key()
            {
                if let Some(emulator) = &mut session.emulator {
                    load_slots_with_toast(emulator, &mut session.toasts, slot);
                }
            }

            if is_key_down(KeyCode::Delete)
                && let Some(slot) = detect_state_key()
            {
                if let Some(emulator) = &mut session.emulator {
                    if session.destructive_action_popup && state_exists(emulator, slot) {
                        ui_state.destructive_action = Some(DestructiveAction::Delete(slot));
                    } else {
                        delete_slots_with_toast(emulator, &mut session.toasts, slot);
                    }
                }
            }
        }
    }
}

pub fn keybinding(session: &mut Session, ui_state: &mut UiState, gamepad_update: &GamepadUpdate) {
    if ui_state.restore_default_bindings {
        if let Some(key_id) = ui_state.target_key_id {
            session.key_bindings.restore_defaults(
                key_id,
                if ui_state.controller_keybinding_tab == KeybindingTab::Gamepad {
                    InputType::Gamepad
                } else {
                    InputType::Keyboard
                },
            );
        }

        ui_state.restore_default_bindings = false;
        ui_state.key_rebinding = None;
    }

    if let (Some(index), Some(key_id)) = (ui_state.key_rebinding, ui_state.target_key_id) {
        let possible_input = match ui_state.controller_keybinding_tab {
            KeybindingTab::Gamepad if key_id != KeyId::Hotkeys => {
                if let Some(button) = gamepad_update.last_pressed {
                    Some(Input::Gamepad(button))
                } else {
                    None
                }
            }
            KeybindingTab::Keyboard => {
                if let Some(keycode) = get_last_key_pressed() {
                    Some(Input::Key(keycode))
                } else {
                    None
                }
            }
            _ => None,
        };

        // TODO: Not supeer perfect but decent enough for now
        if let Some(input) = possible_input {
            let current_input = match input {
                Input::Key(_) => session.key_bindings.keys(key_id)[index],
                Input::Gamepad(_) => session.key_bindings.buttons(key_id)[index],
            };

            let mut rebinding_complete = true;
            match session.key_bindings.index_of(key_id, input) {
                Some(other_index) if other_index == index => {}
                Some(other_index) => {
                    session
                        .key_bindings
                        .rebind(key_id, other_index, current_input);
                    session.key_bindings.rebind(key_id, index, input);

                    session.toasts.info(format!(
                        "Swapped bindings between {} and {}",
                        session.key_bindings.labels(key_id)[index],
                        session.key_bindings.labels(key_id)[other_index]
                    ));
                }
                None if session.key_bindings.reserved(key_id, input) => {
                    let used_by = match key_id {
                        KeyId::Hotkeys => "emulator controller",
                        _ => "hotkeys",
                    };
                    session.toasts.info(format!(
                        "{} already in use by {}",
                        input.to_string(),
                        used_by.to_string()
                    ));

                    rebinding_complete = false;
                }
                None => session.key_bindings.rebind(key_id, index, input),
            }

            if rebinding_complete {
                ui_state.key_rebinding = None;
            }
        }
    }

    if !session.show_key_bindings && !session.show_hotkeys {
        ui_state.key_rebinding = None;
        ui_state.target_key_id = None;
    }

    if session.show_key_bindings
        && ui_state.controller_keybinding_tab == KeybindingTab::Gamepad
        && session.latest_gamepad_id.is_none()
    {
        ui_state.controller_keybinding_tab = KeybindingTab::Keyboard
    }
}

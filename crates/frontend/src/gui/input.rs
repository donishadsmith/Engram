use std::time::Instant;

use gilrs::{Button, Gamepad, GamepadId, Gilrs};
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

// i kinda suck at pressing all three at once but will have to do, this is what i get for primarily playing rpgs but much easier to execute
// wihtout triggering actions on an 8bitdo than a ps5 controller likely due to travel
// also using start instead of select since select and down appear to be reserved on 8bitdo and just straight up never activates the hotkey
fn get_save_slot(gamepad: Gamepad<'_>, save_state: bool) -> Option<usize> {
    let hotkeys_pressed = if save_state {
        gamepad.is_pressed(Button::Start)
            && (gamepad.is_pressed(Button::LeftTrigger) || gamepad.is_pressed(Button::LeftTrigger2))
    } else {
        gamepad.is_pressed(Button::Start)
            && (gamepad.is_pressed(Button::RightTrigger)
                || gamepad.is_pressed(Button::RightTrigger2))
    };

    if gamepad.is_pressed(Button::DPadUp) && hotkeys_pressed {
        Some(1)
    } else if gamepad.is_pressed(Button::DPadRight) && hotkeys_pressed {
        Some(2)
    } else if gamepad.is_pressed(Button::DPadDown) && hotkeys_pressed {
        Some(3)
    } else if gamepad.is_pressed(Button::DPadLeft) && hotkeys_pressed {
        Some(4)
    } else {
        None
    }
}

enum ShortcutType {
    Screenshot,
    Gif,
    Video,
}

// have a eeling that im going to hard regret these button combos later but keep for now
// not all modern controllers have select or triggers, particulary modern replicas of retro controllers
// could consider mapping to certain keybindings set in menu but maybe not
fn display_controller_hotkeys(
    gilrs: Option<&Gilrs>,
    latest_gamepad_id: Option<GamepadId>,
    shortcut_type: ShortcutType,
) -> bool {
    let mut hotkey_triggered = false;
    if let Some(latest_gamepad_id) = latest_gamepad_id
        && let Some(gilrs) = gilrs
    {
        let gamepad = gilrs.gamepad(latest_gamepad_id);

        let other_button = match shortcut_type {
            ShortcutType::Gif => Button::East,
            ShortcutType::Screenshot => Button::South,
            ShortcutType::Video => Button::West,
        };

        if gamepad.is_pressed(Button::Select) && gamepad.is_pressed(other_button) {
            hotkey_triggered = true;
        }
    }

    return hotkey_triggered;
}

pub fn detect_hot_keys(session: &mut Session, ui_state: &mut UiState) {
    if !session.show_hotkeys {
        if !(shift_key_down() || super_key_down() || is_key_down(KeyCode::Delete))
            && is_key_pressed(session.key_bindings.get_hotkey_bind(Hotkeys::Fullscreen))
        {
            session.toggle_fullscreen()
        }

        if !(shift_key_down() || super_key_down() || is_key_down(KeyCode::Delete))
            && is_key_pressed(session.key_bindings.get_hotkey_bind(Hotkeys::Debugger))
        {
            ui_state.debug_toggled = true;
        }

        if let Some(emulator) = &mut session.emulator {
            if !(shift_key_down() || super_key_down() || is_key_down(KeyCode::Delete))
                && is_key_pressed(session.key_bindings.get_hotkey_bind(Hotkeys::Lua))
            {
                session.lua_editor.opened = !session.lua_editor.opened;
            }

            let hotkey_triggered = display_controller_hotkeys(
                session.gilrs.as_ref(),
                session.latest_gamepad_id,
                ShortcutType::Screenshot,
            ) && ui_state.last_screenshot_time.elapsed()
                >= ui_state.controller_hotkey_delay;
            if !(shift_key_down() || super_key_down() || is_key_down(KeyCode::Delete))
                && (is_key_pressed(session.key_bindings.get_hotkey_bind(Hotkeys::Screenshot))
                    || hotkey_triggered)
            {
                match session.data_dirs.get_image_path() {
                    Ok(path) => {
                        screenshot(path);
                        session.toasts.success("Screenshot saved");
                    }
                    Err(err) => {
                        session.toasts.warning(err);
                    }
                }

                if hotkey_triggered {
                    ui_state.last_screenshot_time = Instant::now();
                    session.controller_hotkey_fired = true;
                }
            }

            let hotkey_triggered = display_controller_hotkeys(
                session.gilrs.as_ref(),
                session.latest_gamepad_id,
                ShortcutType::Gif,
            ) && ui_state.last_gif_time.elapsed()
                >= ui_state.controller_hotkey_delay;
            if !(shift_key_down() || super_key_down() || is_key_down(KeyCode::Delete))
                && (is_key_pressed(session.key_bindings.get_hotkey_bind(Hotkeys::Gif))
                    || hotkey_triggered)
            {
                if session.gif.is_recording() {
                    session.gif.stop();

                    session.toasts.success("GIF saved");
                } else {
                    if hotkey_triggered {
                        match session.data_dirs.get_image_path() {
                            Ok(path) => {
                                let _ = session.gif.start(emulator.frontend_ref(), path);
                            }
                            Err(err) => {
                                session.toasts.warning(err);
                            }
                        };
                    } else {
                        ui_state.open_gif_settings = !ui_state.open_gif_settings;
                    }
                }

                if hotkey_triggered {
                    ui_state.last_gif_time = Instant::now();
                    session.controller_hotkey_fired = true;
                }
            }

            let hotkey_triggered = display_controller_hotkeys(
                session.gilrs.as_ref(),
                session.latest_gamepad_id,
                ShortcutType::Video,
            ) && ui_state.last_video_time.elapsed()
                >= ui_state.controller_hotkey_delay;
            if !(shift_key_down() || super_key_down() || is_key_down(KeyCode::Delete))
                && (is_key_pressed(session.key_bindings.get_hotkey_bind(Hotkeys::Video))
                    || hotkey_triggered)
            {
                if emulator.is_video_recording() {
                    match emulator.stop_video() {
                        Ok(_) => session.toasts.success("Mp4 saved"),
                        Err(err) => session.toasts.warning(err.to_string()),
                    };
                } else {
                    match session.data_dirs.get_video_path() {
                        Ok(path) => {
                            let _ = emulator.start_video(path);
                            session.toasts.success("Video started");
                        }
                        Err(err) => {
                            session.toasts.warning(err);
                        }
                    }
                }

                if hotkey_triggered {
                    ui_state.last_video_time = Instant::now();
                    session.controller_hotkey_fired = true;
                }
            }

            if shift_key_down()
                && let Some(slot) = detect_state_key()
            {
                save_slots_with_toast(emulator, &mut session.toasts, slot);
            }

            if super_key_down()
                && let Some(slot) = detect_state_key()
            {
                load_slots_with_toast(emulator, &mut session.toasts, slot);
            }

            if is_key_down(KeyCode::Delete)
                && let Some(slot) = detect_state_key()
            {
                if session.destructive_action_popup && state_exists(emulator, slot) {
                    ui_state.destructive_action = Some(DestructiveAction::Delete(slot));
                } else {
                    delete_slots_with_toast(emulator, &mut session.toasts, slot);
                }
            }

            if let Some(latest_gamepad_id) = session.latest_gamepad_id
                && let Some(gilrs) = &session.gilrs
                && ui_state.last_save_state_time.elapsed() >= ui_state.controller_hotkey_delay
                && let Some(slot) = get_save_slot(gilrs.gamepad(latest_gamepad_id), true)
            {
                save_slots_with_toast(emulator, &mut session.toasts, slot);
                ui_state.last_save_state_time = Instant::now();
                session.controller_hotkey_fired = true;
            }

            if let Some(latest_gamepad_id) = session.latest_gamepad_id
                && let Some(gilrs) = &session.gilrs
                && ui_state.last_load_state_time.elapsed() >= ui_state.controller_hotkey_delay
                && let Some(slot) = get_save_slot(gilrs.gamepad(latest_gamepad_id), false)
            {
                load_slots_with_toast(emulator, &mut session.toasts, slot);
                ui_state.last_load_state_time = Instant::now();
                session.controller_hotkey_fired = true;
            }

            let latest_time = ui_state
                .last_save_state_time
                .max(ui_state.last_load_state_time)
                .max(ui_state.last_screenshot_time)
                .max(ui_state.last_gif_time);
            if latest_time.elapsed() >= ui_state.controller_hotkey_delay {
                session.controller_hotkey_fired = false
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

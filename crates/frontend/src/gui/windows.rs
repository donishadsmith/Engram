use egui::Context;
use shared::{EmulatorState, input::enums::InputType, script::ScriptRequest, utils::screenshot};

use crate::{
    session::Session,
    utils::{
        DestructiveAction, KeybindingTab, UiState, bindings_grid, delete_all_slots_with_toast,
        delete_slots_lua, delete_slots_with_toast, load_slots_lua, save_slots_lua,
        save_slots_with_toast, thin_separator,
    },
};
use egui_phosphor::regular as icons;

pub fn show_controller_bindings(egui_ctx: &Context, session: &mut Session, ui_state: &mut UiState) {
    egui::Window::new("Controller Bindings")
        .open(&mut session.show_key_bindings)
        .show(egui_ctx, |ui| {
            ui.horizontal(|ui| {
                if ui
                    .add(
                        egui::Button::new(format!("{}  Keyboard", icons::KEYBOARD)).selected(
                            ui_state.controller_keybinding_tab == KeybindingTab::Keyboard,
                        ),
                    )
                    .clicked()
                {
                    ui_state.controller_keybinding_tab = KeybindingTab::Keyboard;
                }

                if session.latest_gamepad_id.is_some()
                    && ui
                        .add(
                            egui::Button::new(format!("{}  Controller", icons::GAME_CONTROLLER))
                                .selected(
                                    ui_state.controller_keybinding_tab == KeybindingTab::Gamepad,
                                ),
                        )
                        .clicked()
                {
                    ui_state.controller_keybinding_tab = KeybindingTab::Gamepad;
                }
            });

            ui.separator();

            bindings_grid(
                ui,
                "Controller Bindings",
                &session.key_bindings,
                if ui_state.controller_keybinding_tab == KeybindingTab::Gamepad {
                    InputType::Gamepad
                } else {
                    InputType::Keyboard
                },
                &mut ui_state.key_rebinding,
                ui_state.target_key_id,
                &mut ui_state.restore_default_bindings,
            );
        });
}

pub fn show_hotkey_bindings(egui_ctx: &Context, session: &mut Session, ui_state: &mut UiState) {
    egui::Window::new("Hotkey Bindings")
        .open(&mut session.show_hotkeys)
        .show(egui_ctx, |ui| {
            bindings_grid(
                ui,
                "Hotkey Bindings",
                &session.key_bindings,
                InputType::Keyboard,
                &mut ui_state.key_rebinding,
                ui_state.target_key_id,
                &mut ui_state.restore_default_bindings,
            );
        });
}

pub fn set_data_directories(egui_ctx: &Context, session: &mut Session, ui_state: &mut UiState) {
    let mut flush_configs = false;
    if ui_state.set_dirs {
        egui::Window::new("Choose Data Locations")
            .open(&mut ui_state.set_dirs)
            .show(egui_ctx, |ui| {
                ui.label(
                    egui::RichText::new("Game Saves/Save States")
                        .size(16.0)
                        .strong(),
                );

                thin_separator(ui);

                ui.horizontal(|ui| {
                    if ui.button("Set Location").clicked() {
                        if let Some(path) = rfd::FileDialog::new()
                            .set_directory(&session.data_dir)
                            .pick_folder()
                        {
                            session.data_dir = path;
                            flush_configs = true;
                        }
                    }
                    ui.add(egui::Label::new(session.data_dir.display().to_string()).truncate());
                });

                ui.add_space(8.0);

                ui.label(egui::RichText::new("Screenshots/GIFs").size(16.0).strong());

                thin_separator(ui);

                ui.horizontal(|ui| {
                    if ui.button("Set Location").clicked() {
                        if let Some(path) = rfd::FileDialog::new()
                            .set_directory(&session.image_dir)
                            .pick_folder()
                        {
                            session.image_dir = path;
                            flush_configs = true;
                        }
                    }
                    ui.add(egui::Label::new(session.image_dir.display().to_string()).truncate());
                });
            });
    }

    // will flush other things too but good enough than the directories
    if flush_configs {
        let _ = session.save_configs();
    }
}

pub fn show_lua_editor(egui_ctx: &Context, session: &mut Session) {
    if session.lua_editor.opened {
        if let Some(emulator) = &mut session.emulator
            && let Some(script_engine) = emulator.script_engine()
        {
            if let Some(code) = session.lua_editor.show_ui(egui_ctx) {
                script_engine.load(code);
            }

            if session.lua_editor.take_termination_request() {
                script_engine.stop_all_processes();
            }
        }
    };

    let mut apply_run_updates = false;
    if let Some(emulator) = &mut session.emulator
        && let Some(script_engine) = emulator.script_engine()
    {
        if script_engine.take_clear_request() {
            session.lua_editor.output.clear();
        }

        let lines = script_engine.take_output();
        if !lines.is_empty() {
            session.lua_editor.push_output(lines);
        }

        for request in script_engine.take_requests() {
            match request {
                ScriptRequest::Pause => {
                    // cant reuse functions for pause and running due to a classic borrow checker no no
                    session.state = EmulatorState::Paused;
                    session
                        .lua_editor
                        .push_output(vec!["emulator is paused".to_string()]);
                }
                ScriptRequest::Screenshot => {
                    screenshot(session.image_dir.clone());
                    session
                        .lua_editor
                        .push_output(vec!["screenshot saved".to_string()]);
                }
                ScriptRequest::StartGif => {
                    if !session.gif.is_recording() {
                        if emulator.frame_ready() {
                            match session
                                .gif
                                .start(emulator.frontend_ref(), session.image_dir.clone())
                            {
                                Ok(_) => session
                                    .lua_editor
                                    .push_output(vec!["GIF recording started".to_string()]),
                                Err(_) => session.lua_editor.push_output(vec![
                                    mlua::Error::RuntimeError("GIF failed to start".to_string())
                                        .to_string(),
                                ]),
                            };
                        } else {
                            session.lua_editor.push_output(vec![
                                mlua::Error::RuntimeError(
                                    "frame not ready; gif recording could not start".to_string(),
                                )
                                .to_string(),
                            ]);
                        }
                    }
                }
                ScriptRequest::StopGif => {
                    if session.gif.is_recording() {
                        session.gif.stop();
                        session
                            .lua_editor
                            .push_output(vec!["GIF saved".to_string()]);
                    }
                }
                ScriptRequest::Reset => {
                    session
                        .lua_editor
                        .push_output(vec!["emulator reset".to_string()]);
                    session.state = EmulatorState::Reset;
                    session.skip_toast = true;
                }
                ScriptRequest::StepInstruction | ScriptRequest::StepFrame => {
                    session.pending_steps.push_back(request)
                }
                ScriptRequest::Resume => {
                    session.state = EmulatorState::Running;
                    apply_run_updates = true;
                    session
                        .lua_editor
                        .push_output(vec!["emulator is running".to_string()]);
                    session.skip_toast = true;
                }
                ScriptRequest::SaveState(slot) => {
                    let (mut message, success) = save_slots_lua(emulator, slot, true);
                    if !success {
                        message = mlua::Error::RuntimeError(message).to_string();
                    }

                    session.lua_editor.push_output(vec![message]);
                }
                ScriptRequest::LoadState(slot) => {
                    let (mut message, success) = load_slots_lua(emulator, slot, true);
                    if !success {
                        message = mlua::Error::RuntimeError(message).to_string();
                    }

                    session.lua_editor.push_output(vec![message]);
                }
                ScriptRequest::DeleteState(slot) => {
                    let (mut message, success) = delete_slots_lua(emulator, slot, true);
                    if !success {
                        message = mlua::Error::RuntimeError(message).to_string();
                    }

                    session.lua_editor.push_output(vec![message]);
                }
            }
        }

        // executed outside for loop for one step per next frame await
        if let Some(request) = session.pending_steps.pop_front() {
            if session.state != EmulatorState::Paused {
                session.pending_steps.clear();
                session.lua_editor.push_output(vec![
                    mlua::Error::RuntimeError("emulator must be paused to step".to_string())
                        .to_string(),
                ])
            } else {
                match request {
                    ScriptRequest::StepInstruction => {
                        emulator.step_instruction(session.master_volume);
                    }
                    ScriptRequest::StepFrame => {
                        emulator.step_frame(session.master_volume);
                    }
                    _ => {}
                }
            }
        }
    }

    if apply_run_updates {
        session.reset_fps();
        session.update_frame_deadline();
    }
}

pub fn show_gif_settings(egui_ctx: &Context, session: &mut Session, ui_state: &mut UiState) {
    egui::Window::new("GIF Settings")
        .open(&mut ui_state.open_gif_settings)
        .show(egui_ctx, |ui| {
            egui::Grid::new("GIF Settings")
                .num_columns(1)
                .show(ui, |ui| {
                    ui.add(
                        egui::Slider::new(&mut session.gif.every_n_frame, 1..=10)
                            .text("Keep 1 in every n frames."),
                    );

                    ui.end_row();

                    ui.add(
                        egui::Slider::new(&mut session.gif.delay, 2..=20)
                            .text("Frame delay (1/100 s)."),
                    );

                    ui.end_row();

                    if ui
                        .add_enabled(
                            session.emulator.is_some(),
                            egui::Button::new("Start Recording"),
                        )
                        .clicked()
                    {
                        ui_state.start_recording = true;
                    }
                });
        });

    if ui_state.start_recording {
        if let Some(emulator) = &session.emulator {
            match session
                .gif
                .start(emulator.frontend_ref(), session.image_dir.clone())
            {
                Ok(_) => session.toasts.success("GIF recording started"),
                Err(_) => session.toasts.error("GIF failed to start"),
            };
        }

        ui_state.start_recording = false;
        ui_state.open_gif_settings = false;
    }
}

pub fn show_destructive_action_popup(
    egui_ctx: &Context,
    session: &mut Session,
    ui_state: &mut UiState,
) {
    // i think these are mostly, kinda centered? horizontal_centered didnt work as expected, learn that later
    if let Some(action) = ui_state.destructive_action
        && let Some(emulator) = &mut session.emulator
    {
        let mut add_transient_state = false;
        egui::Window::new(
            egui::RichText::new(format!("{}  Destructive Action", icons::WARNING))
                .strong()
                .color(egui::Color32::RED),
        )
        .collapsible(false)
        .resizable(false)
        .show(egui_ctx, |ui| {
            ui.horizontal(|ui| {
                match action {
                    DestructiveAction::Overwrite(slot) => {
                        ui.add_space(31.0);
                        ui.label(
                            egui::RichText::new(format!("Overwrite save slot {slot}?")).strong(),
                        );
                    }
                    DestructiveAction::Delete(slot) => {
                        ui.add_space(39.0);
                        ui.label(egui::RichText::new(format!("Delete save slot {slot}?")).strong());
                    }
                    DestructiveAction::DeleteAll => {
                        ui.add_space(30.0);
                        ui.label(egui::RichText::new("Delete all save states?").strong());
                    }
                    DestructiveAction::Reset => {
                        ui.add_space(44.0);
                        ui.label(egui::RichText::new(format!("Hard reset game?")).strong());
                    }
                };
            });

            ui.add_space(10.0);

            ui.horizontal(|ui| {
                ui.add_space(50.0);

                if ui.button(egui::RichText::new("Yes").strong()).clicked() {
                    match action {
                        DestructiveAction::Overwrite(slot) => {
                            save_slots_with_toast(emulator, &mut session.toasts, slot)
                        }
                        DestructiveAction::Delete(slot) => {
                            delete_slots_with_toast(emulator, &mut session.toasts, slot)
                        }
                        DestructiveAction::DeleteAll => {
                            delete_all_slots_with_toast(emulator, &mut session.toasts)
                        }
                        DestructiveAction::Reset => {
                            add_transient_state = true;
                        }
                    };

                    ui_state.destructive_action = None;
                }

                ui.add_space(20.0);

                if ui.button(egui::RichText::new("No").strong()).clicked() {
                    ui_state.destructive_action = None;
                }
            });
        });

        if add_transient_state {
            session.add_transient_state(EmulatorState::Reset);
        }
    }
}

pub fn show_debugger_pages(egui_ctx: &Context, session: &mut Session, ui_state: &mut UiState) {
    let Some(emulator) = &mut session.emulator else {
        return;
    };
    let emulator_id = emulator.id();
    if let Some(debugger) = emulator.debugger_mut()
        && debugger.active()
    {
        egui::Window::new("Debuggers")
            .collapsible(true)
            .show(egui_ctx, |ui| {
                ui.vertical(|ui| {
                    for debug_page in debugger.available_pages() {
                        let clicked = ui
                            .selectable_label(debugger.visible(debug_page), debug_page.to_str())
                            .clicked();

                        if clicked && !debugger.visible(debug_page) {
                            debugger.toggle(Some(debug_page));
                            let _ = session.last_debug_page.insert(emulator_id, debug_page);
                        }
                    }
                });
            });
    }

    if ui_state.debug_toggled {
        session.toggle_debug_mode();
        ui_state.debug_toggled = false;
    }
}

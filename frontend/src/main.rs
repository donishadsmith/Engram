// TODO: I think I can just flash firmware on the pico, wire then map buttons to the gpio pins, then connect via usb
// https://gp2040-ce.info/web-configurator/menu-pages/gpio-pin-mapping
pub mod session;
pub mod utils;

use crate::{
    session::{InputSource, Session},
    utils::{bindings_grid, file_dialog},
};
use egui_macroquad;
use macroquad::prelude::*;
use shared::{
    EmulatorState,
    config::load_config,
    input::{
        enums::{Hotkeys, Input, InputType, KeyId},
        utils::keycode_to_string,
    },
    script::ScriptRequest,
    utils::screenshot,
};
use std::{io::Error, mem::take, time::Duration};

#[derive(Clone, Copy, PartialEq, Eq)]
enum KeybindingTab {
    Keyboard,
    Gamepad,
}

fn conf() -> Conf {
    let display = load_config().display;

    Conf {
        window_title: "Engram".to_string(),
        window_width: display.width.unwrap_or(1280),
        window_height: display.height.unwrap_or(900),
        fullscreen: display.fullscreen,
        high_dpi: true,
        ..Default::default()
    }
}

// TODO: maybe clean up some areas in the future
#[macroquad::main(conf)]
async fn main() -> Result<(), Error> {
    let mut session = Session::new();
    let mut key_rebinding: Option<usize> = None;
    let mut target_key_id: Option<KeyId> = None;
    let mut restore_default_bindings = false;
    let mut force_clear_inputs = false;
    let mut controller_keybinding_tab = KeybindingTab::Keyboard;

    loop {
        session.save()?;

        if is_quit_requested() {
            session.state = EmulatorState::Quit;
        }

        session.record_window_size();

        let gamepad_update = session.drain_gamepad_events();

        if let (Some(connected), Some(text)) = (gamepad_update.connected, gamepad_update.event_text)
        {
            let toast = if connected {
                session.toasts.info(text)
            } else {
                session.toasts.warning(text)
            };

            toast.duration(Some(Duration::from_secs(5)));
        }

        if let Some(text) = gamepad_update.active_text {
            session
                .toasts
                .warning(text)
                .duration(Some(Duration::from_secs(5)));
        }

        let input_source_changed =
            session.detect_input_source(gamepad_update.last_pressed, key_rebinding.is_some());

        match session.state {
            EmulatorState::RomSelection => {
                let Some(rom_path) = file_dialog() else {
                    session.state = session.take_return_state().unwrap();

                    continue;
                };

                session.rom_path = Some(rom_path.clone());

                let ext = rom_path
                    .extension()
                    .and_then(|e| e.to_str())
                    .map(|e| e.to_ascii_lowercase())
                    .unwrap_or_default();

                match ext.as_str() {
                    "gb" | "gbc" => {
                        session.set_emulator(gb::GameBoySession::new_session(rom_path)?)
                    }
                    "gba" => {
                        session.set_emulator(gba::GBASession::new_session(rom_path)?);
                        session.set_solar_sensor();
                    }
                    _ => continue,
                }

                session.take_return_state();
                session.set_running();
            }
            EmulatorState::Running => {
                if session.run(
                    gamepad_update.changed | input_source_changed | take(&mut force_clear_inputs),
                )? == EmulatorState::Paused
                {
                    session.set_paused();
                };

                if let Some(emulator) = &session.emulator {
                    let frame = emulator.frontend_ref();
                    if emulator.frame_ready() {
                        session.gif.capture(frame);
                    }
                }
            }
            EmulatorState::Reset => {
                let _ = session.reset();
                session.take_return_state();
                session.state = EmulatorState::Running;
            }
            EmulatorState::Quit => {
                session.gif.stop();
                session.save_configs()?;
                break;
            }
            EmulatorState::Launch => {
                // just so file dialog doesnt occur on launch, since it blocks execution
                // TODO: figure out what i wanna put here, leave blank, render an image for this state, or something else?
            }
            EmulatorState::Paused => {
                session.pause();
                if let Some(emulator) = &session.emulator {
                    let frame = emulator.frontend_ref();
                    session.gif.capture(frame); // frame is coonstant but think of better way to handle pause + gif active later
                }
            }
            EmulatorState::BiosSelection => {
                // TODO: update for future emu
            }
            EmulatorState::SwapDisc => {}
        }

        egui_macroquad::ui(|egui_ctx| {
            egui_ctx.set_pixels_per_point(screen_dpi_scale());
            egui::TopBottomPanel::top("Menu Bar").show(egui_ctx, |ui| {
                egui::menu::bar(ui, |ui| {
                    ui.menu_button("File", |ui| {
                        if ui.button("Load ROM").clicked() {
                            session.add_transient_state(EmulatorState::RomSelection);
                            ui.close_menu();
                        }

                        if ui.button("Quit").clicked() {
                            session.state = EmulatorState::Quit;
                            ui.close_menu();
                        }
                    });

                    ui.menu_button("Emulation", |ui| {
                        ui.menu_button("Volume", |ui| {
                            ui.add(
                                egui::Slider::new(&mut session.master_volume, 0..=100)
                                    .text("Adjust volume for the emulator."),
                            );
                        });

                        if let Some(emulator) = &mut session.emulator {
                            if ui.button("Reset").clicked() {
                                session.state = EmulatorState::Reset;
                                ui.close_menu();
                            }

                            if let Some(solar_sensor) = emulator.solar_sensor() {
                                ui.menu_button("Solar", |ui| {
                                    ui.add(
                                        egui::Slider::new(&mut session.solar_level, 0..=10)
                                            .text("Solar sensor level from lowest to highest"),
                                    );
                                    solar_sensor.set_level(session.solar_level);
                                });
                            }

                            if ui
                                .add(
                                    egui::Button::new("Key Bindings")
                                        .wrap_mode(egui::TextWrapMode::Extend),
                                )
                                .clicked()
                            {
                                session.show_key_bindings = true;
                                ui.close_menu();
                            }
                        }
                    });

                    ui.menu_button("Settings", |ui| {
                        if ui
                            .add(
                                egui::Button::new("Configure Hotkeys")
                                    .wrap_mode(egui::TextWrapMode::Extend),
                            )
                            .clicked()
                        {
                            session.show_hotkeys = true;
                            ui.close_menu();
                        }

                        let hotkey = keycode_to_string(
                            session.key_bindings.get_hotkey_bind(Hotkeys::Fullscreen),
                        );

                        let text = if session.display.fullscreen {
                            "Close Fullscreen"
                        } else {
                            "Fullscreen"
                        };

                        if ui
                            .add(
                                egui::Button::new(format!("{} ({})", text, hotkey))
                                    .wrap_mode(egui::TextWrapMode::Extend),
                            )
                            .clicked()
                        {
                            session.toggle_fullscreen();
                        }

                        ui.separator();

                        let source = match session.input_source {
                            Some(InputSource::Gamepad) => "Controller",
                            Some(InputSource::Keyboard) => "Keyboard",
                            None => "Not detected yet",
                        };
                        ui.label(format!("Input Source: {source}"));

                        if let Some(gilrs) = &session.gilrs {
                            ui.menu_button("Connected Controllers", |ui| {
                                let mut controller_present = false;

                                for (gamepad_id, gamepad) in gilrs.gamepads() {
                                    controller_present = true;

                                    let text = if Some(gamepad_id) == session.latest_gamepad_id
                                        && session.input_source == Some(InputSource::Gamepad)
                                    {
                                        egui::RichText::new(format!(
                                            "{} (active)",
                                            gamepad.os_name()
                                        ))
                                        .strong()
                                    } else {
                                        egui::RichText::new(gamepad.os_name()).weak()
                                    };

                                    ui.add(
                                        egui::Label::new(text)
                                            .wrap_mode(egui::TextWrapMode::Extend),
                                    );
                                }

                                if !controller_present {
                                    ui.label(egui::RichText::new("None connected").weak());
                                }
                            });
                        }
                    });

                    if let Some(emulator) = &session.emulator {
                        let key_id = emulator.id().to_key_id().map_to_shared_key_id();
                        egui::Window::new("Controller Bindings")
                            .open(&mut session.show_key_bindings)
                            .show(egui_ctx, |ui| {
                                ui.horizontal(|ui| {
                                    if ui
                                        .add(egui::Button::new("Keyboard").selected(
                                            controller_keybinding_tab == KeybindingTab::Keyboard,
                                        ))
                                        .clicked()
                                    {
                                        controller_keybinding_tab = KeybindingTab::Keyboard;
                                    }

                                    if session.latest_gamepad_id.is_some()
                                        && ui
                                            .add(egui::Button::new("Controller").selected(
                                                controller_keybinding_tab == KeybindingTab::Gamepad,
                                            ))
                                            .clicked()
                                    {
                                        controller_keybinding_tab = KeybindingTab::Gamepad;
                                    }
                                });
                                ui.separator();

                                bindings_grid(
                                    ui,
                                    "Controller Bindings",
                                    &session.key_bindings,
                                    key_id,
                                    if controller_keybinding_tab == KeybindingTab::Gamepad {
                                        InputType::Gamepad
                                    } else {
                                        InputType::Keyboard
                                    },
                                    &mut key_rebinding,
                                    &mut target_key_id,
                                    &mut restore_default_bindings,
                                );
                            });
                    } else {
                        session.show_key_bindings = false;
                    }

                    egui::Window::new("Hotkey Bindings")
                        .open(&mut session.show_hotkeys)
                        .show(egui_ctx, |ui| {
                            bindings_grid(
                                ui,
                                "Hotkey Bindings",
                                &session.key_bindings,
                                KeyId::Hotkeys,
                                InputType::Keyboard,
                                &mut key_rebinding,
                                &mut target_key_id,
                                &mut restore_default_bindings,
                            );
                        });

                    if restore_default_bindings {
                        if let Some(key_id) = target_key_id {
                            session.key_bindings.restore_defaults(
                                key_id,
                                if controller_keybinding_tab == KeybindingTab::Gamepad {
                                    InputType::Gamepad
                                } else {
                                    InputType::Keyboard
                                },
                            );
                        }

                        restore_default_bindings = false;
                        target_key_id = None;
                        key_rebinding = None;
                    }

                    if let (Some(index), Some(key_id)) = (key_rebinding, target_key_id) {
                        let possible_input = match controller_keybinding_tab {
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

                                    session
                                        .toasts
                                        .info(format!(
                                            "Swapped bindings between {} and {}",
                                            session.key_bindings.labels(key_id)[index],
                                            session.key_bindings.labels(key_id)[other_index]
                                        ))
                                        .duration(Some(Duration::from_secs(3)));
                                }
                                None if session.key_bindings.reserved(key_id, input) => {
                                    let used_by = match key_id {
                                        KeyId::Hotkeys => "emulator controller",
                                        _ => "hotkeys",
                                    };
                                    session
                                        .toasts
                                        .info(format!(
                                            "{} already in use by {}",
                                            input.to_string(),
                                            used_by.to_string()
                                        ))
                                        .duration(Some(Duration::from_secs(3)));

                                    rebinding_complete = false;
                                }
                                None => session.key_bindings.rebind(key_id, index, input),
                            }

                            if rebinding_complete {
                                key_rebinding = None;
                                target_key_id = None;
                            }
                        }
                    }

                    if !session.show_key_bindings && !session.show_hotkeys {
                        key_rebinding = None;
                        target_key_id = None;
                    }

                    ui.menu_button("Tools", |ui| {
                        if session
                            .emulator
                            .as_mut()
                            .map(|emulator| emulator.script_engine())
                            .is_some()
                        {
                            let keycode = keycode_to_string(
                                session.key_bindings.get_hotkey_bind(Hotkeys::Lua),
                            );
                            let text = if session.lua_editor.opened {
                                format!("Close Lua Editor ({})", keycode)
                            } else {
                                format!("Open Lua Editor ({})", keycode)
                            };

                            if ui
                                .add(egui::Button::new(text).wrap_mode(egui::TextWrapMode::Extend))
                                .clicked()
                            {
                                session.lua_editor.opened = !session.lua_editor.opened;
                                ui.close_menu();
                            }
                        }

                        let text = format!(
                            "Screenshot ({})",
                            keycode_to_string(
                                session.key_bindings.get_hotkey_bind(Hotkeys::Screenshot)
                            )
                        );
                        if ui
                            .add(egui::Button::new(text).wrap_mode(egui::TextWrapMode::Extend))
                            .clicked()
                        {
                            screenshot(session.get_image_path());
                            session
                                .toasts
                                .success("Screenshot saved")
                                .duration(Some(Duration::from_secs(5)));
                            ui.close_menu();
                        }

                        let keycode =
                            keycode_to_string(session.key_bindings.get_hotkey_bind(Hotkeys::Gif));
                        let text = if session.gif.is_recording() {
                            format!("Stop GIF ({})", keycode)
                        } else {
                            format!("Record GIF ({})", keycode)
                        };

                        if ui
                            .add(egui::Button::new(text).wrap_mode(egui::TextWrapMode::Extend))
                            .clicked()
                        {
                            if session.gif.is_recording() {
                                if let Some(emulator) = &session.emulator {
                                    let _ = session
                                        .gif
                                        .toggle(emulator.frontend_ref(), session.get_image_path());

                                    session
                                        .toasts
                                        .success("GIF saved")
                                        .duration(Some(Duration::from_secs(5)));
                                }
                            } else {
                                session.open_gif_settings = true;
                            }

                            ui.close_menu();
                        }

                        if ui
                            .add(
                                egui::Button::new("Choose Image Save Location")
                                    .wrap_mode(egui::TextWrapMode::Extend),
                            )
                            .clicked()
                        {
                            session.set_image_dir = true;
                            ui.close_menu();
                        }
                    });

                    if session.set_image_dir {
                        egui::Window::new("Choose Image File Location")
                            .open(&mut session.set_image_dir)
                            .show(egui_ctx, |ui| {
                                egui::Grid::new("Choose File Location").num_columns(1).show(
                                    ui,
                                    |ui| {
                                        if ui.button("Set Location").clicked() {
                                            if let Some(path) = rfd::FileDialog::new()
                                                .set_directory(&session.image_dir)
                                                .pick_folder()
                                            {
                                                session.image_dir = path;
                                            }
                                        }

                                        ui.monospace(session.image_dir.display().to_string())
                                            .on_hover_text("GIFs and screenshots are saved here")
                                    },
                                );
                            });
                    }

                    let mut toggle_requested = false;

                    if !session.show_hotkeys {
                        if is_key_pressed(session.key_bindings.get_hotkey_bind(Hotkeys::Fullscreen))
                        {
                            session.toggle_fullscreen()
                        }

                        if is_key_pressed(session.key_bindings.get_hotkey_bind(Hotkeys::Screenshot))
                        {
                            session
                                .toasts
                                .success("Screenshot saved")
                                .duration(Some(Duration::from_secs(5)));
                            screenshot(session.get_image_path());
                        }

                        if is_key_pressed(session.key_bindings.get_hotkey_bind(Hotkeys::Debugger)) {
                            toggle_requested = true;
                        }

                        if let Some(emulator) = &session.emulator {
                            if is_key_pressed(session.key_bindings.get_hotkey_bind(Hotkeys::Lua)) {
                                session.lua_editor.opened = !session.lua_editor.opened;
                            }

                            if is_key_pressed(session.key_bindings.get_hotkey_bind(Hotkeys::Gif)) {
                                if session.gif.is_recording() {
                                    let _ = session
                                        .gif
                                        .toggle(emulator.frontend_ref(), session.get_image_path());
                                    session
                                        .toasts
                                        .success("GIF saved")
                                        .duration(Some(Duration::from_secs(5)));
                                } else {
                                    session.open_gif_settings = !session.open_gif_settings;
                                }
                            }
                        }
                    }

                    let mut start_recording = false;
                    egui::Window::new("GIF Settings")
                        .open(&mut session.open_gif_settings)
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

                                    if session.emulator.is_some() {
                                        if ui.button("Start Recording").clicked() {
                                            start_recording = true;
                                        }
                                    }
                                });
                        });

                    if start_recording {
                        if let Some(emulator) = &session.emulator {
                            let _ = session
                                .gif
                                .toggle(emulator.frontend_ref(), session.get_image_path());
                        }

                        session.open_gif_settings = false;
                    }

                    let recording = session.gif.is_recording();
                    ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                        if let Some(emulator) = &mut session.emulator
                            && let Some(debugger) = emulator.debugger_ref()
                        {
                            let (text, hover) = if debugger.active() {
                                (
                                    egui::RichText::new("ON").color(egui::Color32::LIGHT_GREEN),
                                    "Click to close debugger",
                                )
                            } else {
                                (egui::RichText::new("OFF").weak(), "Click to open debugger")
                            };

                            let state = ui.add(egui::Label::new(text).sense(egui::Sense::click()));
                            let title = ui.add(
                                egui::Label::new(egui::RichText::new("| Debug Mode:").strong())
                                    .sense(egui::Sense::click()),
                            );

                            if title
                                .union(state)
                                .on_hover_text(hover)
                                .on_hover_cursor(egui::CursorIcon::PointingHand)
                                .clicked()
                            {
                                toggle_requested = true;
                            }
                        }

                        if session.emulator.is_some() {
                            let (text, hover) = if session.is_paused()
                                || session
                                    .return_state
                                    .is_some_and(|state| state == EmulatorState::Paused)
                            {
                                (
                                    egui::RichText::new("PAUSED").color(egui::Color32::YELLOW),
                                    "Click to resume emulator",
                                )
                            } else {
                                (
                                    egui::RichText::new("LIVE").weak(),
                                    "Click to pause emulator",
                                )
                            };

                            let state = ui.add(egui::Label::new(text).sense(egui::Sense::click()));
                            let title = ui.add(
                                egui::Label::new(
                                    egui::RichText::new("| Emulator Status:").strong(),
                                )
                                .sense(egui::Sense::click()),
                            );

                            if title
                                .union(state)
                                .on_hover_text(hover)
                                .on_hover_cursor(egui::CursorIcon::PointingHand)
                                .clicked()
                            {
                                session.toggle_emulator_status();
                            }
                        }

                        if recording {
                            ui.label(egui::RichText::new("RECORDING").color(egui::Color32::RED));
                        }
                    });

                    if let Some(emulator) = &mut session.emulator {
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
                                                .selectable_label(
                                                    debugger.visible(debug_page),
                                                    debug_page.to_str(),
                                                )
                                                .clicked();

                                            if clicked && !debugger.visible(debug_page) {
                                                debugger.toggle(Some(debug_page));
                                                let _ = session
                                                    .last_debug_page
                                                    .insert(emulator_id, debug_page);
                                            }
                                        }
                                    });
                                });
                        }
                    }

                    if toggle_requested {
                        session.toggle_debug_mode();
                    }

                    if session.lua_editor.opened {
                        if let Some(emulator) = &mut session.emulator
                            && let Some(script_engine) = emulator.script_engine()
                        {
                            if let Some(code) = session.lua_editor.show_ui(&egui_ctx) {
                                script_engine.load(code);
                            }
                        }
                    };

                    if let Some(emulator) = &mut session.emulator
                        && let Some(script_engine) = emulator.script_engine()
                    {
                        let lines = script_engine.take_output();
                        if !lines.is_empty() {
                            session.lua_editor.push_output(lines);
                        }

                        for request in script_engine.take_requests() {
                            match request {
                                ScriptRequest::Pause => {
                                    // cant reuse functions for pause and running due to a classic borrow checker no no
                                    session.state = EmulatorState::Paused
                                }
                                ScriptRequest::Screenshot => screenshot(session.image_dir.clone()),
                                ScriptRequest::StartGif => {
                                    if !session.gif.is_recording() {
                                        if emulator.frame_ready() {
                                            let _ = session.gif.start(
                                                emulator.frontend_ref(),
                                                session.image_dir.clone(),
                                            );
                                        } else {
                                            session.lua_editor.push_output(vec![
                                                "frame not ready; gif recording could not start"
                                                    .to_string(),
                                            ]);
                                        }
                                    }
                                }
                                ScriptRequest::StopGif => {
                                    if session.gif.is_recording() {
                                        session.gif.stop();
                                    }
                                }
                                ScriptRequest::Reset => session.state = EmulatorState::Reset,
                                ScriptRequest::StepInstruction | ScriptRequest::StepFrame => {
                                    session.pending_steps.push_back(request)
                                }
                                ScriptRequest::Resume => {
                                    session.state = EmulatorState::Running;
                                }
                            }
                        }

                        // executed outside for loop for one step per next frame await
                        if let Some(request) = session.pending_steps.pop_front() {
                            if session.state != EmulatorState::Paused {
                                session.pending_steps.clear();
                                session.lua_editor.push_output(vec![
                                    "emulator must be paused to step".to_string(),
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
                });

                session.toasts.show(egui_ctx);
            });

            if let Some(emulator) = &mut session.emulator
                && let Some(debugger) = emulator.debugger_mut()
            {
                debugger.show_ui(egui_ctx);
            }
        });

        egui_macroquad::draw();

        next_frame().await;
    }

    Ok(())
}

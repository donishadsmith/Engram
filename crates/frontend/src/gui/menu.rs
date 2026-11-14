use egui::{Color32, Ui};
use egui_phosphor::regular as icons;
use macroquad::{
    miniquad::window::{request_quit, set_fullscreen},
    window::{request_new_screen_size, screen_dpi_scale},
};
use shared::{
    EmulatorId, EmulatorState,
    constants::{FAST_FORWARD, SAVE_STATE_SLOTS},
    input::{
        enums::{Hotkeys, KeyId},
        utils::keycode_to_string,
    },
    utils::screenshot,
};

use crate::{
    session::{InputSource, Session},
    utils::{
        DestructiveAction, UiState, delete_all_slots_with_toast, delete_slots_with_toast,
        load_slots_with_toast, save_slots_with_toast, slot_label, update_controller_hotkey_text,
    },
};

pub fn show_file_menu(ui: &mut Ui, session: &mut Session) {
    ui.menu_button("File", |ui| {
        if ui
            .add_enabled(
                session.rom_dialog.is_none(),
                egui::Button::new(format!("{} Load ROM", icons::FOLDER_OPEN)),
            )
            .clicked()
        {
            session.add_transient_state(EmulatorState::RomSelection);
            ui.close();
        }

        ui.separator();

        if ui.button(format!("{} Quit", icons::SIGN_OUT)).clicked() {
            request_quit();
            ui.close();
        }
    });
}

pub fn show_emulation_menu(ui: &mut Ui, session: &mut Session, ui_state: &mut UiState) {
    let has_emulator = session.emulator.is_some();

    ui.menu_button("Emulation", |ui| {
        if ui
            .add_enabled(
                has_emulator && !session.rom_dialog.is_some(),
                egui::Button::new(format!("{}  Hard Reset", icons::ARROWS_COUNTER_CLOCKWISE)),
            )
            .clicked()
        {
            if session.destructive_action_popup {
                ui_state.destructive_action = Some(DestructiveAction::Reset);
            } else {
                session.add_transient_state(EmulatorState::Reset);
            }

            ui.close();
        }

        ui.horizontal(|ui| {
            ui.add_space(2.0);

            ui.label(
                egui::RichText::new(format!("{}  Show FPS", icons::GAUGE))
                    .color(ui.visuals().widgets.inactive.fg_stroke.color),
            );

            ui.add_space(80.0);

            ui.checkbox(&mut session.show_fps, "");
        });

        ui.add_enabled_ui(has_emulator, |ui| {
            ui.menu_button(format!("{}  Save State", icons::FLOPPY_DISK), |ui| {
                if let Some(emulator) = &mut session.emulator {
                    for slot in 1..=SAVE_STATE_SLOTS {
                        let (state_text, exists) = slot_label(emulator, slot, "Shift");
                        let state_text =
                            if (1..5).contains(&slot) && session.latest_gamepad_id.is_some() {
                                update_controller_hotkey_text(state_text, slot, true)
                            } else {
                                state_text
                            };

                        if ui
                            .add(
                                egui::Button::new(state_text).wrap_mode(egui::TextWrapMode::Extend),
                            )
                            .clicked()
                        {
                            if session.destructive_action_popup && exists {
                                ui_state.destructive_action =
                                    Some(DestructiveAction::Overwrite(slot));
                                ui.close();
                            } else {
                                save_slots_with_toast(emulator, &mut session.toasts, slot);
                            }
                        }
                    }
                }
            });
        });

        ui.add_enabled_ui(has_emulator, |ui| {
            ui.menu_button(format!("{}  Load State", icons::FILE_ARROW_UP), |ui| {
                if let Some(emulator) = &mut session.emulator {
                    for slot in 1..=SAVE_STATE_SLOTS {
                        let (state_text, exists) = slot_label(emulator, slot, "Ctrl");
                        let state_text =
                            if (1..5).contains(&slot) && session.latest_gamepad_id.is_some() {
                                update_controller_hotkey_text(state_text, slot, false)
                            } else {
                                state_text
                            };

                        if ui
                            .add_enabled(
                                exists,
                                egui::Button::new(state_text).wrap_mode(egui::TextWrapMode::Extend),
                            )
                            .clicked()
                        {
                            load_slots_with_toast(emulator, &mut session.toasts, slot);
                        }
                    }
                }
            });
        });

        ui.add_enabled_ui(has_emulator, |ui| {
            ui.menu_button(format!("{}  Delete State", icons::TRASH), |ui| {
                if let Some(emulator) = &mut session.emulator {
                    for slot in 1..=SAVE_STATE_SLOTS {
                        let (state_text, exists) = slot_label(emulator, slot, "Del");
                        if ui
                            .add_enabled(
                                exists,
                                egui::Button::new(state_text).wrap_mode(egui::TextWrapMode::Extend),
                            )
                            .clicked()
                        {
                            if session.destructive_action_popup {
                                ui_state.destructive_action = Some(DestructiveAction::Delete(slot));
                                ui.close();
                            } else {
                                delete_slots_with_toast(emulator, &mut session.toasts, slot);
                            }
                        }
                    }

                    ui.separator();

                    if ui
                        .add_enabled(
                            emulator.current_save_states().iter().any(|x| x.is_some()),
                            egui::Button::new("Delete All States")
                                .wrap_mode(egui::TextWrapMode::Extend),
                        )
                        .highlight()
                        .clicked()
                    {
                        if session.destructive_action_popup {
                            ui_state.destructive_action = Some(DestructiveAction::DeleteAll);
                            ui.close();
                        } else {
                            delete_all_slots_with_toast(emulator, &mut session.toasts);
                        }
                    }
                }
            });
        });

        let solar_sensor = session
            .emulator
            .as_mut()
            .and_then(|emulator| emulator.solar_sensor());
        ui.add_enabled_ui(solar_sensor.is_some(), |ui| {
            ui.menu_button(format!("{}  Solar", icons::SUN), |ui| {
                ui.add(
                    egui::Slider::new(&mut session.solar_level, 0..=10)
                        .text("Solar sensor level from lowest to highest"),
                );

                if let Some(solar_sensor) = solar_sensor {
                    solar_sensor.set_level(session.solar_level);
                }
            })
            .response
        });
    });
}

pub fn show_tools_menu(ui: &mut Ui, session: &mut Session, ui_state: &mut UiState) {
    let dialog_open = session.rom_dialog.is_some();
    ui.menu_button("Tools", |ui| {
        ui.add_enabled_ui(
            session
                .emulator
                .as_mut()
                .map(|emulator| emulator.script_engine())
                .is_some(),
            |ui| {
                let keycode = keycode_to_string(session.key_bindings.get_hotkey_bind(Hotkeys::Lua));

                let text = if session.lua_lab.opened {
                    format!("Close Lua Lab ({})", keycode)
                } else {
                    format!("Open Lua Lab ({})", keycode)
                };

                if ui
                    .add(
                        egui::Button::new(format!("{}  {text}", icons::CODE_SIMPLE))
                            .wrap_mode(egui::TextWrapMode::Extend),
                    )
                    .clicked()
                {
                    session.lua_lab.opened = !session.lua_lab.opened;
                    ui.close();
                }

                let mut binding_text =
                    keycode_to_string(session.key_bindings.get_hotkey_bind(Hotkeys::Screenshot));

                if session.latest_gamepad_id.is_some() {
                    binding_text.push_str("/Select+South");
                }

                let text = format!("{}  Screenshot ({})", icons::CAMERA, binding_text);
                if ui
                    .add(egui::Button::new(text).wrap_mode(egui::TextWrapMode::Extend))
                    .clicked()
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
                }
            },
        );

        let mut binding_text =
            keycode_to_string(session.key_bindings.get_hotkey_bind(Hotkeys::Gif));

        if session.latest_gamepad_id.is_some() {
            binding_text.push_str("/Select+East");
        }

        let text = if session.gif.is_recording() {
            format!("Stop GIF ({})", binding_text)
        } else {
            format!("Record GIF ({})", binding_text)
        };

        if ui
            .add(
                egui::Button::new(format!("{}  {text}", icons::GIF))
                    .wrap_mode(egui::TextWrapMode::Extend),
            )
            .clicked()
        {
            if session.gif.is_recording() {
                let _ = session.gif.stop();
                session.toasts.success("GIF saved");
            } else {
                ui_state.open_gif_settings = true;
            }

            ui.close();
        }

        let mut binding_text =
            keycode_to_string(session.key_bindings.get_hotkey_bind(Hotkeys::Video));

        if session.latest_gamepad_id.is_some() {
            binding_text.push_str("/Select+West");
        }

        let is_video_recording = session
            .emulator
            .as_mut()
            .is_some_and(|emulator| emulator.is_video_recording());
        let text = if is_video_recording {
            format!("Stop Video ({})", binding_text)
        } else {
            format!("Start Video ({})", binding_text)
        };

        if ui
            .add_enabled(
                session.emulator.is_some() && !(dialog_open && !is_video_recording),
                egui::Button::new(format!("{}  {text}", icons::VIDEO_CAMERA))
                    .wrap_mode(egui::TextWrapMode::Extend),
            )
            .clicked()
        {
            if is_video_recording {
                if let Some(emulator) = session.emulator.as_mut() {
                    match emulator.stop_video() {
                        Ok(_) => session.toasts.success("Mp4 saved"),
                        Err(err) => session.toasts.warning(err.to_string()),
                    };
                }
            } else if session.rom_dialog.is_none() {
                match session.data_dirs.get_video_path() {
                    Ok(path) => {
                        session
                            .emulator
                            .as_mut()
                            .map(|emulator| emulator.start_video(path));
                        session.toasts.success("Video started");
                    }
                    Err(err) => {
                        session.toasts.warning(err);
                    }
                }
            }

            ui.close();
        }
    });
}

pub fn show_settings_menu(ui: &mut Ui, session: &mut Session, ui_state: &mut UiState) {
    ui.menu_button("Settings", |ui| {
        ui.menu_button(
            format!("{}  Key/Controller Bindings", icons::KEYBOARD),
            |ui| {
                if ui
                    .add(
                        egui::Button::new("Game Boy/Game Boy Advance")
                            .wrap_mode(egui::TextWrapMode::Extend),
                    )
                    .clicked()
                {
                    session.show_key_bindings = true;
                    ui_state.target_key_id = Some(KeyId::Emulator(EmulatorId::Gba));
                    ui.close();
                }
            },
        );

        if ui
            .add(
                egui::Button::new(format!("{}  Configure Hotkeys", icons::KEYBOARD))
                    .wrap_mode(egui::TextWrapMode::Extend),
            )
            .clicked()
        {
            session.show_hotkeys = true;
            ui_state.target_key_id = Some(KeyId::Hotkeys);
            ui.close();
        }

        if ui
            .add(egui::Button::new(format!("{}  Restore Default Window Size", icons::FRAME_CORNERS))
            .wrap_mode(egui::TextWrapMode::Extend))
            .clicked() {
                set_fullscreen(false);
                let screen_width = 1600.0 / screen_dpi_scale();
                let screen_height = 1200.0 / screen_dpi_scale();
                request_new_screen_size(screen_width, screen_height);
                session.display.fullscreen = false;
            }

        ui.horizontal(|ui| {
            ui.add_space(3.0);

            ui.label(
                egui::RichText::new(format!("{}  Show Notifications", icons::BELL_RINGING))
                    .color(ui.visuals().widgets.inactive.fg_stroke.color),
            )
            .on_hover_text("Enable/disable toast notifications.");

            ui.add_space(45.0);

            ui.checkbox(&mut session.show_notifications, "");
        });

        ui.horizontal(|ui| {
            ui.add_space(2.0);

            ui.label(
                egui::RichText::new(format!("{}  Destructive Action Pop-up", icons::WARNING))
                    .color(ui.visuals().widgets.inactive.fg_stroke.color),
            )
            .on_hover_text(
                "Dialog box pops up whenever hard resetting or overwritting/deleting a save state.",
            );

            ui.add_space(5.0);

            ui.checkbox(&mut session.destructive_action_popup, "");
        });

        if ui
            .add(
                egui::Button::new(format!("{}  Choose Data Locations", icons::FOLDER))
                    .wrap_mode(egui::TextWrapMode::Extend),
            )
            .clicked()
        {
            ui_state.set_dirs = true;
            ui.close();
        }

        ui.separator();

        let source = match session.input_source {
            Some(InputSource::Gamepad) => "Controller",
            Some(InputSource::Keyboard) => "Keyboard",
            None => "Not detected yet",
        };
        ui.label(format!("Input Source: {source}"));

        if let Some(gilrs) = &session.gilrs {
            ui.menu_button(
                format!("{}  Connected Controllers", icons::GAME_CONTROLLER),
                |ui| {
                    let mut controller_present = false;

                    for (gamepad_id, gamepad) in gilrs.gamepads() {
                        controller_present = true;

                        let text = if Some(gamepad_id) == session.latest_gamepad_id
                            && session.input_source == Some(InputSource::Gamepad)
                        {
                            egui::RichText::new(format!("{} (active)", gamepad.os_name())).strong()
                        } else {
                            egui::RichText::new(gamepad.os_name()).weak()
                        };

                        ui.add(egui::Label::new(text).wrap_mode(egui::TextWrapMode::Extend));
                    }

                    if !controller_present {
                        ui.label(egui::RichText::new("None connected").weak());
                    }
                },
            );
        }

        ui.menu_button(format!("{}  Analog Deadzone Radius", icons::TARGET), |ui| {
            ui.add(
                egui::Slider::new(&mut session.analog_deadzone_radius, 0.0..=1.0)
                    .text("Deadzone Radius"),
            ).on_hover_text("Increase to combat infamous joystick drift or sticks that take insanely long to center.");
        });

        ui.menu_button(
            format!("{}  Map Analog to D-Pad", icons::ARROWS_OUT_CARDINAL),
            |ui| {
                ui.checkbox(
                    &mut session.map_analog_to_dpad.gba,
                    "Game Boy/Game Boy Advance",
                )
                .on_hover_text("Register analog inputs as D-pad controls");

                // for psx warn checking this ignores multidirectional support
            },
        );

        ui.menu_button(
            format!("{}  Rumble", icons::VIBRATE),
            |ui| {
                ui.menu_button(
                    "Rumble Effect On",
                    |ui| {
                        ui.checkbox(
                            &mut session.rumble_effect_on.gameboy,
                            "Game Boy",
                        );
                    },
                );

                ui.separator();

                ui.add(
                    egui::Slider::new(&mut session.rumble_strength, 0.0..=1.0)
                        .text("Intensity"),
                ).on_hover_text("Rumble strength multiplier");

            },
        );
    });
}

pub fn show_status_section(ui: &mut Ui, session: &mut Session, ui_state: &mut UiState) {
    let recording = session.gif.is_recording()
        || session
            .emulator
            .as_ref()
            .is_some_and(|emulator| emulator.is_video_recording());
    ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
        // kinda emulating the exit look that a lot of modern apps follow
        if session.display.fullscreen {
            if ui
                .scope(|ui| {
                    let widget = &mut ui.visuals_mut().widgets;
                    widget.hovered.weak_bg_fill = Color32::RED;
                    widget.active.weak_bg_fill = Color32::RED;
                    ui.add(egui::Button::new(egui::RichText::new(icons::X).strong()))
                })
                .inner
                .on_hover_cursor(egui::CursorIcon::PointingHand)
                .clicked()
            {
                request_quit();
            };

            ui.label(egui::RichText::new("| ").strong());
        }

        let hotkey = keycode_to_string(session.key_bindings.get_hotkey_bind(Hotkeys::Fullscreen));
        let (resize_icon, resize_text) = if session.display.fullscreen {
            (icons::ARROWS_IN, format!("Close Fullscreen ({})", hotkey))
        } else {
            (icons::ARROWS_OUT, format!("Fullscreen ({})", hotkey))
        };

        if ui
            .scope(|ui| ui.add(egui::Button::new(egui::RichText::new(resize_icon).strong())))
            .inner
            .on_hover_text(resize_text)
            .clicked()
        {
            session.toggle_fullscreen();
        }

        ui.label(egui::RichText::new("| ").strong());

        if let Some(emulator) = &mut session.emulator
            && let Some(debugger) = emulator.debugger_ref()
        {
            let (text, hover) = if debugger.active() {
                (
                    // dont know what icons should go here
                    egui::RichText::new("ON").color(egui::Color32::LIGHT_GREEN),
                    "Click to close debugger",
                )
            } else {
                (egui::RichText::new("OFF").weak(), "Click to open debugger")
            };

            let state = ui.add(egui::Label::new(text).sense(egui::Sense::click()));
            let title = ui.add(
                egui::Label::new(egui::RichText::new(format!("{}  Debug:", icons::BUG)).strong())
                    .sense(egui::Sense::click()),
            );

            if title
                .union(state)
                .on_hover_text(hover)
                .on_hover_cursor(egui::CursorIcon::PointingHand)
                .clicked()
            {
                ui_state.debug_toggled = true;
            }

            ui.label(egui::RichText::new("| ").strong());
        }

        let rom_name = session
            .emulator
            .as_ref()
            .map(|emulator| emulator.get_rom_name())
            .unwrap_or("".to_string());
        let fps = if session.is_running() {
            session
                .emulator
                .as_ref()
                .map(|emulator| emulator.get_fps())
                .unwrap_or(0.0)
        } else {
            0.0
        };

        let volume_icon = match session.master_volume {
            0 => icons::SPEAKER_X,
            1..=50 => icons::SPEAKER_LOW,
            _ => icons::SPEAKER_HIGH,
        };

        ui.menu_button(
            egui::RichText::new(format!("{volume_icon}")).strong(),
            |ui| {
                ui.add(egui::Slider::new(&mut session.master_volume, 0..=100).text("Volume"));
            },
        );

        ui.label(egui::RichText::new("| ").strong());

        let mut layout_job = egui::text::LayoutJob::default();
        let font = egui::TextStyle::Body.resolve(ui.style());
        layout_job.append(
            &format!("{}  Speed: ", icons::FAST_FORWARD),
            0.0,
            egui::TextFormat::simple(font.clone(), ui.visuals().strong_text_color()),
        );

        layout_job.append(
            &format!("{}x", session.speed),
            0.0,
            egui::TextFormat::simple(font, ui.visuals().text_color()),
        );

        let old_speed = session.speed;
        egui::ComboBox::from_id_salt("Speed")
            .selected_text(layout_job)
            .show_ui(ui, |ui| {
                for speed in 1..=FAST_FORWARD {
                    ui.selectable_value(&mut session.speed, speed, format!("{speed}x"));
                }
            });

        if old_speed != session.speed {
            let message = format!("Emulator set to {}x speed", session.speed);
            session.toasts.warning(message);
        }

        ui.label(egui::RichText::new("| ").strong());

        let (text, hover) = if session.is_paused()
            || session
                .return_state
                .is_some_and(|state| state == EmulatorState::Paused)
            || session.pause_on_launch
        {
            (
                egui::RichText::new(icons::PAUSE).color(egui::Color32::YELLOW),
                "Click to resume emulator",
            )
        } else {
            (
                egui::RichText::new(icons::PLAY).color(egui::Color32::WHITE),
                "Click to pause emulator",
            )
        };

        if ui
            .add(egui::Label::new(text).sense(egui::Sense::click()))
            .on_hover_text(hover)
            .on_hover_cursor(egui::CursorIcon::PointingHand)
            .clicked()
        {
            let mut alt_message = false;
            match (session.state, session.return_state) {
                (EmulatorState::Launch, _)
                | (_, Some(EmulatorState::Launch))
                | (EmulatorState::RomSelection, Some(EmulatorState::SwapDisc))
                | (EmulatorState::SwapDisc, _) => {
                    session.pause_on_launch = !session.pause_on_launch;
                    alt_message = true;
                }
                // a transient state (e.g. the ROM dialog) is active over a loaded game:
                // flip the state we return to and leave the transient state alone
                (_, Some(EmulatorState::Paused)) => {
                    session.swap_return_state(EmulatorState::Running)
                }
                (_, Some(EmulatorState::Running)) => {
                    session.swap_return_state(EmulatorState::Paused)
                }
                _ => session.toggle_emulator_status(),
            }

            let (emulator_pause_message, emulator_run_message) = if alt_message {
                (
                    "Next ROM will pause after loading",
                    "Next ROM will start after loading",
                )
            } else {
                ("Emulator is paused", "Emulator is running")
            };

            let paused = session.is_paused()
                || session.return_state == Some(EmulatorState::Paused)
                || session.pause_on_launch;

            if paused {
                session
                    .emulator
                    .as_mut()
                    .and_then(|emulator| Some(emulator.take_debugger_snapshot()));
                session.toasts.warning(emulator_pause_message);
            } else {
                session.toasts.success(emulator_run_message);
            }
        }

        ui.label(egui::RichText::new("| ").strong());

        if session.emulator.is_some() {
            if session.show_fps {
                ui.label(
                    egui::RichText::new(format!("{}  FPS: {:.1}", icons::GAUGE, fps)).strong(),
                );

                ui.label(egui::RichText::new("| ").strong());
            }

            if recording {
                if ui
                    .add(egui::Label::new(
                        egui::RichText::new(format!("{}  RECORDING", icons::RECORD))
                            .color(egui::Color32::RED),
                    ))
                    .on_hover_cursor(egui::CursorIcon::PointingHand)
                    .clicked()
                {
                    session.gif.stop();
                    session
                        .emulator
                        .as_mut()
                        .map(|emulator| emulator.stop_video());
                    session.toasts.success("All recordings stopped");
                }
            }

            ui.add_sized(
                [ui.available_width(), ui.available_height()],
                egui::Label::new(&rom_name).wrap_mode(egui::TextWrapMode::Truncate),
            );
        }
    });
}

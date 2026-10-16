use egui::Ui;
use egui_phosphor::regular as icons;
use macroquad::{
    miniquad::window::set_fullscreen,
    window::{request_new_screen_size, screen_dpi_scale},
};
use shared::{
    EmulatorId, EmulatorState,
    constants::SAVE_STATE_SLOTS,
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
        load_slots_with_toast, save_slots_with_toast, slot_label,
    },
};

pub fn show_file_menu(ui: &mut Ui, session: &mut Session) {
    ui.menu_button("File", |ui| {
        if ui
            .button(format!("{} Load ROM", icons::FOLDER_OPEN))
            .clicked()
        {
            session.add_transient_state(EmulatorState::RomSelection);
            ui.close_menu();
        }

        if ui.button(format!("{} Quit", icons::SIGN_OUT)).clicked() {
            session.state = EmulatorState::Quit;
            ui.close_menu();
        }
    });
}

pub fn show_emulation_menu(ui: &mut Ui, session: &mut Session, ui_state: &mut UiState) {
    let has_emulator = session.emulator.is_some();
    ui.menu_button("Emulation", |ui| {
        if ui
            .add_enabled(
                has_emulator,
                egui::Button::new(format!("{}  Hard Reset", icons::ARROWS_COUNTER_CLOCKWISE)),
            )
            .clicked()
        {
            if session.destructive_action_popup {
                ui_state.destructive_action = Some(DestructiveAction::Reset);
            } else {
                session.state = EmulatorState::Reset;
            }

            ui.close_menu();
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
                        if ui
                            .add(
                                egui::Button::new(state_text).wrap_mode(egui::TextWrapMode::Extend),
                            )
                            .clicked()
                        {
                            if session.destructive_action_popup && exists {
                                ui_state.destructive_action =
                                    Some(DestructiveAction::Overwrite(slot));
                                ui.close_menu();
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
                                ui.close_menu();
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
                            ui.close_menu();
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
            .on_disabled_hover_text("Available for ROMs with solar sensing (e.g., Boktai)");
        });
    });
}

pub fn show_tools_menu(ui: &mut Ui, session: &mut Session, ui_state: &mut UiState) {
    ui.menu_button("Tools", |ui| {
        ui.add_enabled_ui(
            session
                .emulator
                .as_mut()
                .map(|emulator| emulator.script_engine())
                .is_some(),
            |ui| {
                let keycode = keycode_to_string(session.key_bindings.get_hotkey_bind(Hotkeys::Lua));

                let text = if session.lua_editor.opened {
                    format!("Close Lua Editor ({})", keycode)
                } else {
                    format!("Open Lua Editor ({})", keycode)
                };

                if ui
                    .add(
                        egui::Button::new(format!("{}  {text}", icons::CODE_SIMPLE))
                            .wrap_mode(egui::TextWrapMode::Extend),
                    )
                    .clicked()
                {
                    session.lua_editor.opened = !session.lua_editor.opened;
                    ui.close_menu();
                }
            },
        )
        .response
        .on_disabled_hover_text("Accessible when ROM is loaded");

        let text = format!(
            "{}  Screenshot ({})",
            icons::CAMERA,
            keycode_to_string(session.key_bindings.get_hotkey_bind(Hotkeys::Screenshot))
        );
        if ui
            .add(egui::Button::new(text).wrap_mode(egui::TextWrapMode::Extend))
            .clicked()
        {
            screenshot(session.get_image_path());
            session.toasts.success("Screenshot saved");
            ui.close_menu();
        }

        let keycode = keycode_to_string(session.key_bindings.get_hotkey_bind(Hotkeys::Gif));
        let text = if session.gif.is_recording() {
            format!("Stop GIF ({})", keycode)
        } else {
            format!("Record GIF ({})", keycode)
        };

        if ui
            .add(
                egui::Button::new(format!("{}  {text}", icons::GIF))
                    .wrap_mode(egui::TextWrapMode::Extend),
            )
            .clicked()
        {
            if session.gif.is_recording() {
                if session.emulator.is_some() {
                    let _ = session.gif.stop();
                    // maybe make this more accurate instead of assuming
                    session.toasts.success("GIF saved");
                }
            } else {
                ui_state.open_gif_settings = true;
            }

            ui.close_menu();
        }
    });
}

pub fn show_settings_menu(ui: &mut Ui, session: &mut Session, ui_state: &mut UiState) {
    ui.menu_button("Settings", |ui| {
        ui.menu_button(format!("{}  Volume", icons::SPEAKER_HIGH), |ui| {
            ui.add(
                egui::Slider::new(&mut session.master_volume, 0..=100)
                    .text("Adjust volume for the emulator."),
            );
        });

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
                    ui.close_menu();
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
            ui.close_menu();
        }

        let hotkey = keycode_to_string(session.key_bindings.get_hotkey_bind(Hotkeys::Fullscreen));

        ui.menu_button(format!("{}  Window", icons::FRAME_CORNERS), |ui| {
            let fullscreen_text = if session.display.fullscreen {
                "Close Fullscreen"
            } else {
                "Fullscreen"
            };

            if ui
                .add(
                    egui::Button::new(format!("{} ({})", fullscreen_text, hotkey))
                        .wrap_mode(egui::TextWrapMode::Extend),
                )
                .clicked()
            {
                session.toggle_fullscreen();
            }

            if ui
                .add(
                    egui::Button::new("Restore Default Size").wrap_mode(egui::TextWrapMode::Extend),
                )
                .clicked()
            {
                set_fullscreen(false);
                let screen_width = 1280.0 / screen_dpi_scale();
                let screen_height = 900.0 / screen_dpi_scale();
                request_new_screen_size(screen_width, screen_height);
                session.display.fullscreen = false;
            }
        });

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

            // no dialog box icon :(
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
            ui.close_menu();
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
    let recording = session.gif.is_recording();
    ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
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
                egui::Label::new(
                    egui::RichText::new(format!("| {}  Debug Mode:", icons::BUG)).strong(),
                )
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
        }

        if let Some(emulator) = &session.emulator {
            let fps = if session.is_running() {
                emulator.get_fps()
            } else {
                0.0
            };

            let (text, hover) = if session.is_paused()
                || session
                    .return_state
                    .is_some_and(|state| state == EmulatorState::Paused)
            {
                (
                    egui::RichText::new(icons::PAUSE).color(egui::Color32::YELLOW),
                    "Click to resume emulator",
                )
            } else {
                (
                    egui::RichText::new(icons::PLAY).weak(),
                    "Click to pause emulator",
                )
            };

            let state = ui.add(egui::Label::new(text).sense(egui::Sense::click()));
            let title = ui.add(
                egui::Label::new(
                    egui::RichText::new(format!("| {}  Emulator Status:", icons::MONITOR)).strong(),
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

                if session.is_paused() {
                    session.toasts.warning("Emulator is paused");
                } else {
                    session.toasts.success("Emulator is running");
                }
            }

            if session.show_fps {
                ui.label(
                    egui::RichText::new(format!("| {}  FPS: {:.1}", icons::GAUGE, fps)).strong(),
                );
            }

            if recording {
                ui.label(
                    egui::RichText::new(format!("{}  RECORDING", icons::RECORD))
                        .color(egui::Color32::RED),
                );
            }
        }
    });
}

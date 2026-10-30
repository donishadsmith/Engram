// TODO: I think I can just flash firmware on the pico, wire then map buttons to the gpio pins, then connect via usb
// https://gp2040-ce.info/web-configurator/menu-pages/gpio-pin-mapping
pub mod gui;
pub mod session;
pub mod utils;

use crate::{
    gui::{
        input::{detect_hot_keys, keybinding},
        menu::{
            show_emulation_menu, show_file_menu, show_settings_menu, show_status_section,
            show_tools_menu,
        },
        windows::{
            set_data_directories, show_controller_bindings, show_debugger_pages,
            show_destructive_action_popup, show_gif_settings, show_hotkey_bindings,
            show_lua_editor,
        },
    },
    session::Session,
    utils::{UiState, file_dialog},
};
use egui_macroquad;
use macroquad::{miniquad::conf::Platform, prelude::*};
use shared::{EmulatorState, config::load_config};
use spin_sleep::sleep_until;
use std::{
    io::Error,
    mem::take,
    time::{Duration, Instant},
};

fn conf() -> Conf {
    let display = load_config().display;

    Conf {
        window_title: "Engram".to_string(),
        window_width: display.width.unwrap_or(1280),
        window_height: display.height.unwrap_or(900),
        fullscreen: display.fullscreen,
        high_dpi: true,
        // apparantly vsync is kinda sorta togglelable in miniquad: https://docs.rs/miniquad/0.4.11/miniquad/conf/struct.Platform.html
        platform: Platform {
            swap_interval: Some(0),
            ..Default::default()
        },
        ..Default::default()
    }
}

// TODO: moved contents to several files, clean more later
#[macroquad::main(conf)]
async fn main() -> Result<(), Error> {
    let mut session = Session::new();
    let mut ui_state = UiState::new();

    egui_phosphor::add_to_fonts(&mut ui_state.fonts, egui_phosphor::Variant::Regular);

    loop {
        // i swear my pc wasnt overheating this badly even with vsync off, limiting fixes that
        let fps_limit = Instant::now() + Duration::from_secs_f64(1.0 / 60.0);

        session.save()?;

        if is_quit_requested() {
            session.state = EmulatorState::Quit;
        }

        session.record_window_size();

        session.pause_on_minimize();

        let gamepad_update = session.drain_gamepad_events();

        if let (Some(connected), Some(text)) =
            (gamepad_update.connected, gamepad_update.event_text.as_ref())
        {
            if connected {
                session.toasts.info(text)
            } else {
                session.toasts.warning(text)
            };
        }

        if let Some(text) = gamepad_update.active_text.as_ref() {
            session.toasts.warning(text);
        }

        let input_source_changed = session.detect_input_source(
            gamepad_update.last_pressed,
            ui_state.key_rebinding.is_some(),
        );

        detect_hot_keys(&mut session, &mut ui_state);

        match session.state {
            EmulatorState::RomSelection => {
                let Some(rom_path) = file_dialog() else {
                    session.state = session.take_return_state().unwrap();

                    session.reset_fps();

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
                    _ => {}
                }

                if let Some(state) = session.take_return_state()
                    && state == EmulatorState::Paused
                {
                    session.state = state;
                } else {
                    session.set_running();
                }
            }
            EmulatorState::Running => {
                if session.run(
                    gamepad_update.changed
                        | input_source_changed
                        | take(&mut ui_state.force_clear_inputs),
                )? == EmulatorState::Paused
                {
                    session.set_paused();
                    if !take(&mut session.skip_toast) {
                        session.toasts.info("Emulator is paused");
                    }
                }

                session.apply_rumble();

                if let Some(emulator) = &session.emulator {
                    let frame = emulator.frontend_ref();
                    if emulator.frame_ready() {
                        session.gif.capture(frame);
                    }
                }
            }
            EmulatorState::Reset => {
                let state = if let Some(state) = session.take_return_state()
                    && state == EmulatorState::Paused
                {
                    state
                } else {
                    EmulatorState::Running
                };

                let _ = session.reset();
                session.state = state;

                if !take(&mut session.skip_toast) {
                    session.toasts.success("Emulator reset");
                }
            }
            EmulatorState::Quit => {
                session.gif.stop();
                session.save_configs()?;
                break;
            }
            EmulatorState::Launch => {
                // just so file dialog doesnt occur on launch, since it blocks execution
                // TODO: figure out what i wanna put here, leave blank, render an image for this state, or something else?
                // so far open rom was the thing i came up with so far :(
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
            let menu = egui::TopBottomPanel::top("Menu Bar").show(egui_ctx, |ui| {
                egui_ctx.set_fonts(ui_state.fonts.clone());
                egui::menu::bar(ui, |ui| {
                    show_file_menu(ui, &mut session);
                    show_emulation_menu(ui, &mut session, &mut ui_state);
                    show_tools_menu(ui, &mut session, &mut ui_state);
                    show_settings_menu(ui, &mut session, &mut ui_state);
                    show_status_section(ui, &mut session, &mut ui_state);
                });

                // dont feel like going back and conditionally adding toast messages so clearing at the end will have to do
                if session.show_notifications {
                    session.toasts.show(egui_ctx);
                } else {
                    session.toasts.dismiss_all_toasts();
                }
            });

            if session.state == EmulatorState::Launch {
                egui::CentralPanel::default()
                    .frame(egui::Frame::NONE)
                    .show(egui_ctx, |ui| {
                        let response = ui
                            .allocate_response(ui.available_size(), egui::Sense::click())
                            .on_hover_cursor(egui::CursorIcon::PointingHand);

                        ui.painter().text(
                            response.rect.center(),
                            egui::Align2::CENTER_CENTER,
                            "Open ROM",
                            egui::FontId::proportional(28.0),
                            ui.visuals().text_color(),
                        );

                        if response.clicked() {
                            session.add_transient_state(EmulatorState::RomSelection);
                        }
                    });
            }

            show_controller_bindings(&egui_ctx, &mut session, &mut ui_state);
            show_hotkey_bindings(&egui_ctx, &mut session, &mut ui_state);
            keybinding(&mut session, &mut ui_state, &gamepad_update);

            set_data_directories(&egui_ctx, &mut session, &mut ui_state);
            show_destructive_action_popup(&egui_ctx, &mut session, &mut ui_state);
            show_gif_settings(&egui_ctx, &mut session, &mut ui_state);
            show_lua_editor(&egui_ctx, &mut session);
            show_debugger_pages(&egui_ctx, &mut session, &mut ui_state);

            if let Some(emulator) = session.emulator.as_mut() {
                emulator.set_top_height(menu.response.rect.height());
                if let Some(debugger) = emulator.debugger_mut() {
                    debugger.show_ui(egui_ctx);
                }
            }
        });

        egui_macroquad::draw();

        if session.emulator.is_none() {
            sleep_until(fps_limit);
        }

        next_frame().await;
    }

    Ok(())
}

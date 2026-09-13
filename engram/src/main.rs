use egui_macroquad;
use macroquad::prelude::*;
use rfd::FileDialog;
use shared::{
    EmulatorId, EmulatorSession, EmulatorState,
    debug::DEBUG_PAGES,
    input::{GBA_LABELS, RESERVED_KEYS},
    utils::{GifRecorder, screenshot},
};
use std::{io::Error, path::PathBuf};

fn conf() -> Conf {
    Conf {
        window_title: "Engram".to_string(),
        window_width: 2000,
        window_height: 1400,
        high_dpi: true,
        ..Default::default()
    }
}

fn keycode_to_string(key: KeyCode) -> String {
    format!("{:?}", key)
}

struct Session {
    state: EmulatorState,
    emulator: Option<Box<dyn EmulatorSession>>,
    rom_path: Option<PathBuf>,
    gif: GifRecorder,
    key_bindings: Vec<KeyCode>,
    show_key_bindings: bool,
    open_gif_settings: bool,
}

impl Session {
    fn new() -> Self {
        prevent_quit();

        Self {
            state: EmulatorState::Selection,
            emulator: None,
            rom_path: None,
            gif: GifRecorder::new(),
            key_bindings: Vec::with_capacity(14),
            show_key_bindings: false,
            open_gif_settings: false,
        }
    }

    fn set_emulator<T: EmulatorSession + 'static>(&mut self, emu: T) {
        let old_id = match &self.emulator {
            Some(emu) => Some(emu.id()),
            None => None,
        };

        self.emulator = Some(Box::new(emu));
        let new_id = match &self.emulator {
            Some(emu) => Some(emu.id()),
            None => None,
        };

        // attempt to make this more agostic to future emu additions but make bindings persist across systems with
        // essentially the same keys or a reset
        if old_id.is_some() {
            if !(matches!(old_id, Some(EmulatorId::Gb) | Some(EmulatorId::Gba))
                && matches!(new_id, Some(EmulatorId::Gb) | Some(EmulatorId::Gba)))
            {
                self.key_bindings = Vec::with_capacity(14);
            }
        }

        self.state = EmulatorState::Running;
    }

    fn save(&mut self) -> Result<(), Error> {
        match &mut self.emulator {
            Some(emulator) => emulator.save_game()?,
            None => {}
        }

        Ok(())
    }

    fn run(&mut self) -> Result<EmulatorState, Error> {
        match &mut self.emulator {
            Some(emu) => emu.run(&self.key_bindings, self.show_key_bindings),
            None => Ok(EmulatorState::Selection),
        }
    }

    fn reset(&mut self) -> Result<(), Error> {
        if let Some(emulator) = &mut self.emulator {
            emulator.reset(self.rom_path.clone().unwrap())?
        }

        Ok(())
    }

    fn restore_key_bindings(&mut self) {
        let Some(emu) = &self.emulator else {
            return;
        };

        self.key_bindings = emu.default_keys().to_vec();
    }

    fn set_new_key_bindings(&mut self) {
        if self.key_bindings.len() != 0 {
            return;
        }

        self.restore_key_bindings();
    }
}

fn file_dialog() -> Option<PathBuf> {
    FileDialog::new()
        .set_title("Select a ROM file")
        .add_filter("GameBoy/GBA ROMs", &["gb", "gbc", "gba"])
        .pick_file()
}

#[macroquad::main(conf)]
async fn main() -> Result<(), Error> {
    let mut session = Session::new();
    let mut solar_level: u8 = 0;
    let mut key_rebinding: Option<usize> = None;

    loop {
        session.save()?;

        if is_quit_requested() {
            session.state = EmulatorState::Quit;
        }

        match session.state {
            EmulatorState::Selection => {
                let Some(rom_path) = file_dialog() else {
                    if session.emulator.is_some() {
                        session.state = EmulatorState::Running;
                        continue;
                    }

                    return Ok(());
                };

                session.rom_path = Some(rom_path.clone());

                let ext = rom_path
                    .extension()
                    .and_then(|e| e.to_str())
                    .map(|e| e.to_ascii_lowercase())
                    .unwrap_or_default();

                match ext.as_str() {
                    "gb" | "gbc" => {
                        session.set_emulator(engram_gb::GameBoySession::new_session(rom_path)?)
                    }
                    "gba" => session.set_emulator(engram_gba::GBASession::new_session(rom_path)?),
                    _ => continue,
                }

                session.set_new_key_bindings();
            }
            EmulatorState::Running => {
                session.state = session.run()?;
                screenshot();
                if let Some(emu) = &session.emulator {
                    let frame = emu.reference_frontend();
                    if emu.frame_ready() {
                        session.gif.capture(frame);
                    }
                }
            }
            EmulatorState::Reset => {
                let _ = session.reset();
                session.state = EmulatorState::Running;
            }
            EmulatorState::Quit => {
                break;
            }
        }

        egui_macroquad::ui(|egui_ctx| {
            egui_ctx.set_pixels_per_point(screen_dpi_scale());
            egui::TopBottomPanel::top("Menu Bar").show(egui_ctx, |ui| {
                egui::menu::bar(ui, |ui| {
                    ui.menu_button("File", |ui| {
                        if ui.button("Open ROM").clicked() {
                            session.state = EmulatorState::Selection;
                            ui.close_menu();
                        }

                        if ui.button("Quit").clicked() {
                            session.state = EmulatorState::Quit;
                            ui.close_menu();
                        }
                    });

                    ui.menu_button("Emulation", |ui| {
                        if ui.button("Reset").clicked() {
                            session.state = EmulatorState::Reset;
                            ui.close_menu();
                        }

                        if let Some(emu) = &mut session.emulator {
                            if emu.has_solar() {
                                ui.menu_button("Solar", |ui| {
                                    ui.add(
                                        egui::Slider::new(&mut solar_level, 0..=10)
                                            .text("Solar sensor level from lowest to highest"),
                                    );
                                    emu.solar_level(solar_level);
                                });
                            }
                        }

                        if ui.button("Key Bindings").clicked() {
                            session.show_key_bindings = true;
                            ui.close_menu();
                        }
                    });

                    if let Some(emu) = &mut session.emulator {
                        if matches!(emu.id(), EmulatorId::Gb | EmulatorId::Gba) {
                            egui::Window::new("Key Bindings")
                                .open(&mut session.show_key_bindings)
                                .show(egui_ctx, |ui| {
                                    egui::Grid::new("Key Bindings")
                                        .num_columns(2)
                                        .show(ui, |ui| {
                                            let labels: &[&str] = match emu.id() {
                                                EmulatorId::Gb => &GBA_LABELS[..8],
                                                EmulatorId::Gba => &GBA_LABELS,
                                            };

                                            for (index, (label, key)) in labels
                                                .iter()
                                                .zip(session.key_bindings.iter())
                                                .enumerate()
                                            {
                                                ui.label(*label);
                                                let text = if key_rebinding == Some(index) {
                                                    "".to_string()
                                                } else {
                                                    keycode_to_string(*key)
                                                };

                                                if ui.button(text).clicked() {
                                                    key_rebinding = Some(index);
                                                }

                                                ui.end_row();
                                            }
                                        });
                                });
                        } else {
                            session.show_key_bindings = false;
                        }
                    }

                    if let Some(index) = key_rebinding {
                        if let Some(key) = get_last_key_pressed() {
                            if session.key_bindings[index] == key
                                || !(RESERVED_KEYS.contains(&key)
                                    || session.key_bindings.contains(&key))
                            {
                                session.key_bindings[index] = key;
                                key_rebinding = None;
                            }
                        }
                    }

                    if !session.show_key_bindings {
                        key_rebinding = None;
                    }

                    if let Some(emu) = &mut session.emulator {
                        if emu.has_debug_ui() {
                            ui.menu_button("Debug", |ui| {
                                for debug_page in DEBUG_PAGES {
                                    if ui
                                        .selectable_label(
                                            emu.debug_visible(debug_page),
                                            debug_page.to_str(),
                                        )
                                        .clicked()
                                    {
                                        emu.toggle_debug(debug_page);
                                        ui.close_menu();
                                    }
                                }
                            });
                        }
                    }

                    ui.menu_button("Tools", |ui| {
                        if ui.button("Screenshot  (F7)").clicked() {
                            get_screen_data().export_png("screenshot.png");
                            ui.close_menu();
                        }

                        if let Some(emu) = &session.emulator {
                            if ui
                                .button(if !session.gif.is_recording() {
                                    "Record GIF  (F12)"
                                } else {
                                    "Stop GIF  (F12)"
                                })
                                .clicked()
                            {
                                if session.gif.is_recording() {
                                    let _ = session.gif.toggle(emu.reference_frontend()).unwrap();
                                } else {
                                    session.open_gif_settings = true;
                                }

                                ui.close_menu();
                            }
                        }
                    });

                    if is_key_pressed(KeyCode::F12) {
                        if session.gif.is_recording() {
                            if let Some(emu) = &session.emulator {
                                let _ = session.gif.toggle(emu.reference_frontend()).unwrap();
                            }
                        } else {
                            session.open_gif_settings = !session.open_gif_settings;
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
                                        egui::Slider::new(&mut session.gif.delay, 2..=50)
                                            .text("Frame delay (1/100 s)."),
                                    );

                                    ui.end_row();

                                    if ui.button("Start Recording").clicked() {
                                        if let Some(emu) = &mut session.emulator {
                                            let _ = session
                                                .gif
                                                .toggle(emu.reference_frontend())
                                                .unwrap();

                                            start_recording = true;
                                        }
                                    }
                                });
                        });

                    if start_recording {
                        session.open_gif_settings = false;
                    }

                    if session.gif.is_recording() {
                        ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                            ui.label(egui::RichText::new("RECORDING").color(egui::Color32::RED));
                        });
                    }
                });
            });
            if let Some(emu) = &mut session.emulator {
                emu.debug_ui(egui_ctx);
            }
        });

        egui_macroquad::draw();

        next_frame().await;
    }

    Ok(())
}

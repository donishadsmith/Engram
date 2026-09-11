use egui_macroquad;
use macroquad::prelude::*;
use rfd::FileDialog;
use shared::{
    EmulatorSession, EmulatorState,
    utils::{GifRecorder, screenshot},
};
use std::{io::Error, path::PathBuf};

fn conf() -> Conf {
    Conf {
        window_title: "Engram".to_string(),
        window_width: 1800,
        window_height: 1200,
        high_dpi: true,
        ..Default::default()
    }
}

struct Session {
    state: EmulatorState,
    emulator: Option<Box<dyn EmulatorSession>>,
    rom_path: Option<PathBuf>,
    gif: GifRecorder,
}

impl Session {
    fn new() -> Self {
        Self {
            state: EmulatorState::Selection,
            emulator: None,
            rom_path: None,
            gif: GifRecorder::new(),
        }
    }

    fn set_emulator<T: EmulatorSession + 'static>(&mut self, emu: T) {
        self.emulator = Some(Box::new(emu));
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
            Some(emu) => emu.run(),
            None => Ok(EmulatorState::Selection),
        }
    }

    fn reset(&mut self) -> Result<(), Error> {
        if let Some(emulator) = &mut self.emulator {
            emulator.reset(self.rom_path.clone().unwrap())?
        }

        Ok(())
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

    loop {
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
            }
            EmulatorState::Running => {
                session.state = session.run()?;
                screenshot();
                if let Some(emu) = &session.emulator {
                    let frame = emu.reference_frontend();
                    session.gif.keybind(frame)?;

                    if emu.frame_ready() {
                        session.gif.capture(frame);
                    }
                }
            }
            EmulatorState::Reset => {
                session.save()?;
                let _ = session.reset();
                session.state = EmulatorState::Running;
            }
            EmulatorState::Quit => {
                session.save()?;
                break;
            }
        }

        egui_macroquad::ui(|egui_ctx| {
            egui_ctx.set_pixels_per_point(screen_dpi_scale());
            egui::TopBottomPanel::top("menu_bar").show(egui_ctx, |ui| {
                egui::menu::bar(ui, |ui| {
                    ui.menu_button("File", |ui| {
                        if ui.button("Open ROM").clicked() {
                            session.state = EmulatorState::Selection;
                            ui.close_menu();
                        }

                        if ui.button("Save Game  (F1)").clicked() {
                            let _ = session.save();
                            ui.close_menu();
                        }

                        if ui.button("Quit  (Esc)").clicked() {
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
                    });

                    if let Some(emu) = &mut session.emulator {
                        if emu.has_debug_ui() {
                            ui.menu_button("Debug", |ui| {
                                if ui.button("Audio Debugger  (F12)").clicked() {
                                    emu.toggle_debug();
                                    ui.close_menu();
                                }
                            });
                        }
                    }

                    ui.menu_button("Tools", |ui| {
                        if ui.button("Screenshot  (F2)").clicked() {
                            get_screen_data().export_png("screenshot.png");
                            ui.close_menu();
                        }

                        if let Some(emu) = &session.emulator {
                            let text = if !session.gif.is_recording() {
                                "Record GIF  (F7)"
                            } else {
                                "Stop GIF  (F7)"
                            };

                            if ui.button(text).clicked() {
                                let _ = session.gif.toggle(emu.reference_frontend()).unwrap();
                                ui.close_menu();
                            }
                        }
                    });
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

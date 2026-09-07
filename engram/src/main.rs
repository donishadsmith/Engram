use egui_macroquad;
use macroquad::prelude::*;
use rfd::FileDialog;
use shared::{EmulatorSession, EmulatorState, utils::screenshot};
use std::{io::Error, path::PathBuf};
struct Session {
    state: EmulatorState,
    emulator: Option<Box<dyn EmulatorSession>>,
}

impl Session {
    fn new() -> Self {
        Self {
            state: EmulatorState::Selection,
            emulator: None,
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
}

fn file_dialog() -> Option<PathBuf> {
    FileDialog::new()
        .set_title("Select a ROM file")
        .add_filter("GameBoy/GBA ROMs", &["gb", "gbc", "gba"])
        .pick_file()
}

#[macroquad::main("Engram")]
async fn main() -> Result<(), Error> {
    let mut session = Session::new();

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
            EmulatorState::Running => session.state = session.run()?,
            EmulatorState::Quit => {
                session.save()?;
                break;
            }
        }

        egui_macroquad::ui(|egui_ctx| {
            egui::TopBottomPanel::top("menu_bar").show(egui_ctx, |ui| {
                egui::menu::bar(ui, |ui| {
                    ui.menu_button("File", |ui| {
                        if ui.button("Open ROM").clicked() {
                            session.state = EmulatorState::Selection;
                            ui.close_menu();
                        }

                        if ui.button("Save Game").clicked() {
                            let _ = session.save();
                            ui.close_menu();
                        }

                        if ui.button("Quit").clicked() {
                            session.state = EmulatorState::Quit;
                            ui.close_menu();
                        }
                    });

                    if let Some(emu) = &mut session.emulator {
                        if emu.has_debug_ui() {
                            ui.menu_button("Debug", |ui| {
                                if ui.button("Audio Debugger").clicked() {
                                    emu.toggle_debug();
                                    ui.close_menu();
                                }
                            });
                        }
                    }
                });
            });
            if let Some(emu) = &mut session.emulator {
                emu.debug_ui(egui_ctx);
            }
        });

        egui_macroquad::draw();
        screenshot();

        next_frame().await;
    }

    Ok(())
}

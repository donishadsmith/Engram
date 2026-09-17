use chrono::{DateTime, Local, TimeDelta};
use egui_macroquad;
use macroquad::prelude::*;
use rfd::FileDialog;
use shared::{
    EmulatorId, EmulatorSession, EmulatorState,
    config::{Config, load_config, save_config},
    debug::DEBUG_PAGES,
    keybind::{Hotkeys, KeyBindings, KeyId, keycode_to_string},
    utils::{GifRecorder, screenshot},
};
use std::{collections::VecDeque, fs::create_dir_all, io::Error, path::PathBuf};

fn conf() -> Conf {
    Conf {
        window_title: "Engram".to_string(),
        window_width: 2000,
        window_height: 1400,
        high_dpi: true,
        ..Default::default()
    }
}

struct Session {
    state: EmulatorState,
    emulator: Option<Box<dyn EmulatorSession>>,
    rom_path: Option<PathBuf>,
    gif: GifRecorder,
    image_dir: PathBuf,
    set_image_dir: bool,
    key_bindings: KeyBindings,
    show_key_bindings: bool,
    show_hotkeys: bool,
    open_gif_settings: bool,
    start_time: Option<DateTime<Local>>,
    message_queue: VecDeque<&'static str>,
    master_volume: u8,
    solar_level: u8,
}

impl Session {
    fn new() -> Self {
        prevent_quit();

        let config = load_config();
        let master_volume = config.master_volume.unwrap_or_else(|| 100).min(100);
        let solar_level = config.solar_level;
        let gif_settings = &config.gif_settings;

        Self {
            state: EmulatorState::Launch,
            emulator: None,
            rom_path: None,
            gif: GifRecorder::new(gif_settings),
            key_bindings: KeyBindings::new().load_keys(&config),
            image_dir: PathBuf::from(config.image_dir.unwrap()),
            show_key_bindings: false,
            show_hotkeys: false,
            open_gif_settings: false,
            set_image_dir: false,
            start_time: None,
            message_queue: VecDeque::new(),
            master_volume,
            solar_level,
        }
    }

    fn set_emulator<T: EmulatorSession + 'static>(&mut self, emu: T) {
        self.emulator = Some(Box::new(emu));
        self.state = EmulatorState::Running;
    }

    fn save(&mut self) -> Result<(), Error> {
        if let Some(emulator) = &mut self.emulator {
            emulator.save_game()?;
        }

        Ok(())
    }

    fn run(&mut self) -> Result<EmulatorState, Error> {
        match &mut self.emulator {
            Some(emu) => {
                let key_id = match emu.id() {
                    EmulatorId::Gb | EmulatorId::Gba => KeyId::Gba,
                };

                emu.run(
                    &self.key_bindings.keys(key_id),
                    self.show_key_bindings || self.show_hotkeys,
                    self.master_volume,
                )
            }
            None => Ok(EmulatorState::Selection),
        }
    }

    fn reset(&mut self) -> Result<(), Error> {
        if let Some(emulator) = &mut self.emulator {
            emulator.reset(self.rom_path.clone().unwrap())?;
            emulator.solar_level(self.solar_level);
        }

        Ok(())
    }

    fn save_configs(&self) -> Result<(), Error> {
        let save_keys = self.key_bindings.save_keys()?;

        let config = Config {
            gbakeys: save_keys.gbakeys,
            hotkeys: save_keys.hotkeys,
            image_dir: Some(
                self.image_dir
                    .clone()
                    .into_os_string()
                    .into_string()
                    .unwrap(),
            ),
            solar_level: self.solar_level,
            master_volume: Some(self.master_volume),
            gif_settings: self.gif.settings(),
        };

        save_config(&config)
    }

    // TODO: do better
    fn create_image_path(&self) -> Result<(), Error> {
        create_dir_all(self.image_dir.parent().unwrap())?;

        Ok(())
    }

    fn get_image_path(&self) -> PathBuf {
        let _ = self.create_image_path();
        self.image_dir.clone()
    }

    // unless i can think of a better way only the messages will be a queue
    // unfortunately time will always be the same, technically can extend, to avoid wierd flash messages
    // do fifo, lowkey assumes things were actually saved
    fn display_message(&mut self) -> bool {
        if let Some(time) = &self.start_time {
            if (Local::now() - *time) >= TimeDelta::seconds(3) {
                self.start_time = None;
                self.message_queue.pop_front();

                if !self.message_queue.is_empty() {
                    self.start_time = Some(Local::now());

                    return true;
                } else {
                    return false;
                }
            } else {
                return true;
            }
        }

        false
    }

    fn get_message(&self) -> Option<&'static str> {
        self.message_queue.front().map(|&s| s)
    }

    fn add_message(&mut self, message: &'static str) {
        self.message_queue.push_back(message);
        self.start_time = Some(Local::now());
    }
}

impl Drop for Session {
    fn drop(&mut self) {
        let _ = self.save();
        let _ = self.save_configs();
    }
}

fn file_dialog() -> Option<PathBuf> {
    FileDialog::new()
        .set_title("Select a ROM file")
        .add_filter("ROMs", &["gb", "gbc", "gba"])
        .pick_file()
}

fn bindings_grid(
    ui: &mut egui::Ui,
    grid_id: &str,
    key_bindings: &KeyBindings,
    key_id: KeyId,
    key_rebinding: &mut Option<usize>,
    target_key_id: &mut Option<KeyId>,
    restore_default_bindings: &mut bool,
) {
    egui::Grid::new(grid_id).num_columns(2).show(ui, |ui| {
        for (index, (label, key)) in key_bindings
            .labels(key_id)
            .iter()
            .zip(key_bindings.keys(key_id))
            .enumerate()
        {
            ui.label(*label);

            let text = if *key_rebinding == Some(index) && *target_key_id == Some(key_id) {
                "".to_string()
            } else {
                keycode_to_string(key)
            };

            if ui.button(text).clicked() {
                *key_rebinding = Some(index);
                *target_key_id = Some(key_id);
            }

            ui.end_row();
        }

        if ui.button("Restore Defaults").clicked() {
            *target_key_id = Some(key_id);
            *restore_default_bindings = true;
        }
    });
}

#[macroquad::main(conf)]
async fn main() -> Result<(), Error> {
    let mut session = Session::new();
    let mut key_rebinding: Option<usize> = None;
    let mut target_key_id: Option<KeyId> = None;
    let mut restore_default_bindings = false;

    loop {
        session.save()?;

        if is_quit_requested() {
            session.state = EmulatorState::Quit;
        }

        match session.state {
            EmulatorState::Selection => {
                let Some(rom_path) = file_dialog() else {
                    session.state = if session.emulator.is_some() {
                        EmulatorState::Running
                    } else {
                        EmulatorState::Launch
                    };

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
                        session.set_emulator(engram_gb::GameBoySession::new_session(rom_path)?)
                    }
                    "gba" => session.set_emulator(engram_gba::GBASession::new_session(rom_path)?),
                    _ => continue,
                }

                let emulator = session.emulator.as_mut().unwrap();
                emulator.solar_level(session.solar_level);
            }
            EmulatorState::Running => {
                session.state = session.run()?;
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
                session.gif.stop();
                session.save_configs()?;
                break;
            }
            EmulatorState::Launch => {
                // just so file dialog doesnt occur on launch, since it blocks execution
                // TODO: figure out what i wanna put here, leave blank, render an image for this state, or something else?
            }
        }

        egui_macroquad::ui(|egui_ctx| {
            egui_ctx.set_pixels_per_point(screen_dpi_scale());
            egui::TopBottomPanel::top("Menu Bar").show(egui_ctx, |ui| {
                egui::menu::bar(ui, |ui| {
                    ui.menu_button("File", |ui| {
                        if ui.button("Load ROM").clicked() {
                            session.state = EmulatorState::Selection;
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

                        if let Some(emu) = &mut session.emulator {
                            if ui.button("Reset").clicked() {
                                session.state = EmulatorState::Reset;
                                ui.close_menu();
                            }

                            if emu.has_solar() {
                                ui.menu_button("Solar", |ui| {
                                    ui.add(
                                        egui::Slider::new(&mut session.solar_level, 0..=10)
                                            .text("Solar sensor level from lowest to highest"),
                                    );
                                    emu.solar_level(session.solar_level);
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
                    });

                    if let Some(emu) = &session.emulator {
                        let key_id = match emu.id() {
                            EmulatorId::Gb | EmulatorId::Gba => KeyId::Gba,
                        };

                        egui::Window::new("Controller Bindings")
                            .open(&mut session.show_key_bindings)
                            .show(egui_ctx, |ui| {
                                bindings_grid(
                                    ui,
                                    "Controller Bindings",
                                    &session.key_bindings,
                                    key_id,
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
                                &mut key_rebinding,
                                &mut target_key_id,
                                &mut restore_default_bindings,
                            );
                        });

                    if restore_default_bindings {
                        if let Some(key_id) = target_key_id {
                            let emu_id = session
                                .emulator
                                .as_ref()
                                .map(|emu| emu.id())
                                .unwrap_or(EmulatorId::Gba);
                            session.key_bindings.restore_defaults(key_id, emu_id);
                        }

                        restore_default_bindings = false;
                        target_key_id = None;
                        key_rebinding = None;
                    }

                    if let (Some(index), Some(key_id)) = (key_rebinding, target_key_id) {
                        if let Some(key) = get_last_key_pressed() {
                            let same_key = session.key_bindings.keys(key_id)[index] == key;
                            let taken = session.key_bindings.keys(key_id.reserved()).contains(&key);

                            if same_key || !taken {
                                session.key_bindings.rebind(key_id, index, key);
                                key_rebinding = None;
                                target_key_id = None;
                            }
                        }
                    }

                    if !session.show_key_bindings && !session.show_hotkeys {
                        key_rebinding = None;
                        target_key_id = None;
                    }

                    if let Some(emu) = &mut session.emulator {
                        if emu.has_debug_ui() {
                            ui.menu_button("Debug", |ui| {
                                for debug_page in DEBUG_PAGES {
                                    if !emu.debug_page_available(debug_page) {
                                        continue;
                                    }

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
                        let text = &format!(
                            "Screenshot ({})",
                            keycode_to_string(
                                session.key_bindings.get_hotkey_bind(Hotkeys::Screenshot)
                            )
                        )
                        .to_string();
                        if ui
                            .add(egui::Button::new(text).wrap_mode(egui::TextWrapMode::Extend))
                            .clicked()
                        {
                            screenshot(session.get_image_path());
                            session.add_message("Screenshot saved");
                            ui.close_menu();
                        }

                        let text = if session.gif.is_recording() {
                            &format!(
                                "Stop GIF ({})",
                                keycode_to_string(
                                    session.key_bindings.get_hotkey_bind(Hotkeys::Gif)
                                )
                            )
                            .to_string()
                        } else {
                            &format!(
                                "Record GIF ({})",
                                keycode_to_string(
                                    session.key_bindings.get_hotkey_bind(Hotkeys::Gif)
                                )
                            )
                            .to_string()
                        };

                        if ui
                            .add(egui::Button::new(text).wrap_mode(egui::TextWrapMode::Extend))
                            .clicked()
                        {
                            if session.gif.is_recording() {
                                if let Some(emu) = &session.emulator {
                                    let _ = session
                                        .gif
                                        .toggle(emu.reference_frontend(), session.get_image_path());

                                    session.add_message("GIF saved");
                                }
                            } else {
                                session.open_gif_settings = true;
                            }

                            ui.close_menu();
                        }

                        if ui
                            .add(
                                egui::Button::new("Choose Save Location")
                                    .wrap_mode(egui::TextWrapMode::Extend),
                            )
                            .clicked()
                        {
                            session.set_image_dir = true;
                            ui.close_menu();
                        }
                    });

                    if session.set_image_dir {
                        egui::Window::new("Choose File Location")
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

                    if !session.show_hotkeys {
                        if is_key_pressed(session.key_bindings.get_hotkey_bind(Hotkeys::Screenshot))
                        {
                            session.add_message("Screenshot saved");
                            screenshot(session.get_image_path());
                        }

                        if is_key_pressed(session.key_bindings.get_hotkey_bind(Hotkeys::Gif)) {
                            if session.gif.is_recording() {
                                if let Some(emu) = &session.emulator {
                                    let _ = session
                                        .gif
                                        .toggle(emu.reference_frontend(), session.get_image_path());
                                    session.add_message("GIF saved");
                                }
                            } else {
                                session.open_gif_settings = !session.open_gif_settings;
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
                        if let Some(emu) = &session.emulator {
                            let _ = session
                                .gif
                                .toggle(emu.reference_frontend(), session.get_image_path());
                        }

                        session.open_gif_settings = false;
                    }

                    if session.gif.is_recording() && !session.display_message() {
                        ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                            ui.label(egui::RichText::new("RECORDING").color(egui::Color32::RED));
                        });
                    }

                    if session.display_message() {
                        let message = session.get_message().unwrap();
                        ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                            ui.label(egui::RichText::new(message).color(egui::Color32::WHITE));
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

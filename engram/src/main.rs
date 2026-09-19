use chrono::{DateTime, Local, TimeDelta};
use egui_macroquad;
use macroquad::prelude::*;
use rfd::FileDialog;
use shared::{
    EmulatorId, EmulatorSession, EmulatorState,
    config::{Config, load_config, save_config},
    debug::{DEBUG_PAGES, DebugPage},
    editor::LuaEditor,
    keybind::{Hotkeys, KeyBindings, KeyId, keycode_to_string},
    utils::{GifRecorder, screenshot},
};
use std::{
    collections::{HashMap, VecDeque},
    fs::create_dir_all,
    io::Error,
    path::PathBuf,
};

fn conf() -> Conf {
    Conf {
        window_title: "Engram".to_string(),
        window_width: 2000,
        window_height: 1400,
        high_dpi: true,
        ..Default::default()
    }
}

fn initialize_debug_hashmap() -> HashMap<EmulatorId, DebugPage> {
    let mut map = HashMap::new();

    map.insert(EmulatorId::Gba, DebugPage::Video);

    map
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
    last_debug_page: HashMap<EmulatorId, DebugPage>,
    lua_editor: LuaEditor,
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
            last_debug_page: initialize_debug_hashmap(),
            lua_editor: LuaEditor::new(),
        }
    }

    fn set_emulator<T: EmulatorSession + 'static>(&mut self, emu: T) {
        self.emulator = Some(Box::new(emu));
        self.state = EmulatorState::Running;
    }

    fn debug_active(&self) -> bool {
        self.emulator
            .as_ref()
            .is_some_and(|emu| DEBUG_PAGES.iter().any(|&page| emu.debug_visible(page)))
    }

    fn toggle_debug_mode(&mut self) {
        let active = self.debug_active();
        let Some(emu) = self.emulator.as_mut() else {
            return;
        };

        if !emu.has_debug_ui() {
            return;
        }

        if active {
            emu.toggle_debug(None);

            return;
        }

        let last_debug_page = self.last_debug_page.get(&emu.id()).copied();
        let page = if emu.debug_page_available(last_debug_page) {
            last_debug_page
        } else {
            DEBUG_PAGES
                .iter()
                .copied()
                .find(|&page| emu.debug_page_available(Some(page)))
        };

        if let Some(page) = page {
            emu.toggle_debug(Some(page));
        }
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
                    self.show_key_bindings || self.show_hotkeys || self.lua_editor.occupied(),
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

    // TODO: do better, probably should refactor
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
        .set_title("Select ROM")
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

                    ui.menu_button("Tools", |ui| {
                        if session
                            .emulator
                            .as_mut()
                            .map(|emu| emu.script_engine())
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
                            session.add_message("Screenshot saved");
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
                        if is_key_pressed(session.key_bindings.get_hotkey_bind(Hotkeys::Screenshot))
                        {
                            session.add_message("Screenshot saved");
                            screenshot(session.get_image_path());
                        }

                        if is_key_pressed(session.key_bindings.get_hotkey_bind(Hotkeys::Debugger)) {
                            toggle_requested = true;
                        }

                        if let Some(emu) = &session.emulator {
                            if is_key_pressed(session.key_bindings.get_hotkey_bind(Hotkeys::Lua)) {
                                session.lua_editor.opened = !session.lua_editor.opened;
                            }

                            if is_key_pressed(session.key_bindings.get_hotkey_bind(Hotkeys::Gif)) {
                                if session.gif.is_recording() {
                                    let _ = session
                                        .gif
                                        .toggle(emu.reference_frontend(), session.get_image_path());
                                    session.add_message("GIF saved");
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
                        if let Some(emu) = &session.emulator {
                            let _ = session
                                .gif
                                .toggle(emu.reference_frontend(), session.get_image_path());
                        }

                        session.open_gif_settings = false;
                    }

                    let debug_active = session.debug_active();
                    let has_debug_ui = session
                        .emulator
                        .as_ref()
                        .is_some_and(|emu| emu.has_debug_ui());
                    let show_message = session.display_message();
                    let message = session.get_message();
                    let recording = session.gif.is_recording();

                    ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                        if has_debug_ui {
                            let (text, hover) = if debug_active {
                                (
                                    egui::RichText::new("ON").color(egui::Color32::LIGHT_GREEN),
                                    "Click to close debugger",
                                )
                            } else {
                                (egui::RichText::new("OFF").weak(), "Click to open debugger")
                            };

                            let state = ui.add(egui::Label::new(text).sense(egui::Sense::click()));
                            let title = ui.add(
                                egui::Label::new(egui::RichText::new("Debug Mode:").strong())
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

                        if show_message {
                            if let Some(message) = message {
                                ui.label(egui::RichText::new(message).color(egui::Color32::WHITE));
                            }
                        } else if recording {
                            ui.label(egui::RichText::new("RECORDING").color(egui::Color32::RED));
                        }
                    });

                    if debug_active {
                        if let Some(emu) = &mut session.emulator {
                            egui::Window::new("Debuggers")
                                .collapsible(true)
                                .show(egui_ctx, |ui| {
                                    ui.vertical(|ui| {
                                        for debug_page in DEBUG_PAGES {
                                            if !emu.debug_page_available(Some(debug_page)) {
                                                continue;
                                            }

                                            let clicked = ui
                                                .selectable_label(
                                                    emu.debug_visible(debug_page),
                                                    debug_page.to_str(),
                                                )
                                                .clicked();

                                            if clicked && !emu.debug_visible(debug_page) {
                                                emu.toggle_debug(Some(debug_page));
                                                let _ = session
                                                    .last_debug_page
                                                    .insert(emu.id(), debug_page);
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
                        if let Some(emu) = &mut session.emulator
                            && let Some(script_engine) = emu.script_engine()
                        {
                            if let Some(code) = session.lua_editor.show_ui(&egui_ctx) {
                                script_engine.load(code);
                            }

                            let lines = script_engine.take_output();
                            if !lines.is_empty() {
                                session.lua_editor.push_output(lines);
                            }
                        }
                    };
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

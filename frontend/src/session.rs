use egui_notify::{Anchor, Toasts};
use gilrs::{Button, EventType, GamepadId, Gilrs};
use macroquad::{
    input::prevent_quit,
    miniquad::window::set_fullscreen,
    window::{request_new_screen_size, screen_dpi_scale, screen_height, screen_width},
};
use shared::{
    EmulatorId, EmulatorSession, EmulatorState,
    config::{Config, Display, load_config, save_config},
    debug::DebugPage,
    editor::LuaEditor,
    input::{KeyBindings, utils::get_relevant_key_presses},
    script::ScriptRequest,
    utils::GifRecorder,
};
use std::{
    collections::{HashMap, VecDeque},
    fs::create_dir_all,
    io::Error,
    path::PathBuf,
};

use crate::utils::initialize_debug_hashmap;

#[derive(Default)]
pub struct GamepadUpdate {
    pub last_pressed: Option<Button>,
    pub connected: Option<bool>,
    pub event_text: Option<String>,
    pub active_text: Option<String>,
    pub changed: bool,
}

pub struct Session {
    pub state: EmulatorState,
    pub emulator: Option<Box<dyn EmulatorSession>>,
    pub rom_path: Option<PathBuf>,
    pub gif: GifRecorder,
    pub image_dir: PathBuf,
    pub set_image_dir: bool,
    pub key_bindings: KeyBindings,
    pub show_key_bindings: bool,
    pub show_hotkeys: bool,
    pub open_gif_settings: bool,
    pub master_volume: u8,
    pub solar_level: u8,
    pub last_debug_page: HashMap<EmulatorId, DebugPage>,
    pub lua_editor: LuaEditor,
    pub return_state: Option<EmulatorState>,
    pub pending_steps: VecDeque<ScriptRequest>,
    pub gilrs: Option<Gilrs>,
    pub latest_gamepad_id: Option<GamepadId>,
    pub active_gamepad_changed: bool,
    pub toasts: Toasts, // worth the extra dependency, far more visually appealing than my ugly queue solution
    pub display: Display,
}

impl Session {
    pub fn new() -> Self {
        prevent_quit();

        let config = load_config();
        let master_volume = config.master_volume.unwrap_or_else(|| 100).min(100);
        let solar_level = config.solar_level;
        let gif_settings = &config.gif_settings;
        let mut display = config.display.clone();
        display.width = Some(display.width.unwrap_or(1280));
        display.height = Some(display.height.unwrap_or(900));

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
            master_volume,
            solar_level,
            last_debug_page: initialize_debug_hashmap(),
            lua_editor: LuaEditor::new(),
            return_state: None,
            pending_steps: VecDeque::new(),
            gilrs: Gilrs::new().ok(),
            latest_gamepad_id: None,
            active_gamepad_changed: false,
            toasts: Toasts::default().with_anchor(Anchor::BottomRight),
            display,
        }
    }

    pub fn set_emulator<T: EmulatorSession + 'static>(&mut self, emulator: T) {
        self.emulator = Some(Box::new(emulator));
    }

    pub fn toggle_debug_mode(&mut self) {
        let Some(emulator) = &mut self.emulator else {
            return;
        };

        let emulator_id = emulator.id();

        let Some(debugger) = emulator.debugger_mut() else {
            return;
        };

        if debugger.active() {
            debugger.toggle(None);

            return;
        }

        let debug_page =
            if let Some(last_debug_page) = self.last_debug_page.get(&emulator_id).copied() {
                Some(last_debug_page)
            } else {
                Some(debugger.available_pages()[0])
            };

        debugger.toggle(debug_page);
    }

    pub fn toggle_emulator_status(&mut self) {
        self.state = if self.state == EmulatorState::Running {
            EmulatorState::Paused
        } else {
            EmulatorState::Running
        }
    }

    pub fn save(&mut self) -> Result<(), Error> {
        if let Some(emulator) = &mut self.emulator {
            emulator.save_game()?;
        }

        Ok(())
    }

    pub fn run(&mut self, clear_inputs: bool) -> Result<EmulatorState, Error> {
        let emulator = self.emulator.as_mut().unwrap();

        let key_id = emulator.id().to_key_id().map_to_shared_key_id();

        let index = match emulator.id() {
            EmulatorId::Gb => 8,
            EmulatorId::Gba => 10,
        };

        let gamepad = if let Some(gilrs_instance) = &self.gilrs
            && let Some(gamepad_id) = self.latest_gamepad_id
        {
            Some(gilrs_instance.gamepad(gamepad_id))
        } else {
            None
        };

        let input_blocked =
            self.show_key_bindings || self.show_hotkeys || self.lua_editor.occupied();

        // shocked that i managed to get gamepad working
        let keymap = if gamepad.is_some() {
            self.key_bindings.buttons(key_id)
        } else {
            self.key_bindings.keys(key_id)
        };

        let inputs = if clear_inputs {
            vec![false; index].into_boxed_slice()
        } else {
            get_relevant_key_presses(&keymap[..index], gamepad, input_blocked).into_boxed_slice()
        };
        emulator.run(&inputs, self.master_volume)
    }

    pub fn pause(&mut self) {
        self.emulator.as_mut().unwrap().pause();
    }

    pub fn reset(&mut self) -> Result<(), Error> {
        self.emulator
            .as_mut()
            .unwrap()
            .reset(self.rom_path.clone().unwrap())?;

        self.set_solar_sensor();

        Ok(())
    }

    pub fn set_solar_sensor(&mut self) {
        let emulator = self.emulator.as_mut().unwrap();

        if let Some(solar_sensor) = emulator.solar_sensor() {
            solar_sensor.set_level(self.solar_level);
        }
    }

    pub fn save_configs(&self) -> Result<(), Error> {
        let save_keys = self.key_bindings.save_keys()?;

        let config = Config {
            gba_keyboard: save_keys.gba_keyboard,
            gba_gamepad: save_keys.gba_gamepad,
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
            display: self.display,
        };

        save_config(&config)
    }

    // TODO: do better, probably should refactor
    pub fn create_image_path(&self) -> Result<(), Error> {
        create_dir_all(self.image_dir.parent().unwrap())?;

        Ok(())
    }

    pub fn get_image_path(&self) -> PathBuf {
        let _ = self.create_image_path();
        self.image_dir.clone()
    }

    pub fn set_paused(&mut self) {
        self.state = EmulatorState::Paused;
    }

    pub fn set_running(&mut self) {
        self.state = EmulatorState::Running;
    }

    pub fn is_paused(&self) -> bool {
        self.state == EmulatorState::Paused
    }

    pub fn add_transient_state(&mut self, state: EmulatorState) {
        self.return_state = Some(self.state);
        self.state = state
    }

    pub fn take_return_state(&mut self) -> Option<EmulatorState> {
        self.return_state.take()
    }

    // super extra just to get text for toast and still some annoying edge cases
    pub fn drain_gamepad_events(&mut self) -> GamepadUpdate {
        let mut gamepad_update = GamepadUpdate::default();

        let Some(gilrs) = &mut self.gilrs else {
            return gamepad_update;
        };

        let previous_gamepad_id = self.latest_gamepad_id;

        while let Some(event) = gilrs.next_event() {
            match event.event {
                EventType::Connected => {
                    self.latest_gamepad_id = Some(event.id);
                    gamepad_update.connected = Some(true);
                    gamepad_update.event_text = Some(format!(
                        "Controller connected: {}",
                        gilrs.gamepad(event.id).os_name()
                    ));
                }
                EventType::Disconnected if Some(event.id) == self.latest_gamepad_id => {
                    self.latest_gamepad_id = None;
                    gamepad_update.connected = Some(false);
                    gamepad_update.event_text = Some(format!(
                        "Controller disconnected: {}",
                        gilrs.gamepad(event.id).os_name()
                    ));
                }
                EventType::ButtonPressed(button, _) if button != Button::Unknown => {
                    gamepad_update.last_pressed = Some(button);
                    self.latest_gamepad_id = Some(event.id);
                }
                _ => {}
            }
        }

        if previous_gamepad_id.is_some() && previous_gamepad_id != self.latest_gamepad_id {
            gamepad_update.changed = true;
            gamepad_update.active_text = Some(match self.latest_gamepad_id {
                Some(id) => format!(
                    "Active controller changed to: {}",
                    gilrs.gamepad(id).os_name()
                ),
                None => "No controller input detected, falling back to keyboard".to_string(),
            });
        }

        gamepad_update
    }

    pub fn toggle_fullscreen(&mut self) {
        self.display.fullscreen = !self.display.fullscreen;
        set_fullscreen(self.display.fullscreen);

        if !self.display.fullscreen
            && let (Some(screen_width), Some(screen_height)) =
                (self.display.width, self.display.height)
        {
            let scale = screen_dpi_scale();
            request_new_screen_size(screen_width as f32 / scale, screen_height as f32 / scale);
        }
    }

    pub fn record_window_size(&mut self) {
        if !self.display.fullscreen {
            let scale = screen_dpi_scale();
            let (screen_width, screen_height) = (screen_width() * scale, screen_height() * scale);

            self.display.height = Some(screen_height.round() as i32);
            self.display.width = Some(screen_width.round() as i32);
        }
    }
}

impl Drop for Session {
    fn drop(&mut self) {
        let _ = self.save();
        let _ = self.save_configs();
    }
}

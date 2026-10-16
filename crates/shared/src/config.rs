use dirs::{config_dir, data_dir, picture_dir};
use serde::{Deserialize, Serialize};
use std::{
    collections::BTreeMap,
    fs::{create_dir_all, read_to_string, write},
    io::Error,
    path::PathBuf,
};
use toml::{from_str, to_string_pretty};

use crate::EmulatorId;

#[derive(Default, Serialize, Deserialize, Clone, Copy)]
#[serde(default)]
pub struct Display {
    pub fullscreen: bool,
    pub width: Option<i32>,
    pub height: Option<i32>,
}

#[derive(Serialize, Deserialize, Clone, Copy)]
#[serde(default)]
pub struct AnalogToDpad {
    pub gba: bool,
}

impl AnalogToDpad {
    pub fn get(self, emulator_id: EmulatorId) -> bool {
        match emulator_id {
            EmulatorId::Gb | EmulatorId::Gba => self.gba,
        }
    }
}

impl Default for AnalogToDpad {
    fn default() -> Self {
        AnalogToDpad { gba: true }
    }
}

#[derive(Serialize, Deserialize, Clone, Copy)]
#[serde(default)]
pub struct RumbleEffect {
    pub gameboy: bool,
}

impl RumbleEffect {
    pub fn get(self, emulator_id: EmulatorId) -> bool {
        match emulator_id {
            EmulatorId::Gb => self.gameboy,
            EmulatorId::Gba => false,
        }
    }
}

impl Default for RumbleEffect {
    fn default() -> Self {
        RumbleEffect { gameboy: true }
    }
}

#[derive(Default, Serialize, Deserialize)]
#[serde(default)]
pub struct Config {
    // just to save deterministically since random ordering bugs me
    pub gba_keyboard: BTreeMap<String, String>,
    pub gba_gamepad: BTreeMap<String, String>,
    pub hotkeys: BTreeMap<String, String>,
    pub image_dir: Option<String>,
    pub master_volume: Option<u8>,
    pub solar_level: u8,
    pub gif_settings: BTreeMap<String, u8>,
    pub display: Display,
    pub show_fps: bool,
    pub data_dir: Option<String>,
    pub show_notifications: Option<bool>,
    pub destructive_action_popup: Option<bool>,
    pub analog_deadzone_radius: Option<f32>,
    pub map_analog_to_dpad: AnalogToDpad,
    pub rumble_effect_on: RumbleEffect,
    pub rumble_strength: Option<f32>,
}

fn get_config_path() -> PathBuf {
    config_dir()
        .unwrap_or_else(|| PathBuf::from("."))
        .join("Engram")
        .join("config.toml")
}

fn get_picture_path() -> PathBuf {
    picture_dir().unwrap_or_else(|| PathBuf::from("."))
}

fn get_data_path() -> PathBuf {
    data_dir()
        .unwrap_or_else(|| PathBuf::from("."))
        .join("Engram")
}

pub fn system_dir(system: &str, kind: &str) -> Result<PathBuf, Error> {
    let dir = PathBuf::from(load_config().data_dir.unwrap())
        .join(system)
        .join(kind);

    create_dir_all(&dir)?;

    Ok(dir)
}

pub fn load_config() -> Config {
    //dbg!(get_config_path());
    let mut config: Config = read_to_string(get_config_path())
        .ok()
        .and_then(|text| from_str(&text).ok())
        .unwrap_or_default();

    if config.image_dir.is_none() {
        config.image_dir = Some(get_picture_path().into_os_string().into_string().unwrap())
    }

    if config.data_dir.is_none() {
        config.data_dir = Some(get_data_path().into_os_string().into_string().unwrap())
    }

    config
}

pub fn save_config(config: &Config) -> Result<(), Error> {
    let path = get_config_path();

    create_dir_all(path.parent().unwrap())?;
    let _ = write(path, to_string_pretty(config).map_err(Error::other)?);

    Ok(())
}

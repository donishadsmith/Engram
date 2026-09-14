use dirs::config_dir;
use serde::{Deserialize, Serialize};
use std::{
    collections::BTreeMap,
    fs::{create_dir_all, read_to_string, write},
    io::Error,
    path::PathBuf,
};
use toml::{from_str, to_string_pretty};

#[derive(Default, Serialize, Deserialize)]
pub struct Config {
    // just to save deterministically since random ordering bugs mes
    pub gbakeys: BTreeMap<String, String>,
    pub hotkeys: BTreeMap<String, String>,
}

fn get_config_path() -> PathBuf {
    config_dir()
        .unwrap_or_else(|| PathBuf::from("."))
        .join("engram")
        .join("config.toml")
}

pub fn load_config() -> Config {
    //dbg!(get_config_path());
    read_to_string(get_config_path())
        .ok()
        .and_then(|text| from_str(&text).ok())
        .unwrap_or_default()
}

pub fn save_config(config: &Config) -> Result<(), Error> {
    let path = get_config_path();

    create_dir_all(path.parent().unwrap())?;
    let _ = write(path, to_string_pretty(config).map_err(Error::other)?);

    Ok(())
}

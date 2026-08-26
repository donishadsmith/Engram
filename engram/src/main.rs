use rfd::FileDialog;
use std::{io::Error, path::PathBuf};

pub fn file_dialog() -> Option<PathBuf> {
    FileDialog::new()
        .set_title("Select a ROM file")
        .add_filter("GameBoy/GBA ROMs", &["gb", "gbc", "gba"])
        .pick_file()
}

#[macroquad::main("Engram")]
async fn main() -> Result<(), Error> {
    let Some(rom_path) = file_dialog() else {
        return Ok(());
    };

    let ext = rom_path
        .extension()
        .and_then(|e| e.to_str())
        .map(|e| e.to_ascii_lowercase())
        .unwrap();

    match ext.as_str() {
        "gb" | "gbc" => engram_gb::run(rom_path).await?,
        "gba" => engram_gba::run(rom_path).await?,
        _ => {}
    }

    Ok(())
}

use rfd::FileDialog;
use std::path::PathBuf;

pub enum EmulatorState {
    Active,
    Quit,
    Start,
}

pub fn file_dialog() -> Option<PathBuf> {
    FileDialog::new()
        .set_title("Select a GameBoy ROM file")
        .add_filter("GameBoy Roms", &["gb", "gbc"])
        .pick_file()
}

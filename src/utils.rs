use macroquad::window::next_frame;
use rfd::FileDialog;
use std::{
    path::PathBuf,
    time::{Duration, Instant},
};

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

pub async fn fps_lock(frame_start_time: Instant) {
    let frame_duration = Duration::from_secs_f64(1.0 / 60.0);

    let elapsed_time = frame_start_time.elapsed();
    if elapsed_time < frame_duration {
        spin_sleep::sleep(frame_duration - elapsed_time);
    }

    next_frame().await
}

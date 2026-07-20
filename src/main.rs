/* References:
   - https://gbdev.io/pandocs
   - https://aquova.net/emudev/gb
   - https://github.com/mvdnes/rboy
   - https://github.com/smparsons/retroboy
*/

//#![windows_subsystem = "windows"]

use engram::{
    audio::{AUDIO_BUFFER_CAPACITY, AUDIO_TARGET_OCCUPANCY, AudioOutput},
    components::{gameboy::GameBoy, rom::cartridge::Cartridge},
    render::Screen,
};
use macroquad::prelude::*;
use rfd::FileDialog;
use std::path::PathBuf;

const KEYMAP: [KeyCode; 8] = [
    KeyCode::W,
    KeyCode::A,
    KeyCode::S,
    KeyCode::D,
    KeyCode::K,
    KeyCode::L,
    KeyCode::Enter,
    KeyCode::RightShift,
];

const GB_CLOCK_SPEED: u32 = 4194304;

pub fn file_dialog() -> Option<PathBuf> {
    FileDialog::new()
        .set_title("Select a GameBoy ROM file")
        .add_filter("GameBoy Roms", &["gb", "gbc"])
        .pick_file()
}

#[macroquad::main("Engram")]
async fn main() -> Result<(), std::io::Error> {
    let mut audio = AudioOutput::new();
    let rom = file_dialog();
    let cartridge = Cartridge::load(rom)?;
    let mut gameboy = GameBoy::boot(cartridge);
    let mut screen = Screen::new();

    let cycles_per_sample = GB_CLOCK_SPEED / audio.sample_rate;

    loop {
        if is_key_pressed(KeyCode::Escape) {
            gameboy.battery_save()?;
            break;
        }

        if is_key_pressed(KeyCode::F1) {
            gameboy.battery_save()?;
        }

        let pressed = KEYMAP.map(is_key_down);
        println!("{}", audio.producer.slots());
        while AUDIO_BUFFER_CAPACITY - audio.producer.slots() < AUDIO_TARGET_OCCUPANCY {
            gameboy.run(pressed, cycles_per_sample);
            for sample in gameboy.cpu.bus.memory.apu.sample_buffer.drain(..) {
                let _ = audio.producer.push(sample);
            }
        }

        if gameboy.take_frame() {
            screen.update(&gameboy.cpu.bus.memory.ppu);
        }

        screen.draw();

        if is_key_pressed(KeyCode::O) {
            get_screen_data().export_png("screenshot.png");
        }

        next_frame().await;
    }

    Ok(())
}

/* References:
   - https://gbdev.io/pandocs
   - https://aquova.net/emudev/gb
   - https://github.com/mvdnes/rboy
   - https://github.com/smparsons/retroboy
*/

#![windows_subsystem = "windows"]

use engram::{
    audio::AudioOutput,
    components::{gameboy::GameBoy, rom::cartridge::Cartridge},
    render::Screen,
    utils::{file_dialog, fps_lock},
};
use macroquad::prelude::*;
use std::time::Instant;

const KEYMAP: [KeyCode; 8] = [
    KeyCode::W,
    KeyCode::A,
    KeyCode::S,
    KeyCode::D,
    KeyCode::N,
    KeyCode::M,
    KeyCode::Enter,
    KeyCode::RightShift,
];

#[macroquad::main("Engram")]
async fn main() -> Result<(), std::io::Error> {
    let mut audio = AudioOutput::new();
    let rom = file_dialog();
    let cartridge = Cartridge::load(rom)?;
    let mut gameboy = GameBoy::boot(cartridge);
    let mut screen = Screen::new();

    loop {
        let frame_start_time = Instant::now();

        if is_key_pressed(KeyCode::Escape) {
            gameboy.battery_save()?;
            break;
        }

        if is_key_pressed(KeyCode::F1) {
            gameboy.battery_save()?;
        }

        if is_key_pressed(KeyCode::P) {
            gameboy.ppu_debug_dump();
        }

        let pressed = KEYMAP.map(is_key_down);
        gameboy.run(pressed);

        if gameboy.take_frame() {
            screen.update(&gameboy.cpu.bus.memory.ppu);
        }

        screen.draw();

        if is_key_pressed(KeyCode::O) {
            get_screen_data().export_png("screenshot.png");
        }

        for sample in gameboy.cpu.bus.memory.apu.sample_buffer.drain(..) {
            let _ = audio.producer.push(sample);
        }

        fps_lock(frame_start_time).await;
    }

    Ok(())
}

/* References:
   - https://gbdev.io/pandocs
   - https://aquova.net/emudev/gb
   - https://github.com/mvdnes/rboy
   - https://github.com/smparsons/retroboy
*/

use engram::{
    components::{gameboy::GameBoy, rom::cartridge::Cartridge},
    render::Screen,
    utils::{file_dialog, fps_lock},
};
use macroquad::prelude::*;
use std::time::Instant;

const KEYMAP: [KeyCode; 8] = [
    KeyCode::Up,
    KeyCode::Left,
    KeyCode::Down,
    KeyCode::Right,
    KeyCode::Z,
    KeyCode::X,
    KeyCode::Enter,
    KeyCode::RightShift,
];

#[macroquad::main("Engram")]
async fn main() -> Result<(), std::io::Error> {
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

        if is_key_pressed(KeyCode::O) {
            gameboy.ppu_debug_dump();
        }

        let pressed = KEYMAP.map(is_key_down);
        gameboy.run(pressed);

        if gameboy.take_frame() {
            screen.update(&gameboy.cpu.bus.memory.ppu);
        }

        screen.draw();

        if is_key_pressed(KeyCode::P) {
            get_screen_data().export_png("screenshot.png");
        }

        fps_lock(frame_start_time).await;
    }

    Ok(())
}

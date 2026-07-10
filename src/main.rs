/* References:
   - https://gbdev.io/pandocs
   - https://aquova.net/emudev/gb
   - https://github.com/mvdnes/rboy
   - https://github.com/smparsons/retroboy
*/

use engram::{
    components::{gameboy::GameBoy, rom::cartridge::Cartridge},
    render::render_to_window,
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
    let mut frame = 0;

    loop {
        let frame_start_time = Instant::now();

        if is_key_pressed(KeyCode::Escape) {
            gameboy.battery_save()?;
            break;
        }

        let pressed = KEYMAP.map(is_key_down);
        gameboy.run(pressed);

        render_to_window(&gameboy.cpu.bus.memory.ppu);

        if (frame % 100000 == 0 && gameboy.ram_changed()) || is_key_pressed(KeyCode::S) {
            gameboy.battery_save()?;
            frame = 0;
        }

        frame += 1;

        fps_lock(frame_start_time).await;
    }

    Ok(())
}

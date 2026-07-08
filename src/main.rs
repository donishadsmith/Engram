/* References:
   - https://gbdev.io/pandocs
   - https://aquova.net/emudev/gb
   - https://github.com/mvdnes/rboy
   - https://github.com/smparsons/retroboy
*/

use gameboy_emulator::{
    components::{gameboy::GameBoy, rom::cartridge::Cartridge},
    utils::{file_dialog, fps_lock},
};
use macroquad::prelude::*;
use std::time::Instant;

const KEYMAP: [KeyCode; 8] = [
    KeyCode::W,
    KeyCode::A,
    KeyCode::S,
    KeyCode::D,
    KeyCode::K,
    KeyCode::L,
    KeyCode::I,
    KeyCode::P,
];

#[macroquad::main("GameBoy Emulator")]
async fn main() -> Result<(), std::io::Error> {
    let rom = file_dialog();
    let cartridge = Cartridge::load(rom)?;
    let mut gameboy = GameBoy::boot(cartridge);

    loop {
        let frame_start_time = Instant::now();

        if is_key_pressed(KeyCode::Escape) {
            break;
        }

        let pressed = KEYMAP.map(is_key_down);
        gameboy.run(pressed);

        fps_lock(frame_start_time).await;
    }

    Ok(())
}

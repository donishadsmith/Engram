// https://www.copetti.org/writings/consoles/game-boy-advance/
// https://mgba.io/2015/06/27/cycle-counting-prefetch/
// https://github.com/ioncodes/ayyboy-advance
// https://www.gregorygaines.com/blog/decoding-the-arm7tdmi-instruction-set-game-boy-advance/
// https://ia903206.us.archive.org/34/items/NintendoGbaManualV1.1/Nintendo%20Gba%20Manual%20V1.1.pdf
// https://ww1.microchip.com/downloads/en/DeviceDoc/DDI0029G_7TDMI_R3_trm.pdf
// https://medium.com/@julio.vidaurre/making-a-gba-emulator-fbf91b85979a

#![windows_subsystem = "windows"]

mod components;
mod debug;

use crate::components::{gamepak::GamePak, gba::GBA};
use macroquad::prelude::*;
use shared::{
    audio::AudioOutput,
    input::{GBA_KEYMAP, get_relevant_key_presses},
    render::Screen,
    utils::{quit_emulator, save_progress},
};
use std::{io::Error, path::PathBuf};

pub async fn run(rom_path: PathBuf) -> Result<(), Error> {
    let gamepak = GamePak::load(rom_path)?;
    let mut gba = GBA::boot(gamepak);

    loop {
        if quit_emulator(&gba)? {
            break;
        }

        let _ = save_progress(&gba);

        let keypad: [bool; 10] = get_relevant_key_presses(&GBA_KEYMAP)
            .as_slice()
            .try_into()
            .unwrap();

        gba.run(keypad);

        next_frame().await;
    }

    Ok(())
}

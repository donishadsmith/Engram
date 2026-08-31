// https://www.copetti.org/writings/consoles/game-boy-advance/
// https://mgba.io/2015/06/27/cycle-counting-prefetch/
// https://github.com/ioncodes/ayyboy-advance
// https://www.gregorygaines.com/blog/decoding-the-arm7tdmi-instruction-set-game-boy-advance/
// https://ia903206.us.archive.org/34/items/NintendoGbaManualV1.1/Nintendo%20Gba%20Manual%20V1.1.pdf
// https://ww1.microchip.com/downloads/en/DeviceDoc/DDI0029G_7TDMI_R3_trm.pdf
// https://medium.com/@julio.vidaurre/making-a-gba-emulator-fbf91b85979a

#![windows_subsystem = "windows"]

const CYCLES_PER_FRAME: u64 = 280896;

pub mod components;
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

// **FIX SPEED EVENTUALLY, run on audio when implemented
pub async fn run(rom_path: PathBuf) -> Result<(), Error> {
    let gamepak = GamePak::load(rom_path)?;
    let mut gba = GBA::boot(gamepak);
    let mut screen = Screen::new(gba.bus.ppu.frame.width, gba.bus.ppu.frame.height);

    loop {
        if quit_emulator(&gba)? {
            break;
        }

        if gba.backup_updated() {
            let _ = save_progress(&gba);
        }

        gba.keypad = get_relevant_key_presses(&GBA_KEYMAP)
            .as_slice()
            .try_into()
            .unwrap();

        let frame_deadline = gba.bus.scheduler.current + CYCLES_PER_FRAME;
        while gba.bus.scheduler.current < frame_deadline {
            gba.run();
        }

        if gba.take_frame() {
            screen.update(&gba.bus.ppu.frontend);
        }

        screen.draw(&gba.bus.ppu.frontend);

        next_frame().await;
    }

    Ok(())
}

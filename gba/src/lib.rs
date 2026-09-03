// https://www.copetti.org/writings/consoles/game-boy-advance/
// https://mgba.io/2015/06/27/cycle-counting-prefetch/
// https://github.com/ioncodes/ayyboy-advance
// https://www.gregorygaines.com/blog/decoding-the-arm7tdmi-instruction-set-game-boy-advance/
// https://ia903206.us.archive.org/34/items/NintendoGbaManualV1.1/Nintendo%20Gba%20Manual%20V1.1.pdf
// https://ww1.microchip.com/downloads/en/DeviceDoc/DDI0029G_7TDMI_R3_trm.pdf
// https://medium.com/@julio.vidaurre/making-a-gba-emulator-fbf91b85979a

#![windows_subsystem = "windows"]

mod audio_debugger;
pub mod components;
mod dump;

use crate::components::{gamepak::GamePak, gba::GBA};
use audio_debugger::AudioDebugger;
use macroquad::prelude::*;
use shared::{
    audio::{AUDIO_BUFFER_CAPACITY, AUDIO_TARGET_OCCUPANCY, AudioOutput},
    input::{GBA_KEYMAP, get_relevant_key_presses},
    render::Screen,
    utils::{quit_emulator, save_progress, screenshot},
};
use std::{io::Error, path::PathBuf};

const GBA_CLOCK_SPEED: u32 = 16777216;

pub async fn run(rom_path: PathBuf) -> Result<(), Error> {
    let mut audio_debugger = AudioDebugger::new();
    let mut audio = AudioOutput::new();
    let gamepak = GamePak::load(rom_path)?;
    let apu_sample_cycles = GBA_CLOCK_SPEED / audio.sample_rate;
    let mut gba = GBA::boot(gamepak, apu_sample_cycles);
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

        audio_debugger.turn_on();
        audio_debugger.freeze();

        while AUDIO_BUFFER_CAPACITY - audio.producer.slots() < AUDIO_TARGET_OCCUPANCY {
            gba.run();
            for sample in gba.bus.apu.sample_buffer.drain(..) {
                let _ = audio.producer.push(sample);
            }
        }

        while AUDIO_BUFFER_CAPACITY - audio.producer.slots() < AUDIO_TARGET_OCCUPANCY {
            gba.run();
            for sample in gba.bus.apu.sample_buffer.drain(..) {
                let _ = audio.producer.push(sample);
            }
        }

        if gba.take_frame() {
            screen.update(&gba.bus.ppu.frontend);
        }

        screen.draw(&gba.bus.ppu.frontend);
        audio_debugger.show_ui(&mut gba);
        screenshot();

        next_frame().await;
    }

    Ok(())
}

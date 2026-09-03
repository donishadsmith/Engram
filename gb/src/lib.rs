/* References:
   - https://gbdev.io/pandocs
   - https://aquova.net/emudev/gb
   - https://github.com/mvdnes/rboy
   - https://github.com/smparsons/retroboy
*/

#![windows_subsystem = "windows"]

pub mod components;

use crate::components::{gameboy::GameBoy, gamepak::GamePak};
use macroquad::prelude::*;
use shared::{
    audio::{AUDIO_BUFFER_CAPACITY, AUDIO_TARGET_OCCUPANCY, AudioOutput},
    input::{GB_KEYMAP, get_relevant_key_presses},
    render::Screen,
    utils::{quit_emulator, save_progress, screenshot},
};
use std::path::PathBuf;

const GB_CLOCK_SPEED: u32 = 4194304;

pub async fn run(rom_path: PathBuf) -> Result<(), std::io::Error> {
    let mut audio = AudioOutput::new();
    let gamepak = GamePak::load(rom_path)?;
    let mut gameboy = GameBoy::boot(gamepak);
    let mut screen = Screen::new(
        gameboy.cpu.bus.ppu.frame.width,
        gameboy.cpu.bus.ppu.frame.height,
    );

    let cycles_per_sample = GB_CLOCK_SPEED / audio.sample_rate;

    loop {
        if quit_emulator(&gameboy)? {
            break;
        }

        screenshot();

        if gameboy.ram_changed() {
            let _ = save_progress(&gameboy);
        }

        gameboy.keypad = get_relevant_key_presses(&GB_KEYMAP)
            .as_slice()
            .try_into()
            .unwrap();

        // https://nightshade256.github.io/2021/03/27/gb-sound-emulation.html
        while AUDIO_BUFFER_CAPACITY - audio.producer.slots() < AUDIO_TARGET_OCCUPANCY {
            gameboy.run(cycles_per_sample);
            for sample in gameboy.cpu.bus.apu.sample_buffer.drain(..) {
                let _ = audio.producer.push(sample);
            }
        }

        if gameboy.take_frame() {
            screen.update(&gameboy.cpu.bus.ppu.frame);
        }

        screen.draw(&gameboy.cpu.bus.ppu.frame);

        next_frame().await;
    }

    Ok(())
}

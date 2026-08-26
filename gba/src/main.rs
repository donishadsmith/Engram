// https://www.copetti.org/writings/consoles/game-boy-advance/
// https://mgba.io/2015/06/27/cycle-counting-prefetch/
// https://github.com/ioncodes/ayyboy-advance
// https://www.gregorygaines.com/blog/decoding-the-arm7tdmi-instruction-set-game-boy-advance/
// https://ia903206.us.archive.org/34/items/NintendoGbaManualV1.1/Nintendo%20Gba%20Manual%20V1.1.pdf
// https://ww1.microchip.com/downloads/en/DeviceDoc/DDI0029G_7TDMI_R3_trm.pdf
// https://medium.com/@julio.vidaurre/making-a-gba-emulator-fbf91b85979a

use std::{collections::HashSet, error::Error, path::PathBuf};

use engram_gba::components::gamepak::GamePak;

use macroquad::prelude::*;

const KEYMAP: [KeyCode; 10] = [
    KeyCode::W,
    KeyCode::A,
    KeyCode::S,
    KeyCode::D,
    KeyCode::L,
    KeyCode::K,
    KeyCode::Enter,
    KeyCode::RightShift,
    KeyCode::I,
    KeyCode::O,
];

fn get_relevent_key_presses() -> [bool; 10] {
    let down_keys: HashSet<KeyCode> = get_keys_down();

    let vec_arr: Vec<bool> = KEYMAP.iter().map(|k| down_keys.contains(k)).collect();

    vec_arr.as_slice().try_into().unwrap()
}

fn main() -> Result<(), Box<dyn Error>> {
    let file = Some(PathBuf::from(r""));
    let gamepak = GamePak::load(file).unwrap();
    //hexdump(&gamepak.rom, DumpWidth::Byte)?;

    Ok(())
}

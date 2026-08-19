use engram_gba::components::{cpu::HaltState, gamepak::GamePak, gba::GBA};
use std::path::PathBuf;

fn initialize_gba(rom: &str) -> GBA {
    let path = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("tests/custom_roms")
        .join(rom);

    let gba = GBA::boot(GamePak::load(Some(path)).unwrap());

    gba
}

fn run_instructions(gba: &mut GBA, max_iterations: usize) -> u32 {
    for _ in 0..max_iterations {
        gba.run();

        if let HaltState::TestExit(code) = gba.cpu.halt_state {
            return code;
        }
    }

    panic!("Rom failed to complete in {max_iterations} iterations.");
}

#[test]
fn test_mode_change() {
    let mut gba = initialize_gba("test_mode_change.gba");

    gba.cpu.skip_boot();

    assert_eq!(run_instructions(&mut gba, 1000), 42);
}

#[test]
fn test_state_change() {
    let mut gba = initialize_gba("test_state_change.gba");

    gba.cpu.skip_boot();

    assert_eq!(run_instructions(&mut gba, 1000), 42);
}

#[test]
fn test_single_data_transfer_basic() {
    let mut gba = initialize_gba("test_single_data_transfer_basic.gba");

    gba.cpu.skip_boot();

    assert_eq!(run_instructions(&mut gba, 1000), 42);
}

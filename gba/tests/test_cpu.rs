use engram_gba::components::{gamepak::GamePak, gba::GBA};
use std::path::PathBuf;

fn initialize_gba(rom: &str) -> GBA {
    let path = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("tests/asm")
        .join(rom);

    let gba = GBA::boot(GamePak::load(Some(path)).unwrap());

    gba
}

fn run_arm_only_instructions(gba: &mut GBA) {
    for _ in 0..=((gba.bus.gamepak.rom.len() / 4) + 2) {
        gba.run();
    }
}

#[test]
fn test_mode_change() {
    let mut gba = initialize_gba("test_mode_change.gba");

    gba.cpu.skip_boot();

    run_arm_only_instructions(&mut gba);

    assert_eq!(gba.cpu.registers.r[0], 42);
}

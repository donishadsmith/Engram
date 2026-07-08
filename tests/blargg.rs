use gameboy_emulator::components::{gameboy::GameBoy, rom::cartridge::Cartridge};
use std::path::PathBuf;

fn run_blargg_cpu_rom(rom: &str) {
    let path = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("tests/gb-test-roms/cpu_instrs/individual")
        .join(rom);

    let mut gameboy = GameBoy::boot(Cartridge::load(Some(path)).unwrap());
    gameboy.run([false; 8]);
}

#[test]
fn test_special() {
    run_blargg_cpu_rom("01-special.gb");
}

#[test]
fn test_interrupts() {
    run_blargg_cpu_rom("02-interrupts.gb");
}

#[test]
fn test_op_sp_hl() {
    run_blargg_cpu_rom("03-op sp,hl.gb");
}

#[test]
fn test_instr_timing() {
    let path = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("tests/gb-test-roms/instr_timing/instr_timing.gb");

    let mut gameboy = GameBoy::boot(Cartridge::load(Some(path)).unwrap());
    gameboy.run([false; 8]);
}

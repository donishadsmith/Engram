use engram::components::{gameboy::GameBoy, rom::cartridge::Cartridge};
use std::path::PathBuf;

fn run_blargg_cpu_rom(rom: &str) {
    let path = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("tests/blargg/cpu_instrs/individual")
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
fn test_op_r_imm() {
    run_blargg_cpu_rom("04-op r,imm.gb");
}

#[test]
fn test_op_rp() {
    run_blargg_cpu_rom("05-op rp.gb");
}

#[test]
fn test_ld_r_r() {
    run_blargg_cpu_rom("06-ld r,r.gb");
}

#[test]
fn test_jr_jp_jp_call_ret_rst() {
    run_blargg_cpu_rom("07-jr,jp,call,ret,rst.gb");
}

#[test]
fn test_misc() {
    run_blargg_cpu_rom("08-misc instrs.gb");
}

#[test]
fn test_op_r_r() {
    run_blargg_cpu_rom("09-op r,r.gb");
}

#[test]
fn test_bit_ops() {
    run_blargg_cpu_rom("10-bit ops.gb");
}

#[test]
fn test_op_a_hl() {
    run_blargg_cpu_rom("11-op a,(hl).gb");
}

#[test]
fn test_instr_timing() {
    let path =
        PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("tests/blargg/instr_timing/instr_timing.gb");

    let mut gameboy = GameBoy::boot(Cartridge::load(Some(path)).unwrap());
    gameboy.run([false; 8]);
}

#[test]
fn test_interrupt_timing() {
    let path = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("tests/blargg/interrupt_time/interrupt_time.gb");

    let mut gameboy = GameBoy::boot(Cartridge::load(Some(path)).unwrap());
    gameboy.run([false; 8]);
}

#[test]
fn test_mem_timing() {
    let path =
        PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("tests/blargg/mem_timing/mem_timing.gb");

    let mut gameboy = GameBoy::boot(Cartridge::load(Some(path)).unwrap());
    gameboy.run([false; 8]);
}

#[test]
fn test_mem_timing2() {
    let path =
        PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("tests/blargg/mem_timing-2/mem_timing.gb");

    let mut gameboy = GameBoy::boot(Cartridge::load(Some(path)).unwrap());
    gameboy.run([false; 8]);
}

#[test]
fn test_oam_bug() {
    let path = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("tests/blargg/oam_bug/oam_bug.gb");

    let mut gameboy = GameBoy::boot(Cartridge::load(Some(path)).unwrap());
    gameboy.run([false; 8]);
}

#[test]
fn test_halt_bug() {
    let path = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("tests/blargg/halt_bug.gb");

    let mut gameboy = GameBoy::boot(Cartridge::load(Some(path)).unwrap());
    gameboy.run([false; 8]);
}

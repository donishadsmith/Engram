use engram_gb::components::{gameboy::GameBoy, gamepak::GamePak};
use std::path::PathBuf;

fn blargg_cpu_path(rom: &str) -> PathBuf {
    let path = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("tests/vendored/blargg/cpu_instrs/individual")
        .join(rom);

    path
}

fn run_blargg_rom(path: PathBuf) {
    let mut gameboy = GameBoy::boot(GamePak::load(Some(path.clone())).unwrap());

    for _ in 0..1000 {
        gameboy.run([false; 8], 87);
        gameboy.take_frame();

        let output = &gameboy.cpu.bus.serial_output;

        if output.contains("Passed") {
            return;
        }

        if output.contains("Failed") {
            panic!("{:?} failed. Serial output:\n{output}", path.clone());
        }
    }

    panic!(
        "{:?} timed out. Serial output:\n{}",
        path, gameboy.cpu.bus.serial_output
    );
}

#[test]
fn test_special() {
    run_blargg_rom(blargg_cpu_path("01-special.gb"));
}

#[test]
fn test_interrupts() {
    run_blargg_rom(blargg_cpu_path("02-interrupts.gb"));
}

#[test]
fn test_op_sp_hl() {
    run_blargg_rom(blargg_cpu_path("03-op sp,hl.gb"));
}

#[test]
fn test_op_r_imm() {
    run_blargg_rom(blargg_cpu_path("04-op r,imm.gb"));
}

#[test]
fn test_op_rp() {
    run_blargg_rom(blargg_cpu_path("05-op rp.gb"));
}

#[test]
fn test_ld_r_r() {
    run_blargg_rom(blargg_cpu_path("06-ld r,r.gb"));
}

#[test]
fn test_jr_jp_jp_call_ret_rst() {
    run_blargg_rom(blargg_cpu_path("07-jr,jp,call,ret,rst.gb"));
}

#[test]
fn test_misc() {
    run_blargg_rom(blargg_cpu_path("08-misc instrs.gb"));
}

#[test]
fn test_op_r_r() {
    run_blargg_rom(blargg_cpu_path("09-op r,r.gb"));
}

#[test]
fn test_bit_ops() {
    run_blargg_rom(blargg_cpu_path("10-bit ops.gb"));
}

#[test]
fn test_op_a_hl() {
    run_blargg_rom(blargg_cpu_path("11-op a,(hl).gb"));
}

use engram_gba::components::{bus::AccessType, cpu::HaltState, gamepak::GamePak, gba::GBA};
use std::path::PathBuf;

fn get_custom_rom_path(filename: &str) -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("tests/custom_roms")
        .join(filename)
}

fn get_vendored_path(filename: &str) -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("tests/vendored")
        .join(filename)
}

fn initialize_gba(path: PathBuf) -> GBA {
    GBA::boot(GamePak::load(path).unwrap())
}

fn run_custom_instructions(gba: &mut GBA, max_iterations: usize) -> u32 {
    for _ in 0..max_iterations {
        gba.run();

        if let HaltState::TestExit(code) = gba.cpu.halt_state {
            return code;
        }
    }

    panic!("Rom failed to complete in {max_iterations} iterations.");
}

#[derive(Clone, Copy, PartialEq, Eq)]
enum JsmolkaState {
    Pass,
    Fail(u32),
    Timeout { pc: u32, value: u32 },
}

fn run_vendored_instructions(
    gba: &mut GBA,
    max_iterations: usize,
    _test_name: &str,
    target_register: usize,
) -> JsmolkaState {
    let mut counter = max_iterations;

    loop {
        gba.run();

        counter -= 1;
        if counter == 0 {
            eprintln!("Max iterations ({max_iterations}) reached");

            return JsmolkaState::Timeout {
                pc: gba.cpu.registers.r[15],
                value: gba.cpu.registers.r[target_register],
            };
        }

        if gba.cpu.entered_idle_loop {
            let value = gba.cpu.registers.r[target_register];
            return if value == 0 {
                JsmolkaState::Pass
            } else {
                JsmolkaState::Fail(value)
            };
        }
    }
}

fn check_status(status: JsmolkaState) {
    match status {
        JsmolkaState::Fail(test_id) => panic!("The following test ID failed: {test_id}"),
        JsmolkaState::Pass => {}
        JsmolkaState::Timeout { pc, value } => {
            panic!("The rom failed to complete, pc={pc:08x}, register={value}")
        }
    }
}

#[test]
fn test_mode_change() {
    let mut gba = initialize_gba(get_custom_rom_path("test_mode_change.gba"));

    gba.cpu.skip_boot();

    assert_eq!(run_custom_instructions(&mut gba, 1000), 42);
}

#[test]
fn test_state_change() {
    let mut gba = initialize_gba(get_custom_rom_path("test_state_change.gba"));

    gba.cpu.skip_boot();

    assert_eq!(run_custom_instructions(&mut gba, 1000), 42);
}

#[test]
fn test_single_data_transfer_basic() {
    let mut gba = initialize_gba(get_custom_rom_path("test_single_data_transfer_basic.gba"));

    gba.cpu.skip_boot();

    assert_eq!(run_custom_instructions(&mut gba, 1000), 42);
}

#[test]
fn test_jsmolka_arm_test() {
    let mut gba = initialize_gba(get_vendored_path("jsmolka/arm.gba"));

    gba.cpu.skip_boot();

    let max_iterations = gba.bus.gamepak.rom.len() * 1000;

    check_status(run_vendored_instructions(
        &mut gba,
        max_iterations,
        "arm",
        12,
    ));
}

#[test]
fn test_jsmolka_thumb_test() {
    let mut gba = initialize_gba(get_vendored_path("jsmolka/thumb.gba"));

    gba.cpu.skip_boot();

    let max_iterations = gba.bus.gamepak.rom.len() * 1000;

    check_status(run_vendored_instructions(
        &mut gba,
        max_iterations,
        "thumb",
        7,
    ));
}

#[test]
fn test_jsmolka_memory_test() {
    let mut gba = initialize_gba(get_vendored_path("jsmolka/memory.gba"));

    gba.cpu.skip_boot();

    let max_iterations = gba.bus.gamepak.rom.len() * 1000;

    check_status(run_vendored_instructions(
        &mut gba,
        max_iterations,
        "memory",
        12,
    ));
}

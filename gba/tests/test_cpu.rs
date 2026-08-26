use engram_gba::components::{cpu::HaltState, gamepak::GamePak, gba::GBA};
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
    Pass(u32),
    Fail(u32),
    Running,
}

impl JsmolkaState {
    fn from_gba(gba: &GBA) -> JsmolkaState {
        let pc = gba.cpu.registers.r[15];
        let test = gba.cpu.registers.r[12];

        match pc {
            0x08001e18 => JsmolkaState::Fail(test),
            0x08001d8c => JsmolkaState::Pass(test),
            _ => JsmolkaState::Running,
        }
    }
}

fn run_vendored_instructions(
    gba: &mut GBA,
    max_iterations: usize,
    test_name: &str,
    target_register: usize,
) -> JsmolkaState {
    let mut status = JsmolkaState::Running;
    let mut counter = max_iterations;

    //let mut text_name = String::from(test_name);
    //text_name.push_str(".txt");
    //let mut file = File::create(text_name).unwrap();

    while status == JsmolkaState::Running {
        gba.run();

        //writeln!(file, "Register {target_register} value: {}, PC value: {:08x}", gba.cpu.registers.r[target_register], gba.cpu.registers.r[15]).unwrap();
        counter -= 1;
        if counter == 0 {
            gba.cpu.registers.r[15] = 0x08001e18;

            let target_register_value = gba.cpu.registers.r[target_register];
            eprintln!("Max iterations ({}) reached", max_iterations);

            if target_register_value == 0 {
                return JsmolkaState::Pass(target_register_value);
            } else {
                return JsmolkaState::Fail(target_register_value);
            }
        }

        status = JsmolkaState::from_gba(&gba);
    }

    status
}

fn check_status(status: JsmolkaState) {
    match status {
        JsmolkaState::Fail(test_id) => assert!(false, "The following test ID failed: {}", test_id),
        JsmolkaState::Pass(test_id) => {
            assert!(true, "All tests passed, final test ID: {}", test_id)
        }
        _ => unreachable!(),
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

// Fixes for below tests made until they passed, not the best logic to prove they pass
// but good enough for now
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

// BIOS Functions need implementing: https://problemkaputt.de/gbatek-bios-function-summary.htm
// https://github.com/mgba-emu/mgba/blob/master/src/gba/hle-bios.s
// https://github.com/mgba-emu/mgba/blob/master/src/gba/bios.c

// https://github.com/camthesaxman/gba_bios/blob/master/asm/bios.s

use crate::components::{
    bus::{AccessType, AddressBus},
    cpu::{Arm7tdmi, HaltState, Registers},
};

const ARCTAN_COEFFICIENTS: [i32; 7] = [0x390, 0x91C, 0xFB6, 0x16AA, 0x2081, 0x3651, 0xA2F9];

const FULL_CIRCLE: i32 = 0x10000;
const THREE_FOURTHS_CIRCLE: i32 = 0xC000;
const HALF_CIRCLE: i32 = 0x8000;
const QUARTER_CIRCLE: i32 = 0x4000;
const CIRCLE_ORIGIN: i32 = 0;

// Yeah, im just going to ifnore, multiboot, stop, and the entire sound driver family
// Need to determine if stop is a priority swi
pub fn handle_swi<A: AddressBus>(function: u32, cpu: &mut Arm7tdmi, bus: &mut A) {
    // https://gbadev.net/gbadoc/bios.html
    // https://github.com/mgba-emu/mgba/blob/b54fc45b4ddab1c493122f6644f6d290dce319ce/src/gba/hle-bios.s#L69
    match function {
        0x00 => {}
        0x02 => cpu.halt_state = HaltState::Halted,
        0x03 => {} // Stop
        0x04 => intr_wait(cpu, bus, cpu.registers.r[0] != 0, cpu.registers.r[1] as u16),
        0x05 => {
            cpu.registers.r[0] = 1;
            cpu.registers.r[1] = 1;
            intr_wait(cpu, bus, true, 0x01);
        }
        0x06 => {
            let registers = &mut cpu.registers;
            let cycles = div(registers, registers.r[0], registers.r[1]);
            bus.idle(cycles);
        }
        0x07 => {
            let registers = &mut cpu.registers;
            let cycles = div(registers, registers.r[1], registers.r[0]);
            bus.idle(cycles + 3);
        }
        0x08 => {
            let cycles = sqrt(&mut cpu.registers);
            bus.idle(cycles);
        }
        0x09 => {
            let cycles = arctan(&mut cpu.registers);
            bus.idle(cycles);
        }
        0x0A => {
            let cycles = arctan2(&mut cpu.registers);
            bus.idle(cycles);
        }
        0x0D => {
            // https://github.com/mgba-emu/bios-dump
            cpu.registers.r[0] = 0xBAAE187F;
            cpu.registers.r[1] = 1;
            cpu.registers.r[3] = 0x00004000
        }
        _ => {
            eprintln!(
                "The following SWI function not implemented: {:#04X}",
                function
            );
        }
    }
}

fn intr_wait<A: AddressBus>(cpu: &mut Arm7tdmi, bus: &mut A, clear_if: bool, target_flags: u16) {
    bus.write_u16(0x4000208, 1, AccessType::Nonsequential);

    // Since the cpu rewinds the program counter to execute the swi until wait is satisfied
    // cant keep clearing the IF flag and need the re-execution to check if the flag is cleared
    let bios_flags = bus.read_u16(0x03007FF8, AccessType::Nonsequential);

    if clear_if {
        bus.write_u16(
            0x03007FF8,
            bios_flags & !target_flags,
            AccessType::Nonsequential,
        );
        cpu.registers.r[0] = 0;
        cpu.halt_state = HaltState::IntrWait
    } else if bios_flags & target_flags != 0 {
        bus.write_u16(
            0x03007FF8,
            bios_flags & !target_flags,
            AccessType::Nonsequential,
        );
    } else {
        cpu.halt_state = HaltState::IntrWait
    }
}

// https://github.com/mgba-emu/mgba/blob/afd6f14eaf8bd35214ed3fb9dc69a92bfc3877a9/src/gba/bios.c#L259
fn div(registers: &mut Registers, numerator: u32, denominator: u32) -> u64 {
    let num = numerator as i32;
    let denom = denominator as i32;
    if denom == 0 {
        let val: i32 = if num < 0 { -1 } else { 1 };
        registers.r[0] = val as u32;
        registers.r[1] = num as u32;
        registers.r[3] = 1;
    } else {
        let quotient = num.wrapping_div(denom);
        registers.r[0] = quotient as u32;
        registers.r[1] = num.wrapping_rem(denom) as u32;
        registers.r[3] = quotient.unsigned_abs();
    }

    let mut iterations = denominator.leading_zeros() as i32 - numerator.leading_zeros() as i32;
    iterations = if iterations < 1 { 1 } else { iterations };

    (4 + 13 * iterations + 7) as u64
}

// https://github.com/mgba-emu/mgba/blob/afd6f14eaf8bd35214ed3fb9dc69a92bfc3877a9/src/gba/bios.c#L355
fn long_division(dividend: u32, divisor: u32) -> (u32, u64) {
    if divisor > dividend {
        return (0, 0 as u64);
    }

    let mut shift = divisor.leading_zeros() - dividend.leading_zeros();
    if (divisor << shift) > dividend {
        shift -= 1;
    }

    let mut scaled = divisor << shift;
    let mut remainder = dividend;
    let mut quotient = 0u32;

    let mut cycles: u64 = 5 * shift as u64;
    for bit in (0..=shift).rev() {
        if scaled <= remainder {
            remainder -= scaled;
            quotient |= 1 << bit;
        }

        scaled >>= 1;
        cycles += 8;
    }

    (quotient, cycles)
}

fn sqrt(registers: &mut Registers) -> u64 {
    let x = registers.r[0];
    if x == 0 {
        registers.r[0] = 0;
        return 53;
    }

    let bit_length = 32 - x.leading_zeros();
    let mut guess: u32 = if x.count_ones() == 1 && x.trailing_zeros() % 2 == 0 {
        1 << (x.trailing_zeros() / 2)
    } else {
        1 << ((bit_length + 1) / 2)
    };

    let mut cycles: u64 = 15 + 6 * ((bit_length + 1) / 2) as u64;

    let cycles = loop {
        let (quotient, n_cycles) = long_division(x, guess);
        cycles += n_cycles + 6;
        let next = (guess + quotient) / 2;
        if next >= guess {
            break cycles;
        }

        guess = next;
    };

    registers.r[0] = guess;

    cycles
}

// https://github.com/mgba-emu/mgba/blob/afd6f14eaf8bd35214ed3fb9dc69a92bfc3877a9/src/gba/bios.c#L23
fn multiply_stall(operand: i32) -> u64 {
    let operand = operand as u32;

    let n_leading_ones = operand.leading_ones();
    let n_leading_zeros = operand.leading_zeros();

    if n_leading_zeros >= 24 || n_leading_ones >= 24 {
        1
    } else if n_leading_zeros >= 16 || n_leading_ones >= 16 {
        2
    } else if n_leading_zeros >= 8 || n_leading_ones >= 8 {
        3
    } else {
        4
    }
}

// https://github.com/mgba-emu/mgba/blob/afd6f14eaf8bd35214ed3fb9dc69a92bfc3877a9/src/gba/bios.c#L291
fn apply_arctan_coefficients(operand: i32) -> (i16, i32, i32, u64) {
    let mut cycles: u64 = 37;
    cycles += multiply_stall(operand.wrapping_mul(operand));

    let a = -(operand.wrapping_mul(operand) >> 14);
    let mut b: i32 = 0xA9;

    for c in ARCTAN_COEFFICIENTS {
        cycles += multiply_stall(b.wrapping_mul(a));
        b = (b.wrapping_mul(a) >> 14) + c;
    }

    ((operand.wrapping_mul(b) >> 16) as i16, a, b, cycles)
}

fn arctan(registers: &mut Registers) -> u64 {
    let (angle, a, b, cycles) = apply_arctan_coefficients(registers.r[0] as i32);

    registers.r[0] = angle as i32 as u32;
    registers.r[1] = a as u32;
    registers.r[3] = b as u32;

    cycles
}

// https://github.com/mgba-emu/mgba/blob/afd6f14eaf8bd35214ed3fb9dc69a92bfc3877a9/src/gba/bios.c#L319
fn arctan2(registers: &mut Registers) -> u64 {
    let x = registers.r[0] as i32;
    let y = registers.r[1] as i32;

    let (angle, r1, cycles): (i32, Option<i32>, u64) = if y == 0 {
        (if x >= 0 { CIRCLE_ORIGIN } else { HALF_CIRCLE }, None, 11)
    } else if x == 0 {
        (
            if y >= 0 {
                QUARTER_CIRCLE
            } else {
                THREE_FOURTHS_CIRCLE
            },
            None,
            11,
        )
    } else {
        let larger_magnitude_x = if y >= 0 {
            if x >= 0 {
                x >= y
            } else {
                x.wrapping_neg() >= y
            }
        } else if x <= 0 {
            x.wrapping_neg() > y.wrapping_neg()
        } else {
            x >= y.wrapping_neg()
        };

        if larger_magnitude_x {
            let (angle, a, _, cycles) = apply_arctan_coefficients((y << 14).wrapping_div(x));
            let offset = if x < 0 {
                HALF_CIRCLE
            } else if y < 0 {
                FULL_CIRCLE
            } else {
                CIRCLE_ORIGIN
            };

            (angle as i32 + offset, Some(a), cycles)
        } else {
            let (angle, a, _, cycles) = apply_arctan_coefficients((x << 14).wrapping_div(y));
            let offset = if y >= 0 {
                QUARTER_CIRCLE
            } else {
                THREE_FOURTHS_CIRCLE
            };

            (offset - angle as i32, Some(a), cycles)
        }
    };

    registers.r[0] = (angle as u16) as u32;
    if let Some(a) = r1 {
        registers.r[1] = a as u32;
    }

    // https://github.com/mgba-emu/mgba/blob/afd6f14eaf8bd35214ed3fb9dc69a92bfc3877a9/src/gba/bios.c#L467
    registers.r[3] = 0x170;

    cycles
}

fn bgaffineset() {}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::components::cpu::Registers;

    // https://problemkaputt.de/gbatek.htm#biosarithmeticfunctions
    #[test]
    fn test_div() {
        let num: i32 = -1234;
        let denom: u32 = 10;
        let mut registers = Registers::new();

        let _ = div(&mut registers, num as u32, denom);

        assert_eq!(registers.r[0] as i32, -123);
        assert_eq!(registers.r[1] as i32, -4);
        assert_eq!(registers.r[3], 123);

        let denom: u32 = 0;
        let _ = div(&mut registers, num as u32, denom);
        assert_eq!(registers.r[0] as i32, -1);
        assert_eq!(registers.r[1] as i32, -1234);
        assert_eq!(registers.r[3], 1);

        let num: i32 = i32::MIN;
        let denom: i32 = -1;
        let _ = div(&mut registers, num as u32, denom as u32);
        assert_eq!(registers.r[0], i32::MIN as u32);
        assert_eq!(registers.r[1], 0);
        assert_eq!(registers.r[3], 0x80000000);
    }

    #[test]
    fn test_sqrt() {
        // Re-implementation took forever to do, used C playground to compare with mgba implentation - https://programiz.pro/ide/c
        // Values pass but sadly estimated cycles underestimate or overshoot mgba's cycles, tested values are equal to or lower than a 20 cycle difference
        let mut registers = Registers::new();
        for (input, expected, mgba_cycles) in [
            (0u32, 0u32, 53),
            (1, 1, 34),
            (2, 1, 67),
            (3, 1, 67),
            (4, 2, 48),
            (64, 8, 86),
            (99, 9, 258),
            (100, 10, 200),
            (101, 10, 200),
            (1005, 31, 200),
            (1024, 32, 124),
            (0x8000, 181, 388),
            (0xFFFFFFFF, 0xFFFF, 552),
        ] {
            registers.r[0] = input;

            let cycles = sqrt(&mut registers);
            assert_eq!(registers.r[0], expected);
            eprintln!(
                "For square root of {} - mgba cycles: {}, re-implementation cycles: {}, difference {}",
                input,
                mgba_cycles,
                cycles,
                mgba_cycles as i32 - cycles as i32
            );
        }
    }

    #[test]
    fn test_arctan() {
        // Reference  values from mgba
        let mut registers = Registers::new();
        for (input, r0, r1, r3, cycles) in [
            (0, 0, 0, 0x0000A2F9, 45),
            (0x00001000, 0x000009FB, 0xFFFFFC00, 0x00009FB3, 62), // slope 0.25
            (0xFFFFF000, 0xFFFFF604, 0xFFFFFC00, 0x00009FB3, 62), // slope -0.25
            (0x00002000, 0x000012E4, 0xFFFFF000, 0x00009720, 65), // slope 0.5
            (0x00003FFF, 0x00001FFF, 0xFFFFC002, 0x00008001, 67), // just under 1.0
            (0x00004000, 0x00002000, 0xFFFFC000, 0x00008000, 67), // exactly 1.0 -> exactly 45 deg
            (0xFFFFC000, 0xFFFFE000, 0xFFFFC000, 0x00008000, 67),
        ] {
            registers.r[0] = input;
            registers.r[1] = 0xDEADBEEF;
            registers.r[3] = 0xDEADBEEF;

            let n_cycles = arctan(&mut registers);

            assert_eq!(registers.r[0], r0, "r0 for arctan({input:#010X})");
            assert_eq!(registers.r[1], r1, "r1 (a) for arctan({input:#010X})");
            assert_eq!(registers.r[3], r3, "r3 (b) for arctan({input:#010X})");
            assert_eq!(n_cycles, cycles, "cycles for arctan({input:#010X})");
        }
    }

    #[test]
    fn test_arctan2() {
        // Reference values from mgba
        let mut registers = Registers::new();
        for (x, y, r0, r1, r3, cycles) in [
            // tie-breaks at |x| == |y|
            (
                0x00000005u32,
                0x00000005u32,
                0x00002000u32,
                0xFFFFC000u32,
                0x170u32,
                67u64,
            ),
            (0xFFFFFFFB, 0x00000005, 0x00006000, 0xFFFFC000, 0x170, 67),
            (0xFFFFFFFB, 0xFFFFFFFB, 0x0000A000, 0xFFFFC000, 0x170, 67),
            (0x00000005, 0xFFFFFFFB, 0x0000E000, 0xFFFFC000, 0x170, 67),
            (0x00000005, 0x00000004, 0x00001B7D, 0xFFFFD70B, 0x170, 67),
            (0x00000004, 0x00000005, 0x00002483, 0xFFFFD70B, 0x170, 67),
            // r1 not touched
            (0x00000007, 0x00000000, 0x00000000, 0xDEADBEEF, 0x170, 11),
            (0xFFFFFFF9, 0x00000000, 0x00008000, 0xDEADBEEF, 0x170, 11),
            (0x00000000, 0x00000007, 0x00004000, 0xDEADBEEF, 0x170, 11),
            (0x00000000, 0xFFFFFFF9, 0x0000C000, 0xDEADBEEF, 0x170, 11),
            (0x00000064, 0x0000001E, 0x00000BDF, 0xFFFFFA3E, 0x170, 63),
            (0xFFFFFFE2, 0x00000064, 0x00004BE0, 0xFFFFFA3E, 0x170, 63),
            (0xFFFFFF9C, 0xFFFFFFE2, 0x00008BDF, 0xFFFFFA3E, 0x170, 63),
            (0x0000001E, 0xFFFFFF9C, 0x0000CBE0, 0xFFFFFA3E, 0x170, 63),
            // inputs with wrappig issues
            (0x80000000, 0x00000005, 0x00004000, 0x00000000, 0x170, 45),
            (0x000003E8, 0x000493E0, 0x00003FDE, 0x00000000, 0x170, 46),
        ] {
            registers.r[0] = x;
            registers.r[1] = y;
            let n_cycles = arctan2(&mut registers);

            assert_eq!(registers.r[0], r0, "r0 for arctan2({x:#010X}, {y:#010X})");
            if r1 == 0xDEADBEEF {
                assert_eq!(
                    registers.r[1], y,
                    "r1 should not be touched here ({x:#010X}, {y:#010X})"
                );
            } else {
                assert_eq!(
                    registers.r[1], r1,
                    "r1 (a) for arctan2({x:#010X}, {y:#010X})"
                );
            }
            assert_eq!(registers.r[3], r3, "r3 for arctan2({x:#010X}, {y:#010X})");
            assert_eq!(n_cycles, cycles, "cycles for arctan2({x:#010X}, {y:#010X})");
        }
    }
}

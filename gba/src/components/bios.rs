// BIOS Functions need implementing: https://problemkaputt.de/gbatek-bios-function-summary.htm
// https://github.com/mgba-emu/mgba/blob/master/src/gba/hle-bios.s
// https://github.com/mgba-emu/mgba/blob/master/src/gba/bios.c

use crate::components::{
    bus::AddressBus,
    cpu::{HaltState, Registers},
};

pub fn handle_swi<A: AddressBus>(
    function: u32,
    registers: &mut Registers,
    halt_state: &mut HaltState,
    bus: &mut A,
) {
    // https://gbadev.net/gbadoc/bios.html
    // https://github.com/mgba-emu/mgba/blob/b54fc45b4ddab1c493122f6644f6d290dce319ce/src/gba/hle-bios.s#L69
    match function {
        0x00 => {} // SoftReset
        0x02 => halt(halt_state),
        0x06 => {
            let cycles = div(registers, registers.r[0], registers.r[1]);
            bus.idle(cycles);
        }
        0x07 => {
            let cycles = div(registers, registers.r[1], registers.r[0]);
            bus.idle(cycles + 3);
        }
        0x08 => {
            let cycles = sqrt(registers);
            bus.idle(cycles);
        }
        _ => {
            eprintln!(
                "The following SWI function not implemented: {:#04X}",
                function
            );
        }
    }
}

fn halt(halt_state: &mut HaltState) {
    *halt_state = HaltState::Halted
}

fn instwait() {}

fn vblankintrwait() {}

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

fn arctan() {}

fn arctan2() {}

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
}

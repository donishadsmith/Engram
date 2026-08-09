// BIOS Functions need implementing: https://problemkaputt.de/gbatek-bios-function-summary.htm
// https://github.com/mgba-emu/mgba/blob/master/src/gba/hle-bios.s
// https://github.com/mgba-emu/mgba/blob/master/src/gba/bios.c

// https://github.com/camthesaxman/gba_bios/blob/master/asm/bios.s

use crate::components::{
    bus::{AccessType, AddressBus},
    cpu::{Arm7tdmi, HaltState, Registers},
    utils::BitOps,
};

const ARCTAN_COEFFICIENTS: [i32; 7] = [0x390, 0x91C, 0xFB6, 0x16AA, 0x2081, 0x3651, 0xA2F9];

const FULL_CIRCLE: i32 = 0x10000;
const THREE_FOURTHS_CIRCLE: i32 = 0xC000;
const HALF_CIRCLE: i32 = 0x8000;
const QUARTER_CIRCLE: i32 = 0x4000;
const CIRCLE_ORIGIN: i32 = 0;

#[derive(Clone, Copy, PartialEq, Eq)]
enum BitSize {
    EightBit,
    SixteenBit,
}

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
        0x10 => bit_unpack(&cpu.registers, bus),
        0x16 => diff_unfilter(&cpu.registers, bus, BitSize::EightBit, BitSize::EightBit),
        0x17 => diff_unfilter(&cpu.registers, bus, BitSize::EightBit, BitSize::SixteenBit),
        0x18 => diff_unfilter(
            &cpu.registers,
            bus,
            BitSize::SixteenBit,
            BitSize::SixteenBit,
        ),
        0x0D => {
            // https://github.com/mgba-emu/bios-dump
            // https://problemkaputt.de/gbatek-bios-misc-functions.htm
            cpu.registers.r[0] = 0xBAAE187F;
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

fn bg_affine_set() {}

fn obj_affine_set() {}

struct BitUnpackMetadata {
    source_length: u16,
    source_width: u8,
    destination_width: u8,
    data_offset: u32,
}

impl BitUnpackMetadata {
    fn from_register<A: AddressBus>(pointer: u32, bus: &mut A) -> Self {
        let source_length = bus.read_u16(pointer, AccessType::Nonsequential);
        let source_width = bus.read_u8(pointer + 2, AccessType::Sequential);
        let destination_width = bus.read_u8(pointer + 3, AccessType::Sequential);
        let data_offset = bus.read_u32(pointer + 4, AccessType::Sequential);

        Self {
            source_length,
            source_width,
            destination_width,
            data_offset,
        }
    }

    fn z_set(&self) -> bool {
        self.data_offset.is_set(31)
    }

    fn offset(&self) -> u32 {
        self.data_offset.get_bit_range(0..31)
    }
}

// https://problemkaputt.de/gbatek-bios-decompression-functions.htm
struct BitUnpackPixelData {
    data: u32,
    bits_filled: u8,
}

impl BitUnpackPixelData {
    fn new() -> Self {
        Self {
            data: 0,
            bits_filled: 0,
        }
    }

    fn push(&mut self, pixel: u32, destination_width: u8) -> bool {
        self.data |= pixel << self.bits_filled;
        self.bits_filled += destination_width;

        self.bits_filled >= 32
    }

    fn flush(&mut self) -> u32 {
        let word = self.data;
        self.data = 0;
        self.bits_filled = 0;

        word
    }
}

fn bit_unpack<A: AddressBus>(registers: &Registers, bus: &mut A) {
    let mut source_address = registers.r[0];
    let mut destination_address = registers.r[1];
    let metadata = BitUnpackMetadata::from_register(registers.r[2], bus);

    // mgba logs bad bit width, so doing the same just in case
    debug_assert!(
        matches!(metadata.source_width, 1 | 2 | 4 | 8),
        "invalid BitUnPack source width: {}",
        metadata.source_width
    );

    debug_assert!(
        matches!(metadata.destination_width, 1 | 2 | 4 | 8 | 16 | 32),
        "invalid BitUnPack destination width: {}",
        metadata.destination_width
    );

    let mut bytes_consumed = metadata.source_length;
    let mut buffer = BitUnpackPixelData::new();

    while bytes_consumed != 0 {
        let source_byte = bus.read_u8(source_address, AccessType::Sequential) as u32;
        source_address += 1;

        let n_pixels = 8 / metadata.source_width;
        let source_mask = (1 << metadata.source_width) - 1;
        let destination_mask = ((1u64 << metadata.destination_width) - 1) as u32;

        for i in 0..n_pixels {
            let mut pixel = (source_byte >> (i * metadata.source_width) & source_mask) as u32;

            if pixel != 0 || metadata.z_set() {
                pixel += metadata.offset()
            }

            if buffer.push(pixel & destination_mask, metadata.destination_width) {
                let word = buffer.flush();
                bus.write_u32(destination_address, word, AccessType::Sequential);
                destination_address += 4
            }
        }

        bytes_consumed -= 1;
    }
}

struct DiffMetadata {
    source_address: u32,
    source_data_size: u32,
}

impl DiffMetadata {
    fn from_register<A: AddressBus>(pointer: u32, bus: &mut A) -> Self {
        let data_header = bus.read_u32(pointer, AccessType::Nonsequential);
        let source_data_size = data_header.get_bit_range(8..32);

        Self {
            source_address: pointer,
            source_data_size,
        }
    }

    fn transform_data<A: AddressBus>(&self, bus: &mut A, read_width: BitSize) -> Vec<u8> {
        let mut arr = vec![0u8; self.source_data_size as usize];
        let data_start = self.source_address + 4;

        match read_width {
            BitSize::EightBit => {
                let mut accumulator: u8 = 0;
                for i in 0..self.source_data_size {
                    let byte = bus.read_u8(data_start + i, AccessType::Sequential);
                    accumulator = accumulator.wrapping_add(byte);
                    arr[i as usize] = accumulator;
                }
            }
            BitSize::SixteenBit => {
                let mut accumulator: u16 = 0;
                for i in 0..(self.source_data_size / 2) {
                    let halfword = bus.read_u16(data_start + i * 2, AccessType::Sequential);
                    accumulator = accumulator.wrapping_add(halfword);
                    arr[(i * 2) as usize] = accumulator as u8;
                    arr[(i * 2 + 1) as usize] = (accumulator >> 8) as u8;
                }
            }
        }

        arr
    }
}

fn diff_unfilter<A: AddressBus>(
    registers: &Registers,
    bus: &mut A,
    read_width: BitSize,
    write_width: BitSize,
) {
    let metadata = DiffMetadata::from_register(registers.r[0], bus);
    let destination_address = registers.r[1];

    let data = metadata.transform_data(bus, read_width);

    match write_width {
        BitSize::EightBit => {
            for i in 0..metadata.source_data_size {
                bus.write_u8(
                    destination_address + i,
                    data[i as usize],
                    AccessType::Sequential,
                );
            }
        }
        BitSize::SixteenBit => {
            for i in 0..(metadata.source_data_size / 2) {
                let low_byte = data[(i * 2) as usize] as u16;
                let high_byte = data[((i * 2) + 1) as usize] as u16;
                let halfword = low_byte | high_byte << 8;
                bus.write_u16(
                    destination_address + i * 2,
                    halfword,
                    AccessType::Sequential,
                );
            }
        }
    }
}

fn huff_uncomp<A: AddressBus, Bus>(registers: &Registers, bus: &mut A) {}

fn lz77_uncomp<A: AddressBus>(registers: &Registers, bus: &mut A, write_width: BitSize) {}

fn rl_uncomp<A: AddressBus>(registers: &Registers, bus: &mut A, write_width: BitSize) {}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::components::{bus::Bus, cpu::Registers, gamepak::GamePak};

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

    fn write_metadata(bus: &mut Bus, address: u32, len: u16, src: u8, dst: u8, offset: u32) {
        let index = Bus::iwram_index(address);
        bus.iwram[index] = len.to_le_bytes()[0];
        bus.iwram[index + 1] = len.to_le_bytes()[1];
        bus.iwram[index + 2] = src;
        bus.iwram[index + 3] = dst;
        bus.iwram[index + 4..index + 8].copy_from_slice(&offset.to_le_bytes());
    }

    fn ewram_word(bus: &Bus, index: usize) -> u32 {
        u32::from_le_bytes(bus.ewram[index..index + 4].try_into().unwrap())
    }

    #[test]
    fn test_bit_unpack_offset_skips_zero_pixels() {
        let mut bus = Bus::new(GamePak::mock());
        let mut registers = Registers::new();

        registers.r[0] = 0x08000000; // rom
        registers.r[1] = 0x02000000; // ewram
        registers.r[2] = 0x03000000; // iwram

        // 2 source bytes, 4 -> 8, offset = 1, z clear
        write_metadata(&mut bus, 0x03000000, 2, 4, 8, 1);

        bit_unpack(&registers, &mut bus);

        // pixels: 8+1, 0, 8+1, 0  ->  bytes 09 00 09 00
        assert_eq!(ewram_word(&bus, 0), 0x00090009);
    }

    #[test]
    fn test_bit_unpack_z_flag_offsets_zero_pixels() {
        let mut bus = Bus::new(GamePak::mock());
        let mut registers = Registers::new();

        registers.r[0] = 0x08000000;
        registers.r[1] = 0x02000000;
        registers.r[2] = 0x03000000;

        // 2 source bytes, 4 -> 8, offset = 1, z set
        write_metadata(&mut bus, 0x03000000, 2, 4, 8, 0x80000000 | 1);

        bit_unpack(&registers, &mut bus);

        // pixels - 9, 1, 9, 1  ->  bytes 09 01 09 01
        assert_eq!(ewram_word(&bus, 0), 0x01090109);
    }

    #[test]
    fn test_bit_unpack_1_to_4_from_ram() {
        let mut bus = Bus::new(GamePak::mock());
        let mut registers = Registers::new();

        registers.r[0] = 0x03000100; // iwram
        registers.r[1] = 0x02000000; // ewram
        registers.r[2] = 0x03000000; // iwram

        let source = Bus::iwram_index(0x03000100);
        bus.iwram[source] = 0xA5;
        bus.iwram[source + 1] = 0x00;
        // 2 bytes, 1 -> 4, offset = 5, z clear
        write_metadata(&mut bus, 0x03000000, 2, 1, 4, 5);

        bit_unpack(&registers, &mut bus);

        // 0xA5 - ones become 6, zeros stay 0
        assert_eq!(ewram_word(&bus, 0), 0x60600606);

        // 0x00 - all pixels zero, z clear -> word of zeros is still written
        assert_eq!(ewram_word(&bus, 4), 0x00000000);
    }

    fn write_diff_source(bus: &mut Bus, address: u32, header: u32, data: &[u8]) {
        let index = Bus::iwram_index(address);
        bus.iwram[index..index + 4].copy_from_slice(&header.to_le_bytes());
        bus.iwram[index + 4..index + 4 + data.len()].copy_from_slice(data);
    }

    #[test]
    fn test_diff_unfilter_8bit_read_8bit_write() {
        let mut bus = Bus::new(GamePak::mock());
        let mut registers = Registers::new();

        registers.r[0] = 0x03000000; // iwram
        registers.r[1] = 0x02000000; // ewram

        // original: 10, 11, 10, 11 -> filtered: 10, +1, -1, +1
        let header = 4 << 8;
        write_diff_source(&mut bus, 0x03000000, header, &[10, 1, 0xFF, 1]);

        diff_unfilter(&registers, &mut bus, BitSize::EightBit, BitSize::EightBit);

        assert_eq!(&bus.ewram[0..4], &[10, 11, 10, 11]);
    }

    #[test]
    fn test_diff_unfilter_16bit() {
        let mut bus = Bus::new(GamePak::mock());
        let mut registers = Registers::new();

        registers.r[0] = 0x03000000;
        registers.r[1] = 0x02000000;

        let header = 6 << 8;
        write_diff_source(
            &mut bus,
            0x03000000,
            header,
            &[0xFF, 0x00, 0x01, 0x00, 0xFF, 0xFF],
        );

        diff_unfilter(
            &registers,
            &mut bus,
            BitSize::SixteenBit,
            BitSize::SixteenBit,
        );

        assert_eq!(
            u16::from_le_bytes(bus.ewram[0..2].try_into().unwrap()),
            0x00FF
        );
        assert_eq!(
            u16::from_le_bytes(bus.ewram[2..4].try_into().unwrap()),
            0x0100
        );
        assert_eq!(
            u16::from_le_bytes(bus.ewram[4..6].try_into().unwrap()),
            0x00FF
        );
    }

    #[test]
    fn test_diff_unfilter_8bit_read_16bit_write() {
        let mut bus = Bus::new(GamePak::mock());
        let mut registers = Registers::new();

        registers.r[0] = 0x03000000;
        registers.r[1] = 0x02000000;

        let header = 4 << 8;
        write_diff_source(&mut bus, 0x03000000, header, &[10, 1, 1, 1]);

        diff_unfilter(&registers, &mut bus, BitSize::EightBit, BitSize::SixteenBit);

        assert_eq!(ewram_word(&bus, 0), 0x0D0C0B0A);
    }
}

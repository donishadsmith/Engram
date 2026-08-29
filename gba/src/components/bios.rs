// BIOS Functions need implementing: https://problemkaputt.de/gbatek-bios-function-summary.htm
// https://github.com/mgba-emu/mgba/blob/master/src/gba/hle-bios.s
// https://github.com/mgba-emu/mgba/blob/master/src/gba/bios.c

// https://github.com/camthesaxman/gba_bios/blob/master/asm/bios.s

// *****CHECK THE THREE ADITIONAL IDLE CHARGES IN THE LZ COMPRESSION *******
use crate::components::{
    bus::{AccessType, Bus},
    cpu::{Arm7tdmi, HaltState, Registers},
    utils::BitOps,
};
use std::f32::consts::PI;

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
    ThirtyTwoBit,
}

#[derive(Clone, Copy, PartialEq, Eq)]
enum CpuSetMode {
    CpuSet,
    CpuSetFast,
}

// Yeah, im just going to ifnore, multiboot, stop, and the entire sound driver family
// Need to determine if stop is a priority swi
pub fn handle_swi(function: u32, cpu: &mut Arm7tdmi, bus: &mut Bus) {
    // https://gbadev.net/gbadoc/bios.html
    // https://github.com/mgba-emu/mgba/blob/b54fc45b4ddab1c493122f6644f6d290dce319ce/src/gba/hle-bios.s#L69
    // eprintln!("BIOS CODE CALLED: {}", function);
    match function {
        0x00 => soft_reset(cpu, bus), // https://problemkaputt.de/gbatek-bios-reset-functions.htm
        0x01 => register_ram_reset(cpu, bus),
        0x02 => cpu.halt_state = HaltState::Halted,
        0x04 => intr_wait(cpu, bus, cpu.registers.r[0] != 0, cpu.registers.r[1] as u16),
        0x05 => {
            if !cpu.intr_wait_resume {
                cpu.registers.r[0] = 1;
                cpu.registers.r[1] = 1;
            }

            intr_wait(cpu, bus, cpu.registers.r[0] != 0, 0x01);
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
        0x0B => cpuset(cpu, bus, CpuSetMode::CpuSet),
        0x0C => cpuset(cpu, bus, CpuSetMode::CpuSetFast),
        0x0D => {
            // https://github.com/mgba-emu/bios-dump
            // https://problemkaputt.de/gbatek-bios-misc-functions.htm
            cpu.registers.r[0] = 0xBAAE187F;
        }
        0x0E => bg_affine_set(&cpu.registers, bus),
        0x0F => obj_affine_set(&cpu.registers, bus),
        0x10 => bit_unpack(&cpu.registers, bus),
        0x11 => lz77_uncomp(&cpu.registers, bus, BitSize::EightBit),
        0x12 => lz77_uncomp(&cpu.registers, bus, BitSize::SixteenBit),
        0x13 => huff_uncomp(&cpu.registers, bus),
        0x14 => rl_uncomp(&cpu.registers, bus, BitSize::EightBit),
        0x15 => rl_uncomp(&cpu.registers, bus, BitSize::SixteenBit),
        0x16 => diff_unfilter(&cpu.registers, bus, BitSize::EightBit, BitSize::EightBit),
        0x17 => diff_unfilter(&cpu.registers, bus, BitSize::EightBit, BitSize::SixteenBit),
        0x18 => diff_unfilter(
            &cpu.registers,
            bus,
            BitSize::SixteenBit,
            BitSize::SixteenBit,
        ),
        0x1F => midi_key_2_freq(&mut cpu.registers, bus),
        0xFF => cpu.halt_state = HaltState::TestExit(cpu.registers.r[0]),
        _ => {
            /*eprintln!(
                "The following SWI function not implemented: {:#04X}",
                function
            )*/
        }
    }
}

fn soft_reset(cpu: &mut Arm7tdmi, bus: &mut Bus) {
    let return_flag = bus.read_u8(0x03007FFA, AccessType::Nonsequential);

    for address in 0x03007E00..0x03008000 {
        bus.write_u8(address, 0, AccessType::Sequential);
    }

    let entry_point = if return_flag == 0 {
        0x08000000
    } else {
        0x02000000
    };

    cpu.soft_reset(entry_point);

    bus.last_bios_fetch = 0xE129F000;

    cpu.branch_to(entry_point);
}

// https://github.com/Cult-of-GBA/BIOS/blob/master/bios_calls/register_ram_reset.s
// https://problemkaputt.de/gbatek-bios-reset-functions.htm
fn register_ram_reset(cpu: &mut Arm7tdmi, bus: &mut Bus) {
    let reset_flags = cpu.registers.r[0];

    if reset_flags.is_set(0) {
        bus.ewram.fill(0);
    }

    if reset_flags.is_set(1) {
        bus.iwram.fill(0);
    }

    if reset_flags.is_set(2) {
        bus.ppu.palette_ram.fill(0);
    }

    if reset_flags.is_set(3) {
        bus.ppu.vram.fill(0);
    }

    if reset_flags.is_set(4) {
        bus.ppu.oam.fill(0);
    }

    if reset_flags.is_set(5) {
        bus.serial.reset_sio_registers();
    }

    if reset_flags.is_set(6) {
        bus.apu.reset_registers();
    }

    if reset_flags.is_set(7) {
        bus.write_u16(0x04000200, 0, AccessType::Nonsequential);

        bus.interrupt_flag = 0;

        bus.write_u16(0x04000208, 0, AccessType::Sequential);

        // check for other registers
        bus.keypad.reset();
    }
}

fn intr_wait(cpu: &mut Arm7tdmi, bus: &mut Bus, clear_interrupt_flag: bool, target_flags: u16) {
    bus.write_u16(0x4000208, 1, AccessType::Nonsequential);

    // Since the cpu rewinds the program counter to execute the swi until wait is satisfied
    // cant keep clearing the IF flag and need the re-execution to check if the flag is cleared
    let bios_flags = bus.read_u16(0x03007FF8, AccessType::Nonsequential);

    if clear_interrupt_flag {
        bus.write_u16(
            0x03007FF8,
            bios_flags & !target_flags,
            AccessType::Nonsequential,
        );
        cpu.registers.r[0] = 0;
        cpu.intr_wait_resume = true;
        cpu.halt_state = HaltState::IntrWait
    } else if bios_flags & target_flags != 0 {
        bus.write_u16(
            0x03007FF8,
            bios_flags & !target_flags,
            AccessType::Nonsequential,
        );
        cpu.intr_wait_resume = false;
    } else {
        cpu.intr_wait_resume = true;
        cpu.halt_state = HaltState::IntrWait
    }
}

fn cpuset(cpu: &Arm7tdmi, bus: &mut Bus, cpu_mode: CpuSetMode) {
    let mut source_address = cpu.registers.r[0];
    let mut destination_address = cpu.registers.r[1];
    let metadata = cpu.registers.r[2];

    let mut remaining_count = metadata.get_bit_range(0..21);
    let fixed_source_address = metadata.get_bit(24);

    let bit_mode = match cpu_mode {
        CpuSetMode::CpuSetFast => BitSize::ThirtyTwoBit,
        CpuSetMode::CpuSet => {
            if metadata.is_set(26) {
                BitSize::ThirtyTwoBit
            } else {
                BitSize::SixteenBit
            }
        }
    };

    if cpu_mode == CpuSetMode::CpuSetFast {
        remaining_count = (remaining_count + 7) & !7;
    }

    let fill_data: Option<u32> = if fixed_source_address == 1 {
        match bit_mode {
            BitSize::SixteenBit => {
                Some(bus.read_u16(source_address, AccessType::Nonsequential) as u32)
            }
            BitSize::ThirtyTwoBit => Some(bus.read_u32(source_address, AccessType::Nonsequential)),
            _ => unreachable!(),
        }
    } else {
        None
    };

    let mut first = true;
    while remaining_count != 0 {
        let access_type = if first {
            AccessType::Nonsequential
        } else {
            AccessType::Sequential
        };
        match fill_data {
            Some(data) => match bit_mode {
                BitSize::SixteenBit => {
                    bus.write_u16(destination_address, data as u16, access_type);
                    destination_address += 2;
                }
                BitSize::ThirtyTwoBit => {
                    bus.write_u32(destination_address, data, access_type);
                    destination_address += 4;
                }
                _ => unreachable!(),
            },
            None => match bit_mode {
                BitSize::SixteenBit => {
                    let halfword = bus.read_u16(source_address, access_type);
                    source_address += 2;

                    bus.write_u16(destination_address, halfword, access_type);
                    destination_address += 2;
                }
                BitSize::ThirtyTwoBit => {
                    let word = bus.read_u32(source_address, access_type);
                    source_address += 4;

                    bus.write_u32(destination_address, word, access_type);
                    destination_address += 4;
                }
                _ => unreachable!(),
            },
        }

        first = false;
        remaining_count -= 1;
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
pub fn multiply_stall(operand: u32) -> u64 {
    let n = operand.leading_zeros().max(operand.leading_ones());
    match n {
        24..=32 => 1,
        16..=23 => 2,
        8..=15 => 3,
        _ => 4,
    }
}

// https://github.com/mgba-emu/mgba/blob/afd6f14eaf8bd35214ed3fb9dc69a92bfc3877a9/src/gba/bios.c#L291
fn apply_arctan_coefficients(operand: i32) -> (i16, i32, i32, u64) {
    let mut cycles: u64 = 37;
    cycles += multiply_stall(operand.wrapping_mul(operand) as u32);

    let a = -(operand.wrapping_mul(operand) >> 14);
    let mut b: i32 = 0xA9;

    for c in ARCTAN_COEFFICIENTS {
        cycles += multiply_stall(b.wrapping_mul(a) as u32);
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

// Took comment from mgba
// [ sx   0  0 ]   [ cos(theta)  -sin(theta)  0 ]   [ 1  0  cx - ox ]   [ A B rx ]
// [  0  sy  0 ] * [ sin(theta)   cos(theta)  0 ] * [ 0  1  cy - oy ] = [ C D ry ]
// [  0   0  1 ]   [     0            0       1 ]   [ 0  0     1    ]   [ 0 0  1 ]
fn bg_affine_set(registers: &Registers, bus: &mut Bus) {
    let mut source_address = registers.r[0];
    let mut destination_address = registers.r[1];
    let mut number_of_calculations = registers.r[2];

    while number_of_calculations != 0 {
        let data_origin_x =
            (bus.read_u32(source_address, AccessType::Nonsequential) as i32) as f32 / 256.0;
        let data_origin_y =
            (bus.read_u32(source_address + 4, AccessType::Sequential) as i32) as f32 / 256.0;
        let display_center_x =
            (bus.read_u16(source_address + 8, AccessType::Sequential) as i16) as f32;
        let display_center_y =
            (bus.read_u16(source_address + 10, AccessType::Sequential) as i16) as f32;
        let scale_ratio_x =
            (bus.read_u16(source_address + 12, AccessType::Sequential) as i16) as f32 / 256.0;
        let scale_ratio_y =
            (bus.read_u16(source_address + 14, AccessType::Sequential) as i16) as f32 / 256.0;
        let theta = ((bus.read_u16(source_address + 16, AccessType::Sequential) >> 8) as i32)
            as f32
            / 128.0
            * PI;
        source_address += 20;

        let cos = theta.cos();
        let sin = theta.sin();

        let a = scale_ratio_x * cos;
        let b = scale_ratio_x * -sin;
        let c = scale_ratio_y * sin;
        let d = scale_ratio_y * cos;

        let rotate_x = data_origin_x - (a * display_center_x + b * display_center_y);
        let rotate_y = data_origin_y - (c * display_center_x + d * display_center_y);

        bus.write_u16(
            destination_address,
            (a * 256.0) as i32 as u16,
            AccessType::Sequential,
        );
        bus.write_u16(
            destination_address + 2,
            (b * 256.0) as i32 as u16,
            AccessType::Sequential,
        );
        bus.write_u16(
            destination_address + 4,
            (c * 256.0) as i32 as u16,
            AccessType::Sequential,
        );
        bus.write_u16(
            destination_address + 6,
            (d * 256.0) as i32 as u16,
            AccessType::Sequential,
        );
        bus.write_u32(
            destination_address + 8,
            (rotate_x * 256.0) as i32 as u32,
            AccessType::Sequential,
        );
        bus.write_u32(
            destination_address + 12,
            (rotate_y * 256.0) as i32 as u32,
            AccessType::Sequential,
        );
        destination_address += 16;

        number_of_calculations -= 1;
    }
}

// Took comment from mgba
// [ sx   0 ]   [ cos(theta)  -sin(theta) ]   [ A B ]
// [  0  sy ] * [ sin(theta)   cos(theta) ] = [ C D ]
fn obj_affine_set(registers: &Registers, bus: &mut Bus) {
    let mut source_address = registers.r[0];
    let mut destination_address = registers.r[1];
    let mut number_of_calculations = registers.r[2];
    let offset_between_calculations = registers.r[3];

    while number_of_calculations != 0 {
        let scale_ratio_x =
            (bus.read_u16(source_address, AccessType::Sequential) as i16) as f32 / 256.0;
        let scale_ratio_y =
            (bus.read_u16(source_address + 2, AccessType::Sequential) as i16) as f32 / 256.0;
        let theta = ((bus.read_u16(source_address + 4, AccessType::Sequential) >> 8) as i32) as f32
            / 128.0
            * PI;

        source_address += 8;

        let cos = theta.cos();
        let sin = theta.sin();

        let a = scale_ratio_x * cos;
        let b = scale_ratio_x * -sin;
        let c = scale_ratio_y * sin;
        let d = scale_ratio_y * cos;

        bus.write_u16(
            destination_address,
            (a * 256.0) as i32 as u16,
            AccessType::Sequential,
        );
        bus.write_u16(
            destination_address + offset_between_calculations,
            (b * 256.0) as i32 as u16,
            AccessType::Sequential,
        );
        bus.write_u16(
            destination_address + offset_between_calculations * 2,
            (c * 256.0) as i32 as u16,
            AccessType::Sequential,
        );
        bus.write_u16(
            destination_address + offset_between_calculations * 3,
            (d * 256.0) as i32 as u16,
            AccessType::Sequential,
        );

        destination_address += offset_between_calculations * 4;
        number_of_calculations -= 1;
    }
}

fn midi_key_2_freq(registers: &mut Registers, bus: &mut Bus) {
    let key = bus.read_u32(registers.r[0] + 4, AccessType::Nonsequential);
    let exponent = (180.0 - registers.r[1] as f32 - registers.r[2] as f32 / 256.0) / 12.0;
    registers.r[0] = (key as f32 / exponent.exp2()) as u32;
}

struct BitUnpackMetadata {
    source_length: u16,
    source_width: u8,
    destination_width: u8,
    data_offset: u32,
}

impl BitUnpackMetadata {
    fn from_register(pointer: u32, bus: &mut Bus) -> Self {
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
struct WordBuffer {
    data: u32,
    bits_filled: u8,
}

impl WordBuffer {
    fn new() -> Self {
        Self {
            data: 0,
            bits_filled: 0,
        }
    }

    fn push(&mut self, data: u32, destination_width: u8) -> bool {
        self.data |= data << self.bits_filled;
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

fn bit_unpack(registers: &Registers, bus: &mut Bus) {
    let mut source_address = registers.r[0];
    let mut destination_address = registers.r[1];
    let metadata = BitUnpackMetadata::from_register(registers.r[2], bus);

    let mut bytes_consumed = metadata.source_length;
    let mut buffer = WordBuffer::new();

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
    fn from_register(pointer: u32, bus: &mut Bus) -> Self {
        let data_header = bus.read_u32(pointer, AccessType::Nonsequential);
        let source_data_size = data_header.get_bit_range(8..32);

        Self {
            source_address: pointer,
            source_data_size,
        }
    }

    fn transform_data(&self, bus: &mut Bus, read_width: BitSize) -> Vec<u8> {
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
            _ => unreachable!(),
        }

        arr
    }
}

fn diff_unfilter(registers: &Registers, bus: &mut Bus, read_width: BitSize, write_width: BitSize) {
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
        _ => unreachable!(),
    }
}

enum CompressionType {
    Compressed,
    Uncompressed,
}

impl CompressionType {
    fn from_flag(flag: u8) -> CompressionType {
        match flag.get_bit(7) {
            0 => CompressionType::Uncompressed,
            _ => CompressionType::Compressed,
        }
    }
}

struct Packer {
    write_width: BitSize,
    pending: Option<u8>,
}

impl Packer {
    fn new(write_width: BitSize) -> Self {
        Self {
            write_width,
            pending: None,
        }
    }

    fn push(&mut self, bus: &mut Bus, destination_address: &mut u32, byte: u8) {
        match self.write_width {
            BitSize::EightBit => {
                bus.write_u8(*destination_address, byte, AccessType::Sequential);
                *destination_address += 1;
            }
            BitSize::SixteenBit => match self.pending.take() {
                Some(low_byte) => {
                    let halfword = low_byte as u16 | ((byte as u16) << 8);
                    bus.write_u16(*destination_address, halfword, AccessType::Sequential);
                    *destination_address += 2;
                }
                None => self.pending = Some(byte),
            },
            _ => unreachable!(),
        }
    }

    fn flush_unpaired_byte(&mut self, bus: &mut Bus, destination_address: u32) {
        match self.pending.take() {
            Some(low_byte) => {
                bus.write_u16(destination_address, low_byte as u16, AccessType::Sequential);
            }
            None => {}
        }
    }
}

fn rl_uncomp(registers: &Registers, bus: &mut Bus, write_width: BitSize) {
    let mut source_address = registers.r[0];
    let mut destination_address = registers.r[1];

    let mut remaining_bytes = bus
        .read_u32(source_address, AccessType::Nonsequential)
        .get_bit_range(8..32);
    source_address += 4;

    let mut packer = Packer::new(write_width);

    while remaining_bytes != 0 {
        let flag = bus.read_u8(source_address, AccessType::Sequential);
        let compression_type = CompressionType::from_flag(flag);
        let mut data_length = flag.get_bit_range(0..7);
        source_address += 1;

        data_length += match compression_type {
            CompressionType::Uncompressed => 1,
            CompressionType::Compressed => 3,
        };

        match compression_type {
            CompressionType::Compressed => {
                let byte = bus.read_u8(source_address, AccessType::Sequential);
                source_address += 1;
                for _ in 0..data_length {
                    if remaining_bytes == 0 {
                        break;
                    }

                    remaining_bytes -= 1;
                    packer.push(bus, &mut destination_address, byte);
                }
            }
            CompressionType::Uncompressed => {
                for _ in 0..data_length {
                    if remaining_bytes == 0 {
                        break;
                    }

                    remaining_bytes -= 1;
                    let byte = bus.read_u8(source_address, AccessType::Sequential);
                    source_address += 1;
                    packer.push(bus, &mut destination_address, byte);
                }
            }
        }
    }

    packer.flush_unpaired_byte(bus, destination_address);
}

// Additional cycles added based on mgba implementation, but should check compatibility with my implementations later
fn lz77_uncomp(registers: &Registers, bus: &mut Bus, write_width: BitSize) {
    bus.idle(20); // CHECK THIS LATER
    let mut source_address = registers.r[0];
    let mut destination_address = registers.r[1];

    let mut remaining_bytes = bus
        .read_u32(source_address, AccessType::Nonsequential)
        .get_bit_range(8..32);
    source_address += 4;

    let mut packer = Packer::new(write_width);

    while remaining_bytes != 0 {
        bus.idle(14); // CHECK THIS LATER
        let flag = bus.read_u8(source_address, AccessType::Sequential);
        source_address += 1;

        for bit in (0..8).rev() {
            if remaining_bytes == 0 {
                break;
            }

            bus.idle(18); // CHECK THIS LATER
            if flag.is_set(bit) {
                let byte1 = bus.read_u8(source_address, AccessType::Sequential);
                source_address += 1;
                let byte2 = bus.read_u8(source_address, AccessType::Sequential);
                source_address += 1;

                let metadata = byte1 as u16 | ((byte2 as u16) << 8);

                let n_bytes = metadata.get_bit_range(4..8) + 3;

                let msb_displacement = metadata.get_bit_range(0..4);
                let lsb_displacement = metadata.get_bit_range(8..16);
                let displacement = ((msb_displacement as u32) << 8) | lsb_displacement as u32;

                for _ in 0..n_bytes {
                    if remaining_bytes == 0 {
                        break;
                    }

                    bus.idle(10); // CHECK THIS LATER
                    remaining_bytes -= 1;

                    let byte = if displacement == 0 && packer.pending.is_some() {
                        packer.pending.unwrap()
                    } else {
                        bus.read_u8(
                            destination_address + packer.pending.is_some() as u32
                                - displacement
                                - 1,
                            AccessType::Sequential,
                        )
                    };

                    packer.push(bus, &mut destination_address, byte);
                }
            } else {
                remaining_bytes -= 1;

                let byte = bus.read_u8(source_address, AccessType::Sequential);
                source_address += 1;
                packer.push(bus, &mut destination_address, byte);
            }
        }
    }

    packer.flush_unpaired_byte(bus, destination_address);
}

struct Tree {
    root_address: u32,
    current_address: u32,
}

impl Tree {
    fn new(parent_address: u32) -> Self {
        Self {
            root_address: parent_address,
            current_address: parent_address,
        }
    }

    fn step(&mut self, bus: &mut Bus, current_bit: u32) -> Option<u8> {
        let node = bus.read_u8(self.current_address, AccessType::Nonsequential);
        let offset = node.get_bit_range(0..6) as u32;
        let child_address = (self.current_address & !1) + offset * 2 + 2 + current_bit;
        let is_leaf = if current_bit == 0 {
            node.is_set(7)
        } else {
            node.is_set(6)
        };

        if is_leaf {
            self.current_address = self.root_address;

            Some(bus.read_u8(child_address, AccessType::Nonsequential))
        } else {
            self.current_address = child_address;

            None
        }
    }
}

fn huff_uncomp(registers: &Registers, bus: &mut Bus) {
    let mut source_address = registers.r[0];
    let mut destination_address = registers.r[1];

    let header = bus.read_u32(source_address, AccessType::Nonsequential);

    let data_size = header.get_bit_range(0..4);
    let mut remaining_bits = header.get_bit_range(8..32) * 8;
    source_address += 4;
    let tree_size = bus.read_u8(source_address, AccessType::Sequential);
    let mut bitstream = source_address + (tree_size as u32 + 1) * 2;
    source_address += 1;

    let mut tree = Tree::new(source_address);
    let mut buffer = WordBuffer::new();

    while remaining_bits != 0 {
        let word = bus.read_u32(bitstream, AccessType::Nonsequential);
        bitstream += 4;

        for bit in (0..32).rev() {
            if let Some(symbol) = tree.step(bus, word.get_bit(bit) as u32) {
                if buffer.push(
                    symbol.get_bit_range(0..data_size as usize) as u32,
                    data_size as u8,
                ) {
                    bus.write_u32(destination_address, buffer.flush(), AccessType::Sequential);
                    destination_address += 4;
                }

                remaining_bits -= data_size;
                if remaining_bits == 0 {
                    break;
                }
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::components::{bus::Bus, cpu::Registers, gamepak::BackupType, utils::create_bus};

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

            assert_eq!(registers.r[0], r0, "r0 for arctan({input:10x})");
            assert_eq!(registers.r[1], r1, "r1 (a) for arctan({input:10x})");
            assert_eq!(registers.r[3], r3, "r3 (b) for arctan({input:10x})");
            assert_eq!(n_cycles, cycles, "cycles for arctan({input:10x})");
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
            // edge cases - wrapping issue and large ratio
            (0x80000000, 0x00000005, 0x00004000, 0x00000000, 0x170, 45),
            (0x000003E8, 0x000493E0, 0x00003FDE, 0x00000000, 0x170, 46),
        ] {
            registers.r[0] = x;
            registers.r[1] = y;
            let n_cycles = arctan2(&mut registers);

            assert_eq!(registers.r[0], r0, "r0 for arctan2({x:10x}, {y:10x})");
            if r1 == 0xDEADBEEF {
                assert_eq!(
                    registers.r[1], y,
                    "r1 should not be touched here ({x:10x}, {y:10x})"
                );
            } else {
                assert_eq!(registers.r[1], r1, "r1 (a) for arctan2({x:10x}, {y:10x})");
            }
            assert_eq!(registers.r[3], r3, "r3 for arctan2({x:10x}, {y:10x})");
            assert_eq!(n_cycles, cycles, "cycles for arctan2({x:10x}, {y:10x})");
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
        let mut bus = create_bus(BackupType::Flash);
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
        let mut bus = create_bus(BackupType::Flash);
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
        let mut bus = create_bus(BackupType::Flash);
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
        let mut bus = create_bus(BackupType::Flash);
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
        let mut bus = create_bus(BackupType::Flash);
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
        let mut bus = create_bus(BackupType::Flash);
        let mut registers = Registers::new();

        registers.r[0] = 0x03000000;
        registers.r[1] = 0x02000000;

        let header = 4 << 8;
        write_diff_source(&mut bus, 0x03000000, header, &[10, 1, 1, 1]);

        diff_unfilter(&registers, &mut bus, BitSize::EightBit, BitSize::SixteenBit);

        assert_eq!(ewram_word(&bus, 0), 0x0D0C0B0A);
    }

    #[test]
    fn test_rl_uncomp_8bit() {
        let mut bus = create_bus(BackupType::Flash);
        let mut registers = Registers::new();

        registers.r[0] = 0x03000000;
        registers.r[1] = 0x02000000;

        // 7x AA, just 11 22 33, 4x 00 -> 14 bytes
        let header = 14 << 8;
        write_diff_source(
            &mut bus,
            0x03000000,
            header,
            &[0x84, 0xAA, 0x02, 0x11, 0x22, 0x33, 0x81, 0x00],
        );

        rl_uncomp(&registers, &mut bus, BitSize::EightBit);

        assert_eq!(
            &bus.ewram[0..14],
            &[
                0xAA, 0xAA, 0xAA, 0xAA, 0xAA, 0xAA, 0xAA, 0x11, 0x22, 0x33, 0x00, 0x00, 0x00, 0x00
            ]
        );
    }

    #[test]
    fn test_rl_uncomp_16bit_with_unpaired_byte() {
        let mut bus = create_bus(BackupType::Flash);
        let mut registers = Registers::new();

        registers.r[0] = 0x03000000;
        registers.r[1] = 0x02000000;

        // 3x AA then just BB -> AA AA AA BB, run ends on odd byte
        let header = 4 << 8;
        write_diff_source(&mut bus, 0x03000000, header, &[0x80, 0xAA, 0x00, 0xBB]);

        rl_uncomp(&registers, &mut bus, BitSize::SixteenBit);

        assert_eq!(ewram_word(&bus, 0), 0xBBAA_AAAA);
    }

    #[test]
    fn test_lz77_uncomp_8bit_lookback() {
        let mut bus = create_bus(BackupType::Flash);
        let mut registers = Registers::new();

        registers.r[0] = 0x03000000;
        registers.r[1] = 0x02000000;

        // literal A, literal B, then copy 3 from displacement 1 (2 back) -> ABABA
        let header = 5 << 8;
        write_diff_source(
            &mut bus,
            0x03000000,
            header,
            &[0x20, 0x41, 0x42, 0x00, 0x01],
        );

        lz77_uncomp(&registers, &mut bus, BitSize::EightBit);

        assert_eq!(&bus.ewram[0..5], &[0x41, 0x42, 0x41, 0x42, 0x41]);
    }

    #[test]
    fn test_cpuset_copy_16bit() {
        let mut bus = create_bus(BackupType::Flash);
        let mut cpu = Arm7tdmi::new();

        bus.iwram[0..6].copy_from_slice(&[0x11, 0x22, 0x33, 0x44, 0x55, 0x66]);
        cpu.registers.r[0] = 0x03000000;
        cpu.registers.r[1] = 0x02000000;
        cpu.registers.r[2] = 3;

        cpuset(&cpu, &mut bus, CpuSetMode::CpuSet);

        assert_eq!(&bus.ewram[0..6], &[0x11, 0x22, 0x33, 0x44, 0x55, 0x66]);
    }

    #[test]
    fn test_cpuset() {
        let mut bus = create_bus(BackupType::Flash);
        let mut cpu = Arm7tdmi::new();

        // fill value, then 0x99 garbage data that should not appear
        bus.iwram[0..8].copy_from_slice(&[0xEF, 0xBE, 0xAD, 0xDE, 0x99, 0x99, 0x99, 0x99]);
        cpu.registers.r[0] = 0x03000000;
        cpu.registers.r[1] = 0x02000000;
        cpu.registers.r[2] = (1 << 26) | (1 << 24) | 3;

        cpuset(&cpu, &mut bus, CpuSetMode::CpuSet);

        assert_eq!(ewram_word(&bus, 0), 0xDEADBEEF);
        assert_eq!(ewram_word(&bus, 4), 0xDEADBEEF);
        assert_eq!(ewram_word(&bus, 8), 0xDEADBEEF);
        assert_eq!(ewram_word(&bus, 12), 0);
    }

    #[test]
    fn test_cpuset_fast_8_word_round_up() {
        let mut bus = create_bus(BackupType::Flash);
        let mut cpu = Arm7tdmi::new();

        for i in 0..32 {
            bus.iwram[i] = i as u8;
        }

        cpu.registers.r[0] = 0x03000000;
        cpu.registers.r[1] = 0x02000000;
        cpu.registers.r[2] = 3;

        cpuset(&cpu, &mut bus, CpuSetMode::CpuSetFast);

        assert_eq!(&bus.ewram[0..32], &bus.iwram[0..32]);
        assert_eq!(ewram_word(&bus, 28), 0x1F1E1D1C); // word 7
    }

    #[test]
    fn test_huff() {
        let mut bus = create_bus(BackupType::Flash);
        let mut cpu = Arm7tdmi::new();

        cpu.registers.r[0] = 0x03000000;
        cpu.registers.r[1] = 0x02000000;

        let source_address = Bus::iwram_index(cpu.registers.r[0]);
        // first 4 - header, 5 - tree size byte, 6-8 is root "f", 9 = "u", 10 = "H", padding + bitstream word 0xB0000000
        bus.iwram[source_address..source_address + 16].copy_from_slice(&[
            0x28, 0x04, 0, 0, 0x03, 0x80, 0x66, 0xC0, 0x48, 0x75, 0, 0, 0, 0, 0, 0xB0,
        ]);

        huff_uncomp(&cpu.registers, &mut bus);

        assert_eq!(&bus.ewram[0..4], b"Huff");
    }
}

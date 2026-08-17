// https://github.com/michelhe/rustboyadvance-ng/blob/master/arm7tdmi/src/memory.rs
// https://github.com/michelhe/rustboyadvance-ng/blob/master/arm7tdmi/src/arm/exec.rs
// https://problemkaputt.de/gbatek.htm#armcpureference
// https://student.cs.uwaterloo.ca/~cs452/docs/ts7200/arm-architecture.pdf
// https://problemkaputt.de/gbatek-arm-cpu-memory-alignments.htm
// https://github.com/jsmolka/gba-tests - **IMPLEMNT MORE TESTS**

use super::decode::{
    AddressingMode, ArmInstruction, BitSize, DataOp, HalfwordOffset, MsrSource, Operand2,
    SdtOffset, ShiftAmount, ShiftType, ThumbBranchType, TransferAction, TransferKind,
};

use crate::components::{
    bios::multiply_stall,
    bus::{AccessType, AddressBus},
    cpu::{ProcessorMode, ProcessorState, Registers, SideEffect},
    utils::{BitOps, ShiftOps},
};

fn branch(registers: &mut Registers, link: bool, offset: i32) -> SideEffect {
    if link {
        registers.copy_pc_to_lr();
    }

    let address = registers.r[15].wrapping_add(offset as u32);

    SideEffect::Branch(address)
}

fn branch_and_exchange(registers: &mut Registers, rn: u8) -> SideEffect {
    let mut value = registers.r[rn as usize];

    if value.is_set(0) {
        registers.set_state(ProcessorState::Thumb);

        value.clear_bit(0);
        SideEffect::Branch(value)
    } else {
        registers.set_state(ProcessorState::Arm);

        value.clear_bit_range(0..2);
        SideEffect::Branch(value)
    }
}

fn barrel_shifter(
    base_value: u32,
    shift_amount: ShiftAmount,
    shift_type: ShiftType,
    registers: &Registers,
) -> (u32, Option<bool>) {
    match shift_amount {
        ShiftAmount::Immediate(amount) => {
            base_value.shift_imm(shift_type, amount, registers.is_c_set())
        }
        ShiftAmount::Register(n) => {
            let amount = registers.r[n as usize] as u8;
            base_value.shift_reg(shift_type, amount, registers.is_c_set())
        }
    }
}

fn process_operand2(registers: &Registers, operand2: Operand2) -> (u32, Option<bool>, bool) {
    match operand2 {
        Operand2::Immediate { value, rotate } => {
            let value = (value as u32).rotate_right((rotate as u32) * 2);
            let carry_out = if rotate == 0 {
                None
            } else {
                Some(value.is_set(31))
            };

            (value, carry_out, false)
        }
        Operand2::Register(shifted_register) => {
            let base_value = registers.r[shifted_register.rm as usize];
            let is_register_shift =
                matches!(shifted_register.shift_amount, ShiftAmount::Register(_));

            let (value, carry_out) = barrel_shifter(
                base_value,
                shifted_register.shift_amount,
                shifted_register.shift_type,
                registers,
            );

            (value, carry_out, is_register_shift)
        }
    }
}

fn update_flags(registers: &mut Registers, n: bool, z: bool, c: Option<bool>, v: Option<bool>) {
    if n {
        registers.set_n();
    } else {
        registers.clear_n();
    }

    if z {
        registers.set_z();
    } else {
        registers.clear_z();
    }

    match c {
        Some(flag) => {
            if flag {
                registers.set_c();
            } else {
                registers.clear_c();
            }
        }
        None => {}
    }

    match v {
        Some(flag) => {
            if flag {
                registers.set_v();
            } else {
                registers.clear_v();
            }
        }
        None => {}
    }
}

// Page 4-11 of arm7tdmi data sheet. These dont affect the V flag
// logical ops: AND, EOR, TST, TEQ, ORR, MOV, BIC, MVN
fn discards_result(opcode: DataOp) -> bool {
    matches!(
        opcode,
        DataOp::Tst | DataOp::Teq | DataOp::Cmp | DataOp::Cmn
    )
}

fn data_processing<A: AddressBus>(
    bus: &mut A,
    registers: &mut Registers,
    opcode: DataOp,
    set_flags: bool,
    rn: u8,
    rd: u8,
    operand2: Operand2,
) -> Option<SideEffect> {
    let op1 = registers.r[rn as usize];
    let (op2, shifter_carry_out, shifted_register) = process_operand2(registers, operand2);

    if shifted_register {
        bus.idle(1);
    }

    let (result, n, z, c, v) = match opcode {
        DataOp::And
        | DataOp::Tst
        | DataOp::Eor
        | DataOp::Teq
        | DataOp::Mov
        | DataOp::Orr
        | DataOp::Bic
        | DataOp::Mvn => {
            let result = match opcode {
                DataOp::And | DataOp::Tst => op1 & op2,
                DataOp::Eor | DataOp::Teq => op1 ^ op2,
                DataOp::Mov => op2,
                DataOp::Orr => op1 | op2,
                DataOp::Bic => op1 & !op2,
                DataOp::Mvn => !op2,
                _ => unreachable!(),
            };

            (
                result,
                result.is_negative(),
                result.is_zero(),
                shifter_carry_out,
                None,
            )
        }
        DataOp::Add | DataOp::Cmn => {
            let (result, overflow_unsigned) = op1.overflowing_add(op2);
            let (_, overflow_signed) = (op1 as i32).overflowing_add(op2 as i32);

            (
                result,
                result.is_negative(),
                result.is_zero(),
                Some(overflow_unsigned),
                Some(overflow_signed),
            )
        }
        DataOp::Sub | DataOp::Cmp | DataOp::Rsb => {
            let (a, b) = if opcode == DataOp::Rsb {
                (op2, op1)
            } else {
                (op1, op2)
            };
            let (result, underflow_unsigned) = a.overflowing_sub(b);
            let (_, underflow_signed) = (a as i32).overflowing_sub(b as i32);

            (
                result,
                result.is_negative(),
                result.is_zero(),
                Some(!underflow_unsigned),
                Some(underflow_signed),
            )
        }
        DataOp::Adc => {
            let carry = registers.is_c_set() as u32;
            let result_u64 = (op1 as u64) + (op2 as u64) + (carry as u64);
            let result = result_u64 as u32;

            let c = result_u64 > u32::MAX as u64;

            let result_i64 = (op1 as i32 as i64) + (op2 as i32 as i64) + carry as i64;
            let v = result_i64 != (result as i32 as i64);

            (
                result,
                result.is_negative(),
                result.is_zero(),
                Some(c),
                Some(v),
            )
        }
        DataOp::Sbc | DataOp::Rsc => {
            let carry = registers.is_c_set() as u64;
            let (a, b) = if opcode == DataOp::Sbc {
                (op1, op2)
            } else {
                (op2, op1)
            };

            let result_u64 = (a as u64) + (!b as u64) + carry;
            let result = result_u64 as u32;
            let c = result_u64 > u32::MAX as u64;

            let result_i64 = (a as i32 as i64) + (!b as i32 as i64) + carry as i64;
            let v = result_i64 != (result as i32 as i64);

            (
                result,
                result.is_negative(),
                result.is_zero(),
                Some(c),
                Some(v),
            )
        }
    };

    // https://student.cs.uwaterloo.ca/~cs452/docs/ts7200/arm-architecture.pdf
    // future note: page A2-10 & A1-7 result to PC is a jump
    // A2-55 when s bit is set and rd is 15, copy spsr from cpsr, thhis is an
    // exception return
    if !discards_result(opcode) {
        if rd == 15 {
            return Some(if set_flags {
                SideEffect::BranchRestoreCpsr(result)
            } else {
                SideEffect::Branch(result)
            });
        }

        registers.r[rd as usize] = result;
    }

    if set_flags {
        update_flags(registers, n, z, c, v);
    }

    None
}

fn mrs(registers: &mut Registers, rd: u8, use_spsr: bool) {
    let source_psr = if use_spsr {
        registers.banked_spsr[registers.mode().spsr_index()]
    } else {
        registers.cpsr
    };

    registers.r[rd as usize] = source_psr;
}

fn msr(registers: &mut Registers, source: MsrSource, use_spsr: bool, flags_only: bool) {
    let mut source_value = match source {
        MsrSource::Register(rs) => registers.r[rs as usize],
        MsrSource::Immediate { value, rotate } => (value as u32).rotate_right((rotate as u32) * 2),
    };

    let old_mode = registers.mode();

    let destination = if use_spsr {
        &mut registers.banked_spsr[old_mode.spsr_index()]
    } else {
        &mut registers.cpsr
    };

    if flags_only {
        destination.clear_bit_range(28..32);
        source_value.clear_bit_range(0..28);
        *destination |= source_value;
    } else {
        *destination = source_value;
    }

    if !use_spsr && !flags_only {
        let new_mode = registers.mode();
        registers.bank_registers(old_mode, new_mode);
    }
}

fn multiply<A: AddressBus>(
    bus: &mut A,
    registers: &mut Registers,
    rm: u8,
    rs: u8,
    rn: u8,
    rd: u8,
    accumulate: bool,
    set_flags: bool,
) {
    let op1 = registers.r[rm as usize];
    let op2 = registers.r[rs as usize];
    let i = multiply_stall(op2);
    bus.idle(if accumulate { i + 1 } else { i });

    // https://bmchtech.github.io/post/multiply/
    // Carry flag can be anything but is just consistent
    let mut product = op1.wrapping_mul(op2);
    if accumulate {
        product = product.wrapping_add(registers.r[rn as usize]);
    }

    registers.r[rd as usize] = product;

    if set_flags {
        update_flags(
            registers,
            product.is_negative(),
            product.is_zero(),
            None,
            None,
        );
    }
}

fn multiply_stall_umull(operand: u32) -> u64 {
    match operand.leading_zeros() {
        24..=32 => 1,
        16..=23 => 2,
        8..=15 => 3,
        _ => 4,
    }
}

fn multiply_long<A: AddressBus>(
    bus: &mut A,
    registers: &mut Registers,
    rm: u8,
    rs: u8,
    rdlo: u8,
    rdhi: u8,
    accumulate: bool,
    signed: bool,
    set_flags: bool,
) {
    let op1 = registers.r[rm as usize];
    let op2 = registers.r[rs as usize];

    let i = if signed {
        multiply_stall(op2 as u32)
    } else {
        multiply_stall_umull(op2 as u32)
    };

    bus.idle(if accumulate { i + 2 } else { i + 1 });

    let mut product = if signed {
        (op1 as i32 as i64).wrapping_mul(op2 as i32 as i64) as u64
    } else {
        (op1 as u64).wrapping_mul(op2 as u64) as u64
    };

    if accumulate {
        let acc_reg =
            ((registers.r[rdhi as usize] as u64) << 32) | (registers.r[rdlo as usize] as u64);
        if signed {
            product = (product as i64).wrapping_add(acc_reg as i64) as u64;
        } else {
            product = product.wrapping_add(acc_reg) as u64;
        }
    }

    registers.r[rdhi as usize] = product.get_bit_range(32..64) as u32;
    registers.r[rdlo as usize] = product.get_bit_range(0..32) as u32;

    if set_flags {
        update_flags(
            registers,
            product.is_negative(),
            product.is_zero(),
            None,
            None,
        );
    }
}

fn process_sdt_address_offset(registers: &Registers, offset_type: SdtOffset) -> u32 {
    match offset_type {
        SdtOffset::Immediate(offset) => offset as u32,
        SdtOffset::Register(shifted_register) => {
            let base_value = registers.r[shifted_register.rm as usize];

            let (offset, _) = barrel_shifter(
                base_value,
                shifted_register.shift_amount,
                shifted_register.shift_type,
                registers,
            );

            offset
        }
    }
}

fn compute_new_start_address(
    start_address: u32,
    address_offset: u32,
    addressing_mode: AddressingMode,
) -> u32 {
    if matches!(
        addressing_mode,
        AddressingMode::IncrementBefore | AddressingMode::IncrementAfter
    ) {
        start_address.wrapping_add(address_offset)
    } else {
        start_address.wrapping_sub(address_offset)
    }
}

fn single_data_transfer<A: AddressBus>(
    bus: &mut A,
    registers: &mut Registers,
    rn: u8,
    rd: u8,
    transfer_action: TransferAction,
    write_back: bool,
    transfer_size: BitSize,
    addressing_mode: AddressingMode,
    offset: SdtOffset,
) -> Option<SideEffect> {
    let mut start_address = registers.r[rn as usize];
    let address_offset = process_sdt_address_offset(registers, offset);

    if matches!(
        addressing_mode,
        AddressingMode::IncrementBefore | AddressingMode::DecrementBefore
    ) {
        start_address = compute_new_start_address(start_address, address_offset, addressing_mode);
        if write_back {
            registers.r[rn as usize] = start_address;
        }
    }

    let mut side_effect = None;

    match transfer_action {
        TransferAction::Load => {
            let value = if transfer_size == BitSize::Byte {
                bus.read_u8(start_address, AccessType::Nonsequential) as u32
            } else {
                let word = bus.read_u32(start_address, AccessType::Nonsequential);
                // Page a1-8 & A2-38, A2-40 regarding unaligned addreses on load of architecture manual, < Armv6 (arm7tdmi is armv4) rotated data
                let misalignment_bits = (start_address.get_bit_range(0..2)) * 8;
                word.rotate_right(misalignment_bits)
            };

            bus.idle(1);

            // https://student.cs.uwaterloo.ca/~cs452/docs/ts7200/arm-architecture.pdf
            // future note: page A2-10 & A1-9 Loads to PC is a jump
            if rd == 15 {
                side_effect = Some(SideEffect::Branch(value));
            }

            registers.r[rd as usize] = value
        }
        TransferAction::Store => {
            let value = if rd == 15 {
                registers.r[15].wrapping_add(4)
            } else {
                registers.r[rd as usize]
            };

            if transfer_size == BitSize::Byte {
                bus.write_u8(start_address, value as u8, AccessType::Nonsequential);
            } else {
                bus.write_u32(start_address, value, AccessType::Nonsequential);
            }
        }
    }

    if matches!(
        addressing_mode,
        AddressingMode::IncrementAfter | AddressingMode::DecrementAfter
    ) {
        start_address = compute_new_start_address(start_address, address_offset, addressing_mode);

        if rd != rn {
            registers.r[rn as usize] = start_address;
        }
    }

    side_effect
}

// S bit for load on PC means cpsr is loaded from the spsr
// if no load on pc and all stm, if on privelefed mode, the user mode bank registers are transferred
// **assuming that gba should never be in fiq mode, so shouldnt need high register user mode banking**
fn get_transfer_data(
    registers: &mut Registers,
    current_register: u16,
    use_user_bank: bool,
) -> &mut u32 {
    let in_usr_mode = matches!(registers.mode(), ProcessorMode::Usr | ProcessorMode::Sys);

    if !use_user_bank || in_usr_mode || current_register < 13 {
        return &mut registers.r[current_register as usize];
    } else {
        let index = ProcessorMode::Usr.sp_and_lr_index();
        return &mut registers.banked_special_registers[index][(current_register - 13) as usize];
    }
}

fn block_data_transfer<A: AddressBus>(
    bus: &mut A,
    registers: &mut Registers,
    rn: u8,
    transfer_action: TransferAction,
    write_back: bool,
    psr: bool,
    addressing_mode: AddressingMode,
    register_list: u16,
) -> Option<SideEffect> {
    bus.idle(1);

    let base_address = registers.r[rn as usize];
    let mut is_first_access = true;

    let loads_pc = transfer_action == TransferAction::Load && register_list.is_set(15);
    let use_user_bank = psr && !loads_pc;

    let n_ones = register_list.count_ones();
    let mut start_address = match addressing_mode {
        AddressingMode::IncrementAfter => base_address,
        AddressingMode::IncrementBefore => base_address.wrapping_add(4),
        AddressingMode::DecrementAfter => base_address.wrapping_sub(n_ones * 4).wrapping_add(4),
        AddressingMode::DecrementBefore => base_address.wrapping_sub(n_ones * 4),
    };

    if write_back {
        registers.r[rn as usize] = match addressing_mode {
            AddressingMode::IncrementAfter | AddressingMode::IncrementBefore => {
                base_address.wrapping_add(n_ones * 4)
            }
            AddressingMode::DecrementAfter | AddressingMode::DecrementBefore => {
                base_address.wrapping_sub(n_ones * 4)
            }
        };
    }

    let mut side_effect = None;

    for bit in 0..16 {
        if register_list.is_clear(bit) {
            continue;
        }

        let value = get_transfer_data(registers, bit as u16, use_user_bank);
        match transfer_action {
            TransferAction::Load => {
                let word = bus.read_u32(
                    start_address,
                    if is_first_access {
                        AccessType::Nonsequential
                    } else {
                        AccessType::Sequential
                    },
                );

                if bit == 15 {
                    side_effect = Some(if psr {
                        SideEffect::BranchRestoreCpsr(word)
                    } else {
                        SideEffect::Branch(word)
                    });
                }

                *value = word;
            }
            TransferAction::Store => {
                bus.write_u32(
                    start_address,
                    *value,
                    if is_first_access {
                        AccessType::Nonsequential
                    } else {
                        AccessType::Sequential
                    },
                );
            }
        }

        is_first_access = false;
        start_address = start_address.wrapping_add(4);
    }

    side_effect
}

fn single_data_swap<A: AddressBus>(
    bus: &mut A,
    registers: &mut Registers,
    rn: u8,
    rd: u8,
    rm: u8,
    swap_size: BitSize,
) {
    let swap_address = registers.r[rn as usize];

    let word = match swap_size {
        BitSize::Byte => {
            let word = bus.read_u8(swap_address, AccessType::Nonsequential) as u32;
            bus.write_u8(
                swap_address,
                registers.r[rm as usize] as u8,
                AccessType::Sequential,
            );

            word
        }
        BitSize::Word => {
            let word = bus.read_u32(swap_address, AccessType::Nonsequential) as u32;
            bus.write_u32(
                swap_address,
                registers.r[rm as usize],
                AccessType::Sequential,
            );

            word
        }
    };

    registers.r[rd as usize] = word;
}

fn halfword_data_transfer<A: AddressBus>(
    bus: &mut A,
    registers: &mut Registers,
    offset: HalfwordOffset,
    transfer_kind: TransferKind,
    rd: u8,
    rn: u8,
    write_back: bool,
    addressing_mode: AddressingMode,
    transfer_action: TransferAction,
) {
    let mut start_address = registers.r[rn as usize];
    let address_offset = match offset {
        HalfwordOffset::Register(rn) => registers.r[rn as usize],
        HalfwordOffset::Immediate(value) => value as u32,
    };

    if matches!(
        addressing_mode,
        AddressingMode::IncrementBefore | AddressingMode::DecrementBefore
    ) {
        start_address = compute_new_start_address(start_address, address_offset, addressing_mode);
        if write_back {
            registers.r[rn as usize] = start_address;
        }
    }

    let transfer_kind = if transfer_kind == TransferKind::SignedHalfword && start_address.is_set(0)
    {
        TransferKind::SignedByte
    } else {
        transfer_kind
    };

    match transfer_action {
        TransferAction::Load => {
            bus.idle(1);

            let word = match transfer_kind {
                TransferKind::SignedByte => {
                    let mut word = bus.read_u8(start_address, AccessType::Nonsequential) as u32;

                    if word.is_set(7) {
                        word.set_bit_range(8..32);
                    }

                    word
                }
                // https://problemkaputt.de/gbatek-arm-cpu-memory-alignments.htm
                /*
                   On ARM7 aka ARMv4 aka NDS7/GBA:
                   LDRH Rd,[odd]   -->  LDRH Rd,[odd-1] ROR 8  ;read to bit0-7 and bit24-31
                   LDRSH Rd,[odd]  -->  LDRSB Rd,[odd]         ;sign-expand BYTE value
                */
                TransferKind::UnsignedHalfword => {
                    let misaligned_bit = start_address.get_bit(0) as u32;
                    let word = bus.read_u16(start_address, AccessType::Nonsequential) as u32;

                    word.rotate_right(8 * misaligned_bit)
                }
                TransferKind::SignedHalfword => {
                    let mut word = bus.read_u16(start_address, AccessType::Nonsequential) as u32;

                    word.set_bit_range(16..32);

                    word
                }
            };

            registers.r[rd as usize] = word;
        }
        TransferAction::Store => {
            let value = if rd == 15 {
                registers.r[15].wrapping_add(4)
            } else {
                registers.r[rd as usize]
            };

            bus.write_u32(start_address, value, AccessType::Nonsequential);
        }
    }

    if matches!(
        addressing_mode,
        AddressingMode::IncrementAfter | AddressingMode::DecrementAfter
    ) {
        start_address = compute_new_start_address(start_address, address_offset, addressing_mode);

        if rd != rn {
            registers.r[rn as usize] = start_address;
        }
    }
}

fn thumb_branch(
    registers: &mut Registers,
    branch_type: ThumbBranchType,
    offset: u32,
) -> Option<SideEffect> {
    match branch_type {
        ThumbBranchType::Low => {
            let target_address = registers.r[14].wrapping_add(offset);
            registers.r[14] = registers.r[15].wrapping_sub(2) | 1;

            Some(SideEffect::Branch(target_address))
        }
        ThumbBranchType::High => {
            registers.r[14] = registers.r[15].wrapping_add(offset);

            None
        }
    }
}

// Note always use overflowing_add and overflowing_sub
pub fn execute_arm<A: AddressBus>(
    instruction: ArmInstruction,
    registers: &mut Registers,
    bus: &mut A,
) -> Option<SideEffect> {
    match instruction {
        ArmInstruction::DataProcessing {
            opcode,
            set_flags,
            rn,
            rd,
            operand2,
        } => data_processing(bus, registers, opcode, set_flags, rn, rd, operand2),
        ArmInstruction::Branch { link, offset } => Some(branch(registers, link, offset)),
        ArmInstruction::Mrs { rd, use_spsr } => {
            mrs(registers, rd, use_spsr);

            None
        }
        ArmInstruction::Msr {
            source,
            use_spsr,
            flags_only,
        } => {
            msr(registers, source, use_spsr, flags_only);

            None
        }
        ArmInstruction::Multiply {
            rm,
            rs,
            rn,
            rd,
            accumulate,
            set_flags,
        } => {
            multiply(bus, registers, rm, rs, rn, rd, accumulate, set_flags);

            None
        }
        ArmInstruction::MultiplyLong {
            rm,
            rs,
            rdlo,
            rdhi,
            accumulate,
            signed,
            set_flags,
        } => {
            multiply_long(
                bus, registers, rm, rs, rdlo, rdhi, accumulate, signed, set_flags,
            );

            None
        }
        ArmInstruction::SingleDataTransfer {
            rn,
            rd,
            transfer_action,
            write_back,
            transfer_size,
            addressing_mode,
            offset,
        } => single_data_transfer(
            bus,
            registers,
            rn,
            rd,
            transfer_action,
            write_back,
            transfer_size,
            addressing_mode,
            offset,
        ),
        ArmInstruction::BlockDataTransfer {
            rn,
            transfer_action,
            write_back,
            psr,
            addressing_mode,
            register_list,
        } => block_data_transfer(
            bus,
            registers,
            rn,
            transfer_action,
            write_back,
            psr,
            addressing_mode,
            register_list,
        ),
        ArmInstruction::Undefined => None,
        ArmInstruction::SoftwareInterrupt { comment } => Some(SideEffect::Swi(comment)),
        ArmInstruction::BranchExchange { rn } => Some(branch_and_exchange(registers, rn)),
        ArmInstruction::SingleDataSwap {
            rm,
            rd,
            rn,
            swap_size,
        } => {
            single_data_swap(bus, registers, rn, rd, rm, swap_size);

            None
        }
        ArmInstruction::HalfwordDataTransfer {
            offset,
            transfer_kind,
            rd,
            rn,
            write_back,
            addressing_mode,
            transfer_action,
        } => {
            halfword_data_transfer(
                bus,
                registers,
                offset,
                transfer_kind,
                rd,
                rn,
                write_back,
                addressing_mode,
                transfer_action,
            );

            None
        }
        ArmInstruction::ThumbBranch {
            branch_type,
            offset,
        } => thumb_branch(registers, branch_type, offset),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::components::{
        bus::Bus,
        cpu::{
            Condition, ProcessorMode,
            arm::decode::{ArmInstruction, DataOp, decode_arm},
        },
        gamepak::GamePak,
    };

    fn create_bus() -> Bus {
        let gamepak = GamePak::mock();
        let bus = Bus::new(gamepak);

        bus
    }

    #[test]
    fn test_mov() {
        // echo "movs r1, #42" | arm-none-eabi-as -mcpu=arm7tdmi -o x.o && arm-none-eabi-objdump -d x.o
        let mut registers = Registers::new();
        let mut bus = create_bus();
        let instruction: u32 = 0xe3b0102a;
        let decoded_arm = decode_arm(instruction);

        assert_eq!(decoded_arm.condition, Condition::Al);
        assert!(matches!(
            decoded_arm.instruction,
            ArmInstruction::DataProcessing {
                opcode: DataOp::Mov,
                ..
            }
        ));

        let side_effect = execute_arm(decoded_arm.instruction, &mut registers, &mut bus);
        assert!(matches!(side_effect, None));

        assert_eq!(registers.r[1], 42);
        assert_eq!(registers.is_z_set(), false);
        assert_eq!(registers.is_c_set(), false);
        assert_eq!(registers.is_n_set(), false);
        assert_eq!(registers.is_v_set(), false);
    }

    #[test]
    fn test_mode_change() {
        /*
            From page 4-20 of arm7tdmi data sheet:
            MRS R0,CPSR ; Take a copy of the CPSR.
            BIC R0,R0,#0x1F ; Clear the mode bits.
            ORR R0,R0,#new_mode ; Select new mode
            MSR CPSR,R0 ; Write
        */
        let mut bus = create_bus();
        let mut registers = Registers::new();
        registers.cpsr |= ProcessorMode::Und as u32;
        registers.r[13] = 0xFFFFFFFF;

        assert_eq!(registers.mode(), ProcessorMode::Und);

        // echo "mrs r0, cpsr" | arm-none-eabi-as -mcpu=arm7tdmi -o x.o && arm-none-eabi-objdump -d x.o
        let instruction: u32 = 0xe10f0000;

        let decoded_arm = decode_arm(instruction);
        let _ = execute_arm(decoded_arm.instruction, &mut registers, &mut bus);
        assert_eq!(registers.r[0], registers.cpsr);

        // echo "bic r0, r0, #0x1F" | arm-none-eabi-as -mcpu=arm7tdmi -o x.o && arm-none-eabi-objdump -d x.o
        let instruction: u32 = 0xe3c0001f;
        let decoded_arm = decode_arm(instruction);
        _ = execute_arm(decoded_arm.instruction, &mut registers, &mut bus);
        assert_eq!(registers.r[0], 0);

        // echo "orr r0, r0, #0x1F" | arm-none-eabi-as -mcpu=arm7tdmi -o x.o && arm-none-eabi-objdump -d x.o
        // Sys mode
        let instruction: u32 = 0xe380001f;
        let decoded_arm = decode_arm(instruction);
        _ = execute_arm(decoded_arm.instruction, &mut registers, &mut bus);
        assert_eq!(registers.r[0], 0x1F);

        // echo "msr cpsr, r0" | arm-none-eabi-as -mcpu=arm7tdmi -o x.o && arm-none-eabi-objdump -d x.o
        // Sys mode
        let instruction: u32 = 0xe129f000;
        let decoded_arm = decode_arm(instruction);
        _ = execute_arm(decoded_arm.instruction, &mut registers, &mut bus);
        assert_eq!(registers.mode(), ProcessorMode::Sys);
        assert_eq!(registers.r[13], 0);
        assert_eq!(registers.banked_special_registers[5][0], 0xFFFFFFFF);
    }

    #[test]
    fn test_branch_and_exchange() {
        let mut bus = create_bus();
        let mut registers = Registers::new();

        // echo "mov r1, #101" | arm-none-eabi-as -mcpu=arm7tdmi -o x.o && arm-none-eabi-objdump -d x.o
        let instruction = 0xe3a01065;
        let decoded_arm = decode_arm(instruction);
        let _ = execute_arm(decoded_arm.instruction, &mut registers, &mut bus);

        assert_eq!(registers.r[1], 101);
        assert_eq!(registers.state(), ProcessorState::Arm);

        // echo "bx r1" | arm-none-eabi-as -mcpu=arm7tdmi -o x.o && arm-none-eabi-objdump -d x.o
        let instruction: u32 = 0xe12fff11;
        let decoded_arm = decode_arm(instruction);
        let side_effect = execute_arm(decoded_arm.instruction, &mut registers, &mut bus);

        assert!(matches!(side_effect, Some(SideEffect::Branch(100))));
        assert_eq!(registers.state(), ProcessorState::Thumb);
    }
}

use super::decode::{ArmInstruction, DataOp, Operand2, ShiftAmount, ShiftType};

use crate::components::{
    bus::AddressBus,
    cpu::{Registers, SideEffect},
    utils::{BitOps, ShiftOps},
};

fn branch(registers: &mut Registers, link: bool, offset: i32) -> SideEffect {
    if link {
        registers.copy_pc_to_lr();
    }

    let address = registers.r[15].wrapping_add(offset as u32);

    SideEffect::Branch(address)
}

fn barrel_shifter(
    base_value: u32,
    shift_amount: ShiftAmount,
    shift_type: ShiftType,
    registers: &Registers,
) -> (u32, Option<bool>) {
    match shift_amount {
        ShiftAmount::Immediate(amount) => base_value.shift_imm(shift_type, amount, registers.C()),
        ShiftAmount::Register(n) => {
            let amount = registers.r[n as usize] as u8;
            base_value.shift_reg(shift_type, amount, registers.C())
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
        registers.set_N();
    } else {
        registers.clear_N();
    }

    if z {
        registers.set_Z();
    } else {
        registers.clear_Z();
    }

    match c {
        Some(flag) => {
            if flag {
                registers.set_C();
            } else {
                registers.clear_C();
            }
        }
        None => {}
    }

    match v {
        Some(flag) => {
            if flag {
                registers.set_V();
            } else {
                registers.clear_V();
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
        // ****put the rest of the log ops here later***
        DataOp::And | DataOp::Tst | DataOp::Eor | DataOp::Teq | DataOp::Mov => {
            let result = match opcode {
                DataOp::And | DataOp::Tst => op1 & op2,
                DataOp::Eor | DataOp::Teq => op1 ^ op2,
                DataOp::Mov => op2,
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

            let (n, z, c, v) = (
                result.is_negative(),
                result.is_zero(),
                Some(overflow_unsigned),
                Some(overflow_signed),
            );

            (result, n, z, c, v)
        }
        DataOp::Sub | DataOp::Cmp => {
            let (result, underflow_unsigned) = op1.overflowing_sub(op2);
            let (_, underflow_signed) = (op1 as i32).overflowing_sub(op2 as i32);

            let (n, z, c, v) = (
                result.is_negative(),
                result.is_zero(),
                Some(!underflow_unsigned),
                Some(underflow_signed),
            );

            (result, n, z, c, v)
        }
        _ => (0, false, false, Some(false), Some(false)), // ***PLACEHOLDER***
    };

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
        _ => None,
    }
}

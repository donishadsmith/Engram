use super::decode::{ArmInstruction, DataOp, Operand2, ShiftAmount, ShiftType};

use crate::components::{
    bus::AddressBus,
    cpu::{Registers, SideEffect},
    utils::BitOps,
};

fn branch(registers: &mut Registers, link: bool, offset: i32) -> SideEffect {
    if link {
        registers.copy_pc_to_lr();
    }

    let address = registers.r[15].wrapping_add(offset as u32);

    SideEffect::Branch(address)
}

/*
Page 4-13 in ARM7TDMI Technical Reference

Note LSL #0 is a special case, where the shifter carry out is the old value of the CPSR C
flag. The contents of Rm are used directly as the second operand.
*/

fn barrel_shifter(registers: &mut Registers, value: u32, shift_amount: u8, shift_type: ShiftType) {}

fn process_operand2(registers: &mut Registers, operand2: Operand2) {
    match operand2 {
        Operand2::Immediate { value, rotate } => {
            (value as u32).rotate_right((rotate as u32) * 2);
        }
        Operand2::Register(shifted_register) => {
            let value = registers.r[shifted_register.rm as usize];
            let shift_amount = match shifted_register.shift_amount {
                ShiftAmount::Immediate(amount) => amount,
                ShiftAmount::Register(n) => registers.r[n as usize] as u8,
            };

            //barrel_shifter(value, shift_amount, shifted_register.shift_type);
        }
    }
}

fn data_processing(
    registers: &mut Registers,
    opcode: DataOp,
    set_flags: bool,
    rn: u8,
    rd: u8,
    operand2: Operand2,
) {
    let op1 = registers.r[rn as usize];
    let op2 = process_operand2(registers, operand2);
}

// Note always use overflowing_add and overflowing_sub
pub fn execute_arm<A: AddressBus>(
    instruction: ArmInstruction,
    registers: &mut Registers,
    bus: &mut A,
) -> Option<SideEffect> {
    match instruction {
        ArmInstruction::Branch { link, offset } => Some(branch(registers, link, offset)),
        _ => None,
    }
}

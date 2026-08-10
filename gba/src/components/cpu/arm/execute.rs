use super::decode::ArmInstruction;

use crate::components::{
    bus::AddressBus,
    cpu::{Registers, SideEffect},
    utils::BitOps,
};

// Note always use overflowing_add and overflowing_sub
pub fn execute_arm<A: AddressBus>(
    instruction: ArmInstruction,
    registers: &mut Registers,
    bus: &mut A,
) -> Option<SideEffect> {
    None
}

fn update_flags(registers: &mut Registers, n: bool, z: bool, c: bool, v: bool) {
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

    if c {
        registers.set_C();
    } else {
        registers.clear_C();
    }

    if v {
        registers.set_V();
    } else {
        registers.clear_V();
    }
}

fn is_zero(result: u32) -> bool {
    result == 0
}

fn is_negative(result: u32) -> bool {
    result.is_set(31)
}

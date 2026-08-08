use super::decode::ArmInstruction;

use crate::components::{
    bus::AddressBus,
    cpu::{Registers, SideEffect},
};

pub fn execute_arm<A: AddressBus>(
    instruction: ArmInstruction,
    registers: &mut Registers,
    bus: &mut A,
) -> Option<SideEffect> {
    None
}

use crate::components::cpu::{
    Condition,
    arm::decode::{ArmInstruction, DecodedArm},
};

//**Thumb instruction set format is in 5-2**/
// the condition will be Al so that it always executed, thumb is just a subset of arm anyway
// Probably fold that the bl instrucion into arm, based on programmers model 3-8, in thumb state
// only the branch instruction is capable of conditional execution - this is the conditional branch
pub fn decode_thumb(instruction: u16) -> DecodedArm {
    DecodedArm {
        condition: Condition::Al,
        instruction: ArmInstruction::Undefined,
    }
}

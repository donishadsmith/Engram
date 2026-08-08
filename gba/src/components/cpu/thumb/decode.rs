use crate::components::cpu::{
    Condition,
    arm::decode::{ArmInstruction, DecodedArm},
};

// the condition will be Al so that it always executed, thumb is just a subset of arm anyway
// Probably fold that the bl instrucion into arm
pub fn decode_thumb(instruction: u16) -> DecodedArm {
    DecodedArm {
        condition: Condition::Al,
        instruction: ArmInstruction::Undefined,
    }
}

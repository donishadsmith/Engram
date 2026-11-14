use crate::components::cpu::MipsInstruction;

pub struct Dissasembler(pub MipsInstruction);

impl Default for Dissasembler {
    fn default() -> Self {
        Self(MipsInstruction(0))
    }
}

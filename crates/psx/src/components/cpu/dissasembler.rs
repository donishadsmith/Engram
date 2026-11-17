use crate::components::cpu::{MipsInstruction, Registers, SignOps};

pub struct Dissasembler(pub MipsInstruction);

impl Dissasembler {
    pub fn translate(&self, pc: u32) -> String {
        let rs = self.0.rs();
        let rd = self.0.rd();
        let rt = self.0.rt();
        let immediate = self.0.immediate_16().sign_extend();

        match self.0.primary_opcode() {
            0b0 => match self.0.tertiary_opcode() {
                0b100000 => format!("ADD r{rd}, r{rs}, r{rt}"),
                0b100001 => format!("ADDU r{rd}, r{rs}, r{rt}"),
                0b100100 => format!("AND r{rd}, r{rs}, r{rt}"),
                0b001101 => "BREAK".to_string(),
                0b011010 => format!("DIV r{rs}, r{rt}"),
                _ => todo!(),
            },
            _ => todo!(),
        }
    }
}

impl Default for Dissasembler {
    fn default() -> Self {
        Self(MipsInstruction(0))
    }
}

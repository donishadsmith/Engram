use crate::components::bus::AddressBus;

// Maybe a mode index helper
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Mode {
    User = 0x10,
    Fiq = 0x11,
    Irq = 0x12,
    Supervisor = 0x13,
    Abort = 0x17,
    Undefined = 0x1B,
    System = 0x1F,
}

pub struct Arm7tdmi {
    r: [u32; 16],
    banked_usr: [u32; 7],
    banked_fiq: [u32; 7],
    banked: [[u32; 2]; 5],
    cpsr: u32,
    spsr: [u32; 5],
}

impl Arm7tdmi {
    pub fn step<A: AddressBus>(&mut self, bus: &mut A) {}
}

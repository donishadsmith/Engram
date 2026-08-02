use crate::components::bus::AddressBus;

// Maybe a mode index helper
// Section 2.7 in https://vision.gel.ulaval.ca/~jflalonde/cours/1001/h19/docs/ARM7TDMI.pdf
// Mode is determined by the bits 0-4
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
#[repr(u16)]
pub enum Mode {
    Usr = 0x10,
    Fiq = 0x11,
    Irq = 0x12,
    Svc = 0x13,
    Abt = 0x17,
    Und = 0x1B,
    Sys = 0x1F,
}

impl Mode {
    fn from_cpsr(cpsr: u32) -> Mode {
        match cpsr & 0x1F {
            0x10 => Mode::Usr,
            0x11 => Mode::Fiq,
            0x12 => Mode::Irq,
            0x13 => Mode::Svc,
            0x17 => Mode::Abt,
            0x1B => Mode::Und,
            0x1F => Mode::Sys,
            _ => Mode::Usr,
        }

    }
}

pub struct Registers {
    r: [u32; 16],
    banked_usr: [u32; 7],
    banked_fiq: [u32; 7],
    banked: [[u32; 2]; 4],
    cpsr: u32,
    spsr: [u32; 5],
}

impl Registers {
    fn new() {}
}

pub struct Arm7tdmi {
    registers: Registers,
}

impl Arm7tdmi {

    pub fn step<A: AddressBus>(&mut self, bus: &mut A) {}

    fn execute_arm() {}
}

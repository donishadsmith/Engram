/*
https://problemkaputt.de/psxspx-cpu-registers.htm

All registers are 32bit wide.
  Name       Alias    Common Usage
  (R0)       zero     Constant (always 0) (this one isn't a real register)
  R1         at       Assembler temporary (destroyed by some pseudo opcodes!)
  R2-R3      v0-v1    Subroutine return values, may be changed by subroutines
  R4-R7      a0-a3    Subroutine arguments, may be changed by subroutines
  R8-R15     t0-t7    Temporaries, may be changed by subroutines
  R16-R23    s0-s7    Static variables, must be saved by subs
  R24-R25    t8-t9    Temporaries, may be changed by subroutines
  R26-R27    k0-k1    Reserved for kernel (destroyed by some IRQ handlers!)
  R28        gp       Global pointer (rarely used)
  R29        sp       Stack pointer
  R30        fp(s8)   Frame Pointer, or 9th Static variable, must be saved
  R31        ra       Return address (used so by JAL,BLTZAL,BGEZAL opcodes)
  -          pc       Program counter
  -          hi,lo    Multiply/divide results, may be changed by subroutines

*/

use crate::components::bus::Bus;
use serde::{Deserialize, Serialize};
use shared::EmulatorState;
use std::collections::HashMap;

pub trait SignOps {
    fn sign_extend(self) -> u32;
}

impl SignOps for u32 {
    fn sign_extend(self) -> u32 {
        self as u16 as i16 as i32 as u32
    }
}

use shared::traits::BitOps;

#[derive(Clone, Copy)]
pub struct MipsInstruction(pub u32);

impl MipsInstruction {
    pub fn primary_opcode(self) -> u32 {
        self.0.get_bit_range(26..32)
    }

    pub fn secondary_opcode(self) -> u32 {
        self.0.get_bit_range(0..6)
    }

    pub fn is_special(self) -> bool {
        self.primary_opcode() == 0
    }

    pub fn rs(self) -> usize {
        self.0.get_bit_range(21..26) as usize
    }

    pub fn rt(self) -> usize {
        self.0.get_bit_range(16..21) as usize
    }

    pub fn rd(self) -> usize {
        self.0.get_bit_range(11..16) as usize
    }

    pub fn immediate_16(self) -> u32 {
        self.0.get_bit_range(0..16)
    }

    pub fn offset_16(self) -> u32 {
        self.immediate_16()
    }

    pub fn code(self) -> u32 {
        self.0.get_bit_range(6..26)
    }

    pub fn cofun(self) -> u32 {
        self.0.get_bit_range(0..25)
    }

    pub fn target(self) -> u32 {
        self.0.get_bit_range(0..26)
    }

    pub fn base(self) -> u32 {
        self.rs() as u32
    }

    pub fn sa(self) -> u32 {
        self.0.get_bit_range(6..11)
    }
}

// mostly will be used for lua scripttarget to map registers properly
// for ``read_cpu_registers``
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[repr(usize)]
pub enum RegisterAlias {
    Zero = 0, // only lua can mess with this
    At = 1,
    V0 = 2,
    V1 = 3,
    A0 = 4,
    A1 = 5,
    A2 = 6,
    A3 = 7,
    T0 = 8,
    T1 = 9,
    T2 = 10,
    T3 = 11,
    T4 = 12,
    T5 = 13,
    T6 = 14,
    T7 = 15,
    S0 = 16,
    S1 = 17,
    S2 = 18,
    S3 = 19,
    S4 = 20,
    S5 = 21,
    S6 = 22,
    S7 = 23,
    T8 = 24,
    T9 = 25,
    K0 = 26,
    K1 = 27,
    Gp = 28,
    Sp = 29,
    Fp = 30,
    Ra = 31,
    Pc,
    Hi,
    Lo,
}

#[derive(Serialize, Deserialize)]
pub struct Registers {
    pub r: [u32; 32],
    pub shadow_r: [u32; 32],
    pub pc: u32,
    next_pc: u32,
    pub hi: u32,
    pub lo: u32,
}

impl Registers {
    fn new() -> Self {
        Self {
            r: [0; 32],
            shadow_r: [0; 32],
            pc: 0xBFC00000,
            next_pc: 0xBFC00004,
            hi: 0,
            lo: 0,
        }
    }

    pub fn lua_read_cpu_register(&self, alias: RegisterAlias) -> u32 {
        match alias {
            RegisterAlias::Pc => self.pc,
            RegisterAlias::Hi => self.hi,
            RegisterAlias::Lo => self.lo,
            _ => self.r[alias as usize],
        }
    }

    pub fn lua_read_shadow_register(&self, alias: RegisterAlias) -> u32 {
        match alias {
            RegisterAlias::Pc | RegisterAlias::Hi | RegisterAlias::Lo => {
                panic!("register not shadowed: {:?}", alias)
            }
            _ => self.shadow_r[alias as usize],
        }
    }

    pub fn lua_write_cpu_register(&mut self, alias: RegisterAlias, value: u32) {
        match alias {
            RegisterAlias::Pc => {
                self.pc = value;
                self.next_pc = self.pc.wrapping_add(4)
            }
            RegisterAlias::Hi => self.hi = value,
            RegisterAlias::Lo => self.lo = value,
            _ => self.r[alias as usize] = value,
        }
    }

    pub fn lua_write_shadow_register(&mut self, alias: RegisterAlias, value: u32) {
        match alias {
            RegisterAlias::Pc | RegisterAlias::Hi | RegisterAlias::Lo => {
                panic!("register not shadowed: {:?}", alias)
            }
            _ => self.shadow_r[alias as usize] = value,
        }
    }

    pub fn write_cpu_register(&mut self, index: usize, value: u32) {
        if index != 0 {
            self.r[index] = value;
        }
    }

    pub fn branch(&mut self, target_address: u32) {
        self.next_pc = target_address;
    }

    pub fn link(&mut self, destination: usize) {
        self.write_cpu_register(destination, self.next_pc);
    }
}

#[derive(Serialize, Deserialize)]
pub struct R3000a {
    pub registers: Registers,
    #[serde(skip)]
    pub breakpoint_hit: Option<u32>,
    #[serde(skip)]
    pub breakpoint_queue: HashMap<u32, EmulatorState>,
    #[serde(skip)]
    pub resume_from: Option<u32>,
}

impl R3000a {
    pub fn new() -> Self {
        Self {
            registers: Registers::new(),
            breakpoint_hit: None,
            breakpoint_queue: HashMap::new(),
            resume_from: None,
        }
    }

    pub fn step(&mut self, bus: &mut Bus) {}

    pub fn set_breakpoint(&mut self, address: u32, pause: bool) -> bool {
        let action = if pause {
            EmulatorState::Paused
        } else {
            EmulatorState::Running
        };

        self.breakpoint_queue.insert(address, action).is_none()
    }

    //fn exception(&mut self, code)

    // https://cgi.cse.unsw.edu.au/~cs3231/doc/R3000.pdf
    //  https://psx-spx.consoledev.net/cpuspecifications/
    fn add(&mut self, instruction: MipsInstruction) {
        let (value, overflow) = (self.registers.r[instruction.rs()] as i32)
            .overflowing_add(self.registers.r[instruction.rt()] as i32);
        if !overflow {
            self.registers
                .write_cpu_register(instruction.rd(), value as u32);
        } else {
            // some exception
        }
    }

    fn addi(&mut self, instruction: MipsInstruction) {
        let (value, overflow) = (self.registers.r[instruction.rs()] as i32)
            .overflowing_add(instruction.immediate_16().sign_extend() as i32);
        if !overflow {
            self.registers
                .write_cpu_register(instruction.rt(), value as u32);
        } else {
            // some exception
        }
    }

    fn addiu(&mut self, instruction: MipsInstruction) {
        self.registers.write_cpu_register(
            instruction.rt(),
            self.registers.r[instruction.rs()]
                .wrapping_add(instruction.immediate_16().sign_extend()),
        );
    }

    fn addu(&mut self, instruction: MipsInstruction) {
        self.registers.write_cpu_register(
            instruction.rd(),
            self.registers.r[instruction.rs()].wrapping_add(self.registers.r[instruction.rt()]),
        );
    }

    fn and(&mut self, instruction: MipsInstruction) {
        self.registers.write_cpu_register(
            instruction.rd(),
            self.registers.r[instruction.rs()] & self.registers.r[instruction.rt()],
        );
    }

    fn andi(&mut self, instruction: MipsInstruction) {
        self.registers.write_cpu_register(
            instruction.rt(),
            self.registers.r[instruction.rs()] & instruction.immediate_16(),
        );
    }

    fn branch_if(&mut self, condition: bool, instruction: MipsInstruction) {
        if condition {
            let offset = instruction.offset_16().sign_extend() << 2;
            self.registers
                .branch(self.registers.pc.wrapping_add(offset));
        }
    }

    fn beq(&mut self, instruction: MipsInstruction) {
        self.branch_if(
            self.registers.r[instruction.rs()] == self.registers.r[instruction.rt()],
            instruction,
        );
    }

    fn bgez(&mut self, instruction: MipsInstruction) {
        self.branch_if(self.registers.r[instruction.rs()].is_clear(31), instruction);
    }

    fn bgezal(&mut self, instruction: MipsInstruction) {
        self.registers.link(31);
        self.branch_if(self.registers.r[instruction.rs()].is_clear(31), instruction);
    }

    fn blez(&mut self, instruction: MipsInstruction) {
        self.branch_if(
            self.registers.r[instruction.rs()].is_set(31)
                || self.registers.r[instruction.rs()] == 0,
            instruction,
        );
    }

    fn bltz(&mut self, instruction: MipsInstruction) {
        self.branch_if(self.registers.r[instruction.rs()].is_set(31), instruction);
    }

    fn bltzal(&mut self, instruction: MipsInstruction) {
        self.registers.link(31);
        self.branch_if(self.registers.r[instruction.rs()].is_set(31), instruction);
    }

    fn bne(&mut self, instruction: MipsInstruction) {
        self.branch_if(
            self.registers.r[instruction.rs()] != self.registers.r[instruction.rt()],
            instruction,
        );
    }

    fn breakpoint(&mut self, instruction: MipsInstruction) {}

    fn cfcz(&mut self, instruction: MipsInstruction) {}

    fn copz(&mut self, instruction: MipsInstruction) {}

    fn ctcz(&mut self, instruction: MipsInstruction) {}

    fn div(&mut self, instruction: MipsInstruction) {}

    fn divu(&mut self, instruction: MipsInstruction) {}

    fn jump(&mut self, instruction: MipsInstruction) {
        self.registers
            .branch((self.registers.pc & 0xF0000000) | (instruction.target() << 2))
    }

    fn jal(&mut self, instruction: MipsInstruction) {
        self.registers.link(31);
        self.jump(instruction);
    }

    fn or(&mut self, instruction: MipsInstruction) {
        self.registers.write_cpu_register(
            instruction.rd(),
            self.registers.r[instruction.rs()] | self.registers.r[instruction.rt()],
        );
    }

    fn ori(&mut self, instruction: MipsInstruction) {
        self.registers.write_cpu_register(
            instruction.rt(),
            self.registers.r[instruction.rs()] | instruction.immediate_16(),
        );
    }

    fn sllv(&mut self, instruction: MipsInstruction) {
        self.registers.write_cpu_register(
            instruction.rd(),
            self.registers.r[instruction.rt()]
                << self.registers.r[instruction.rs()].get_bit_range(0..5),
        );
    }

    fn set_on(&mut self, condition: bool, destination: usize) {
        self.registers
            .write_cpu_register(destination, condition as u32)
    }

    fn slt(&mut self, instruction: MipsInstruction) {
        self.set_on(
            (self.registers.r[instruction.rs()] as i32)
                < (self.registers.r[instruction.rt()] as i32),
            instruction.rd(),
        );
    }

    fn slti(&mut self, instruction: MipsInstruction) {
        self.set_on(
            (self.registers.r[instruction.rs()] as i32)
                < (instruction.immediate_16().sign_extend() as i32),
            instruction.rt(),
        );
    }

    fn sltiu(&mut self, instruction: MipsInstruction) {
        self.set_on(
            self.registers.r[instruction.rs()] < instruction.immediate_16().sign_extend(),
            instruction.rt(),
        );
    }

    fn sltu(&mut self, instruction: MipsInstruction) {
        self.set_on(
            self.registers.r[instruction.rs()] < self.registers.r[instruction.rt()],
            instruction.rd(),
        );
    }

    fn srl(&mut self, instruction: MipsInstruction) {
        self.registers.write_cpu_register(
            instruction.rd(),
            self.registers.r[instruction.rt()] >> instruction.sa(),
        );
    }

    fn sra(&mut self, instruction: MipsInstruction) {
        self.registers.write_cpu_register(
            instruction.rd(),
            ((self.registers.r[instruction.rt()] as i32) >> instruction.sa()) as u32,
        );
    }

    fn srav(&mut self, instruction: MipsInstruction) {
        self.registers.write_cpu_register(
            instruction.rd(),
            ((self.registers.r[instruction.rt()] as i32)
                >> self.registers.r[instruction.rs()].get_bit_range(0..5)) as u32,
        );
    }

    fn srlv(&mut self, instruction: MipsInstruction) {
        self.registers.write_cpu_register(
            instruction.rd(),
            self.registers.r[instruction.rt()]
                >> self.registers.r[instruction.rs()].get_bit_range(0..5),
        );
    }

    fn xor(&mut self, instruction: MipsInstruction) {
        self.registers.write_cpu_register(
            instruction.rd(),
            self.registers.r[instruction.rs()] ^ self.registers.r[instruction.rt()],
        );
    }

    fn xori(&mut self, instruction: MipsInstruction) {
        self.registers.write_cpu_register(
            instruction.rt(),
            self.registers.r[instruction.rs()] ^ instruction.immediate_16(),
        );
    }
}

pub fn dissasemble() {}

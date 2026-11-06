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

/*
https://psx-spx.consoledev.net/cpuspecifications/#cpu-alu-opcodes
 __multu_execution_time_____________________________________________________
  Fast  (6 cycles)   rs = 00000000h..000007FFh
  Med   (9 cycles)   rs = 00000800h..000FFFFFh
  Slow  (13 cycles)  rs = 00100000h..FFFFFFFFh
  __mult_execution_time_____________________________________________________
  Fast  (6 cycles)   rs = 00000000h..000007FFh, or rs = FFFFF800h..FFFFFFFFh
  Med   (9 cycles)   rs = 00000800h..000FFFFFh, or rs = FFF00000h..FFFFF801h
  Slow  (13 cycles)  rs = 00100000h..7FFFFFFFh, or rs = 80000000h..FFF00001h
  __divu/div_execution_time________________________________________________
  Fixed (36 cycles)  no matter of rs and rt values
*/

pub mod cop0;

use crate::components::{
    bus::Bus,
    cpu::cop0::{
        Cop0, Cop0Registers,
        ExceptionCode::{self, StoreAddressError},
    },
};
use serde::{Deserialize, Serialize};
use shared::{EmulatorState, script::MemoryAccessor, traits::BitOps};
use std::{collections::HashMap, mem::take};

pub trait SignOps {
    fn sign_extend(self) -> u32;
}

impl SignOps for u32 {
    fn sign_extend(self) -> u32 {
        self as u16 as i16 as i32 as u32
    }
}

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

    pub fn base(self) -> usize {
        self.rs()
    }

    pub fn sa(self) -> u32 {
        self.0.get_bit_range(6..11)
    }

    pub fn coprocessor(self) -> usize {
        self.0.get_bit_range(26..28) as usize
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
    pub current_pc: u32,
    pub pc: u32,
    pub next_pc: u32,
    pub hi: u32,
    pub lo: u32,
    pub pending_branch: bool,
    pub in_delay_slot: bool,
    pub load_delay_1: Option<(usize, u32)>,
    pub load_delay_2: Option<(usize, u32)>,
}

impl Registers {
    fn new() -> Self {
        Self {
            r: [0; 32],
            shadow_r: [0; 32],
            current_pc: 0xBFC00000,
            pc: 0xBFC00000,
            next_pc: 0xBFC00004,
            hi: 0,
            lo: 0,
            pending_branch: false,
            in_delay_slot: false,
            load_delay_1: None,
            load_delay_2: None,
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
            _ => {
                self.cancel_load(alias as usize);
                self.r[alias as usize] = value
            }
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
        self.cancel_load(index);

        if index != 0 {
            self.r[index] = value;
        }
    }

    pub fn branch(&mut self, target_address: u32) {
        self.next_pc = target_address;
        self.pending_branch = true;
    }

    pub fn link(&mut self, destination: usize) {
        self.write_cpu_register(destination, self.next_pc);
    }

    /*
    Forgot about the load delay thing

    https://problemkaputt.de/psxspx-cpu-load-store-opcodes.htm
    Caution - Load Delay
    The loaded data is NOT available to the next opcode, ie. the
    target register isn't updated until the next opcode has completed.
    So, if the next opcode tries to read from the load destination
    register, then it would (usually) receive the OLD value of that
    register (unless an IRQ occurs between the load and next opcode,
    in that case the load would complete during IRQ handling, and so,
    the next opcode would receive the NEW value).

    https://github.com/stenzek/duckstation/blob/master/src/core/cpu_core.cpp
     */
    pub fn take_pending_load(&mut self) {
        if let Some((register, value)) = self.load_delay_1.take() {
            self.write_cpu_register(register, value)
        }

        self.load_delay_1 = self.load_delay_2.take();
    }

    pub fn cancel_load(&mut self, register: usize) {
        if let Some((pending_register, _)) = &self.load_delay_1
            && *pending_register == register
        {
            self.load_delay_1 = None;
        }

        if let Some((pending_register, _)) = &self.load_delay_2
            && *pending_register == register
        {
            self.load_delay_2 = None;
        }
    }

    pub fn set_load(&mut self, register: usize, value: u32) {
        self.cancel_load(register);

        self.load_delay_2 = Some((register, value));
    }
}

#[derive(Serialize, Deserialize)]
pub struct R3000a {
    pub registers: Registers,
    pub cop0: Cop0,
    #[serde(skip)]
    pub breakpoint_range: Option<(u32, u32)>,
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
            cop0: Cop0::new(),
            breakpoint_range: None,
            breakpoint_hit: None,
            breakpoint_queue: HashMap::new(),
            resume_from: None,
        }
    }

    pub fn step(&mut self, bus: &mut Bus) {
        let executing_address = self.registers.pc;
        if self.breakpoint_in_range(executing_address)
            && self.breakpoint_queue.contains_key(&executing_address)
            && self.resume_from != Some(executing_address)
        {
            self.breakpoint_hit = Some(executing_address);

            return;
        }

        self.resume_from = None;

        self.registers.in_delay_slot = take(&mut self.registers.pending_branch);
        self.registers.current_pc = executing_address;
        self.registers.pc = self.registers.next_pc;
        self.registers.next_pc = self.registers.next_pc.wrapping_add(4);

        // instruction fetch stuff with ``executing_address``, error stuff, blah blah
        // need instruction catching stuff, should use peek if in cache to charge scheduler
        // less, read stuff when cache miss occurs, can do cache stuff later for accuracy
        // cache probably should be [CacheLine; 256]

        self.registers.take_pending_load();

        self.execute_instruction(MipsInstruction(
            bus.read::<u32>(executing_address, MemoryAccessor::Cpu),
        ));
    }

    pub fn set_breakpoint(&mut self, address: u32, pause: bool) -> bool {
        let action = if pause {
            EmulatorState::Paused
        } else {
            EmulatorState::Running
        };

        if self.breakpoint_range.is_none() {
            self.breakpoint_range = Some((address, address));
        } else {
            let (min, max) = self.breakpoint_range.unwrap();
            self.breakpoint_range = Some((min.min(address), max.max(address)));
        }

        self.breakpoint_queue.insert(address, action).is_none()
    }

    pub fn breakpoint_in_range(&self, address: u32) -> bool {
        if let Some((min, max)) = self.breakpoint_range {
            return address >= min && address <= max;
        }

        false
    }

    pub fn remove_breakpoint(&mut self, address: u32) {
        self.breakpoint_queue.remove(&address);
        let keys = self.breakpoint_queue.keys().copied();
        self.breakpoint_range = keys.clone().min().zip(keys.max());
    }

    // 3.3.2 in See mips run (TODO; make sure to cross reference the rustation guide + ppsx-spx):
    // cop0 appears to have exception restart address register (epc), normally points to the
    // exception "victim" instruction, but when the instruction exception occurs on is delay slot before branch,
    // epc points to the branch and cause bit (31) should be set
    fn exception(&mut self, code: ExceptionCode) {
        let handler_address = self.cop0.process_exception(
            code,
            self.registers.current_pc,
            self.registers.in_delay_slot,
        );

        self.registers.pc = handler_address;
        self.registers.next_pc = self.registers.pc.wrapping_add(4);
    }

    pub fn execute_instruction(&mut self, instruction: MipsInstruction) {}

    // https://cgi.cse.unsw.edu.au/~cs3231/doc/R3000.pdf
    //  https://psx-spx.consoledev.net/cpuspecifications/
    fn add(&mut self, instruction: MipsInstruction) {
        let (value, overflow) = (self.registers.r[instruction.rs()] as i32)
            .overflowing_add(self.registers.r[instruction.rt()] as i32);
        if !overflow {
            self.registers
                .write_cpu_register(instruction.rd(), value as u32);
        } else {
            self.exception(ExceptionCode::ArithmeticOverflow);
        }
    }

    fn addi(&mut self, instruction: MipsInstruction) {
        let (value, overflow) = (self.registers.r[instruction.rs()] as i32)
            .overflowing_add(instruction.immediate_16().sign_extend() as i32);
        if !overflow {
            self.registers
                .write_cpu_register(instruction.rt(), value as u32);
        } else {
            self.exception(ExceptionCode::ArithmeticOverflow);
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
        self.registers.pending_branch = true;
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

    fn bgtz(&mut self, instruction: MipsInstruction) {
        self.branch_if(
            self.registers.r[instruction.rs()].is_clear(31)
                && self.registers.r[instruction.rs()] != 0,
            instruction,
        );
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

    fn breakpoint(&mut self, instruction: MipsInstruction) {
        self.exception(ExceptionCode::Break);
    }

    fn cfcz(&mut self, instruction: MipsInstruction) {
        todo!()
    }

    fn copz(&mut self, instruction: MipsInstruction) {
        todo!()
    }

    fn ctcz(&mut self, instruction: MipsInstruction) {
        todo!()
    }

    fn div(&mut self, instruction: MipsInstruction, bus: &mut Bus) {
        let rs = self.registers.r[instruction.rs()] as i32;
        let divisor = self.registers.r[instruction.rt()] as i32;
        if divisor == 0 {
            self.registers.lo = if rs >= 0 { 0xFFFFFFFF } else { 1 };
            self.registers.hi = rs as u32;
        } else if rs == i32::MIN && divisor == -1 {
            self.registers.lo = 0x80000000;
            self.registers.hi = 0;
        } else {
            self.registers.lo = (rs / divisor) as u32;
            self.registers.hi = (rs % divisor) as u32;
        }

        bus.add_cycles(36);
    }

    fn divu(&mut self, instruction: MipsInstruction, bus: &mut Bus) {
        // manual says undefined here, rustation and psx-spx use these undefined values
        // https://psx-spx.consoledev.net/cpuspecifications/#cpu-alu-opcodes
        let divisor = self.registers.r[instruction.rt()];
        if divisor == 0 {
            self.registers.hi = self.registers.r[instruction.rs()];
            self.registers.lo = 0xFFFFFFFF;
        } else {
            // now div and mod based on manual
            self.registers.lo = self.registers.r[instruction.rs()] / divisor;
            self.registers.hi = self.registers.r[instruction.rs()] % divisor
        }

        bus.add_cycles(36);
    }

    fn jump(&mut self, instruction: MipsInstruction) {
        self.registers
            .branch((self.registers.pc & 0xF0000000) | (instruction.target() << 2));
    }

    fn jal(&mut self, instruction: MipsInstruction) {
        self.registers.link(31);
        self.jump(instruction);
    }

    fn jalr(&mut self, instruction: MipsInstruction) {
        let address = self.registers.r[instruction.rs()];

        if !address.is_multiple_of(4) {
            self.cop0.set_bad_address(address);
            self.exception(ExceptionCode::LoadAddressError);

            return;
        };

        self.registers.link(instruction.rd());
        self.registers.branch(address);
    }

    fn jr(&mut self, instruction: MipsInstruction) {
        let address = self.registers.r[instruction.rs()];
        if !address.is_multiple_of(4) {
            self.cop0.set_bad_address(address);
            self.exception(ExceptionCode::LoadAddressError);

            return;
        };

        self.registers.branch(address);
    }

    fn create_address(&mut self, instruction: MipsInstruction) -> u32 {
        self.registers.r[instruction.base()].wrapping_add(instruction.offset_16().sign_extend())
    }

    fn lb(&mut self, instruction: MipsInstruction, bus: &mut Bus) {
        let address = self.create_address(instruction);

        self.registers.set_load(
            instruction.rt(),
            bus.read::<u8>(address, MemoryAccessor::Cpu) as i8 as i32 as u32,
        );
    }

    fn lbu(&mut self, instruction: MipsInstruction, bus: &mut Bus) {
        let address = self.create_address(instruction);

        self.registers.set_load(
            instruction.rt(),
            bus.read::<u8>(address, MemoryAccessor::Cpu) as u32,
        );
    }

    fn lh(&mut self, instruction: MipsInstruction, bus: &mut Bus) {
        let address = self.create_address(instruction);
        if !address.is_multiple_of(2) {
            self.cop0.set_bad_address(address);
            self.exception(ExceptionCode::LoadAddressError);

            return;
        }

        self.registers.set_load(
            instruction.rt(),
            bus.read::<u16>(address, MemoryAccessor::Cpu) as i16 as i32 as u32,
        );
    }

    fn lhu(&mut self, instruction: MipsInstruction, bus: &mut Bus) {
        let address = self.create_address(instruction);
        if !address.is_multiple_of(2) {
            self.cop0.set_bad_address(address);
            self.exception(ExceptionCode::LoadAddressError);

            return;
        }

        self.registers.set_load(
            instruction.rt(),
            bus.read::<u16>(address, MemoryAccessor::Cpu) as u32,
        );
    }

    fn lui(&mut self, instruction: MipsInstruction) {
        self.registers
            .write_cpu_register(instruction.rt(), instruction.immediate_16() << 16);
    }

    fn lw(&mut self, instruction: MipsInstruction, bus: &mut Bus) {
        let address = self.create_address(instruction);
        if !address.is_multiple_of(4) {
            self.cop0.set_bad_address(address);
            self.exception(ExceptionCode::LoadAddressError);

            return;
        }

        self.registers.set_load(
            instruction.rt(),
            bus.read::<u32>(address, MemoryAccessor::Cpu),
        );
    }

    fn lwcz(&mut self, instruction: MipsInstruction, bus: &mut Bus) {
        todo!()
    }

    fn get_value(&self, instruction: MipsInstruction) -> u32 {
        match self.registers.load_delay_1 {
            Some((register, pending)) if register == instruction.rt() => pending,
            _ => self.registers.r[instruction.rt()],
        }
    }

    fn lwl(&mut self, instruction: MipsInstruction, bus: &mut Bus) {
        let address = self.create_address(instruction);
        let word = bus.read::<u32>(address & !3, MemoryAccessor::Cpu);
        let offset = ((address & 3) * 8) as usize;
        let start_index = 24 - offset;
        let mut value = self.get_value(instruction);
        value.clear_bit_range(start_index..32);

        self.registers
            .set_load(instruction.rt(), value | (word << start_index));
    }

    fn lwr(&mut self, instruction: MipsInstruction, bus: &mut Bus) {
        let address = self.create_address(instruction);
        let word = bus.read::<u32>(address & !3, MemoryAccessor::Cpu);
        let offset = ((address & 3) * 8) as usize;
        let end_index = 32 - offset;
        let mut value = self.get_value(instruction);
        value.clear_bit_range(0..end_index);

        self.registers
            .set_load(instruction.rt(), value | (word >> offset));
    }

    fn mfcz(&mut self, instruction: MipsInstruction) {
        match instruction.coprocessor() {
            0 => match Cop0Registers::to_register(instruction.rd()) {
                Ok(usable_register) => self
                    .registers
                    .write_cpu_register(instruction.rt(), self.cop0.move_from(usable_register)),
                Err(reserved) => self.exception(reserved),
            },
            2 => todo!(),
            _ => self.exception(ExceptionCode::CoprocessorUnusable),
        }
    }

    fn mfhi(&mut self, instruction: MipsInstruction) {
        self.registers
            .write_cpu_register(instruction.rd(), self.registers.hi);
    }

    fn mflo(&mut self, instruction: MipsInstruction) {
        self.registers
            .write_cpu_register(instruction.rd(), self.registers.lo);
    }

    fn mtcz(&mut self, instruction: MipsInstruction) {
        match instruction.coprocessor() {
            0 => match Cop0Registers::to_register(instruction.rd()) {
                Ok(usable_register) => self
                    .cop0
                    .move_to(usable_register, self.registers.r[instruction.rt()]),
                Err(reserved) => self.exception(reserved),
            },
            2 => todo!(),
            _ => self.exception(ExceptionCode::CoprocessorUnusable),
        }
    }

    fn mthi(&mut self, instruction: MipsInstruction) {
        self.registers.hi = self.registers.r[instruction.rs()]
    }

    fn mtlo(&mut self, instruction: MipsInstruction) {
        self.registers.lo = self.registers.r[instruction.rs()]
    }

    fn mult(&mut self, instruction: MipsInstruction, bus: &mut Bus) {
        let rs = self.registers.r[instruction.rs()];
        let result = (rs as i32 as i64) * (self.registers.r[instruction.rt()] as i32 as i64);
        self.registers.hi = (result as u64).get_bit_range(32..64) as u32;
        self.registers.lo = (result as u64).get_bit_range(0..32) as u32;

        bus.add_cycles(match rs {
            0..=0x000007FF | 0xFFFFF800..=0xFFFFFFFF => 6,
            0x00000800..=0x000FFFFF | 0xFFF00000..=0xFFFFF801 => 9,
            0x00100000..=0x7FFFFFFF | 0x80000000..=0xFFF00001 => 13,
        });
    }

    fn multu(&mut self, instruction: MipsInstruction, bus: &mut Bus) {
        let rs = self.registers.r[instruction.rs()];
        let result = (rs as u64) * (self.registers.r[instruction.rt()] as u64);
        self.registers.hi = result.get_bit_range(32..64) as u32;
        self.registers.lo = result.get_bit_range(0..32) as u32;

        bus.add_cycles(match rs {
            0..=0x000007FF => 6,
            0x00000800..=0x000FFFFF => 9,
            _ => 13,
        });
    }

    fn nor(&mut self, instruction: MipsInstruction) {
        self.registers.write_cpu_register(
            instruction.rd(),
            !(self.registers.r[instruction.rs()] | self.registers.r[instruction.rt()]),
        );
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

    fn sb(&mut self, instruction: MipsInstruction, bus: &mut Bus) {
        let address = self.create_address(instruction);
        bus.write::<u8>(
            address,
            self.registers.r[instruction.rt()] as u8,
            MemoryAccessor::Cpu,
        );
    }

    fn sh(&mut self, instruction: MipsInstruction, bus: &mut Bus) {
        let address = self.create_address(instruction);
        if !address.is_multiple_of(2) {
            self.cop0.set_bad_address(address);
            self.exception(StoreAddressError);

            return;
        }

        bus.write::<u16>(
            address,
            self.registers.r[instruction.rt()] as u16,
            MemoryAccessor::Cpu,
        );
    }

    fn sll(&mut self, instruction: MipsInstruction) {
        self.registers.write_cpu_register(
            instruction.rd(),
            self.registers.r[instruction.rt()] << instruction.sa(),
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

    fn srl(&mut self, instruction: MipsInstruction) {
        self.registers.write_cpu_register(
            instruction.rd(),
            self.registers.r[instruction.rt()] >> instruction.sa(),
        );
    }

    fn srlv(&mut self, instruction: MipsInstruction) {
        self.registers.write_cpu_register(
            instruction.rd(),
            self.registers.r[instruction.rt()]
                >> self.registers.r[instruction.rs()].get_bit_range(0..5),
        );
    }

    fn sub(&mut self, instruction: MipsInstruction) {
        let (value, overflow) = (self.registers.r[instruction.rs()] as i32)
            .overflowing_sub(self.registers.r[instruction.rt()] as i32);
        if !overflow {
            self.registers
                .write_cpu_register(instruction.rd(), value as u32);
        } else {
            self.exception(ExceptionCode::ArithmeticOverflow);
        }
    }

    fn subu(&mut self, instruction: MipsInstruction) {
        self.registers.write_cpu_register(
            instruction.rd(),
            self.registers.r[instruction.rs()].wrapping_sub(self.registers.r[instruction.rt()]),
        );
    }

    fn sw(&mut self, instruction: MipsInstruction, bus: &mut Bus) {
        let address = self.create_address(instruction);
        if !address.is_multiple_of(4) {
            self.cop0.set_bad_address(address);
            self.exception(ExceptionCode::StoreAddressError);

            return;
        }

        bus.write::<u32>(
            address,
            self.registers.r[instruction.rt()],
            MemoryAccessor::Cpu,
        );
    }

    fn swcz(&mut self, instruction: MipsInstruction, bus: &mut Bus) {
        todo!()
    }

    fn swl(&mut self, instruction: MipsInstruction, bus: &mut Bus) {
        todo!()
    }

    fn swr(&mut self, instruction: MipsInstruction, bus: &mut Bus) {
        todo!()
    }

    fn syscall(&mut self, instruction: MipsInstruction) {
        self.exception(ExceptionCode::SysCall);
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

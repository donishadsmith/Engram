use crate::components::{
    bios::handle_swi,
    bus::{AccessType, AddressBus},
    utils::BitOps,
};
/*
The ARM University Program, ARM Architecture Fundamentals: https://www.youtube.com/watch?v=7LqPJGnBPMM

- Load/store architecture: only load and store can deal with memory, hence processing memory values requires loading
into source register, execution of some instruction, then storing value to some register. No instructions like certain
GBC instructions that can add some value to some value at a specific memory address (ADD A, (HL)) in one instruction.

- Each processor mode has access to its own stack space and its own private subset of registers (banked)

13 general purpose registers:
    Low registers: r0-r7
    High registers: r8-r12

3 special registers
    stack pointer (sp) - r13
    link register (lr) - r14
    program counter (pc) - r15

- privileged state can manually change mode bits, set autoomatically when a mode change occurs due to exception (e.g., interrupt)

- Exception Handling Steps:
    1) Save processor status
        - Copy cpsr into spsr_<mode> to hold snapshot of current mode processor state
        - Stores the return address in lr_<mode>
    2) Change processor status for exception
        - Mode field bits
        - ARM or Thumb state
        - interrupt disable bits (if needed)
        - sets pc to vector address
    3) Execute
    4) Return to main
        - Restore cpsr from spsr_<mode>
        - Restore pc from lr_<mode>

Instructions
- Each instruction is conditional; hence it can be a Noop if condition flags fail
- By default condition flags are not changed

Example Instructions:
SUB r0, r1, #5 -> r0 = r1 - 5
ADD r2, r3, r3, LSL #2 (add with inline shift) -> r2 = r3 + (r3 * 4)
ANDS r4, r4, #0x20 (suffix "s" means alu condition codes will reflect operation results) -> r4 &= 0x20;
ADDEQ r5, r5, r6 (instruction only executes if the EQ condition is true at the executation stage of pipeline else its noop) -> if (EQ) r5 += r6
B <Label> (Branch instruction/Pc-relative branch)
LDR r0, [r1] => r0 = *r1
STRNEB r2, [r3, r4] => if (NE) *(r3 + r4) = r2 [B is byte size stor; least significant byte of r2 to the address r3 + r4 but only when NE is true]
*/

// https://users.ece.utexas.edu/~mcdermot/arch/articles/ARM/arm7tdmi_instruction_set_reference.pdf

// Maybe a mode index helper
// Section 2.7 in https://vision.gel.ulaval.ca/~jflalonde/cours/1001/h19/docs/ARM7TDMI.pdf
// Mode is determined by the bits 0-4

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum BankingAction {
    Snapshot,
    Restore,
}

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
enum ProcessorState {
    Arm,
    Thumb,
}

impl ProcessorState {
    fn from_cpsr(cpsr: u32) -> ProcessorState {
        match (cpsr >> 5) & 0x01 {
            0 => ProcessorState::Arm,
            1 => ProcessorState::Thumb,
            _ => unreachable!(),
        }
    }
}

#[repr(usize)]
enum CpuFlag {
    N = 31,
    Z = 30,
    C = 29,
    V = 28,
}

enum VectorTable {
    Fiq = 0x1C,
    Irq = 0x18,
    Reserved = 0x14,
    Abt = 0x10,
    PrefetchAbt = 0x0C,
    SoftwareIrq = 0x08,
    Und = 0x04,
    Reset = 0x00,
}

enum Exception {
    Irq,
    SoftwareInterrupt,
    Undefined,
}

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
enum ProcessorMode {
    Usr = 0x10, // Typical mode most tasks run on (Unprivileged)
    Fiq = 0x11, // Entered when high priority (fast) interrupt is raised (Privileged) - Banks out r8-14, and spsr
    Irq = 0x12, // Entered when normal priority interrupt is raised (Privileged) - Banks out register 13 (sp), 14 (lr), and spsr
    Svc = 0x13, // Entered on reset and when a supervisor call instruction (SVC) us executed- Banks out register 13 (sp), 14 (lr), and spsr
    Abt = 0x17, // Handle memory access violations (Priileged) - Banks out register 13 (sp), 14 (lr), and spsr
    Und = 0x1B, // Undefined instructions (Privileged) - Banks out register 13 (sp), 14 (lr), and spsr
    Sys = 0x1F, // Mode using same registers as User mode (Privileged) - No banks just uses User
}

impl ProcessorMode {
    fn from_cpsr(cpsr: u32) -> ProcessorMode {
        match cpsr & 0x1F {
            0x10 => ProcessorMode::Usr,
            0x11 => ProcessorMode::Fiq,
            0x12 => ProcessorMode::Irq,
            0x13 => ProcessorMode::Svc,
            0x17 => ProcessorMode::Abt,
            0x1B => ProcessorMode::Und,
            0x1F => ProcessorMode::Sys,
            _ => ProcessorMode::Usr,
        }
    }

    fn spsr_index(self) -> usize {
        match self {
            ProcessorMode::Usr | ProcessorMode::Sys => 0,
            ProcessorMode::Fiq => 1,
            ProcessorMode::Irq => 2,
            ProcessorMode::Svc => 3,
            ProcessorMode::Abt => 4,
            ProcessorMode::Und => 5,
        }
    }

    fn sp_and_lr_index(self) -> usize {
        match self {
            ProcessorMode::Usr | ProcessorMode::Sys => 0,
            ProcessorMode::Fiq => 1,
            ProcessorMode::Irq => 2,
            ProcessorMode::Svc => 3,
            ProcessorMode::Abt => 4,
            ProcessorMode::Und => 5,
        }
    }

    fn high_register_index(self) -> usize {
        match self {
            ProcessorMode::Usr | ProcessorMode::Sys => 0,
            ProcessorMode::Fiq => 1,
            _ => unreachable!(),
        }
    }
}

// https://support.arm.com/documentation/ddi0029/g/introduction/instruction-set-summary/format-summary
// https://support.arm.com/documentation/ddi0029/g/introduction/instruction-set-summary/arm-instruction-summary?lang=en
// https://www.gregorygaines.com/blog/decoding-the-arm7tdmi-instruction-set-game-boy-advance/
// ***https://support.arm.com/documentation/ddi0027/latest/ - Page 30*** <- THIS IS THE ARM7DI DATA SHEET
enum DataOp {
    And,
    Eor,
    Sub,
    Rsb,
    Add,
    Adc,
    Sbc,
    Rsc,
    Tst,
    Teq,
    Cmp,
    Cmn,
    Orr,
    Mov,
    Bic,
    Mvn,
}

impl DataOp {
    // [24:21]
    fn from_bits(bits: u32) -> Self {
        match bits & 0xF {
            0b0000 => DataOp::And,
            0b0001 => DataOp::Eor,
            0b0010 => DataOp::Sub,
            0b0011 => DataOp::Rsb,
            0b0100 => DataOp::Add,
            0b0101 => DataOp::Adc,
            0b0110 => DataOp::Sbc,
            0b0111 => DataOp::Rsc,
            0b1000 => DataOp::Tst,
            0b1001 => DataOp::Teq,
            0b1010 => DataOp::Cmp,
            0b1011 => DataOp::Cmn,
            0b1100 => DataOp::Orr,
            0b1101 => DataOp::Mov,
            0b1110 => DataOp::Bic,
            0b1111 => DataOp::Mvn,
            _ => unreachable!(),
        }
    }
}

enum ShiftType {
    LogicalLeft,
    LogicalRight,
    ArithmeticRight,
    RotateRight,
}

// [6:5]
fn from_bits(bits: u32) -> ShiftType {
    match bits & 0x3 {
        0b00 => ShiftType::LogicalLeft,
        0b01 => ShiftType::LogicalRight,
        0b10 => ShiftType::ArithmeticRight,
        0b11 => ShiftType::RotateRight,
        _ => unreachable!(),
    }
}

enum ShiftAmount {
    Immediate(u8),
    Register(u8),
}

enum Operand2 {
    Immediate {
        value: u8,
        rotate: u8,
    },
    Register {
        rm: u8,
        shift_type: ShiftType,
        shift_amount: ShiftAmount,
    },
}

enum ArmInstruction {
    DataProcessing {
        opcode: DataOp,
        set_flags: bool,
        rn: u8,
        rd: u8,
        operand2: Operand2,
    },
    Undefined,
}

pub struct Registers {
    pub r: [u32; 16],
    banked_high_registers: [[u32; 5]; 2],
    banked_special_registers: [[u32; 2]; 6],
    banked_spsr: [u32; 6],
    cpsr: u32, // Holds current mode, status flags; bits 31-28 (N, Z, C, V, Q), mode bits (lower 5 bits), state bits
               // bit 5 (T) is ARM or THUMB
}

impl Registers {
    pub fn new() -> Self {
        Self {
            r: [0; 16],
            banked_high_registers: [[0; 5]; 2],
            banked_special_registers: [[0; 2]; 6],
            banked_spsr: [0; 6],
            cpsr: 0,
        }
    }

    pub fn N(&self) -> bool {
        self.cpsr.is_set(CpuFlag::N as usize)
    }

    pub fn set_N(&mut self) {
        self.cpsr.set_bit(CpuFlag::N as usize)
    }

    pub fn Z(&self) -> bool {
        self.cpsr.is_set(CpuFlag::Z as usize)
    }

    pub fn set_Z(&mut self) {
        self.cpsr.set_bit(CpuFlag::Z as usize)
    }

    pub fn C(&self) -> bool {
        self.cpsr.is_set(CpuFlag::C as usize)
    }

    pub fn set_C(&mut self) {
        self.cpsr.set_bit(CpuFlag::C as usize)
    }

    pub fn V(&self) -> bool {
        self.cpsr.is_set(CpuFlag::V as usize)
    }

    pub fn set_V(&mut self) {
        self.cpsr.set_bit(CpuFlag::V as usize)
    }

    pub fn irq_enabled(&self) -> bool {
        self.cpsr.is_set(7)
    }

    pub fn fiq_enabled(&self) -> bool {
        self.cpsr.is_set(6)
    }
}

#[derive(Clone, Copy, Debug, PartialEq)]
enum FetchedOpcode {
    Arm(u32),
    Thumb(u16),
}

// Store both fetch and decoded instrucion
struct Pipeline {
    fetched: Option<FetchedOpcode>,
    decoded: Option<FetchedOpcode>,
}

impl Pipeline {
    fn new() -> Self {
        Self {
            fetched: None,
            decoded: None,
        }
    }

    fn flush(&mut self) {
        self.fetched = None;
        self.decoded = None;
    }

    fn advance(&mut self, new_fetch: FetchedOpcode) -> Option<FetchedOpcode> {
        let executing = self.decoded;
        self.decoded = self.fetched;
        self.fetched = Some(new_fetch);

        executing
    }
}

#[derive(Eq, PartialEq)]
pub enum HaltState {
    Running,
    Halted,
    IntrWait { flags: u16 },
}

pub enum CpuRequest {
    Swi(u32),
}

pub struct Arm7tdmi {
    pub registers: Registers,
    pipeline: Pipeline,
    pub halt_state: HaltState,
    branched: bool,
}

impl Arm7tdmi {
    pub fn new() -> Self {
        Self {
            registers: Registers::new(),
            pipeline: Pipeline::new(),
            halt_state: HaltState::Running,
            branched: false,
        }
    }

    // https://github.com/Warpten/CowBite/blob/master/GBA.cpp#L70
    pub fn skip_boot(&mut self) {
        self.registers.r[13] = 0x03007F00;
        self.registers.r[15] = 0x08000000;
        self.registers.cpsr = 0x00000013;

        self.registers.banked_special_registers[3][0] = 0x03007FE0; // sp_svc
        self.registers.banked_special_registers[2][0] = 0x03007FA0; // sp_irq
        self.registers.banked_special_registers[0][0] = 0x03007F00; // sp_usr/sys

        self.pipeline.flush();
    }

    pub fn step<A: AddressBus>(&mut self, bus: &mut A) {
        self.branched = false;
        let new_fetch = match self.state() {
            ProcessorState::Arm => {
                FetchedOpcode::Arm(bus.read_u32(self.registers.r[15], AccessType::Sequential))
            } // TODO: Just assume always sequential for now
            ProcessorState::Thumb => {
                FetchedOpcode::Thumb(bus.read_u16(self.registers.r[15], AccessType::Sequential))
            }
        };

        let executing_instruction = self.pipeline.advance(new_fetch);
        let executing_address = self.registers.r[15].wrapping_sub(2 * self.pc_offset());

        let mut cpu_request: Option<CpuRequest> = None;
        if let Some(opcode) = executing_instruction {
            cpu_request = match opcode {
                FetchedOpcode::Arm(word) => {
                    if !self.is_noop(word) {
                        let instruction = self.decode_arm(word);
                        self.execute_arm(instruction)
                    } else {
                        None
                    }
                }
                FetchedOpcode::Thumb(halfword) => {
                    let instruction = self.decode_thumb(halfword);
                    // self.execute_thumb(instruction)
                    Some(CpuRequest::Swi(0x01))
                }
            };
        }

        if let Some(request) = cpu_request {
            match request {
                CpuRequest::Swi(function) => {
                    handle_swi(function, &mut self.registers, &mut self.halt_state, bus)
                }
            }
        }

        if let HaltState::IntrWait { .. } = self.halt_state {
            self.registers.r[15] = executing_address;
            self.flush_pipeline();
        }

        if !self.branched {
            self.increment_pc();
        }
    }

    fn flush_pipeline(&mut self) {
        self.pipeline.flush();
        self.branched = true;
    }

    pub fn is_halted(&self) -> bool {
        self.halt_state != HaltState::Running
    }

    pub fn awake(&mut self, pending: usize) {
        //self.halt_state = HaltState::Running;
    }

    fn decode_arm(&self, word: u32) -> ArmInstruction {
        ArmInstruction::Undefined
    }

    fn execute_arm(&self, instruction: ArmInstruction) -> Option<CpuRequest> {
        None
    }

    fn decode_thumb(&self, halfword: u16) {}

    fn execute_thumb(&self) -> Option<CpuRequest> {
        None
    }

    fn mode(&self) -> ProcessorMode {
        ProcessorMode::from_cpsr(self.registers.cpsr)
    }

    fn change_mode(&mut self, mode: ProcessorMode) {
        self.set_sp_and_pc(BankingAction::Snapshot);
        self.registers.cpsr.clear_bit_range(0..5);
        self.registers.cpsr |= mode as u32;

        self.set_sp_and_pc(BankingAction::Restore);
    }

    fn state(&self) -> ProcessorState {
        ProcessorState::from_cpsr(self.registers.cpsr)
    }

    fn change_state(&mut self, state: ProcessorState) {
        self.registers.cpsr.clear_bit(5);
        if state == ProcessorState::Thumb {
            self.registers.cpsr.set_bit(5);
        }
    }

    // Likely will never trigger because apparantly the gba never enters FIQ but keep anyway
    fn set_high_registers(&mut self, banking_action: BankingAction) {
        let mode = self.mode();
        let offset = 8;
        let index = mode.high_register_index();
        for i in 0..5 {
            if banking_action == BankingAction::Restore {
                self.registers.r[i + offset] = self.registers.banked_high_registers[index][i];
            } else {
                self.registers.banked_high_registers[index][i] = self.registers.r[i + offset];
            }
        }
    }

    fn set_sp_and_pc(&mut self, banking_action: BankingAction) {
        let mode = self.mode();
        let index = mode.sp_and_lr_index();
        let offset = 13;

        if banking_action == BankingAction::Restore {
            for i in 0..2 {
                self.registers.r[i + offset] = self.registers.banked_special_registers[index][i];
            }
        } else {
            self.registers.banked_special_registers[index][0] = self.registers.r[13];
            self.registers.banked_special_registers[index][1] = self.registers.r[14];
        }
    }

    fn snapshot_cpsr_to_target_spsr(&mut self, target_mode: ProcessorMode) {
        let index = target_mode.spsr_index();
        self.registers.banked_spsr[index] = self.registers.cpsr
    }

    fn restore_mode(&mut self) {
        self.set_sp_and_pc(BankingAction::Snapshot);
        let mode = self.mode();
        let index = mode.spsr_index();
        let cpsr = self.registers.banked_spsr[index];

        self.registers.cpsr = cpsr;

        self.set_sp_and_pc(BankingAction::Restore);
    }

    fn jump() {}

    fn call() {}

    fn pop() {}

    fn push() {}

    fn pc_offset(&self) -> u32 {
        if self.state() == ProcessorState::Arm {
            4
        } else {
            2
        }
    }

    fn increment_pc(&mut self) {
        let offset = self.pc_offset();
        self.registers.r[15] = self.registers.r[15].wrapping_add(offset);
    }

    pub fn raise_irq(&mut self) {
        self.snapshot_cpsr_to_target_spsr(ProcessorMode::Irq);
        self.change_mode(ProcessorMode::Irq);
        self.registers.cpsr.set_bit(7);
        self.raise_exception(Exception::Irq);
    }

    fn raise_exception(&mut self, exception: Exception) {}

    fn is_noop(&self, opcode: u32) -> bool {
        let opcode_flags = opcode >> 28;
        let regs = &self.registers;

        // cpsr condition flag order NZCV, is 31:28
        // https://support.arm.com/documentation/ddi0027/latest/ - page 26
        // https://support.arm.com/documentation/ddi0029/g/introduction/instruction-set-summary/arm-instruction-summary?lang=en - Table 1.6
        let should_execute = match opcode_flags {
            0b0000 => regs.Z(),
            0b0001 => !regs.Z(),
            0b0010 => regs.C(),
            0b0011 => !regs.C(),
            0b0100 => regs.N(),
            0b0101 => !regs.N(),
            0b0110 => regs.V(),
            0b0111 => !regs.V(),
            0b1000 => regs.C() && !regs.Z(),
            0b1001 => !regs.C() || regs.Z(),
            0b1010 => {
                regs.cpsr.get_bit(CpuFlag::N as usize) == regs.cpsr.get_bit(CpuFlag::V as usize)
            }
            0b1011 => {
                regs.cpsr.get_bit(CpuFlag::N as usize) != regs.cpsr.get_bit(CpuFlag::V as usize)
            }
            0b1100 => {
                !regs.Z()
                    && regs.cpsr.get_bit(CpuFlag::N as usize)
                        == regs.cpsr.get_bit(CpuFlag::V as usize)
            }
            0b1101 => {
                regs.Z()
                    || regs.cpsr.get_bit(CpuFlag::N as usize)
                        != regs.cpsr.get_bit(CpuFlag::V as usize)
            }
            0b1110 => true,
            0b1111 => false,
            _ => unreachable!(),
        };

        !should_execute
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const N: u32 = 0x80000000;
    const Z: u32 = 0x40000000;
    const C: u32 = 0x20000000;
    const V: u32 = 0x10000000;

    const GT: u32 = 0b1100 << 28;
    const LE: u32 = 0b1101 << 28;

    #[test]
    fn test_condition_flag_gt() {
        let mut cpu = Arm7tdmi::new();

        cpu.registers.cpsr = N;
        assert!(cpu.is_noop(GT));

        cpu.registers.cpsr = N | V;
        assert!(!cpu.is_noop(GT));

        cpu.registers.cpsr = N | C;
        assert!(cpu.is_noop(GT));
    }

    #[test]
    fn test_condition_flag_le() {
        let mut cpu = Arm7tdmi::new();

        cpu.registers.cpsr = N | Z;
        assert!(!cpu.is_noop(LE));

        cpu.registers.cpsr = N | V;
        assert!(cpu.is_noop(LE));

        cpu.registers.cpsr = N | C;
        assert!(!cpu.is_noop(LE));
    }

    #[test]
    fn test_raise_irq() {
        let mut cpu = Arm7tdmi::new();

        cpu.registers.cpsr = ProcessorMode::Sys as u32;
        assert_eq!(cpu.mode(), ProcessorMode::Sys);

        cpu.raise_irq();
        assert_eq!(cpu.mode(), ProcessorMode::Irq);

        cpu.restore_mode();
        assert_eq!(cpu.mode(), ProcessorMode::Sys);
    }

    #[test]
    fn test_change_state() {
        let mut cpu = Arm7tdmi::new();
        assert_eq!(cpu.state(), ProcessorState::Arm);

        cpu.change_state(ProcessorState::Thumb);
        assert_eq!(cpu.state(), ProcessorState::Thumb);

        cpu.change_state(ProcessorState::Arm);
        assert_eq!(cpu.state(), ProcessorState::Arm);
    }
}

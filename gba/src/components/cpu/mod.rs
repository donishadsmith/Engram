pub mod arm;
pub mod thumb;

use arm::{decode::*, execute::*};
use thumb::decode::*;

use crate::components::{
    bios::handle_swi,
    bus::{AccessType, Bus},
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

/*
Maybe bring the table back in the future, if add direct bios support
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
*/

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum ProcessorState {
    Arm,
    Thumb,
}

impl ProcessorState {
    fn from_cpsr(cpsr: u32) -> ProcessorState {
        match cpsr.get_bit(5) {
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

#[derive(Clone, Copy, Debug, PartialEq)]
#[repr(u8)]
pub enum Condition {
    Eq = 0b0000,
    Ne = 0b0001,
    Cs = 0b0010,
    Cc = 0b0011,
    Mi = 0b0100,
    Pl = 0b0101,
    Vs = 0b0110,
    Vc = 0b0111,
    Hi = 0b1000,
    Ls = 0b1001,
    Ge = 0b1010,
    Lt = 0b1011,
    Gt = 0b1100,
    Le = 0b1101,
    Al = 0b1110,
    Never = 0b1111,
}

impl Condition {
    fn from_bits(bits: u8) -> Condition {
        match bits {
            0b0000 => Condition::Eq,
            0b0001 => Condition::Ne,
            0b0010 => Condition::Cs,
            0b0011 => Condition::Cc,
            0b0100 => Condition::Mi,
            0b0101 => Condition::Pl,
            0b0110 => Condition::Vs,
            0b0111 => Condition::Vc,
            0b1000 => Condition::Hi,
            0b1001 => Condition::Ls,
            0b1010 => Condition::Ge,
            0b1011 => Condition::Lt,
            0b1100 => Condition::Gt,
            0b1101 => Condition::Le,
            0b1110 => Condition::Al,
            0b1111 => Condition::Never,
            _ => unreachable!(),
        }
    }
}

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
#[repr(u32)]
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
        match cpsr.get_bit_range(0..5) {
            0x10 => ProcessorMode::Usr,
            0x11 => ProcessorMode::Fiq,
            0x12 => ProcessorMode::Irq,
            0x13 => ProcessorMode::Svc,
            0x17 => ProcessorMode::Abt,
            0x1B => ProcessorMode::Und,
            0x1F => ProcessorMode::Sys,
            _ => {
                dbg!("invalid mode: {}", cpsr.get_bit_range(0..5));
                ProcessorMode::Usr
            }
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
            _ => 0,
        }
    }
}

// https://support.arm.com/documentation/ddi0029/g/introduction/instruction-set-summary/format-summary
// https://support.arm.com/documentation/ddi0029/g/introduction/instruction-set-summary/arm-instruction-summary?lang=en
// https://www.gregorygaines.com/blog/decoding-the-arm7tdmi-instruction-set-game-boy-advance/
// ***https://www.dwedit.org/files/ARM7TDMI.pdf - Page 30*** <- THIS IS THE ARM7TDMI DATA SHEET

#[derive(Clone, Copy, Debug, PartialEq)]
enum FetchedInstruction {
    Arm(u32),
    Thumb(u16),
}

#[derive(Eq, PartialEq)]
pub enum HaltState {
    Running,
    Halted,
    IntrWait,
    TestExit(u32),
}

pub enum SideEffect {
    Branch(u32),
    BranchRestoreCpsr(u32),
    Swi(u32),
}

pub struct Registers {
    pub r: [u32; 16],
    pub banked_high_registers: [[u32; 5]; 2],
    pub banked_special_registers: [[u32; 2]; 6],
    pub banked_spsr: [u32; 6],
    pub cpsr: u32, // Holds current mode, status flags; bits 31-28 (N, Z, C, V, Q), mode bits (lower 5 bits), state bits
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

    fn is_n_set(&self) -> bool {
        self.cpsr.is_set(CpuFlag::N as usize)
    }

    fn set_n(&mut self) {
        self.cpsr.set_bit(CpuFlag::N as usize)
    }

    fn clear_n(&mut self) {
        self.cpsr.clear_bit(CpuFlag::N as usize)
    }

    fn is_z_set(&self) -> bool {
        self.cpsr.is_set(CpuFlag::Z as usize)
    }

    fn set_z(&mut self) {
        self.cpsr.set_bit(CpuFlag::Z as usize)
    }

    fn clear_z(&mut self) {
        self.cpsr.clear_bit(CpuFlag::Z as usize)
    }

    fn is_c_set(&self) -> bool {
        self.cpsr.is_set(CpuFlag::C as usize)
    }

    fn set_c(&mut self) {
        self.cpsr.set_bit(CpuFlag::C as usize)
    }

    fn clear_c(&mut self) {
        self.cpsr.clear_bit(CpuFlag::C as usize)
    }

    fn is_v_set(&self) -> bool {
        self.cpsr.is_set(CpuFlag::V as usize)
    }

    fn set_v(&mut self) {
        self.cpsr.set_bit(CpuFlag::V as usize)
    }

    fn clear_v(&mut self) {
        self.cpsr.clear_bit(CpuFlag::V as usize)
    }

    pub fn irq_enabled(&self) -> bool {
        !self.cpsr.is_set(7)
    }

    pub fn enable_irq(&mut self) {
        self.cpsr.clear_bit(7)
    }

    pub fn disable_irq(&mut self) {
        self.cpsr.set_bit(7)
    }

    pub fn fiq_enabled(&self) -> bool {
        !self.cpsr.is_set(6)
    }

    fn mode(&self) -> ProcessorMode {
        ProcessorMode::from_cpsr(self.cpsr)
    }

    fn set_mode(&mut self, mode: ProcessorMode) {
        let old_mode = self.mode();

        self.cpsr.clear_bit_range(0..5);
        self.cpsr |= mode as u32;

        self.bank_registers(old_mode, self.mode());
    }

    fn state(&self) -> ProcessorState {
        ProcessorState::from_cpsr(self.cpsr)
    }

    fn set_state(&mut self, state: ProcessorState) {
        self.cpsr.clear_bit(5);
        if state == ProcessorState::Thumb {
            self.cpsr.set_bit(5);
        }
    }

    fn bank_registers(&mut self, old_mode: ProcessorMode, new_mode: ProcessorMode) {
        let (old_sp, new_sp) = (old_mode.sp_and_lr_index(), new_mode.sp_and_lr_index());
        if old_sp != new_sp {
            self.banked_special_registers[old_sp][0] = self.r[13];
            self.banked_special_registers[old_sp][1] = self.r[14];
            self.r[13] = self.banked_special_registers[new_sp][0];
            self.r[14] = self.banked_special_registers[new_sp][1];
        }

        let (old_high_index, new_high_index) = (
            old_mode.high_register_index(),
            new_mode.high_register_index(),
        );
        if old_high_index != new_high_index {
            for i in 0..5 {
                self.banked_high_registers[old_high_index][i] = self.r[8 + i];
                self.r[8 + i] = self.banked_high_registers[new_high_index][i];
            }
        }
    }

    fn set_spsr(&mut self, cpsr: u32) {
        let index = self.mode().spsr_index();
        self.banked_spsr[index] = cpsr
    }

    fn has_spsr(&self) -> bool {
        !matches!(self.mode(), ProcessorMode::Usr | ProcessorMode::Sys)
    }

    fn restore_cpsr_from_spsr(&mut self) {
        if !self.has_spsr() {
            return;
        }

        let old_mode = self.mode();
        self.cpsr = self.banked_spsr[old_mode.spsr_index()];
        self.bank_registers(old_mode, self.mode());
    }

    fn reset_to_boot(&mut self) {
        self.r[13] = 0x03007F00;
        self.r[15] = 0x08000000;
        self.cpsr = 0x0000001F;

        self.banked_special_registers[3][0] = 0x03007FE0; // sp_svc
        self.banked_special_registers[2][0] = 0x03007FA0; // sp_irq
        self.banked_special_registers[0][0] = 0x03007F00; // sp_usr/sys
    }

    pub fn soft_reset(&mut self, entry_point: u32) {
        self.r[0..13].fill(0);
        self.r[14] = entry_point;

        self.banked_special_registers[3] = [0x03007FE0, 0];
        self.banked_special_registers[2] = [0x03007FA0, 0];
        self.banked_special_registers[0][0] = 0x03007F00;
        self.banked_spsr[3] = 0;
        self.banked_spsr[2] = 0;

        self.cpsr = ProcessorMode::Sys as u32;
        self.r[13] = 0x03007F00;
    }

    fn condition_passed(&self, condition: Condition) -> bool {
        // cpsr condition flag order NZCV, is 31:28
        // https://support.arm.com/documentation/ddi0027/latest/ - page 26
        // https://support.arm.com/documentation/ddi0029/g/introduction/instruction-set-summary/arm-instruction-summary?lang=en - Table 1.6
        let passed = match condition {
            Condition::Eq => self.is_z_set(),
            Condition::Ne => !self.is_z_set(),
            Condition::Cs => self.is_c_set(),
            Condition::Cc => !self.is_c_set(),
            Condition::Mi => self.is_n_set(),
            Condition::Pl => !self.is_n_set(),
            Condition::Vs => self.is_v_set(),
            Condition::Vc => !self.is_v_set(),
            Condition::Hi => self.is_c_set() && !self.is_z_set(),
            Condition::Ls => !self.is_c_set() || self.is_z_set(),
            Condition::Ge => {
                self.cpsr.get_bit(CpuFlag::N as usize) == self.cpsr.get_bit(CpuFlag::V as usize)
            }
            Condition::Lt => {
                self.cpsr.get_bit(CpuFlag::N as usize) != self.cpsr.get_bit(CpuFlag::V as usize)
            }
            Condition::Gt => {
                !self.is_z_set()
                    && self.cpsr.get_bit(CpuFlag::N as usize)
                        == self.cpsr.get_bit(CpuFlag::V as usize)
            }
            Condition::Le => {
                self.is_z_set()
                    || self.cpsr.get_bit(CpuFlag::N as usize)
                        != self.cpsr.get_bit(CpuFlag::V as usize)
            }
            Condition::Al => true,
            Condition::Never => false,
        };

        passed
    }

    fn increment_pc(&mut self) {
        let offset = self.pc_offset();
        self.r[15] = self.r[15].wrapping_add(offset);
    }

    fn pc_offset(&self) -> u32 {
        if self.state() == ProcessorState::Arm {
            4
        } else {
            2
        }
    }

    fn copy_pc_to_lr(&mut self) {
        // Reminder that pc is always +2 instructions ahead of the current executing address,
        // Need to copy the pc - 1 instruction, to return to the instruction that has not been
        // executed yet.
        self.r[14] = self.r[15].wrapping_sub(self.pc_offset());
    }
}

// https://support.arm.com/documentation/ddi0029/g/introduction/about-the-arm7tdmi-core/the-instruction-pipeline
struct Pipeline {
    fetched: Option<FetchedInstruction>,
    decoded: Option<DecodedArm>,
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

    fn advance(&mut self, new_fetch: FetchedInstruction) -> Option<DecodedArm> {
        let executing = self.decoded.take();
        self.decoded = self.fetched.take().map(decode);
        self.fetched = Some(new_fetch);

        executing
    }
}

fn decode(opcode: FetchedInstruction) -> DecodedArm {
    match opcode {
        FetchedInstruction::Arm(word) => decode_arm(word),
        FetchedInstruction::Thumb(halfword) => decode_thumb(halfword),
    }
}

pub struct Arm7tdmi {
    pub registers: Registers,
    pipeline: Pipeline,
    pub halt_state: HaltState,
    branched: bool,
    next_fetch_access: AccessType,
}

impl Arm7tdmi {
    pub fn new() -> Self {
        Self {
            registers: Registers::new(),
            pipeline: Pipeline::new(),
            halt_state: HaltState::Running,
            branched: false,
            next_fetch_access: AccessType::Sequential,
        }
    }

    // https://github.com/Warpten/CowBite/blob/master/GBA.cpp#L70
    pub fn skip_boot(&mut self) {
        self.registers.reset_to_boot();
        self.pipeline.flush();
    }

    pub fn step(&mut self, bus: &mut Bus) {
        self.branched = false;
        let access = self.next_fetch_access;
        self.next_fetch_access = AccessType::Sequential;
        let new_fetch = match self.registers.state() {
            ProcessorState::Arm => {
                FetchedInstruction::Arm(bus.read_u32(self.registers.r[15], access))
            }
            ProcessorState::Thumb => {
                FetchedInstruction::Thumb(bus.read_u16(self.registers.r[15], access))
            }
        };

        let latest_instruction = match new_fetch {
            FetchedInstruction::Arm(instruction) => instruction,
            FetchedInstruction::Thumb(instruction) => {
                instruction as u32 | (instruction as u32) << 16
            }
        };

        bus.last_instruction_read = latest_instruction;

        let decoded_instruction = self.pipeline.advance(new_fetch);

        // Assumes pc is +8 (arm) or +4 (thumb) aheah, essentially used to
        // to keep reversing the pipeline when the instruction wait bios command is called
        let executing_address = self.registers.r[15].wrapping_sub(2 * self.registers.pc_offset());
        let side_effect = match decoded_instruction {
            Some(decoded_arm) => {
                if self.registers.condition_passed(decoded_arm.condition) {
                    execute_arm(decoded_arm.instruction, &mut self.registers, bus)
                } else {
                    None
                }
            }
            None => None,
        };

        if let Some(request) = side_effect {
            match request {
                SideEffect::Swi(function) => {
                    bus.idle(45);
                    handle_swi(function, self, bus)
                }
                SideEffect::Branch(address) => {
                    if address == Self::IRQ_RETURN_ADDRESS {
                        self.handle_irq_return(bus);
                    } else {
                        self.branch_to(address)
                    }
                }
                SideEffect::BranchRestoreCpsr(address) => {
                    self.registers.restore_cpsr_from_spsr();
                    self.branch_to(address);
                }
            }
        }

        if let HaltState::IntrWait { .. } = self.halt_state {
            self.registers.r[15] = executing_address;
            self.flush_pipeline();
        }

        if !self.branched {
            self.registers.increment_pc();
        }
    }

    fn flush_pipeline(&mut self) {
        self.pipeline.flush();
        self.branched = true;
        self.next_fetch_access = AccessType::Nonsequential;
    }

    pub fn branch_to(&mut self, mut address: u32) {
        if self.registers.state() == ProcessorState::Thumb {
            address.clear_bit(0);
        } else {
            address.clear_bit_range(0..2);
        }

        self.registers.r[15] = address;
        self.flush_pipeline();
    }

    const IRQ_RETURN_ADDRESS: u32 = 0x00000138;
    pub fn handle_irq_entry(&mut self, bus: &mut Bus) {
        /*
            From gbatek:

            BIOS Interrupt handling
            Upon interrupt execution, the CPU is switched into IRQ mode, and the physical interrupt vector is called - as this address is located in BIOS ROM, the BIOS will always execute the following code before it forwards control to the user handler:
            00000018  b      128h                ;IRQ vector: jump to actual BIOS handler
            00000128  stmfd  r13!,r0-r3,r12,r14  ;save registers to SP_irq
            0000012C  mov    r0,4000000h         ;ptr+4 to 03FFFFFC (mirror of 03007FFC)
            00000130  add    r14,r15,0h          ;retadr for USER handler $+8=138h
            00000134  ldr    r15,[r0,-4h]        ;jump to [03FFFFFC] USER handler
            00000138  ldmfd  r13!,r0-r3,r12,r14  ;restore registers from SP_irq
            0000013C  subs   r15,r14,4h          ;return from IRQ (PC=LR-4, CPSR=SPSR)
            As shown above, a pointer to the 32bit/ARM-code user handler must be setup in [03007FFCh]. By default, 160 bytes of memory are reserved for interrupt stack at 03007F00h-03007F9Fh.
        */

        let mut sp = self.registers.r[13].wrapping_sub(24);
        self.registers.r[13] = sp;

        let mut first_access = true;
        for i in [0, 1, 2, 3, 12, 14] {
            bus.write_u32(
                sp,
                self.registers.r[i],
                if first_access {
                    AccessType::Nonsequential
                } else {
                    AccessType::Sequential
                },
            );
            sp = sp.wrapping_add(4);
            first_access = false;
        }

        self.registers.r[14] = Self::IRQ_RETURN_ADDRESS;
        let handler = bus.read_u32(0x03FFFFFC, AccessType::Nonsequential);

        bus.idle(8); // estination
        self.branch_to(handler);
    }

    pub fn handle_irq_return(&mut self, bus: &mut Bus) {
        let mut sp = self.registers.r[13];

        let mut first_access = true;
        for i in [0, 1, 2, 3, 12, 14] {
            self.registers.r[i] = bus.read_u32(
                sp,
                if first_access {
                    AccessType::Nonsequential
                } else {
                    AccessType::Sequential
                },
            );
            sp = sp.wrapping_add(4);
            first_access = false;
        }

        self.registers.r[13] = sp;
        bus.idle(3); // estination

        self.registers.restore_cpsr_from_spsr();
        self.branch_to(self.registers.r[14].wrapping_sub(4));
    }

    fn next_executing_address(&self) -> u32 {
        let offset = self.registers.pc_offset();
        if self.pipeline.decoded.is_some() {
            self.registers.r[15].wrapping_sub(2 * offset)
        } else if self.pipeline.fetched.is_some() {
            self.registers.r[15].wrapping_sub(offset)
        } else {
            self.registers.r[15]
        }
    }

    pub fn raise_irq(&mut self, bus: &mut Bus) {
        let lr = self.next_executing_address().wrapping_add(4);
        let old_cpsr = self.registers.cpsr;

        self.registers.set_mode(ProcessorMode::Irq);
        self.registers.set_spsr(old_cpsr);
        self.registers.r[14] = lr;
        self.registers.set_state(ProcessorState::Arm);
        self.registers.disable_irq();

        self.handle_irq_entry(bus);
    }

    pub fn is_halted(&self) -> bool {
        self.halt_state != HaltState::Running
    }

    pub fn awake(&mut self) {
        self.halt_state = HaltState::Running;
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::components::{bus::Bus, gamepak::GamePak};

    const N: u32 = 0x80000000;
    const Z: u32 = 0x40000000;
    const C: u32 = 0x20000000;
    const V: u32 = 0x10000000;

    #[test]
    fn test_condition_flag_gt() {
        let mut registers = Registers::new();

        registers.cpsr = N;
        assert!(!registers.condition_passed(Condition::Gt));

        registers.cpsr = N | V;
        assert!(registers.condition_passed(Condition::Gt));

        registers.cpsr = N | C;
        assert!(!registers.condition_passed(Condition::Gt));
    }

    #[test]
    fn test_condition_flag_le() {
        let mut registers = Registers::new();

        registers.cpsr = N | Z;
        assert!(registers.condition_passed(Condition::Le));

        registers.cpsr = N | V;
        assert!(!registers.condition_passed(Condition::Le));

        registers.cpsr = N | C;
        assert!(registers.condition_passed(Condition::Le));
    }

    #[test]
    fn test_raise_irq() {
        let mut cpu = Arm7tdmi::new();
        let gamepak = GamePak::mock();
        let mut bus = Bus::new(gamepak);

        cpu.registers.cpsr = ProcessorMode::Sys as u32;
        assert_eq!(cpu.registers.mode(), ProcessorMode::Sys);

        cpu.raise_irq(&mut bus);
        assert_eq!(cpu.registers.mode(), ProcessorMode::Irq);

        cpu.registers.restore_cpsr_from_spsr();
        assert_eq!(cpu.registers.mode(), ProcessorMode::Sys);
    }

    #[test]
    fn test_set_state() {
        let mut cpu = Arm7tdmi::new();
        assert_eq!(cpu.registers.state(), ProcessorState::Arm);

        cpu.registers.set_state(ProcessorState::Thumb);
        assert_eq!(cpu.registers.state(), ProcessorState::Thumb);

        cpu.registers.set_state(ProcessorState::Arm);
        assert_eq!(cpu.registers.state(), ProcessorState::Arm);
    }
}

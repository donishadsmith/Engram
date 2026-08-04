use crate::components::bus::{AccessType, AddressBus};
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
ANDS r4, r4, #0x20 (suffix x means alu condition codes will reflect operation results) -> r4 &= 0x20;
ADDEQ r5, r5, r6 (instruction only executes if the EQ condition is true at the executation stage of pipeline else its noop) -> if (EQ) r5 += r6
B <Label> (Branch instruction/Pc-relative branch)
LDR r0, [r1] => r0 = *r1
STRNEB r2, [r3, r4] => if (NE) *(r3 + r4) = r2 [B is byte size stor; least significant byte of r2 to the address r3 + r4 but only when NE is true]
*/
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
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
#[repr(u16)]
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

pub struct Registers {
    r: [u32; 16],
    banked_high_registers: [[u32; 5]; 2],
    banked_special_registers: [[u32; 2]; 6],
    banked_spsr: [u32; 6],
    cpsr: u32, // Holds current mode, status flags; bits 31-28 (N, Z, C, V, Q), mode bits (lower 5 bits), state bits
               // bit 5 (T) is ARM or THUMB
}

impl Registers {
    fn new() {}
}

// Store both fetch and decoded instrucion
struct Pipeline {
    fetched: u32,
    decoded: u32,
}

impl Pipeline {
    fn new() {}

    fn push_instruction(&mut self, instruction: u32) {
        self.decoded = self.fetched;
        self.fetched = instruction;
    }
}

pub struct Arm7tdmi {
    registers: Registers,
    pipeline: Pipeline,
}

impl Arm7tdmi {
    pub fn step<A: AddressBus>(&mut self, bus: &mut A) {
        let executing_instruction = self.pipeline.decoded;
        let new_instruction = match self.state() {
            ProcessorState::Arm => bus.read_u32(self.registers.r[15], AccessType::Sequential), // TODO: Just assume always sequential for now
            ProcessorState::Thumb => {
                bus.read_u16(self.registers.r[15], AccessType::Sequential) as u32
            }
        };

        // probably should do a bios intercept of some sort, the GBA bios actually had
        // support functions that games use
        self.pipeline.push_instruction(new_instruction);
        self.execute_arm(executing_instruction);
    }

    fn execute_arm(&self, instruction: u32) {}

    fn mode(&self) -> ProcessorMode {
        ProcessorMode::from_cpsr(self.registers.cpsr)
    }

    fn state(&self) -> ProcessorState {
        ProcessorState::from_cpsr(self.registers.cpsr)
    }

    fn set_high_registers(&mut self, banking_action: BankingAction) {
        let mode = self.mode();
        let offset = 8;
        if matches!(
            mode,
            ProcessorMode::Usr | ProcessorMode::Sys | ProcessorMode::Fiq
        ) {
            let index = mode.high_register_index();
            for i in 0..5 {
                if banking_action == BankingAction::Restore {
                    self.registers.r[i + offset] = self.registers.banked_high_registers[index][i];
                } else {
                    self.registers.banked_high_registers[index][i] = self.registers.r[i + offset];
                }
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

                if i == 1 {
                    self.registers.r[15] = self.registers.r[14];
                }
            }
        } else {
            self.registers.banked_special_registers[index][0] = self.registers.r[13];
            self.registers.banked_special_registers[index][1] = self.registers.r[15];
        }
    }

    fn set_cpsr(&mut self, banking_action: BankingAction) {
        let mode = self.mode();
        let index = mode.spsr_index();

        if banking_action == BankingAction::Restore {
            self.registers.r[15] = self.registers.banked_spsr[index];
        } else {
            self.registers.banked_spsr[index] = self.registers.r[15];
        }
    }

    fn jump() {}

    fn call() {}

    fn pop() {}

    fn push() {}

    fn increment(&mut self) {
        let offset = if self.state() == ProcessorState::Arm {
            4
        } else {
            2
        };
        self.registers.r[15] = self.registers.r[15].wrapping_add(offset)
    }

    fn raise_exception() {}
}

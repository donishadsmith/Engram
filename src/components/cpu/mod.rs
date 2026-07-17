pub mod alu;
pub mod cycles;
pub mod instructions;
pub mod interrupts;
pub mod registers;

use crate::components::{
    cpu::{interrupts::InterruptMode, registers::Registers},
    memory::bus::AddressBus,
    rom::cartridge::CGBFlag,
    utils::{ByteOps8, MergeByteOps},
};

pub const STARTING_ADDRESS: u16 = 0x0000;

#[derive(Copy, Clone, Debug)]
pub enum StatusFlag {
    Z = 0x80, // 7 bit is 1; Zero flag - condition in which the operation resulted in 0
    N = 0x40, // 6 bit is 1; Subtraction flag (BCD) - Most significant bit is 1, which is a negative value
    H = 0x20, // 5 bit is 1; Half Carry flag (BCD) - a carry or borrow occured between the high and low nibble
    C = 0x10, // 4 bit is 1; Carry flag - overflow or underflow occured
}

impl StatusFlag {
    pub fn u8(self) -> u8 {
        self as u8
    }

    pub fn boot(checksum: u8) -> u8 {
        if checksum == 0x00 {
            StatusFlag::Z.u8()
        } else {
            StatusFlag::Z.u8() | StatusFlag::H.u8() | StatusFlag::C.u8()
        }
    }
}

/*
    https://gbdev.io/gb-asm-tutorial/part1/operations.html

    After every operation, each flag has three states (4 options).
    The CPU status flags, Z, N, H, C are essentially masks, in which the bitwise and operation
    will zero out the bits of the value in the f register for that is not 1 in the specific
    bit position (i.e., 7 but position for Z). These are used to track the side effects of certain operations

    Flag Operations includes:
        - Set: Where the bit is changed to 1 by using a bitwise or operation, to ensure that
        the other bits do not get zeroed out but the bit of interest becomes 1 if it was previosly zero
        - Unset: In which the flag is turned off
        - Dependent: Flag is set or unset based on a specific condition
        - Unmodified: The flag is left as is

    Example of flags purpose:

    0x01FF + 0x0001 = 0x200(512)

    Each register can only hold 8 bit values, (2^8-1) which is 0-255, to represent larger numbers with this restriction
    Low byte add is 0xFF (255) + 0x01 = 0x100, due to the 8 bit restriction, register A gets 0x00 and the C flag carries the overflow.

    The high bytes can be summed 0x01 + 0x00 = 0x01, so the final 16 bit representation would be 0x0100 (256), which is incorrect; however
    adding the overflow 0x01 + 0x00 + 0x01 = 0x02, resulting in 0x0200, which is the correct 512. This is the ADC instruction.
*/
#[derive(PartialEq, Debug)]
pub enum FlagType {
    Set,
    Unset,
    Unmodified,
}

pub struct FlagDelta {
    pub z: FlagType,
    pub n: FlagType,
    pub h: FlagType,
    pub c: FlagType,
}

impl FlagDelta {
    pub fn apply(self, mut f: u8) -> u8 {
        if self.z != FlagType::Unmodified {
            f = f.set_bit(StatusFlag::Z.u8(), self.z == FlagType::Set);
        }

        if self.n != FlagType::Unmodified {
            f = f.set_bit(StatusFlag::N.u8(), self.n == FlagType::Set);
        }

        if self.h != FlagType::Unmodified {
            f = f.set_bit(StatusFlag::H.u8(), self.h == FlagType::Set);
        }

        if self.c != FlagType::Unmodified {
            f = f.set_bit(StatusFlag::C.u8(), self.c == FlagType::Set);
        }

        f & 0xF0
    }
}

pub struct ProgramCounter {
    pub address: u16,
}

impl ProgramCounter {
    pub fn start(next_starting_address: u16) -> Self {
        Self {
            address: next_starting_address,
        }
    }

    // Instructions can be 1 to 3 bytes, presented in 8 bits
    pub fn increment(&mut self, step: u16) {
        self.address = self.address.wrapping_add(step);
    }

    pub fn jump(&mut self, address: u16) {
        self.address = address
    }
}

/*
    https://gbdev.io/pandocs/Interrupt_Sources.html
    https://realboyemulator.wordpress.com/2013/07/01/interrupt-processing-a-real-world-example/
    https://realboyemulator.wordpress.com/2013/01/18/emulating-the-core-2/
    https://gbdev.gg8.se/wiki/articles/Interrupts

    Interrupts - Break in program execution by hardware when a condition is met

    Steps:
        - Check the master enable flag, if false continue executing instructions,
          if true, apply an interrupt
        - Check the interrupt flag and interrupt enable, both must be on for a specific
          bit to be serviced
        - Lowest bit has highest priority, clear the bit, disable master flag
        - Push current address to the stack
        - Jump to some fixed address
        - Execute
        - Pop address from stack and jump back to it

    Types:
       0x0040 + bit * 8
       bit 0 =  Screen finished a frame (V-Blank): 0x0040
       bit 1 = LCD condition: 0x0048
       bit 2 = Timer overflowed: 0x0050
       bit 3 = Serial link: 0x0058 (i.e., 8 * 3 = 24 (16 + 8) = 0x10 + 0x08, add 0x0040 and its 0x0058)
       bit 4 = Button pressed (joypad): 0x0060
*/
pub struct Interrupt {
    pub master_enable: bool,
    pub pending_enable: bool,
}

impl Interrupt {
    fn new() -> Self {
        Self {
            master_enable: false,
            pending_enable: false,
        }
    }
}

pub struct CPU<A>
where
    A: AddressBus,
{
    pub registers: Registers,
    pub bus: A,
    pub halt_bug: bool,
    pub halted: bool,
    pub interrupt: Interrupt,
}

impl<A> CPU<A>
where
    A: AddressBus,
{
    pub fn start(cgb_flag: CGBFlag, checksum: u8, bus: A) -> Self {
        let mut cpu = Self {
            registers: Registers::new(cgb_flag, checksum),
            bus,
            halt_bug: false,
            halted: false,
            interrupt: Interrupt::new(),
        };

        cpu.fetch();
        cpu
    }

    pub fn from_state(registers: Registers, bus: A) -> Self {
        let mut cpu = Self {
            registers,
            bus,
            halt_bug: false,
            halted: false,
            interrupt: Interrupt::new(),
        };

        let opcode_address = cpu.registers.program_counter.address.wrapping_sub(1);
        cpu.registers.instruction_register = Some(cpu.bus.read(opcode_address));
        cpu
    }

    pub fn push(&mut self, address: u16) {
        // High byte stored first, stack grows down
        self.registers.stack_pointer = self.registers.stack_pointer.wrapping_sub(1);
        self.bus
            .write(self.registers.stack_pointer, (address >> 8) as u8);
        self.registers.stack_pointer = self.registers.stack_pointer.wrapping_sub(1);
        self.bus.write(self.registers.stack_pointer, address as u8);
    }

    pub fn pop(&mut self) -> (u8, u8) {
        // Low byte poppped first
        let low_byte = self.bus.read(self.registers.stack_pointer);
        self.registers.stack_pointer = self.registers.stack_pointer.wrapping_add(1);
        let high_byte = self.bus.read(self.registers.stack_pointer);
        self.registers.stack_pointer = self.registers.stack_pointer.wrapping_add(1);

        (low_byte, high_byte)
    }

    pub fn call(&mut self, address: u16) {
        self.push(self.registers.program_counter.address);
        self.registers.program_counter.jump(address);
    }

    pub fn ret(&mut self) {
        let (low_byte, high_byte) = self.pop();
        self.registers
            .program_counter
            .jump(high_byte.merge_bytes(low_byte));
    }

    pub fn cycle(&mut self) -> u8 {
        /*
           https://github.com/geaz/emu-gameboy
           https://github.com/geaz/emu-gameboy/blob/master/docs/The%20Cycle-Accurate%20Game%20Boy%20Docs.pdf

           Exit halt mode when IF and corresponding bit in IE is set, then apply those interrupt
        */
        if self.halted {
            if self.bus.pending_interrupt() == 0 {
                return 1;
            }

            self.halted = false;
            if self.apply_interrupt() {
                return 6;
            }

            self.fetch();
            return 1;
        }

        let apply_interrupt_after = self.interrupt.pending_enable;

        let mut m_cycles = self.decode_and_execute();

        if apply_interrupt_after && self.interrupt.pending_enable {
            self.interrupt.master_enable = true;
            self.interrupt.pending_enable = false
        }

        if self.halted {
            return m_cycles;
        }

        if self.apply_interrupt() {
            m_cycles += 5;
        } else {
            self.fetch();
        }

        m_cycles
    }

    pub fn fetch(&mut self) {
        self.registers.instruction_register =
            Some(self.bus.read(self.registers.program_counter.address));

        if self.halt_bug {
            self.halt_bug = false;
        } else {
            self.registers.program_counter.increment(1u16);
        }
    }

    fn apply_interrupt(&mut self) -> bool {
        if !self.interrupt.master_enable {
            return false;
        }

        let mut interrupt_flag = self.bus.read(0xFF0F);
        let service = self.bus.pending_interrupt();
        if service == 0 {
            return false;
        }

        let interrupt_mode = InterruptMode::to_variant(service.trailing_zeros() as u8);
        self.interrupt.master_enable = false;

        interrupt_flag &= !interrupt_mode.mask();
        self.bus.write(0xFF0F, interrupt_flag);

        if self.halt_bug {
            self.halt_bug = false;
            self.registers.program_counter.address =
                self.registers.program_counter.address.wrapping_sub(1);
        }

        self.call(interrupt_mode.to_address());
        self.fetch();

        true
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::components::{
        cpu::{alu::ArithmeticOperation, registers::Registers},
        rom::cartridge::Cartridge,
    };

    #[test]
    fn test_arithmetic_add() {
        let a: u8 = 255;
        let b: Option<u8> = Some(2);
        let carry: bool = false;
        let (value, delta) = ArithmeticOperation::Add.operation(a, b, carry).unwrap();

        assert_eq!(value, 1);
        assert_eq!(delta.z, FlagType::Unset);
        assert_eq!(delta.n, FlagType::Unset);
        assert_eq!(delta.h, FlagType::Set);
        assert_eq!(delta.c, FlagType::Set);
    }

    #[test]
    fn test_arithmetic_sub() {
        let a: u8 = 2;
        let b: Option<u8> = Some(255);
        let carry: bool = false;
        let (value, delta) = ArithmeticOperation::Sub.operation(a, b, carry).unwrap();

        assert_eq!(value, 3); // 2 - (-1) = 3 -> 2 - 255 = -253 -> (-253 + 256 = 3)
        assert_eq!(delta.z, FlagType::Unset);
        assert_eq!(delta.n, FlagType::Set);
        assert_eq!(delta.h, FlagType::Set);
        assert_eq!(delta.c, FlagType::Set);
    }

    #[test]
    fn test_arithmetic_adc() {
        let a: u8 = 255;
        let b: Option<u8> = Some(2);
        let carry: bool = true;
        let (value, delta) = ArithmeticOperation::Adc.operation(a, b, carry).unwrap();

        assert_eq!(value, 2);
        assert_eq!(delta.z, FlagType::Unset);
        assert_eq!(delta.n, FlagType::Unset);
        assert_eq!(delta.h, FlagType::Set);
        assert_eq!(delta.c, FlagType::Set);
    }

    #[test]
    fn test_arithmetic_sbc() {
        let a: u8 = 2;
        let b: Option<u8> = Some(255);
        let carry: bool = true;
        let (value, delta) = ArithmeticOperation::Sbc.operation(a, b, carry).unwrap();

        assert_eq!(value, 2); // 2 - (-1) - 1 = 2
        assert_eq!(delta.z, FlagType::Unset);
        assert_eq!(delta.n, FlagType::Set);
        assert_eq!(delta.h, FlagType::Set);
        assert_eq!(delta.c, FlagType::Set);
    }

    #[test]
    fn test_flag_setting() -> Result<(), std::io::Error> {
        // Test flag setting
        let monochrome_cartridge = Cartridge::fake()?;
        // Default checksum is 0, so f register is set to 0x10000000
        let mut register = Registers::new(
            monochrome_cartridge.header.cgb_flag,
            monochrome_cartridge.header.checksum,
        );

        let a: u8 = 255;
        let b: Option<u8> = Some(2);
        let carry: bool = register.flag(StatusFlag::C);
        assert_eq!(carry, false);
        let (value, delta) = ArithmeticOperation::Add.operation(a, b, carry).unwrap();

        assert_eq!(value, 1);
        assert_eq!(delta.z, FlagType::Unset);
        assert_eq!(delta.n, FlagType::Unset);
        assert_eq!(delta.h, FlagType::Set);
        assert_eq!(delta.c, FlagType::Set);

        register.apply_flags(delta);
        assert_eq!(register.f, 0b00110000);

        Ok(())
    }
}

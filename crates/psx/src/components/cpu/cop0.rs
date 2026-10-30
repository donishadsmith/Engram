use serde::{Deserialize, Serialize};
use shared::traits::BitOps;

// Exception codes based on rustation-ng: https://github.com/simias/rustation-ng/blob/main/src/psx/cop0.rs and psx-spx
// appears that the general process is that cop0 receives a code from cpu, does
// fancy bit shift stuff then returns 0xBFC00180 if the 22nd bit of cop0 status register is set
// and 0x80000080 if it is clear
#[derive(Clone, Copy, Debug)]
pub enum ExceptionCode {
    ExternalInterrupt = 0x0,
    LoadAddressError = 0x4,
    StoreAddressError = 0x5,
    // bus errors not in rustation enum but are in psx-spx,
    // gathering these are specific "out of range" type errors
    InstructionBusError = 0x6,
    DataBusError = 0x7,
    SysCall = 0x8,
    Break = 0x9,
    ReservedInstruction = 0xA,
    CoprocessorUnusable = 0xB,
    ArithmeticOverflow = 0xC,
}

pub enum Cop0Registers {
    Bpc,
    Bda,
    Tar,
    Dcic,
    Bada,
    Bdam,
    Bpcm,
    Sr,
    Cause,
    Epc,
    Prid,
}

impl Cop0Registers {
    pub fn to_register(value: usize) -> Result<Cop0Registers, ExceptionCode> {
        match value {
            3 => Ok(Cop0Registers::Bpc),
            5 => Ok(Cop0Registers::Bda),
            6 => Ok(Cop0Registers::Tar),
            7 => Ok(Cop0Registers::Dcic),
            8 => Ok(Cop0Registers::Bada),
            9 => Ok(Cop0Registers::Bdam),
            11 => Ok(Cop0Registers::Bpcm),
            12 => Ok(Cop0Registers::Sr),
            13 => Ok(Cop0Registers::Cause),
            14 => Ok(Cop0Registers::Epc),
            15 => Ok(Cop0Registers::Prid),
            0..=2 | 4 | 10 | 16..=31 => Err(ExceptionCode::ReservedInstruction),
            _ => unreachable!(),
        }
    }
}

#[derive(Serialize, Deserialize)]
pub struct Cop0 {
    bpc: u32,
    bda: u32,
    tar: u32,
    dcic: u32,
    bada: u32,
    bdam: u32,
    bpcm: u32,
    sr: u32,
    cause: u32,
    epc: u32,
    prid: u32,
}

impl Cop0 {
    pub fn new() -> Self {
        Self {
            bpc: 0,
            bda: 0,
            tar: 0,
            dcic: 0,
            bada: 0,
            bdam: 0,
            bpcm: 0,
            sr: 0,
            cause: 0,
            epc: 0,
            prid: 0x00000002,
        }
    }

    pub fn process_exception(
        &mut self,
        code: ExceptionCode,
        current_pc: u32,
        in_delay_slot: bool,
    ) -> u32 {
        /*
            Rustation Guide Page 64:
            When an exception occurs the following takes place in the CPU:

            - The current value of the PC is stored in $cop0 14, the EPC (Exception
            PC) register

            - Record the cause of the exception (syscall, overflow, interrupt...) in $cop0 13,
            the CAUSE register

            - Disable interrupts in $cop0 12 (SR)

            - Jump into the exception handler whose address is either 0x80000080 or
            0xbfc00180 depending on the value of the BEV field (bit 22) in $cop0 12
            (SR)
        */
        self.epc = if in_delay_slot {
            current_pc.wrapping_sub(4)
        } else {
            current_pc
        };

        let mode = self.sr.get_bit_range(0..6);
        self.sr.clear_bit_range(0..6);
        self.sr |= (mode << 2).get_bit_range(0..6);

        self.cause.clear_bit_range(2..7);
        self.cause |= (code as u32) << 2;
        self.cause.clear_bit(31);
        if in_delay_slot {
            self.cause.set_bit(31);
        }

        if self.sr.is_set(22) {
            0xBFC00180
        } else {
            0x80000080
        }
    }

    pub fn move_to(&mut self, register: Cop0Registers, value: u32) {
        match register {
            Cop0Registers::Bada => self.bada = value,
            Cop0Registers::Bda => self.bda = value,
            Cop0Registers::Bdam => self.bdam = value,
            Cop0Registers::Bpc => self.bpc = value,
            Cop0Registers::Bpcm => self.bpcm = value,
            Cop0Registers::Cause => {
                self.cause.clear_bit_range(8..10);
                self.cause |= value.get_bit_range(8..10) << 8;
            }
            Cop0Registers::Dcic => self.dcic = value,
            Cop0Registers::Epc => self.epc = value,
            Cop0Registers::Prid => {}
            Cop0Registers::Sr => self.sr = value,
            Cop0Registers::Tar => self.tar = value,
        }
    }

    pub fn move_from(&mut self, register: Cop0Registers) -> u32 {
        match register {
            Cop0Registers::Bada => self.bada,
            Cop0Registers::Bda => self.bda,
            Cop0Registers::Bdam => self.bdam,
            Cop0Registers::Bpc => self.bpc,
            Cop0Registers::Bpcm => self.bpcm,
            Cop0Registers::Cause => self.cause,
            Cop0Registers::Dcic => self.dcic,
            Cop0Registers::Epc => self.epc,
            Cop0Registers::Prid => self.prid,
            Cop0Registers::Sr => self.sr,
            Cop0Registers::Tar => self.tar,
        }
    }

    pub fn set_bad_address(&mut self, address: u32) {
        self.bada = address;
    }
}

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
    IllegalInstruction = 0xA,
    CoprocessorError = 0xB,
    Overflow = 0xC,
}

pub struct Coprocessor0 {}

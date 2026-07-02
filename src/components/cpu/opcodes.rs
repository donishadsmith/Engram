use crate::components::cpu::core::{
    AddressBus, ArithmeticOperation, BitwiseOperation, ByteOps8, ByteOps16, CPU, FlagDelta,
    MergeByteOps, Register8Bits, Register16Bits, Registers, StatusFlag,
};

impl<A> CPU<A>
where
    A: AddressBus,
{
    pub fn decode_and_execute(&mut self) {
        let opcode = self.registers.instruction_register.unwrap();
        eprintln!(
            "Opcode {:#0x}; PC={:#06x}",
            opcode,
            self.registers.program_counter.address.wrapping_sub(1)
        );

        // https://izik1.github.io/gbops/
        // https://gekkio.fi/files/gb-docs/gbctr.pdf
        // https://archive.gbdev.io/salvage/decoding_gbz80_opcodes/Decoding%20Gamboy%20Z80%20Opcodes.html
        match opcode {
            0x22 => {
                // LD (HL+),A - 1byte
                let address = self.registers.get_16bit(Register16Bits::HL);
                self.bus.write(address, self.registers.a);
                self.registers
                    .set_16bit(Register16Bits::HL, address.wrapping_add(1));
            }
            0xf1 => {
                //POP AF - 1 byte
                let (low_byte, high_byte) = self.pop();
                self.registers.a = high_byte;
                self.registers.f = low_byte.mask(0xF0)
            }
            0x80..=0xBF => {
                let source = opcode & 0b00000111;
                let opcode = (opcode >> 3) & 0b00000111;

                let value = match source {
                    0 => self.registers.b,
                    1 => self.registers.c,
                    2 => self.registers.d,
                    3 => self.registers.e,
                    4 => self.registers.h,
                    5 => self.registers.l,
                    6 => self.bus.read(self.registers.get_16bit(Register16Bits::HL)),
                    7 => self.registers.a,
                    _ => unreachable!(),
                };

                match opcode {
                    0 => self.add_instruction(value),
                    1 => self.adc_instruction(value),
                    2 => self.sub_instruction(value),
                    3 => self.sbc_instruction(value),
                    4 => self.bitwise_and_instruction(value),
                    5 => self.bitwise_xor_instruction(value),
                    6 => self.bitwise_or_instruction(value),
                    7 => self.cp_instruction(value),
                    _ => unreachable!(),
                }
            }
            _ => panic!("Opcode {:?} not implemented yet", opcode),
        }
    }

    fn add_instruction(&mut self, value: u8) {
        let (result, delta) = ArithmeticOperation::Add
            .operation(self.registers.a, Some(value), false)
            .unwrap();
        self.apply_alu_results(result, delta);
    }

    fn adc_instruction(&mut self, value: u8) {
        let carry = self.registers.flag(StatusFlag::C);
        let (result, delta) = ArithmeticOperation::Adc
            .operation(self.registers.a, Some(value), carry)
            .unwrap();
        self.apply_alu_results(result, delta);
    }

    fn sub_instruction(&mut self, value: u8) {
        let (result, delta) = ArithmeticOperation::Sub
            .operation(self.registers.a, Some(value), false)
            .unwrap();
        self.apply_alu_results(result, delta);
    }

    fn sbc_instruction(&mut self, value: u8) {
        let carry = self.registers.flag(StatusFlag::C);
        let (result, delta) = ArithmeticOperation::Sbc
            .operation(self.registers.a, Some(value), carry)
            .unwrap();
        self.apply_alu_results(result, delta);
    }

    fn cp_instruction(&mut self, value: u8) {
        let (_, delta) = ArithmeticOperation::Sub
            .operation(self.registers.a, Some(value), false)
            .unwrap();
        self.registers.apply_flags(delta);
    }

    fn bitwise_and_instruction(&mut self, value: u8) {
        let (result, delta) = ArithmeticOperation::Bit(BitwiseOperation::And)
            .operation(self.registers.a, Some(value), false)
            .unwrap();
        self.apply_alu_results(result, delta);
    }

    fn bitwise_or_instruction(&mut self, value: u8) {
        let (result, delta) = ArithmeticOperation::Bit(BitwiseOperation::Or)
            .operation(self.registers.a, Some(value), false)
            .unwrap();
        self.apply_alu_results(result, delta);
    }

    fn bitwise_xor_instruction(&mut self, value: u8) {
        let (result, delta) = ArithmeticOperation::Bit(BitwiseOperation::Xor)
            .operation(self.registers.a, Some(value), false)
            .unwrap();
        self.apply_alu_results(result, delta);
    }

    fn bitwise_not_instruction(&mut self) {
        let (result, delta) = ArithmeticOperation::Bit(BitwiseOperation::Xor)
            .operation(self.registers.a, None, false)
            .unwrap();
        self.apply_alu_results(result, delta);
    }

    fn apply_alu_results(&mut self, result: u8, delta: FlagDelta) {
        self.registers.set_8bit(Register8Bits::A, result);
        self.registers.apply_flags(delta);
    }
}

// For JSON tests
pub struct TestBus {
    ram: Vec<u8>,
}

impl TestBus {
    pub fn new() -> Self {
        Self {
            ram: vec![0u8; 0x10000],
        }
    }
}

impl AddressBus for TestBus {
    fn read(&self, address: u16) -> u8 {
        self.ram[address as usize]
    }
    fn write(&mut self, address: u16, value: u8) {
        self.ram[address as usize] = value;
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    use serde::Deserialize;

    #[derive(Deserialize)]
    struct CpuState {
        a: u8,
        b: u8,
        c: u8,
        d: u8,
        e: u8,
        f: u8,
        h: u8,
        l: u8,
        pc: u16,
        sp: u16,
        ram: Vec<(u16, u8)>,
    }

    #[derive(Deserialize)]
    struct TestCase {
        name: String,
        initial: CpuState,
        #[serde(rename = "final")]
        final_state: CpuState,
    }

    #[test]
    fn add_a_b_runs() {
        let registers = Registers::from_state(255, 0, 2, 0, 0, 0, 0, 0, 0xC001, 0xFFFE);
        let mut bus = TestBus::new();
        bus.write(0xC000, 0x80);
        let mut cpu = CPU::from_state(registers, bus);

        cpu.cycle();

        assert_eq!(cpu.registers.a, 1);
        assert_eq!(cpu.registers.f, 0x30)
    }

    #[test]
    fn json_instructions() {
        let mut implemented_instructions = vec!["22".to_string(), "f1".to_string()];
        for hex in 0x80..=0xBFu8 {
            implemented_instructions.push(format!("{:0x}", hex));
        }

        for opcode in implemented_instructions.iter() {
            let text = std::fs::read_to_string(format!("tests/sm83/v2/{}.json", opcode)).unwrap();
            let cases: Vec<TestCase> = serde_json::from_str(&text).unwrap();

            for case in &cases {
                let mut bus = TestBus::new();
                for (address, value) in &case.initial.ram {
                    bus.write(*address, *value);
                }

                let registers = Registers::from_state(
                    case.initial.a,
                    case.initial.f,
                    case.initial.b,
                    case.initial.c,
                    case.initial.d,
                    case.initial.e,
                    case.initial.h,
                    case.initial.l,
                    case.initial.pc,
                    case.initial.sp,
                );
                let mut cpu = CPU::from_state(registers, bus);

                cpu.cycle();

                let final_state = &case.final_state;

                assert_eq!(cpu.registers.a, final_state.a, "{}: A", case.name);
                assert_eq!(cpu.registers.b, final_state.b, "{}: B", case.name);
                assert_eq!(cpu.registers.c, final_state.c, "{}: C", case.name);
                assert_eq!(cpu.registers.d, final_state.d, "{}: D", case.name);
                assert_eq!(cpu.registers.e, final_state.e, "{}: E", case.name);
                assert_eq!(cpu.registers.h, final_state.h, "{}: H", case.name);
                assert_eq!(cpu.registers.l, final_state.l, "{}: L", case.name);
                assert_eq!(cpu.registers.f, final_state.f, "{}: F", case.name);
                assert_eq!(
                    cpu.registers.stack_pointer, final_state.sp,
                    "{}: SP",
                    case.name
                );

                for (address, value) in &final_state.ram {
                    assert_eq!(
                        cpu.bus.read(*address),
                        *value,
                        "{}: mem[{:#06x}]",
                        case.name,
                        address
                    );
                }
            }
        }
    }
}

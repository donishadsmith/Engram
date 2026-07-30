pub struct Arm7tdmi {
    r: [u32; 16],
    banked_usr: [u32; 7],
    banked_fiq: [u32; 7],
    banked_irq: [u32; 2],
    banked_svc: [u32; 2],
    banked_abt: [u32; 2],
    banked_und: [u32; 2],
    cpsr: u32,
    spsr_fiq: u32,
    spsr_irq: u32,
}

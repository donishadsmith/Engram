use crate::components::{bus::Bus, cpu::R3000a};

#[derive(Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub enum CdromEvent {}

#[derive(Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub enum Event {
    Hblank,
    Vblank,
    Timer(u8),
    Dma(u8),
    SpuSample,
    Sio,
    Cdrom(CdromEvent),
}

pub struct Psx {
    pub bus: Bus,
    pub cpu: R3000a,
    pub scripted_keypad: Option<[bool; 14]>,
    pub keypad: [bool; 14],
    spu_sample_period: u32,
}

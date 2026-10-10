use crate::components::{bus::Bus, cpu::R3000a};
use serde::{Deserialize, Serialize};

#[derive(Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Deserialize, Serialize)]
pub enum CdromEvent {}

#[derive(Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Deserialize, Serialize)]
pub enum Event {
    Hblank,
    Vblank,
    Timer(u8),
    Dma(u8),
    SpuSample,
    Sio,
    Cdrom(CdromEvent),
}

#[derive(Serialize, Deserialize)]
pub struct Psx {
    pub bus: Bus,
    pub cpu: R3000a,
    pub scripted_keypad: Option<[bool; 14]>,
    pub keypad: [bool; 14],
    spu_sample_period: u32,
}

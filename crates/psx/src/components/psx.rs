use serde::{Deserialize, Serialize};
use shared::structs::{ElapsedCycles, RewindManager};

use crate::components::{bus::Bus, cpu::R3000a};

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

// the easist way to get things working sooner is probably to copy verbatum most of the gba code for
// the traits for now then refactor everything around the time i start the fourth emu
#[derive(Serialize, Deserialize)]
pub struct Psx {
    pub bus: Bus,
    pub cpu: R3000a,
    pub scripted_keypad: Option<[bool; 14]>,
    pub keypad: [bool; 14],
    spu_sample_period: u32,
    #[serde(skip)]
    pause_at_cycle: Option<ElapsedCycles>,
    #[serde(skip)]
    elapsed_cycles_pause: Option<ElapsedCycles>,
    #[serde(skip)]
    cycle_pause: bool,
    #[serde(skip)]
    pub force_buffer_change: bool, // for debugger
    #[serde(skip)]
    pub force_screen_update: bool,
    #[serde(skip)]
    pub rewind_manager: RewindManager,
}

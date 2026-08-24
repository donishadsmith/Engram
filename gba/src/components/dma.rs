use crate::components::utils::{BitOps, get_halfword_shift, get_word_mask};

// https://problemkaputt.de/gbatek-gba-dma-transfers.htm
#[derive(Clone, Copy, PartialEq, Eq)]
pub enum Trigger {
    SoundFifo,
    Vcount(u8),
    Hblank,
    Vblank,
}

#[derive(Clone, Copy, PartialEq, Eq)]
enum StartTiming {
    Immediately,
    Vblank,
    Hblank,
    Special,
    None,
}

impl StartTiming {
    fn from_bit(bit: u8, channel: u8) -> StartTiming {
        match bit {
            0 => StartTiming::Immediately,
            1 => StartTiming::Vblank,
            2 => StartTiming::Hblank,
            3 => {
                if channel == 0 {
                    return StartTiming::None;
                }

                StartTiming::Special
            }
            _ => StartTiming::None,
        }
    }
}

#[derive(Clone, Copy, PartialEq, Eq)]
enum GamepakTransfer {
    Normal,
    Drq,
    None,
}

impl GamepakTransfer {
    fn from_bit(is_set: bool, channel: u8) -> GamepakTransfer {
        if channel != 3 {
            return GamepakTransfer::None;
        }

        match is_set {
            false => GamepakTransfer::Normal,
            true => GamepakTransfer::Drq,
        }
    }
}

#[derive(Clone, Copy, PartialEq, Eq)]
pub enum TransferType {
    Halfword,
    Word,
}

impl TransferType {
    fn from_bit(is_set: bool) -> TransferType {
        match is_set {
            false => TransferType::Halfword,
            true => TransferType::Word,
        }
    }
}

#[derive(Clone, Copy, PartialEq, Eq)]
enum IncrementDmaMode {
    Increment,
    Decrement,
    Fixed,
    Reload,
    Prohibited,
}

impl IncrementDmaMode {
    fn from_bit(bit: u8, destination_control: bool) -> IncrementDmaMode {
        match bit {
            0 => IncrementDmaMode::Increment,
            1 => IncrementDmaMode::Decrement,
            2 => IncrementDmaMode::Fixed,
            3 if destination_control => IncrementDmaMode::Reload,
            _ => IncrementDmaMode::Prohibited,
        }
    }
}

pub struct Dma {
    id: u8,
    on: bool,
    repeat: bool,
    completion_interrupt_enabled: bool,
    gamepak_transfer: GamepakTransfer,
    pub transfer_type: TransferType,
    source_address_register: u32,
    destination_address_register: u32,
    pub current_source_address: u32,
    pub current_destination_address: u32,
    pub current_word_count: u32,
    word_count_register: u16,
    control_register: u16,
    source_increment_mode: IncrementDmaMode,
    destination_increment_mode: IncrementDmaMode,
    start_timing: StartTiming,
}

impl Dma {
    fn new(id: u8) -> Self {
        Self {
            id,
            on: false,
            repeat: false,
            completion_interrupt_enabled: false,
            gamepak_transfer: GamepakTransfer::None,
            transfer_type: TransferType::Halfword,
            source_address_register: 0,
            destination_address_register: 0,
            current_source_address: 0,
            current_destination_address: 0,
            word_count_register: 0,
            current_word_count: 0,
            control_register: 0,
            source_increment_mode: IncrementDmaMode::Increment,
            destination_increment_mode: IncrementDmaMode::Increment,
            start_timing: StartTiming::Immediately,
        }
    }

    pub fn write_source_address(&mut self, address: u32, value: u16) {
        self.source_address_register = self.source_address_register & get_word_mask(address)
            | (value as u32) << get_halfword_shift(address);
    }

    pub fn write_destination_address(&mut self, address: u32, value: u16) {
        self.destination_address_register = self.destination_address_register
            & get_word_mask(address)
            | (value as u32) << get_halfword_shift(address);
    }

    pub fn write_word_count(&mut self, value: u16) {
        self.word_count_register = value;
    }

    pub fn read_control_register(&self) -> u16 {
        self.control_register
    }

    pub fn write_control_register(&mut self, value: u16) {
        self.control_register = value;

        self.completion_interrupt_enabled = value.is_set(14);
        self.start_timing = StartTiming::from_bit(value.get_bit_range(12..14) as u8, self.id);
        self.destination_increment_mode =
            IncrementDmaMode::from_bit(value.get_bit_range(5..7) as u8, true);
        self.source_increment_mode =
            IncrementDmaMode::from_bit(value.get_bit_range(7..9) as u8, false);
        self.gamepak_transfer = GamepakTransfer::from_bit(value.is_set(11), self.id);
        self.transfer_type = TransferType::from_bit(value.is_set(10));
        self.repeat = value.is_set(9);

        let channel_previously_off = !self.on;
        self.on = value.is_set(15);
        if channel_previously_off && self.on {
            self.current_source_address = self.source_address_register.get_bit_range(0..28);
            self.current_destination_address =
                self.destination_address_register.get_bit_range(0..28);
            self.current_word_count = self.transfer_size();
        }
    }

    pub fn start_transfer(&self, trigger: Option<Trigger>) -> bool {
        if !self.on {
            return false;
        }

        if self.start_timing == StartTiming::None
            || self.source_increment_mode == IncrementDmaMode::Prohibited
            || self.destination_increment_mode == IncrementDmaMode::Prohibited
        {
            return false;
        }

        if self.id == 3 && self.gamepak_transfer == GamepakTransfer::Drq {
            return false;
        }

        let transfer_condition_met = if let Some(trigger) = trigger {
            match trigger {
                Trigger::Hblank => self.start_timing == StartTiming::Hblank,
                Trigger::Vblank => self.start_timing == StartTiming::Vblank,
                Trigger::SoundFifo => {
                    let is_sound_channel = self.id == 1 || self.id == 2;
                    let is_sound_destination_address = self.current_destination_address
                        == 0x040000A0
                        || self.current_destination_address == 0x040000A4;

                    is_sound_channel
                        && is_sound_destination_address
                        && self.repeat
                        && self.start_timing == StartTiming::Special
                }
                Trigger::Vcount(value) => {
                    self.id == 3
                        && (2..163).contains(&value)
                        && self.start_timing == StartTiming::Special
                }
            }
        } else {
            if self.start_timing == StartTiming::Immediately {
                return true;
            } else {
                return false;
            }
        };

        transfer_condition_met
    }

    pub fn transfer_size(&self) -> u32 {
        match self.start_timing {
            StartTiming::Special if matches!(self.id, 1 | 2) => 4,
            _ => {
                if self.word_count_register == 0 {
                    if self.id != 3 { 0x4000 } else { 0x10000 }
                } else {
                    let max_bit = if self.id != 3 { 14 } else { 16 };

                    self.word_count_register.get_bit_range(0..max_bit) as u32
                }
            }
        }
    }

    pub fn update_address_pointers(&mut self) {
        let offset = match self.transfer_type {
            TransferType::Halfword => 2,
            TransferType::Word => 4,
        };

        self.current_source_address = match self.source_increment_mode {
            IncrementDmaMode::Increment => self.current_source_address.wrapping_add(offset),
            IncrementDmaMode::Decrement => self.current_source_address.wrapping_sub(offset),
            IncrementDmaMode::Fixed => self.current_source_address,
            _ => unreachable!(),
        };

        self.current_destination_address = match self.destination_increment_mode {
            IncrementDmaMode::Increment | IncrementDmaMode::Reload => {
                self.current_destination_address.wrapping_add(offset)
            }
            IncrementDmaMode::Decrement => self.current_destination_address.wrapping_sub(offset),
            IncrementDmaMode::Fixed => self.current_destination_address,
            _ => unreachable!(),
        };
    }

    pub fn reload_destination_address(&mut self) {
        if self.destination_increment_mode == IncrementDmaMode::Reload {
            self.current_destination_address =
                self.destination_address_register.get_bit_range(0..28);
        }
    }

    pub fn transfer_complete(&mut self, interrupt_flag: &mut u16) {
        if self.repeat && self.start_timing != StartTiming::Immediately {
            self.current_word_count = self.transfer_size();
            self.on = true;
        } else {
            self.on = false;
            self.control_register.clear_bit(15);
        }

        self.set_interrupt(interrupt_flag);
    }

    pub fn set_interrupt(&mut self, interrupt_flag: &mut u16) {
        if self.completion_interrupt_enabled {
            interrupt_flag.set_bit((8 + self.id) as usize);
        }
    }
}

pub struct DmaChannels {
    pub channels: [Dma; 4],
}

impl DmaChannels {
    pub fn new() -> Self {
        Self {
            channels: [Dma::new(0), Dma::new(1), Dma::new(2), Dma::new(3)],
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_current_word_count_reload() {
        let mut dma = Dma::new(1);
        dma.write_destination_address(0x40000C0, 0x00A0);
        dma.write_destination_address(0x40000C2, 0x0400);
        dma.write_word_count(0x1234);

        dma.write_control_register(0b1011011000000000);

        assert_eq!(dma.current_word_count, 4);
    }

    #[test]
    fn test_transfer_complete_bit_clears() {
        let mut dma = Dma::new(0);
        dma.write_word_count(1);
        dma.write_control_register(1 << 15);

        let mut if_flag = 0;
        dma.transfer_complete(&mut if_flag);

        assert!(dma.read_control_register().is_clear(15));
    }
}

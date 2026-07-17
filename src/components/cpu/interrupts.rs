#[derive(Clone, Copy, Debug, PartialEq)]
pub enum InterruptMode {
    VBlank = 0, // 0b00000001
    Stat = 1,   // 0b00000010
    Timer = 2,  //0b00000100
    Serial = 3, //0b00001000
    Joypad = 4, //0b00010000
}

impl InterruptMode {
    pub fn to_variant(bit: u8) -> InterruptMode {
        match bit {
            0 => InterruptMode::VBlank,
            1 => InterruptMode::Stat,
            2 => InterruptMode::Timer,
            3 => InterruptMode::Serial,
            4 => InterruptMode::Joypad,
            _ => unreachable!(),
        }
    }

    pub fn to_str(self) -> &'static str {
        match self {
            InterruptMode::VBlank => "VBlank",
            InterruptMode::Stat => "LCD",
            InterruptMode::Timer => "Timer",
            InterruptMode::Serial => "Serial",
            InterruptMode::Joypad => "Joypad",
        }
    }

    pub fn mask(self) -> u8 {
        match self {
            InterruptMode::VBlank => 0b00000001,
            InterruptMode::Stat => 0b00000010,
            InterruptMode::Timer => 0b00000100,
            InterruptMode::Serial => 0b00001000,
            InterruptMode::Joypad => 0b00010000,
        }
    }

    pub fn to_address(self) -> u16 {
        0x0040 + (self as u16) * 8
    }
}

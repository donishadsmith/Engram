pub trait ByteOps8 {
    fn set_bit(&self, mask: u8, flag: bool) -> u8;

    fn i16(&self) -> i16;
}

impl ByteOps8 for u8 {
    fn set_bit(&self, mask: u8, flag: bool) -> u8 {
        if flag { self | mask } else { self & !mask }
    }

    fn i16(&self) -> i16 {
        *self as i8 as i16
    }
}

// Nice trait high and low byte for u16 from https://aquova.net/emudev/, will be using that pattern
// for my implementation
pub trait ByteOps16 {
    fn high_byte(&self) -> u8;
    fn low_byte(&self) -> u8;
}

impl ByteOps16 for u16 {
    fn high_byte(&self) -> u8 {
        (self >> 8) as u8
    }

    fn low_byte(&self) -> u8 {
        (self & 0xFF) as u8
    }
}

pub trait MergeByteOps {
    fn merge_bytes<L: Into<u16>>(self, low_byte: L) -> u16;
}

impl<T: Into<u16>> MergeByteOps for T {
    fn merge_bytes<L: Into<u16>>(self, low_byte: L) -> u16 {
        (self.into() << 8) | low_byte.into()
    }
}

use crate::components::utils::BitOps;
use std::ops::AddAssign;

// i may need to rethink some aspects of this desig
#[derive(Clone, Copy, PartialEq, Eq)]
pub struct Fixed8Fractional(i32);

impl Fixed8Fractional {
    pub fn from_reference(value: u32) -> Self {
        let value = (value.get_bit_range(0..28) << 4) as i32;

        Self(value >> 4)
    }

    pub fn from_parameter(value: u16) -> Self {
        Self((value as i16) as i32)
    }

    pub fn pixel(self) -> i32 {
        self.0 >> 8
    }
}

impl AddAssign for Fixed8Fractional {
    fn add_assign(&mut self, rhs: Self) {
        self.0 += rhs.0;
    }
}

#[derive(Clone, Copy, PartialEq, Eq)]
pub struct AffineCoordinate {
    x: Fixed8Fractional,
    y: Fixed8Fractional,
}

impl AffineCoordinate {
    pub fn pixels(self) -> (i32, i32) {
        (self.x.pixel(), self.y.pixel())
    }
}

#[derive(Clone, Copy, PartialEq, Eq)]
pub struct AffineMatrix {
    pub pa: Fixed8Fractional,
    pub pb: Fixed8Fractional,
    pub pc: Fixed8Fractional,
    pub pd: Fixed8Fractional,
}

impl AffineMatrix {
    pub fn from_registers(pa: u16, pb: u16, pc: u16, pd: u16) -> Self {
        Self {
            pa: Fixed8Fractional::from_parameter(pa),
            pb: Fixed8Fractional::from_parameter(pb),
            pc: Fixed8Fractional::from_parameter(pc),
            pd: Fixed8Fractional::from_parameter(pd),
        }
    }
}

pub struct AffineState {
    pub programmed_reference: AffineCoordinate,
    pub internal_reference: AffineCoordinate,
    pub matrix: AffineMatrix,
}

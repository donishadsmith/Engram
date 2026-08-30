use crate::components::utils::BitOps;
use std::ops::AddAssign;

#[derive(Clone, Copy, PartialEq, Eq)]
pub struct Fixed8Fractional(pub i32);

impl Fixed8Fractional {
    pub fn from_reference(value: u32) -> Self {
        let value = (value.get_bit_range(0..28) << 4) as i32;

        Self(value >> 4)
    }

    pub fn raw(self) -> i32 {
        self.0
    }

    pub fn from_parameter(value: u16) -> Self {
        Self((value as i16) as i32)
    }

    pub fn transformed_pixel(self, step: Self, raw_pixel: usize) -> i32 {
        (self.0 + step.0 * raw_pixel as i32) >> 8
    }
}

impl AddAssign for Fixed8Fractional {
    fn add_assign(&mut self, rhs: Self) {
        self.0 += rhs.0;
    }
}

#[derive(Clone, Copy, PartialEq, Eq)]
pub struct AffineCoordinate {
    pub x: Fixed8Fractional,
    pub y: Fixed8Fractional,
}

impl AffineCoordinate {
    pub fn default() -> Self {
        Self {
            x: Fixed8Fractional(0),
            y: Fixed8Fractional(0),
        }
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
    pub reload: bool,
}

impl AffineState {
    pub fn default() -> Self {
        Self {
            programmed_reference: AffineCoordinate::default(),
            internal_reference: AffineCoordinate::default(),
            reload: true,
        }
    }

    pub fn write_x(&mut self, value: u32) {
        self.programmed_reference.x = Fixed8Fractional::from_reference(value);
        self.internal_reference.x = Fixed8Fractional::from_reference(value);
    }

    pub fn write_y(&mut self, value: u32) {
        self.programmed_reference.y = Fixed8Fractional::from_reference(value);
        self.internal_reference.y = Fixed8Fractional::from_reference(value);
    }

    pub fn reload_internal_reference(&mut self) {
        self.internal_reference = self.programmed_reference;
    }

    pub fn increment_internal_reference(&mut self, matrix: AffineMatrix) {
        self.internal_reference.x += matrix.pb;
        self.internal_reference.y += matrix.pd;
    }
}

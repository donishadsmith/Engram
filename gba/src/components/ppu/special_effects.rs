use crate::components::{
    ppu::{LayerId, Pixel},
    utils::{BitOps, GroupedRegisters},
};

#[derive(Clone, Copy, PartialEq, Eq)]
pub enum SpecialEffects {
    AlphaBlending,
    IncreaseBrightness,
    DecreaseBrightness,
    None,
}

impl SpecialEffects {
    fn from_bits(bits: u16) -> SpecialEffects {
        match bits {
            0 => SpecialEffects::None,
            1 => SpecialEffects::AlphaBlending,
            2 => SpecialEffects::IncreaseBrightness,
            _ => SpecialEffects::DecreaseBrightness,
        }
    }
}

fn from_rgb555(rgb555: u16) -> (u16, u16, u16) {
    let r = rgb555 & 0x1F;
    let g = (rgb555 >> 5) & 0x1F;
    let b = (rgb555 >> 10) & 0x1F;

    (r, g, b)
}

fn to_rgb555(r: u16, g: u16, b: u16) -> u16 {
    r | (g << 5) | b << 10
}

fn alpha_blending(
    r1: u16,
    g1: u16,
    b1: u16,
    r2: u16,
    g2: u16,
    b2: u16,
    bldalpha: u16,
) -> (u16, u16, u16) {
    let eva = bldalpha.get_bit_range(0..5).min(16);
    let evb = bldalpha.get_bit_range(8..13).min(16);

    (
        ((r1 * eva + r2 * evb) >> 4).min(31),
        ((g1 * eva + g2 * evb) >> 4).min(31),
        ((b1 * eva + b2 * evb) >> 4).min(31),
    )
}

// interestingly the agb manual uses 63 for g even though its only 5 bits, ref gbatek formula
fn brightness(r: u16, g: u16, b: u16, bldy: u16, effect: SpecialEffects) -> (u16, u16, u16) {
    let evy = bldy.get_bit_range(0..5).min(16);

    if effect == SpecialEffects::IncreaseBrightness {
        (
            r + (((31 - r) * evy) >> 4),
            g + (((31 - g) * evy) >> 4),
            b + (((31 - b) * evy) >> 4),
        )
    } else {
        (
            r - ((r * evy) >> 4),
            g - ((g * evy) >> 4),
            b - ((b * evy) >> 4),
        )
    }
}

pub fn apply_effects(
    first_pixel: Pixel,
    second_pixel: Pixel,
    color_special_effects: &GroupedRegisters<u16>,
) -> u16 {
    let (r1, g1, b1) = from_rgb555(first_pixel.color);
    let (r2, g2, b2) = from_rgb555(second_pixel.color);

    let bldcnt = color_special_effects.from_index(0);
    let bldalpha = color_special_effects.from_index(1);
    let bldy = color_special_effects.from_index(2);

    let second_is_target = bldcnt.is_set(second_pixel.id as usize + 8);
    let programmed_effect = SpecialEffects::from_bits(bldcnt.get_bit_range(6..8));

    let special_effect =
        if (first_pixel.id == LayerId::Sprite && first_pixel.semitransparent) && second_is_target {
            SpecialEffects::AlphaBlending
        } else if programmed_effect == SpecialEffects::AlphaBlending {
            if bldcnt.is_set(first_pixel.id as usize) && second_is_target {
                SpecialEffects::AlphaBlending
            } else {
                SpecialEffects::None
            }
        } else if bldcnt.is_set(first_pixel.id as usize) {
            programmed_effect
        } else {
            SpecialEffects::None
        };

    if special_effect == SpecialEffects::None {
        return first_pixel.color;
    }

    let (r, g, b) = match special_effect {
        SpecialEffects::AlphaBlending => alpha_blending(r1, g1, b1, r2, g2, b2, bldalpha),
        SpecialEffects::IncreaseBrightness | SpecialEffects::DecreaseBrightness => {
            brightness(r1, g1, b1, bldy, special_effect)
        }
        _ => unreachable!(),
    };

    to_rgb555(r, g, b)
}

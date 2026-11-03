//! Stance draws and tests of the box in front of an opponent (0xa470, 0x7ef4, 0x7f0c, 0x7f19c), with the leftovers of the original's registers.

use super::*;

/// Pointer tables in ROM (one long per table number).
pub(super) const STANCE_POINTERS: u32 = 0xa48c;
pub(super) const BOX_CHANCE_POINTERS: u32 = 0x7fec;
pub(super) const BOX_ANIMATION_POINTERS: u32 = 0x800c;

/// 0xa470: draws the stance of enemy `o` (byte of the table `+0x70`, indexed by the frame counter), stores it
/// at `+0x71` and returns it (the callers' `beq` tests whether it is zero).
pub fn stance(b: &mut Cps2Bus, o: Obj) -> u8 {
    stance_at(b, o).1
}

/// (table read, stance).
pub(super) fn stance_at(b: &mut Cps2Bus, o: Obj) -> (u32, u8) {
    let number = rd8(b, o.0 + f::STANCE_TABLE);
    let table = crate::mem::rd32(b, STANCE_POINTERS + 4 * number as u32);
    let tick = rd16(b, TICK);
    let v = rd8(b, table.wrapping_add(offset(tick)));
    wr8(b, o.0 + f::STANCE_VALUE, v);
    (table, v)
}

/// Box in front of an object: x offset (forwards), half-width, half-depth (D0, D1, D2 of the original).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Hitbox {
    pub offset: u16,
    pub half_width: u16,
    pub half_depth: u16,
}

/// Work registers left by the original's box tests (low words, if they were set).
#[derive(Clone, Copy, Debug, Default)]
pub(crate) struct Leftovers {
    pub(crate) d0: Option<u16>,
    pub(crate) d1: Option<u16>,
    pub(crate) d2: Option<u16>,
    pub(crate) d3: Option<u16>,
    pub(crate) d2_byte: Option<u8>,
    pub(crate) d3_byte: Option<u8>,
    pub(crate) d4_byte: Option<u8>,
}

/// Is object `other` in the box in front of `me`? Biased gaps compared unsigned, like the original.
pub(super) fn in_hitbox(b: &mut Cps2Bus, me: u32, other: u32, hitbox: Hitbox, r: &mut Leftovers) -> bool {
    let facing = rd8(b, me + f::FACING);
    let offset_x = if facing != 0 { hitbox.offset.wrapping_neg() } else { hitbox.offset };
    let other_x = rd16(b, other + f::X);
    r.d3 = Some(other_x);
    let half_x = hitbox.half_width;
    let reach_x = offset_x.wrapping_add(rd16(b, me + f::X)).wrapping_sub(other_x).wrapping_add(half_x);
    r.d0 = Some(reach_x);
    r.d1 = Some(half_x.wrapping_mul(2));
    if reach_x > half_x.wrapping_mul(2) {
        return false;
    }
    let other_depth = rd16(b, other + f::DEPTH);
    r.d1 = Some(other_depth);
    let half_depth = hitbox.half_depth;
    let reach_depth = rd16(b, me + f::DEPTH).wrapping_sub(other_depth).wrapping_add(half_depth);
    r.d0 = Some(reach_depth);
    r.d2 = Some(half_depth.wrapping_mul(2));
    reach_depth <= half_depth.wrapping_mul(2)
}

/// 0x7f0c (`strict`) and 0x7ef4: is an opponent (the targeted object `+0x50`) in the box in front of enemy `o`,
/// facing it, on the ground and in state 12 (or 14 if `strict`)? If so, after a probability draw (table `+0x74`),
/// the enemy goes to state 6 with a randomly drawn animation (table `+0x75`) and the result is true.
pub fn in_front(b: &mut Cps2Bus, o: Obj, strict: bool, hitbox: Hitbox) -> bool {
    in_front_leftovers(b, o.0, strict, hitbox).0
}

/// (result, leftovers, mask of the probability draw: the D2 left, reread by the caller).
pub(super) fn in_front_leftovers(b: &mut Cps2Bus, me: u32, strict: bool, hitbox: Hitbox) -> (bool, Leftovers, Option<u32>) {
    let mut r = Leftovers::default();
    let target = rd16(b, me + f::TARGET);
    if target == 0 {
        return (false, r, None);
    }
    let other = object(target);
    let state = rd8(b, other + f::SUB_STATE);
    let refuse = if strict { rd16(b, other + 0x0c) != 0 || (state != 12 && state != 14) } else { state != 12 };
    if refuse {
        return (false, r, None);
    }
    let facing = rd8(b, me + f::FACING);
    r.d3_byte = Some(facing);
    if facing == rd8(b, other + f::FACING) {
        return (false, r, None);
    }
    let gap = rd16(b, me + f::X).wrapping_sub(rd16(b, other + f::X));
    r.d3 = Some(gap);
    let side = if (gap as i16) >= 0 { 0 } else { 32 };
    r.d4_byte = Some(side);
    if side != facing || !in_hitbox(b, me, other, hitbox, &mut r) {
        return (false, r, None);
    }
    let number = rd8(b, me + f::BOX_CHANCE);
    let chances = crate::mem::rd32(b, BOX_CHANCE_POINTERS + 4 * number as u32);
    let tick = rd16(b, TICK);
    let mask = rd32(b, chances.wrapping_add(offset(tick << 2)));
    let roll = rng::random(b) as u32 & 31;
    if mask >> roll & 1 == 0 {
        return (false, r, Some(mask));
    }
    let number = rd8(b, me + f::BOX_ANIMATION);
    // byte `+0x75` is already a multiple of 4: no shift
    let animations = rd32(b, BOX_ANIMATION_POINTERS.wrapping_add(number as u32)).wrapping_add(offset((tick >> 1) << 5));
    let pick = rng::random(b) as u32 & 31;
    wr8(b, me + f::STATE, 6);
    let v = rd8(b, animations.wrapping_add(pick));
    wr8(b, me + f::SUB_STATE, v);
    wr8(b, me + f::SUB_STEP, 0);
    (true, r, Some(mask))
}

/// Position of the object relative to the box in front of another (0x7f19c).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum InHitbox {
    /// Outside the box, or in the air (D0 = 0).
    No,
    /// In the box, facing (D0 = 1).
    Face,
    /// In the box, from behind (D0 = 2).
    Back,
}

/// 0x7f19c: is object `other` in the box in front of object `me`?
pub fn box_around(b: &mut Cps2Bus, me: Obj, other: Obj, hitbox: Hitbox) -> InHitbox {
    box_reach(b, me.0, other.0, hitbox, &mut Leftovers::default())
}

pub(crate) fn box_reach(b: &mut Cps2Bus, me: u32, other: u32, hitbox: Hitbox, r: &mut Leftovers) -> InHitbox {
    if rd16(b, other + 0x0c) != 0 || !in_hitbox(b, me, other, hitbox, r) {
        return InHitbox::No;
    }
    let facing = rd8(b, me + f::FACING);
    let gap = rd16(b, me + f::X).wrapping_sub(rd16(b, other + f::X));
    r.d1 = Some(gap);
    let side = if (gap as i16) < 0 { 32 } else { 0 };
    r.d2_byte = Some(side);
    if side == facing {
        InHitbox::Face
    } else {
        InHitbox::Back
    }
}

/// Leftovers of the box tests reread afterwards (original glitch, `dispatch::Leftovers`): D2 and D3 (speeds of
/// the spark of `flash_and_shake`, reread by boss 11) and the low byte of D4, in the order in which the original sets
/// them.
#[derive(Clone, Copy, Debug, Default)]
pub struct HitboxLeftovers {
    leftovers: Leftovers,
    /// D2 long: the mask of the `in_front` draw.
    mask: Option<u32>,
}

impl HitboxLeftovers {
    /// Sets the leftovers in D2, D3 and D4 (the registers that the original leaves and that others reread).
    pub fn set(&self, d2: &mut u32, d3: &mut u32, d4: &mut u32) {
        let r = &self.leftovers;
        if let Some(v) = r.d3_byte {
            set_b(d3, v);
        }
        if let Some(v) = r.d4_byte {
            set_b(d4, v);
        }
        if let Some(v) = r.d2 {
            set_w(d2, v);
        }
        if let Some(v) = r.d3 {
            set_w(d3, v);
        }
        if let Some(v) = r.d2_byte {
            set_b(d2, v);
        }
        if let Some(v) = self.mask {
            *d2 = v;
        }
    }
}

/// 0xb5e0 (`ai::target_in_front`): is the target `+0x50` in the box in front of the object? With the leftovers reread.
pub fn target_in_front_leftovers(b: &mut Cps2Bus, o: Obj, hitbox: Hitbox) -> (InHitbox, HitboxLeftovers) {
    let pointer = rd16(b, o.0 + f::TARGET);
    let mut leftovers = Leftovers::default();
    let found = if pointer == 0 { InHitbox::No } else { box_reach(b, o.0, object(pointer), hitbox, &mut leftovers) };
    (found, HitboxLeftovers { leftovers, mask: None })
}

/// 0x7f19c (`box_around`), with the leftovers reread.
pub fn box_around_leftovers(b: &mut Cps2Bus, me: Obj, other: Obj, hitbox: Hitbox) -> (InHitbox, HitboxLeftovers) {
    let mut leftovers = Leftovers::default();
    let found = box_reach(b, me.0, other.0, hitbox, &mut leftovers);
    (found, HitboxLeftovers { leftovers, mask: None })
}

/// 0x7f0c / 0x7ef4 (`in_front`), with the leftovers reread.
pub fn in_front_with_leftovers(b: &mut Cps2Bus, o: Obj, strict: bool, hitbox: Hitbox) -> (bool, HitboxLeftovers) {
    let (yes, leftovers, mask) = in_front_leftovers(b, o.0, strict, hitbox);
    (yes, HitboxLeftovers { leftovers, mask })
}

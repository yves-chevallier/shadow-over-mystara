//! Stance draws and tests of the box in front of an opponent (0xa470, 0x7ef4, 0x7f0c, 0x7f19c), with the leftovers of the original's registers.

use super::*;

/// 0xa470: draws the stance of enemy `o` (byte of the row `+0x70` of `ai/stances`, indexed by the rank), stores it
/// at `+0x71` and returns it (the callers' `beq` tests whether it is zero).
pub fn stance(b: &mut World, o: Obj) -> u8 {
    let e = Enemy(o);
    let number = e.ai_stance_row(b);
    let row = crate::world::tables::engine_table!("ai/stances").nth32(b, number as u32);
    let rank = RANK.get(b);
    let v = engine().byte(b, row.wrapping_add(offset(rank)));
    e.set_ai_stance(b, v);
    v
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
pub(super) fn in_hitbox(b: &mut World, me: Obj, other: Obj, hitbox: Hitbox, r: &mut Leftovers) -> bool {
    let facing = Enemy(me).flip(b);
    let offset_x = if facing != 0 { hitbox.offset.wrapping_neg() } else { hitbox.offset };
    let other_x = other.u16(b, Obj::X);
    r.d3 = Some(other_x);
    let half_x = hitbox.half_width;
    let reach_x = offset_x.wrapping_add(me.u16(b, Obj::X)).wrapping_sub(other_x).wrapping_add(half_x);
    r.d0 = Some(reach_x);
    r.d1 = Some(half_x.wrapping_mul(2));
    if reach_x > half_x.wrapping_mul(2) {
        return false;
    }
    let other_depth = other.u16(b, Obj::Y);
    r.d1 = Some(other_depth);
    let half_depth = hitbox.half_depth;
    let reach_depth = me.u16(b, Obj::Y).wrapping_sub(other_depth).wrapping_add(half_depth);
    r.d0 = Some(reach_depth);
    r.d2 = Some(half_depth.wrapping_mul(2));
    reach_depth <= half_depth.wrapping_mul(2)
}

/// 0x7f0c (`strict`) and 0x7ef4: is an opponent (the targeted object `+0x50`) in the box in front of enemy `o`,
/// facing it, on the ground and in state 12 (or 14 if `strict`)? If so, after a probability draw (table `+0x74`),
/// the enemy goes to state 6 with a randomly drawn animation (table `+0x75`) and the result is true.
pub fn in_front(b: &mut World, o: Obj, strict: bool, hitbox: Hitbox) -> bool {
    in_front_leftovers(b, o, strict, hitbox).0
}

/// (result, leftovers, mask of the probability draw: the D2 left, reread by the caller).
pub(super) fn in_front_leftovers(b: &mut World, me: Obj, strict: bool, hitbox: Hitbox) -> (bool, Leftovers, Option<u32>) {
    let mut r = Leftovers::default();
    let e = Enemy(me);
    let target = e.target(b);
    if target == 0 {
        return (false, r, None);
    }
    let other = Obj::from_word(target);
    let player = Player(other.0);
    let action = player.action(b);
    let refuse = if strict { other.u16(b, Obj::Z) != 0 || (action != 12 && action != 14) } else { action != 12 };
    if refuse {
        return (false, r, None);
    }
    let facing = e.flip(b);
    r.d3_byte = Some(facing);
    if facing == player.facing(b) {
        return (false, r, None);
    }
    let gap = me.u16(b, Obj::X).wrapping_sub(other.u16(b, Obj::X));
    r.d3 = Some(gap);
    let side = if (gap as i16) >= 0 { 0 } else { 32 };
    r.d4_byte = Some(side);
    if side != facing || !in_hitbox(b, me, other, hitbox, &mut r) {
        return (false, r, None);
    }
    let number = e.ai_box_chance(b);
    let chances = crate::world::tables::engine_table!("ai/box_chances").nth32(b, number as u32);
    let rank = RANK.get(b);
    let mask = engine().long(b, chances.wrapping_add(offset(rank << 2)));
    let roll = rng::random(b) as u32 & 31;
    if mask >> roll & 1 == 0 {
        return (false, r, Some(mask));
    }
    // byte `+0x75` is already a multiple of 4: no shift
    let number = e.ai_box_anim(b);
    let animations = crate::world::tables::engine_table!("ai/box_animations").at32(b, number as u32).wrapping_add(offset((rank >> 1) << 5));
    let pick = rng::random(b) as u32 & 31;
    e.set_substate(b, 6);
    let v = engine().byte(b, animations.wrapping_add(pick));
    e.set_step(b, v);
    e.set_sub_step(b, 0);
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
pub fn box_around(b: &mut World, me: Obj, other: Obj, hitbox: Hitbox) -> InHitbox {
    box_reach(b, me, other, hitbox, &mut Leftovers::default())
}

pub(crate) fn box_reach(b: &mut World, me: Obj, other: Obj, hitbox: Hitbox, r: &mut Leftovers) -> InHitbox {
    if other.u16(b, Obj::Z) != 0 || !in_hitbox(b, me, other, hitbox, r) {
        return InHitbox::No;
    }
    let facing = Enemy(me).flip(b);
    let gap = me.u16(b, Obj::X).wrapping_sub(other.u16(b, Obj::X));
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
pub fn target_in_front_leftovers(b: &mut World, o: Obj, hitbox: Hitbox) -> (InHitbox, HitboxLeftovers) {
    let pointer = Enemy(o).target(b);
    let mut leftovers = Leftovers::default();
    let found = if pointer == 0 { InHitbox::No } else { box_reach(b, o, Obj::from_word(pointer), hitbox, &mut leftovers) };
    (found, HitboxLeftovers { leftovers, mask: None })
}

/// 0x7f19c (`box_around`), with the leftovers reread.
pub fn box_around_leftovers(b: &mut World, me: Obj, other: Obj, hitbox: Hitbox) -> (InHitbox, HitboxLeftovers) {
    let mut leftovers = Leftovers::default();
    let found = box_reach(b, me, other, hitbox, &mut leftovers);
    (found, HitboxLeftovers { leftovers, mask: None })
}

/// 0x7f0c / 0x7ef4 (`in_front`), with the leftovers reread.
pub fn in_front_with_leftovers(b: &mut World, o: Obj, strict: bool, hitbox: Hitbox) -> (bool, HitboxLeftovers) {
    let (yes, leftovers, mask) = in_front_leftovers(b, o, strict, hitbox);
    (yes, HitboxLeftovers { leftovers, mask })
}

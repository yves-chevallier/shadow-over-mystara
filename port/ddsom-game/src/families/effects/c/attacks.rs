use super::*;

// ---------------------------------------------------------------------------------------------
// Type 39: chunk thrown by a carrier, which flies toward a point on the screen and bursts as it falls
// ---------------------------------------------------------------------------------------------

pub(super) mod lob {
    pub const ANIM_SET: u32 = 0x16f58c;
    /// Per variant: target (x, y) relative to the camera (words), speed table (bytes), speed
    /// (long, 16.16), program space.
    pub const TARGETS: u32 = 0xb3762;
    pub const SPEED_TABLES: u32 = 0xb3614;
    pub const SPEEDS: u32 = 0xb361e;
    pub const CARRIER: u32 = 0x42;
    pub const TARGET_X: u32 = 0x58;
    pub const TARGET_Y: u32 = 0x5a;
}

/// 0x17ea: unsigned 32-bit division by shifts (quotient; D1 zero: all bits set to 1).
pub(super) fn divide(mut n: u32, d: u32) -> u32 {
    let mut r = 0u32;
    for _ in 0..32 {
        let top = n >> 31;
        n <<= 1;
        r = r.wrapping_shl(1) | top;
        if r >= d {
            r = r.wrapping_sub(d);
            n |= 1;
        }
    }
    n
}

pub(super) fn lob<R: Bench>(x: &mut Cx<'_, R>) {
    if x.suspended() {
        return;
    }
    match x.u8(f::STATE) {
        0 => lob_init(x),
        2 => lob_active(x),
        _ => free(x),
    }
    if x.u8(0) != 0 {
        x.reinsert();
    }
}

pub(super) fn lob_init<R: Bench>(x: &mut Cx<'_, R>) {
    x.add8(f::STATE, 2);
    crate::placement::load_effect_palette(x.b, crate::placement::effect_palette::LIGHT);
    // 0xb3734: target on the screen, direction
    let v = x.u8(f::VARIANT) as u32;
    let tx = x.rom16(lob::TARGETS + 4 * v).wrapping_add(x.a5_u16(0x5a6));
    let ty = x.rom16(lob::TARGETS + 4 * v + 2).wrapping_add(x.a5_u16(0x5aa));
    x.set16(lob::TARGET_X, tx);
    x.set16(lob::TARGET_Y, ty);
    let dir = crate::aids::direction_from(x.b, x.o, (tx, ty));
    x.set8(Obj::SPEED_INDEX, dir);
    let n = (x.rom8(lob::SPEED_TABLES + v) as u32) as u16;
    x.select_speed(&crate::objects::motion::SPEEDS_B, n);
    // flight duration: the greatest distance (x or y) divided by the speed
    let dx = (tx.wrapping_sub(x.u16(Obj::X)) as i16).wrapping_abs() as u16;
    let dy = (ty.wrapping_sub(x.u16(Obj::Y)) as i16).wrapping_abs() as u16;
    let dist = if (dx as i16) < (dy as i16) { dy } else { dx };
    let speed = x.rom16(lob::SPEEDS + 4 * v) as u32;
    let speed = speed << 16 | x.rom16(lob::SPEEDS + 4 * v + 2) as u32;
    let mut left = (dist as u32) << 16;
    let mut frames = 0u32;
    loop {
        left = left.wrapping_sub(speed);
        if left as i32 <= 0 {
            break;
        }
        frames = frames.wrapping_add(1) & 0xffff;
    }
    // vertical speed to reach the ground at the same time
    let q = divide(x.u16(Obj::Z) as u32, frames);
    let vz = (q.wrapping_neg().rotate_left(16) >> 8) as u16;
    x.set16(Obj::VZ, vz);
    x.anim_start(lob::ANIM_SET, 0);
}

pub(super) fn lob_active<R: Bench>(x: &mut Cx<'_, R>) {
    let v = x.u8(f::VARIANT);
    match x.u8(f::SUB_STATE) {
        0 => {
            let carrier = x.linked(lob::CARRIER);
            let launched = if (v as i8) > 4 {
                carrier.u16(x.b, f::STATE) == 0x214
            } else {
                match carrier.u32(x.b, f::STATE) {
                    0x0200_0414 => true,
                    0x0200_0424 => return free(x),
                    _ => false,
                }
            };
            if launched {
                x.add8(f::SUB_STATE, 2);
            }
            x.anim_step();
        }
        2 => {
            x.move_xyz();
            if (x.u16(Obj::Z) as i16) > 0 {
                x.anim_step();
                return;
            }
            // on the ground: sparks (type 50 variant 5), sound, and for variants 4 and 9 a shake
            x.add8(f::SUB_STATE, 2);
            x.set16(Obj::Z, 0);
            let pieces = if (v as i8) < 5 { 3 } else { 2 };
            for _ in 0..pieces {
                if let Some(o) = x.alloc(&pools::EFFCT) {
                    wr8(x.b, o.0, 1);
                    wr8(x.b, o.0 + 2, 50);
                    wr8(x.b, o.0 + 3, 5);
                    wr16(x.b, o.0 + 0x42, x.o.0 as u16);
                    let (px, py) = (x.u16(Obj::X), x.u16(Obj::Y));
                    wr16(x.b, o.0 + Obj::X, px);
                    wr16(x.b, o.0 + Obj::Y, py);
                }
            }
            crate::camera_modes::start_shake(x.b, crate::camera_modes::shake::B, 0xf);
            x.anim_start(lob::ANIM_SET, 2);
            let shake = match v {
                4 => 0xa,
                9 => 6,
                _ => return,
            };
            x.set_a5_u16(0x4be, shake);
            crate::palette::set_flash_color(x.b, crate::palette::FLASH_COLOR_RED);
            crate::sound::play(x.b, crate::sound::CMD_367_467, x.o);
        }
        4 => {
            if x.anim_step() {
                x.add8(f::STATE, 2);
            }
        }
        _ => {}
    }
}

// ---------------------------------------------------------------------------------------------
// Type 40: effects of an enemy attack (0: spark that follows the attacker; 1: spark placed in front of it;
// 2: spark that falls from the top of the screen; 3 and 4: thrown debris, variant 3 as long as the attack lasts)
// ---------------------------------------------------------------------------------------------

pub(super) mod blow {
    /// Variant 1: offset (dx, dz) per pair of attacker variants (program).
    pub const GLOW_PLACES: u32 = 0xb3968;
    /// Variant 2: x offset per pair of variants, animation per setting, stop height per
    /// setting and pair of variants (program).
    pub const FALL_PLACES: u32 = 0xb3a2a;
    pub const FALL_ANIMS: u32 = 0xb3a36;
    pub const FALL_FLOORS: u32 = 0xb3a98;
    pub const ANIM_SET: u32 = 0x18f412;
    /// Variant 0: speeds (x, z) per pair of attacker variants, according to the mode (data).
    pub const SPEEDS_BEHIND: u32 = 0xb3862;
    pub const SPEEDS_FRONT: u32 = 0xb3894;
    /// Variant 4: offset (x, z) and speeds (x, ax), 8 bytes per pair of variants (program).
    pub const DEBRIS: u32 = 0xb3af8;
}

/// Is the attacker still in step `step` of its attack (state `0x204`, `+6`)? Otherwise the effect is
/// removed (state 6).
pub(super) fn blow_holds<R: Bench>(x: &mut Cx<'_, R>, owner: Obj, step: u8) -> bool {
    if owner.u16(x.b, f::STATE) == 0x204 && owner.u8(x.b, 6) == step {
        return true;
    }
    x.set32(f::STATE, 0x0600_0000);
    false
}

pub(super) fn blow<R: Bench>(x: &mut Cx<'_, R>) {
    let v = x.u8(f::VARIANT);
    let owner = x.linked(0x5c);
    match (v, x.u8(f::STATE)) {
        (1, 0) => {
            // spark placed in front of the attacker (offset per pair of attacker variants)
            x.add8(f::STATE, 2);
            let (ox, oz, oy) = (owner.u16(x.b, Obj::X), owner.u16(x.b, Obj::Z), owner.u32(x.b, Obj::Y));
            x.set16(Obj::X, ox);
            x.set16(Obj::Z, oz);
            x.set32(Obj::Y, oy.wrapping_sub(1));
            let facing = owner.u8(x.b, 0x2c);
            x.set8(0x2c, facing);
            let k = 2 * (owner.u8(x.b, f::VARIANT) & !1) as u32;
            let mut dx = x.rom16(blow::GLOW_PLACES + k);
            let dz = x.rom16(blow::GLOW_PLACES + k + 2);
            if facing != 0 {
                dx = dx.wrapping_neg();
            }
            x.add16(Obj::X, dx);
            x.add16(Obj::Z, dz);
            x.anim_start(blow::ANIM_SET, 0xb);
            x.reinsert();
        }
        (1, 2) => {
            if blow_holds(x, owner, 0x3c) {
                x.anim_step();
            }
        }
        (2, 0) => {
            // spark that falls from the top of the screen in front of the attacker
            x.add8(f::STATE, 2);
            let (ox, oy) = (owner.u32(x.b, Obj::X), owner.u32(x.b, Obj::Y));
            x.set32(Obj::X, ox);
            x.set32(Obj::Y, oy);
            let side = if x.u8(0x2f) & 1 != 0 { 1 } else { 0xffff_ffff };
            x.add32(Obj::Y, side);
            let camera = if crate::ram::CAMERA_ALT_ACTIVE.get(x.b) != 0 { 0x55a } else { 0x50a };
            let top = x.a5_u16(camera).wrapping_add(0x100).wrapping_sub(x.u16(Obj::Y));
            x.set16(Obj::Z, top);
            let k = (owner.u8(x.b, f::VARIANT) & !1) as u32;
            let mut dx = x.rom16(blow::FALL_PLACES + k);
            let facing = owner.u8(x.b, 0x2c);
            x.set8(0x2c, facing);
            if facing != 0 {
                dx = dx.wrapping_neg();
            }
            x.add16(Obj::X, dx);
            let setting = x.u8(0x2f) as u32;
            let anim = x.rom8(blow::FALL_ANIMS + setting);
            x.anim_start(blow::ANIM_SET, (anim as u32) as u16);
            x.reinsert();
        }
        (2, 2) => {
            if x.u8(f::SUB_STATE) == 0 {
                // falls 4 pixels per frame down to the planned height; the first one (setting 0) to arrive
                // advances the attacker (`+7`)
                x.add16(Obj::Z, 0xfffc);
                let k = (x.u8(0x2f) & !1) as u32 + (((owner.u8(x.b, f::VARIANT) as u32) >> 1) << 3);
                let floor = x.rom16(blow::FALL_FLOORS + k);
                if floor as i16 >= x.u16(Obj::Z) as i16 {
                    x.add8(f::SUB_STATE, 2);
                    x.set16(Obj::Z, floor);
                    if x.u8(0x2f) == 0 {
                        let step = owner.u8(x.b, 7).wrapping_add(2);
                        owner.set_u8(x.b, 7, step);
                    }
                }
            }
            if blow_holds(x, owner, 0x3a) {
                x.anim_step();
            }
        }
        (3, 2) => {
            if blow_holds(x, owner, 0x38) {
                blow_debris(x);
            }
        }
        (0, 0) => {
            x.add8(f::STATE, 2);
            for off in [Obj::X, Obj::Z, Obj::Y] {
                let w = owner.u32(x.b, off);
                x.set32(off, w);
            }
            let facing = owner.u8(x.b, 0x2c);
            x.set8(0x2c, facing);
            let k = (owner.u8(x.b, f::VARIANT) >> 1) as u32;
            let (table, anim) = match x.u8(0x2f) {
                0 => {
                    x.add32(Obj::Y, 0xffff_ffff);
                    (blow::SPEEDS_BEHIND, 6)
                }
                1 => {
                    x.add32(Obj::Y, 1);
                    (blow::SPEEDS_FRONT, 4)
                }
                _ => return,
            };
            let (mut vx, vz) = (x.data16(table + 4 * k), x.data16(table + 4 * k + 2));
            if x.u8(0x2c) != 0 {
                vx = vx.wrapping_neg();
            }
            x.set16(Obj::VX, vx);
            x.set16(Obj::VZ, vz);
            x.add16(Obj::X, vx);
            x.add16(Obj::Z, vz);
            x.anim_start(blow::ANIM_SET, anim);
            x.reinsert();
        }
        (0, 2) => {
            if owner.u16(x.b, f::STATE) != 0x204 || (owner.u8(x.b, 7) as i8) >= 4 {
                return x.set32(f::STATE, 0x0600_0000);
            }
            for off in [Obj::X, Obj::Z] {
                let w = owner.u32(x.b, off);
                x.set32(off, w);
            }
            let (vx, vz) = (x.u16(Obj::VX), x.u16(Obj::VZ));
            x.add16(Obj::X, vx);
            x.add16(Obj::Z, vz);
            if x.anim_step() {
                x.set32(f::STATE, 0x0600_0000);
            }
        }
        (3 | 4, 0) => {
            x.add8(f::STATE, 2);
            x.set8(0x42, 0x3c);
            x.set16(Obj::VZ, 0x100);
            x.set16(Obj::AZ, 0);
            let (ox, oz) = (owner.u16(x.b, Obj::X), owner.u16(x.b, Obj::Z));
            x.set16(Obj::X, ox);
            x.set16(Obj::Z, oz);
            let depth = owner.u32(x.b, Obj::Y).wrapping_sub(1);
            x.set32(Obj::Y, depth);
            let k = ((owner.u8(x.b, f::VARIANT) & !1) as u32) << 2;
            let mut dx = x.rom16(blow::DEBRIS + k);
            let dz = x.rom16(blow::DEBRIS + k + 2);
            let (mut vx, mut ax) = (x.rom16(blow::DEBRIS + k + 4), x.rom16(blow::DEBRIS + k + 6));
            if owner.u8(x.b, 0x2c) != 0 {
                vx = vx.wrapping_neg();
                ax = ax.wrapping_neg();
                dx = dx.wrapping_neg();
            }
            x.set16(Obj::VX, vx);
            x.set16(Obj::AX, ax);
            x.add16(Obj::X, dx);
            x.add16(Obj::Z, dz);
            let rng = x.random();
            x.anim_start(blow::ANIM_SET, 0x10 + (rng & 1));
            x.reinsert();
        }
        (4, 2) => blow_debris(x),
        (_, 4 | 6) => free(x),
        _ => {}
    }
}

/// Thrown debris (variants 3 and 4): glides to a stop, waits, then finishes its animation.
pub(super) fn blow_debris<R: Bench>(x: &mut Cx<'_, R>) {
    x.move_xz();
    match x.u8(f::SUB_STATE) {
        0 => {
            if x.u16(Obj::VX) == 0 {
                x.set32(Obj::AX, 0);
                x.add8(f::SUB_STATE, 2);
            }
        }
        2 => {
            let n = x.u8(0x42).wrapping_sub(1);
            x.set8(0x42, n);
            if n != 0 {
                x.add8(f::SUB_STATE, 2);
            }
        }
        _ => {
            if x.anim_step() {
                x.add8(f::STATE, 2);
            }
        }
    }
}

// ---------------------------------------------------------------------------------------------
// Type 44: effects of a magician enemy (six modes: aura, flames, beam, spark), tied to `+0x5c`
// ---------------------------------------------------------------------------------------------

pub(super) mod spell {
    pub const ANIM_SET: u32 = 0x194e1e;
    pub const OWNER: u32 = 0x5c;
    /// Offsets (x, z) per variant (4 bytes); (x, z, half-turn) per variant (8 bytes);
    /// animations per variant (bytes); blink tints (bytes): program space.
    pub const OFFSETS_A: u32 = 0xb5298;
    pub const OFFSETS_B: u32 = 0xb530e;
    pub const ANIMS_A: u32 = 0xb51e4;
    pub const ANIMS_B: u32 = 0xb5200;
    pub const TINTS: u32 = 0xb5224;
}

/// Copies position and orientation of `from`, one notch in front or behind (`dy`).
pub(super) fn spell_copy<R: Bench>(x: &mut Cx<'_, R>, from: Obj) {
    for off in [Obj::X, Obj::Y, Obj::Z] {
        let v = from.u32(x.b, off);
        x.set32(off, v);
    }
    let facing = from.u8(x.b, 0x2c);
    x.set8(0x2c, facing);
}

/// 0xb524a / 0xb52b0: placed on the owner, offset according to the variant (D1, D2, D3: those of the original).
pub(super) fn spell_place<R: Bench>(x: &mut Cx<'_, R>, wide: bool) {
    let owner = x.linked(spell::OWNER);
    spell_copy(x, owner);
    x.add16(Obj::Y, 0xffff);
    let v = x.u8(f::VARIANT) as u32;
    let (mut dx, dz, turn) = if wide {
        let k = spell::OFFSETS_B + 8 * v;
        (x.rom16(k), x.rom16(k + 2), Some(x.rom16(k + 4)))
    } else {
        let k = spell::OFFSETS_A + 4 * v;
        (x.rom16(k), x.rom16(k + 2), None)
    };
    if x.u8(0x2c) != 0 {
        dx = dx.wrapping_neg();
    }
    if let Some(t) = turn {
        if t != 0 {
            let facing = x.u8(0x2c) ^ 0x20;
            x.set8(0x2c, facing);
        }
    }
    x.add16(Obj::X, dx);
    x.add16(Obj::Z, dz);
}

/// Placed on the owner's carrier (`+0x50` of the owner), on the ground, right in front.
pub(super) fn spell_on_carrier<R: Bench>(x: &mut Cx<'_, R>, anim: u32) {
    let owner = x.linked(spell::OWNER);
    let carrier = Obj(owner.u16(x.b, 0x50) as i16 as i32 as u32 & 0xff_ffff);
    spell_copy(x, carrier);
    x.add16(Obj::Y, 1);
    x.set16(Obj::Z, 0);
    x.anim_start(spell::ANIM_SET, anim as u16);
}

/// 0xb5206: next blink tint.
pub(super) fn spell_tint<R: Bench>(x: &mut Cx<'_, R>) {
    x.set8(0x2a, 1);
    let k = (x.u8(0x46) + 1) & 3;
    x.set8(0x46, k);
    let tint = x.rom8(spell::TINTS + k as u32);
    x.set8(0x2b, tint);
}

pub(super) fn spell<R: Bench>(x: &mut Cx<'_, R>) {
    if x.u16(spell::OWNER) == 0 {
        return x.set32(f::STATE, 0x0600_0000);
    }
    let mode = x.u8(0x2f);
    let v = x.u8(f::VARIANT) as u32;
    match x.u8(f::STATE) {
        0 => {
            x.add8(f::STATE, 2);
            match mode {
                0 | 4 => {
                    x.set16(0x42, if mode == 0 { 0x26 } else { 0xb });
                    spell_place(x, false);
                    let anim = x.rom8(spell::ANIMS_A + v) as u32;
                    x.anim_start(spell::ANIM_SET, anim as u16);
                }
                1 => spell_on_carrier(x, 5),
                5 => spell_on_carrier(x, 7),
                2 => {
                    x.set16(0x42, 0x26);
                    spell_place(x, true);
                    let anim = x.rom8(spell::ANIMS_B + v) as u32;
                    x.anim_start(spell::ANIM_SET, anim as u16);
                }
                3 => {
                    let owner = x.linked(spell::OWNER);
                    spell_copy(x, owner);
                    let facing = x.u8(0x2c) ^ 0x20;
                    x.set8(0x2c, facing);
                    x.add16(Obj::Y, 0xffff);
                    let dx: u16 = if facing == 0 { 0x42 } else { 0x42u16.wrapping_neg() };
                    x.add16(Obj::X, dx);
                    x.add16(Obj::Z, 0x42);
                    // 0xb5228: palette of the spark
                    for k in 0..8 {
                        let w = x.data32(0x25a6e0 + 4 * k);
                        wr32(x.b, 0x91c2e0 + 4 * k, w);
                    }
                    x.set8(0x2a, 1);
                    x.set8(0x2b, 0x17);
                    crate::sound::play(x.b, crate::sound::CMD_353, x.o);
                    x.anim_start(spell::ANIM_SET, 6);
                }
                _ => return,
            }
            x.reinsert();
        }
        2 => {
            let owner = x.linked(spell::OWNER);
            match mode {
                0 | 2 | 4 => {
                    if owner.u8(x.b, f::STATE) == 4 {
                        free(x);
                    } else {
                        let t = x.u16(0x42).wrapping_sub(1);
                        x.set16(0x42, t);
                        if (t as i16) < 0 {
                            x.add8(f::STATE, 2);
                        }
                        x.anim_step();
                    }
                }
                1 | 3 | 5 => {
                    if owner.u16(x.b, f::STATE) == 0x406 {
                        free(x);
                    } else {
                        if mode != 3 && x.u8(0x33) == 2 {
                            x.set8(0x33, 0);
                            spell_tint(x);
                        }
                        if mode == 1 && x.u8(0x33) == 1 {
                            x.set8(0x33, 0);
                            crate::families::eshl_creators::ring_28(&mut crate::families::effects_creators::leftovers::Pure, x.b, x.o);
                            crate::sound::play(x.b, crate::sound::CMD_346, x.o);
                        }
                        if x.anim_step() {
                            match mode {
                                1 => x.add8(f::STATE, 2),
                                3 => {
                                    x.add8(f::STATE, 2);
                                    crate::families::eshl_creators::single_29(&mut crate::families::effects_creators::leftovers::Pure, x.b, x.o);
                                }
                                _ => {
                                    crate::families::eshl_creators::single_3b(&mut crate::families::effects_creators::leftovers::Pure, x.b, x.o);
                                    x.add8(f::STATE, 2);
                                }
                            }
                        }
                    }
                }
                _ => return,
            }
            // the original puts the object back in the list even when it has just been released
            x.reinsert();
        }
        _ => free(x),
    }
}

// ---------------------------------------------------------------------------------------------
// Type 41: object thrown at random (four kinds), which falls back then glides or bounces
// ---------------------------------------------------------------------------------------------

pub(super) mod toss {
    pub const ANIM_SET: u32 = 0x1944f0;
    /// Per kind: speeds (x, az) (words) and start / landing sound (longs, 0: none),
    /// program space.
    pub const SPEEDS: u32 = 0xb3cbc;
    pub const SOUNDS_START: u32 = 0xb3cd0;
    pub const SOUNDS_LAND: u32 = 0xb3d52;
    /// Bounce speeds (x, az) according to `+0x33` (program).
    pub const BOUNCES: u32 = 0xb3dbe;
}

/// Sound from table `table` for the current kind (the original's `jsr (A1)`, if there is one).
pub(super) fn toss_sound<R: Bench>(x: &mut Cx<'_, R>, table: u32) {
    let k = 4 * x.u8(f::VARIANT) as u32;
    let routine = (x.rom16(table + k) as u32) << 16 | x.rom16(table + k + 2) as u32;
    if routine != 0 {
        crate::sound::play(x.b, crate::sound::Sound::at(routine), x.o);
    }
}

pub(super) fn toss_land<R: Bench>(x: &mut Cx<'_, R>) {
    let kind = x.u8(f::VARIANT) as u32;
    x.anim_start(toss::ANIM_SET, (5 + kind) as u16);
    toss_sound(x, toss::SOUNDS_LAND);
}

pub(super) fn toss<R: Bench>(x: &mut Cx<'_, R>) {
    match x.u8(f::STATE) {
        0 => {
            x.add8(f::STATE, 2);
            x.set16(0x5c, 0x20);
            let rng = x.random();
            let kind = (rng as u32 & 3) as u8;
            x.set8(f::VARIANT, kind);
            let k = 4 * kind as u32;
            let (vx, az) = (x.rom16(toss::SPEEDS + k), x.rom16(toss::SPEEDS + k + 2));
            let vx = if x.u8(0x2c) == 0 { vx.wrapping_neg() } else { vx };
            x.set16(Obj::VX, vx);
            x.set16(Obj::AZ, az);
            toss_sound(x, toss::SOUNDS_START);
            x.anim_start(toss::ANIM_SET, (kind as u32) as u16);
            x.reinsert();
        }
        2 => match x.u8(f::SUB_STATE) {
            0 => {
                x.anim_step();
                if x.move_z().le() {
                    // on the ground
                    x.add8(f::SUB_STATE, 4);
                    x.set16(Obj::Z, 0);
                    return toss_land(x);
                }
                if x.u16(Obj::AZ) != 0 {
                    return;
                }
                let t = x.u16(0x5c).wrapping_sub(1);
                x.set16(0x5c, t);
                if (t as i16) >= 0 {
                    return;
                }
                x.add8(f::SUB_STATE, 8);
                toss_land(x);
            }
            4 => {
                if x.u8(0x01) == 0 {
                    return free(x);
                }
                x.reinsert();
                x.anim_step();
                if x.u8(0x33) != 0 {
                    x.move_x();
                }
            }
            8 => {
                if x.u8(0x01) == 0 {
                    return free(x);
                }
                x.anim_step();
                let k = 4 * x.u8(0x33) as u32;
                let (vx, az) = (x.rom16(toss::BOUNCES + k), x.rom16(toss::BOUNCES + k + 2));
                let vx = if x.u8(0x2c) == 0 { vx.wrapping_neg() } else { vx };
                x.set16(Obj::AX, vx);
                x.set16(Obj::AZ, az);
                x.move_xz();
            }
            _ => {}
        },
        _ => free(x),
    }
}


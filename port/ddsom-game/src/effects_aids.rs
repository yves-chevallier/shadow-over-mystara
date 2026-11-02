//! Effect creation helpers (EFFCT) called by the player and scenery handlers.

use cps2::Cps2Bus;

use crate::families::effects_creators::leftovers::{Pure, Regs};
use crate::mem::{rd16, rd32, rd8, wr16, wr8, A5};
use crate::objects::{pools, Obj};
use crate::ops::Size;

/// Scene in which the shock effects are not created.
const NO_SHOCK_SCENE: u8 = 41;

/// Type of the effects created here.
const SHOCK_KIND: u8 = 25;
const SPARK_KIND: u8 = 4;

/// 0xb10c8: shock effect (dust, spark; type 25) at the creator's position: variant, setting (low byte:
/// `+0x2d`, high: `+0x2f`), speeds (`+0x14`, `+0x1c`). Nothing in scene 41.
pub fn shock(b: &mut Cps2Bus, creator: Obj, variant: u8, setting: u16, vx: u16, vy: u16) -> Option<Obj> {
    shock_by(&mut Pure, b, creator, variant, setting, vx, vy)
}

pub(crate) fn shock_by(r: &mut impl Regs, b: &mut Cps2Bus, me: Obj, variant: u8, setting: u16, vx: u16, vy: u16) -> Option<Obj> {
    let scene = crate::ram::SCENE.get(b);
    r.comparison(NO_SHOCK_SCENE as u32, scene as u32, Size::Byte);
    if scene == NO_SHOCK_SCENE {
        return None;
    }
    let fx = r.allocated(b, &pools::EFFCT)?;
    fx.set_u8(b, Obj::ACTIVE, 1);
    fx.set_u8(b, Obj::KIND, SHOCK_KIND);
    for field in [Obj::X, Obj::Y, Obj::Z] {
        let v = me.u32(b, field);
        fx.set_u32(b, field, v);
    }
    fx.set_u16(b, Obj::VX, vx);
    fx.set_u16(b, Obj::VY, vy);
    let facing = me.u8(b, 0x2c);
    fx.set_u8(b, 0x2c, facing);
    fx.set_u8(b, Obj::VARIANT, variant);
    fx.set_u8(b, 0x2d, setting as u8);
    // `lsr.w #8,D1w`: X = last bit shifted out (bit 7), kept until the return
    let high = setting >> 8;
    r.x(setting & 0x80 != 0);
    r.d_word(1, high);
    fx.set_u8(b, 0x2f, high as u8);
    fx.set_u16(b, 0x42, 0);
    r.flags(0, Size::Word);
    Some(fx)
}

/// 0xaa96c: spark (type 4, variant `variant`) tied to the creator and placed between it and `other` (A1), except for a
/// player in certain states. The processor receives D0 to D3 of the placement (`place_between`); the native returns
/// A1 to A4.
pub(crate) fn spark_by(r: &mut impl Regs, b: &mut Cps2Bus, me: Obj, other: u32, variant: u8) -> Option<Obj> {
    let skip = (rd8(b, me.0 + 0x3c) as i8) < 2 && rd8(b, me.0 + 0x10d).wrapping_sub(16) <= 3;
    if skip {
        return None;
    }
    let fx = r.allocated(b, &pools::EFFCT)?;
    fx.set_u8(b, Obj::ACTIVE, 1);
    fx.set_u8(b, Obj::KIND, SPARK_KIND);
    fx.set_u8(b, Obj::VARIANT, variant);
    let facing = me.u8(b, 0x2c) ^ 32;
    fx.set_u8(b, 0x2c, facing);
    fx.set_u16(b, 0x50, me.word());
    fx.set_u16(b, 0x54, other as u16);
    set(r, b, me, fx, other);
    Some(fx)
}

/// Places the spark (`place_between`); the processor receives D0, D1 (longs) and the low words of D2, D3.
fn set(r: &mut impl Regs, b: &mut Cps2Bus, me: Obj, fx: Obj, other: u32) -> u16 {
    let (d0, d1, d2, d3) = place_between(b, me.0, fx.0, other);
    r.d(0, d0 as u32);
    r.d(1, d1 as u32);
    r.d_word(2, d2);
    r.d_word(3, d3);
    d1
}

/// 0xaaa04 (`owner_first`) / 0xaaa84: spark (type 4, variant `variant`) between the creator and the touched object
/// `other`; `+0x50` / `+0x54` designate one or the other depending on the routine. Flags of D1.w.
pub(crate) fn spark_between_by(r: &mut impl Regs, b: &mut Cps2Bus, me: Obj, other: u32, variant: u8, owner_first: bool) -> Option<Obj> {
    let fx = r.allocated(b, &pools::EFFCT)?;
    fx.set_u8(b, Obj::ACTIVE, 1);
    fx.set_u8(b, Obj::KIND, SPARK_KIND);
    fx.set_u8(b, Obj::VARIANT, variant);
    let facing = me.u8(b, 0x2c) ^ 32;
    fx.set_u8(b, 0x2c, facing);
    let (first, second) = if owner_first { (me.0, other) } else { (other, me.0) };
    fx.set_u16(b, 0x50, first as u16);
    fx.set_u16(b, 0x54, second as u16);
    let d1 = set(r, b, me, fx, other);
    r.flags(d1 as u32, Size::Word);
    Some(fx)
}

/// Collision boxes: per game (`+0x2e`), address of an array of 8-byte boxes (dx, _, dz...),
/// chosen by `+0x38` (hit box) or `+0x3a` (body box).
const HIT_BOXES: u32 = 0x11bffa;
const BODY_BOXES: u32 = 0x11c23a;

/// Box of object `o` (hit or body).
fn hit_box(b: &mut Cps2Bus, o: u32, hit: bool) -> u32 {
    let (table, index) = if hit { (HIT_BOXES, 0x38) } else { (BODY_BOXES, 0x3a) };
    let set = rd8(b, o + 0x2e) as u32;
    let boxes = rd32(b, table + 4 * set);
    let k = rd8(b, o + index) as u32;
    boxes + 8 * k
}

/// 0xab2b4: places the spark `fx` halfway between a box of the attacker `me` and a box of the
/// target `victim`, depending on the variant (variants 0, 1, 3: body of one against hit box of the other...),
/// at halfway depth, just in front of the nearest. Returns D0 to D3 as the original leaves them.
fn place_between(b: &mut Cps2Bus, me: u32, fx: u32, victim: u32) -> (u16, u16, u16, u16) {
    let (me_hit, victim_hit) = match rd8(b, fx + 3) {
        0 | 1 | 3 => (false, true),
        4 | 6 => (false, false),
        _ => (true, false),
    };
    let (mine, theirs) = (hit_box(b, me, me_hit), hit_box(b, victim, victim_hit));
    // middle of the two edges, on one axis: x (offset 0 of the box, flipped according to the orientation) or z
    let middle = |b: &mut Cps2Bus, off: u32, field: u32, flip: bool| -> (u16, u16) {
        let mut d0 = rd8(b, mine + off) as i8 as i16 as u16;
        let mut d1 = rd8(b, theirs + off) as i8 as i16 as u16;
        if flip && rd8(b, me + 0x2c) != 0 {
            d0 = d0.wrapping_neg();
        }
        if flip && rd8(b, victim + 0x2c) != 0 {
            d1 = d1.wrapping_neg();
        }
        let a = d0.wrapping_add(rd16(b, me + field));
        let c = d1.wrapping_add(rd16(b, victim + field));
        let mut half = ((c.wrapping_sub(a) as i16).wrapping_abs() >> 1) as u16;
        if c as i16 <= a as i16 {
            half = half.wrapping_neg();
        }
        wr16(b, fx + field, a.wrapping_add(half));
        (a, c)
    };
    middle(b, 0, 0x08, true);
    let (z, their_z) = middle(b, 2, 0x0c, false);
    let (my_depth, their_depth) = (rd16(b, me + 0x10), rd16(b, victim + 0x10));
    let gap = my_depth.wrapping_sub(their_depth);
    let (front, gap) = if (gap as i16) < 0 { (my_depth, gap.wrapping_neg()) } else { (their_depth, gap) };
    let z = z.wrapping_add(gap >> 1);
    wr16(b, fx + 0x0c, z);
    wr16(b, fx + 0x10, front.wrapping_sub(1));
    (z, front.wrapping_sub(1), gap >> 1, their_z)
}

/// Creates an active object of type `kind` in `pool`, with variant `variant`. On failure returns the A1 that the
/// original leaves (`Pool::alloc_or_leftover`), for the callers that do not test the carry.
fn create(b: &mut Cps2Bus, pool: &pools::Pool, kind: u8, variant: u8) -> Result<Obj, Obj> {
    let object = pool.alloc_or_leftover(b)?;
    object.set_u8(b, Obj::ACTIVE, 1);
    object.set_u8(b, Obj::KIND, kind);
    object.set_u8(b, Obj::VARIANT, variant);
    Ok(object)
}

/// 0xddeaa: scenery (SOBJ) of type 6, variant `variant`.
pub fn scenery_6(b: &mut Cps2Bus, variant: u8) -> Result<Obj, Obj> {
    create(b, &pools::SOBJ, 6, variant)
}

/// 0xdfa4a: scenery (SOBJ) of type 11, variant `variant`, oriented according to `orientation` (`+0x2c`).
pub fn scenery_11(b: &mut Cps2Bus, variant: u8, orientation: u8) -> Result<Obj, Obj> {
    let object = create(b, &pools::SOBJ, 11, variant)?;
    object.set_u8(b, 0x2c, orientation);
    Ok(object)
}

/// 0x517ba: player effect (PEFF) of type 44, variant `variant`, setting `+0x2f` = `setting`.
pub fn effect_44(b: &mut Cps2Bus, variant: u8, setting: u8) -> Option<Obj> {
    let object = create(b, &pools::PEFF, 44, variant).ok()?;
    object.set_u8(b, 0x2f, setting);
    Some(object)
}

/// 0xcc18e: every 32 frames (counter `+0x20` of object `me`), counts the living players into `A5+0x35`;
/// returns the count when it is redone (D0 of the original).
pub fn living_players(b: &mut Cps2Bus, me: Obj) -> Option<u8> {
    let timer = me.u16(b, 0x20).wrapping_sub(1);
    me.set_u16(b, 0x20, timer);
    if (timer as i16) >= 0 {
        return None;
    }
    me.set_u16(b, 0x20, 32);
    let alive = (0..4).filter(|k| rd8(b, A5 + 0x5de + k * 0x200) != 0 && rd16(b, A5 + 0x5de + k * 0x200 + 0x62) != 0).count();
    wr8(b, A5 + 0x35, alive as u8);
    Some(alive as u8)
}

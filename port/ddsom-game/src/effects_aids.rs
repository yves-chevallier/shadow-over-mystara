//! Effect creation helpers (EFFCT) called by the player and scenery handlers.

use cps2::Cps2Bus;
use m68k::Cpu;

use crate::families::effects_creators::leftovers::{Pure, Regs};
use crate::mem::{rd16, rd32, rd8, rts, wr16, wr8, A5};
use crate::{regs, Native};
use crate::objects::{pools, Obj};
use m68k::decode::Size;

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

/// `shock` with the registers of the original (callers by address: `player/`): A0 = the creator; A1, D1.w (high byte
/// of the setting), carry and flags as in the original.
#[track_caller]
pub fn shock_68k(c: &mut Cpu, b: &mut Cps2Bus, creator: Obj, variant: u8, setting: u16, vx: u16, vy: u16) -> Option<Obj> {
    c.a[0] = creator.0;
    let r = shock_by(c, b, creator, variant, setting, vx, vy);
    crate::diag::dead_regs(c, "effects", regs::a(1) | regs::d(1) | regs::CCR);
    r
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

fn shock_native(c: &mut Cpu, b: &mut Cps2Bus) {
    let (me, variant, setting, vx, vy) = (Obj(c.a[0]), c.d[0] as u8, c.d[1] as u16, c.d[2] as u16, c.d[3] as u16);
    shock_by(c, b, me, variant, setting, vx, vy);
    rts(c, b);
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

/// Natives of the sparks: A0 = the creator, A1 = the other object, D1.b = variant; A1 to A4 are returned.
fn spark_native(c: &mut Cpu, b: &mut Cps2Bus, between: Option<bool>) {
    let saved = [c.a[1], c.a[2], c.a[3], c.a[4]];
    let (me, other, variant) = (Obj(c.a[0]), c.a[1], c.d[1] as u8);
    match between {
        None => spark_by(c, b, me, other, variant),
        Some(owner_first) => spark_between_by(c, b, me, other, variant, owner_first),
    };
    [c.a[1], c.a[2], c.a[3], c.a[4]] = saved;
    rts(c, b);
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

/// Creates an object of type `kind` in `pool`, variant D0; flags of D0.b.
fn spawn(c: &mut Cpu, b: &mut Cps2Bus, pool: &pools::Pool, kind: u8) -> Option<Obj> {
    let object = pool.alloc_68k(c, b)?;
    object.set_u8(b, Obj::ACTIVE, 1);
    object.set_u8(b, Obj::KIND, kind);
    object.set_u8(b, Obj::VARIANT, c.d[0] as u8);
    c.flags_logic(c.d[0] & 0xff, Size::Byte);
    Some(object)
}

/// 0xddeaa: scenery of type 6.
fn spawn_scenery_6(c: &mut Cpu, b: &mut Cps2Bus) {
    spawn(c, b, &pools::SOBJ, 6);
}

/// 0xdfa4a: scenery of type 11, oriented according to D1.
fn spawn_scenery_11(c: &mut Cpu, b: &mut Cps2Bus) {
    if let Some(object) = spawn(c, b, &pools::SOBJ, 11) {
        object.set_u8(b, 0x2c, c.d[1] as u8);
        c.flags_logic(c.d[1] & 0xff, Size::Byte);
    }
}

/// 0x517ba: player effect of type 44; its setting `+0x2f` is the next byte in the A4 table.
fn spawn_player_effect_44(c: &mut Cpu, b: &mut Cps2Bus) {
    if let Some(object) = spawn(c, b, &pools::PEFF, 44) {
        let setting = rd8(b, c.a[4]);
        c.a[4] = c.a[4].wrapping_add(1);
        object.set_u8(b, 0x2f, setting);
    }
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

fn count_living_players(c: &mut Cpu, b: &mut Cps2Bus) {
    if let Some(alive) = living_players(b, Obj(c.a[0])) {
        c.d[0] = alive as u32;
    }
}

/// 0x25260: a scripted player places itself in front of the object (`A5+0x46`), shifted according to its number (`+2`).
fn place_scripted_player(c: &mut Cpu, b: &mut Cps2Bus) {
    /// X offsets by player number (words, program space: 0x40, 0x10, -0x18, -0x44).
    const OFFSETS: u32 = 0x2528c;
    let me = c.a[0];
    let focus = rd16(b, A5 + 0x46) as i16 as i32 as u32;
    c.a[1] = focus;
    let x = rd16(b, focus + 0x08);
    wr16(b, me + 0x08, x);
    let depth = rd16(b, focus + 0x10);
    wr16(b, me + 0x10, depth);
    wr16(b, me + 0x0c, 0);
    let index = 2 * rd8(b, me + 2) as u32;
    let shift = crate::mem::prog16(b, OFFSETS + index);
    c.d[0] = shift as u32;
    let x = rd16(b, me + 0x08).wrapping_add(shift);
    wr16(b, me + 0x08, x);
    let depth = crate::ops::sub(c, 18, rd16(b, me + 0x10) as u32, Size::Word);
    wr16(b, me + 0x10, depth as u16);
}

const fn aid(entry: u32, name: &'static str, run: fn(&mut Cpu, &mut Cps2Bus), outputs: u32) -> Native {
    Native { entry, name, run: crate::Body::WithCpu(run), touches_sound: false, outputs }
}

macro_rules! leaf {
    ($entry:literal, $name:literal, $body:path, $outputs:expr) => {
        aid($entry, $name, |c, b| {
            $body(c, b);
            rts(c, b);
        }, $outputs)
    };
}

pub const NATIVES: &[Native] = &[
    leaf!(0xddeaa, "scenery_type_6", spawn_scenery_6, regs::a(1) | regs::CCR),
    leaf!(0xdfa4a, "scenery_type_11", spawn_scenery_11, regs::a(1) | regs::CCR),
    leaf!(0x517ba, "effect_player_44", spawn_player_effect_44, regs::a(1) | regs::a(4) | regs::CCR),
    leaf!(0xcc18e, "count_living_players", count_living_players, regs::d(0)),
    leaf!(0x25260, "player_scripted_place", place_scripted_player, regs::a(1)),
    aid(0xb10c8, "effect_shock", shock_native, regs::a(1) | regs::d(1) | regs::CCR),
    aid(0xaa96c, "effect_spark", |c, b| spark_native(c, b, None), regs::a(1) | regs::a(2) | regs::a(3) | regs::a(4)),
    aid(0xaaa04, "effect_spark_between", |c, b| spark_native(c, b, Some(true)), regs::ALL),
    aid(0xaaa84, "effect_spark_between_b", |c, b| spark_native(c, b, Some(false)), regs::ALL),
];

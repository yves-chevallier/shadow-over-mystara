//! Effect creation helpers (EFFCT) called by the player and scenery handlers.

use cps2::Cps2Bus;

use crate::families::effects_creators::leftovers::{Pure, Regs};
use crate::objects::{pools, Obj};
use crate::ops::Size;
use crate::player::Player;
use crate::record::record;
use crate::tables::engine;

/// The effects created here (EFFCT of types 25 and 4): the fields their creators set.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub struct Created(pub Obj);

impl Created {
    pub fn at(self, off: u32) -> u32 {
        self.0.at(off)
    }
}

record! {
    impl Created {
        /// Shock: low byte of the setting (h: the frame of the dust).
        setting_low, set_setting_low, SETTING_LOW: u8 @ 0x2d;
        /// Shock: high byte of the setting.
        setting, set_setting, SETTING: u8 @ 0x2f;
        /// Shock: counter cleared at the creation.
        timer, set_timer, TIMER: u16 @ 0x42;
        /// Spark: the two objects it lies between (words, RAM addresses).
        first, set_first, FIRST: u16 @ 0x50;
        second, set_second, SECOND: u16 @ 0x54;
    }
}

/// The object that counts the living players every 32 frames (`living_players`).
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub struct Census(pub Obj);

impl Census {
    pub fn at(self, off: u32) -> u32 {
        self.0.at(off)
    }
}

record! {
    impl Census {
        /// Frames before the next count.
        timer, set_timer, TIMER: u16 @ 0x20;
    }
}

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
    let facing = me.flip(b);
    fx.set_flip(b, facing);
    fx.set_u8(b, Obj::VARIANT, variant);
    Created(fx).set_setting_low(b, setting as u8);
    // `lsr.w #8,D1w`: X = last bit shifted out (bit 7), kept until the return
    let high = setting >> 8;
    r.x(setting & 0x80 != 0);
    r.d_word(1, high);
    Created(fx).set_setting(b, high as u8);
    Created(fx).set_timer(b, 0);
    r.flags(0, Size::Word);
    Some(fx)
}

/// 0xaa96c: spark (type 4, variant `variant`) tied to the creator and placed between it and `other` (A1), except for a
/// player in certain states. The processor receives D0 to D3 of the placement (`place_between`); the native returns
/// A1 to A4.
pub(crate) fn spark_by(r: &mut impl Regs, b: &mut Cps2Bus, me: Obj, other: u32, variant: u8) -> Option<Obj> {
    let p = Player::of(me);
    let skip = (p.attack_kind(b) as i8) < 2 && p.weapon_a(b).wrapping_sub(16) <= 3;
    if skip {
        return None;
    }
    let fx = r.allocated(b, &pools::EFFCT)?;
    fx.set_u8(b, Obj::ACTIVE, 1);
    fx.set_u8(b, Obj::KIND, SPARK_KIND);
    fx.set_u8(b, Obj::VARIANT, variant);
    let facing = me.flip(b) ^ 32;
    fx.set_flip(b, facing);
    Created(fx).set_first(b, me.word());
    Created(fx).set_second(b, other as u16);
    set(r, b, me, fx, other);
    Some(fx)
}

/// Places the spark (`place_between`); the processor receives D0, D1 (longs) and the low words of D2, D3.
fn set(r: &mut impl Regs, b: &mut Cps2Bus, me: Obj, fx: Obj, other: u32) -> u16 {
    let (d0, d1, d2, d3) = place_between(b, me, fx, Obj(other));
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
    let facing = me.flip(b) ^ 32;
    fx.set_flip(b, facing);
    let (first, second) = if owner_first { (me.0, other) } else { (other, me.0) };
    Created(fx).set_first(b, first as u16);
    Created(fx).set_second(b, second as u16);
    let d1 = set(r, b, me, fx, other);
    r.flags(d1 as u32, Size::Word);
    Some(fx)
}

/// Box of object `o`, in the collision boxes (`sprites/tables.json`): of the table `body` with the index `+0x38`
/// (`hit`), otherwise of the table `attack` with the index `+0x3a`; one set per `+0x2e`, eight bytes per box.
fn hit_box(b: &mut Cps2Bus, o: Obj, hit: bool) -> u32 {
    let tables = &crate::data::data().sprite_tables.box_tables;
    let (table, index) = if hit { (tables[0].rom, o.body_box(b)) } else { (tables[2].rom, o.attack_box(b)) };
    let set = o.box_set(b) as u32;
    let boxes = engine().long(b, table + 4 * set);
    boxes + 8 * index as u32
}

/// 0xab2b4: places the spark `fx` halfway between a box of the attacker `me` and a box of the
/// target `victim`, depending on the variant (variants 0, 1, 3: body of one against hit box of the other...),
/// at halfway depth, just in front of the nearest. Returns D0 to D3 as the original leaves them.
fn place_between(b: &mut Cps2Bus, me: Obj, fx: Obj, victim: Obj) -> (u16, u16, u16, u16) {
    let (me_hit, victim_hit) = match fx.variant(b) {
        0 | 1 | 3 => (false, true),
        4 | 6 => (false, false),
        _ => (true, false),
    };
    let (mine, theirs) = (hit_box(b, me, me_hit), hit_box(b, victim, victim_hit));
    // middle of the two edges, on one axis: x (offset 0 of the box, flipped according to the orientation) or z
    let middle = |b: &mut Cps2Bus, off: u32, field: u32, flip: bool| -> (u16, u16) {
        let mut d0 = engine().byte(b, mine + off) as i8 as i16 as u16;
        let mut d1 = engine().byte(b, theirs + off) as i8 as i16 as u16;
        if flip && me.flip(b) != 0 {
            d0 = d0.wrapping_neg();
        }
        if flip && victim.flip(b) != 0 {
            d1 = d1.wrapping_neg();
        }
        let a = d0.wrapping_add(me.u16(b, field));
        let c = d1.wrapping_add(victim.u16(b, field));
        let mut half = ((c.wrapping_sub(a) as i16).wrapping_abs() >> 1) as u16;
        if c as i16 <= a as i16 {
            half = half.wrapping_neg();
        }
        fx.set_u16(b, field, a.wrapping_add(half));
        (a, c)
    };
    middle(b, 0, Obj::X, true);
    let (z, their_z) = middle(b, 2, Obj::Z, false);
    let (my_depth, their_depth) = (me.u16(b, Obj::Y), victim.u16(b, Obj::Y));
    let gap = my_depth.wrapping_sub(their_depth);
    let (front, gap) = if (gap as i16) < 0 { (my_depth, gap.wrapping_neg()) } else { (their_depth, gap) };
    let z = z.wrapping_add(gap >> 1);
    fx.set_u16(b, Obj::Z, z);
    fx.set_u16(b, Obj::Y, front.wrapping_sub(1));
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
    object.set_flip(b, orientation);
    Ok(object)
}

/// 0x517ba: player effect (PEFF) of type 44, variant `variant`, setting `+0x2f` = `setting`.
pub fn effect_44(b: &mut Cps2Bus, variant: u8, setting: u8) -> Option<Obj> {
    let object = create(b, &pools::PEFF, 44, variant).ok()?;
    Created(object).set_setting(b, setting);
    Some(object)
}

/// 0xcc18e: every 32 frames (counter `+0x20` of object `me`), counts the living players into `ram::PLAYERS_ALIVE`;
/// returns the count when it is redone (D0 of the original).
pub fn living_players(b: &mut Cps2Bus, me: Obj) -> Option<u8> {
    let census = Census(me);
    let timer = census.timer(b).wrapping_sub(1);
    census.set_timer(b, timer);
    if (timer as i16) >= 0 {
        return None;
    }
    census.set_timer(b, 32);
    let alive = crate::ram::PLAYERS.iter().map(Player::of).filter(|p| p.active(b) != 0 && p.hp(b) != 0).count();
    crate::ram::PLAYERS_ALIVE.set(b, alive as u8);
    Some(alive as u8)
}

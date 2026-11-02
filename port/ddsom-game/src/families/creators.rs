//! Small object creation routines called by the player, enemy and scene code: they allocate a slot in a pool, then set
//! the class, the type and the link to the creator.
//!
//! Common contract: A0 = the creator; on return A1 = the created object and the carry set if the pool is full (callers
//! test `bcs`); the other flags are those of the last write.

use cps2::Cps2Bus;

use crate::mem::{prog32, prog8, rd16, rd8, wr16, wr8, A5};
use crate::objects::Obj;
use crate::objects::pools;
use super::effects_creators::leftovers::{Pure, Regs};
use crate::ops::Size;

/// Address word of an object (`move.w An,...`).
fn word(o: Obj) -> u16 {
    o.0 as u16
}

/// 0x51c06: player shot (PSHL) that accompanies the weapon of player A0, if its kind `+2` (shifted by 4 when `+0x28` !=
/// 2) is in the mask of table 0x51c5c; the shot and the player link to each other through `+0x5c`.
pub(crate) fn weapon_shot(r: &mut impl Regs, b: &mut Cps2Bus, me: Obj) -> Option<Obj> {
    me.set_u16(b, 0x5c, 0);
    let shift = if me.u8(b, 0x28) == 2 { 0 } else { 4 };
    let mask = prog32(b, 0x51c5c + shift);
    r.d(1, mask);
    let bit = (shift as u8).wrapping_add(me.u8(b, 0x2));
    r.d(0, bit as u32);
    if mask >> (bit & 31) & 1 == 0 {
        r.carry_only();
        return None;
    }
    let Some(shot) = r.allocated(b, &pools::PSHL) else {
        r.carry_only();
        return None;
    };
    me.set_u8(b, 0x38, 0);
    shot.set_u8(b, 0x0, 1);
    shot.set_u8(b, 0x2, 0);
    let flip = me.u8(b, 0x2c);
    shot.set_u8(b, 0x2c, flip);
    let owner = me.u16(b, 0x54);
    shot.set_u16(b, 0x50, owner);
    me.set_u16(b, 0x5c, word(shot));
    shot.set_u16(b, 0x5c, word(me));
    let power = me.u8(b, 0x49);
    shot.set_u8(b, 0x42, power);
    r.flags(power as u32, Size::Byte);
    Some(shot)
}

/// PEFF effect of kind `kind` (to be linked to the player afterwards by `link_effect`).
fn player_effect(r: &mut impl Regs, b: &mut Cps2Bus, kind: u8) -> Option<Obj> {
    let fx = r.allocated(b, &pools::PEFF)?;
    fx.set_u8(b, Obj::ACTIVE, 1);
    fx.set_u8(b, Obj::KIND, kind);
    Some(fx)
}

/// Common end of an effect: `+0x5d` = step `+7` of the player, `+0x50` = the player (flags of this `move.w`).
fn link_effect(r: &mut impl Regs, b: &mut Cps2Bus, fx: Obj, player: Obj) {
    let step = player.u8(b, 0x7);
    fx.set_u8(b, 0x5d, step);
    fx.set_u16(b, 0x50, player.word());
    r.flags(player.word() as u32, Size::Word);
}

/// 0x4b7a0: lightning of the spell (PEFF 10, variant `variant`, rank `+3` of the player in `+0x2f`).
pub fn bolt(b: &mut Cps2Bus, player: Obj, variant: u8) -> Option<Obj> {
    bolt_via(&mut Pure, b, player, variant)
}

pub(crate) fn bolt_via(r: &mut impl Regs, b: &mut Cps2Bus, player: Obj, variant: u8) -> Option<Obj> {
    let fx = player_effect(r, b, 10)?;
    fx.set_u8(b, Obj::VARIANT, variant);
    let rank = player.u8(b, 0x3);
    fx.set_u8(b, 0x2f, rank);
    link_effect(r, b, fx, player);
    Some(fx)
}

/// 0x4dc9a: two sparks of the spell (PEFF 25, the second with `+0x2f` = 1). Returns the second.
pub fn sparks(b: &mut Cps2Bus, player: Obj) -> Option<Obj> {
    sparks_via(&mut Pure, b, player)
}

pub(crate) fn sparks_via(r: &mut impl Regs, b: &mut Cps2Bus, player: Obj) -> Option<Obj> {
    let fx = player_effect(r, b, 25)?;
    link_effect(r, b, fx, player);
    let fx = player_effect(r, b, 25)?;
    fx.set_u8(b, 0x2f, 1);
    link_effect(r, b, fx, player);
    Some(fx)
}

pub(crate) fn enemy_shot_9_via(r: &mut impl Regs, b: &mut Cps2Bus, me: Obj) -> Option<Obj> {
    let phase = rd16(b, A5 + 0x6e) & 3;
    r.d_word(0, phase);
    if phase != 0 {
        r.flags(phase as u32, Size::Word);
        return None;
    }
    let shot = r.allocated(b, &pools::ESHL)?;
    shot.set_u8(b, 0x0, 1);
    shot.set_u8(b, 0x2, 9);
    shot.set_u8(b, 0x3, 0);
    wr16(b, shot.0 + 0x50, me.word());
    r.flags(me.word() as u32, Size::Word);
    Some(shot)
}

/// 0xcdedc: SET object 13 of variant D0. Active object of type `kind` in `pool`, of variant `variant`. On failure it
/// returns the A1 that the original leaves (`Pool::alloc_or_leftover`), for callers that do not test the carry.
fn object(b: &mut Cps2Bus, pool: &pools::Pool, kind: u8, variant: Option<u8>) -> Result<Obj, Obj> {
    let o = pool.alloc_or_leftover(b)?;
    o.set_u8(b, Obj::ACTIVE, 1);
    o.set_u8(b, Obj::KIND, kind);
    if let Some(v) = variant {
        o.set_u8(b, Obj::VARIANT, v);
    }
    Ok(o)
}

/// 0xcdedc: SET object of type 13, of variant `variant`.
pub fn set_13_pure(b: &mut Cps2Bus, variant: u8) -> Result<Obj, Obj> {
    object(b, &pools::SET, 13, Some(variant))
}

/// 0xce00a: SET object of type 15, of variant `variant`.
pub fn set_15_pure(b: &mut Cps2Bus, variant: u8) -> Result<Obj, Obj> {
    object(b, &pools::SET, 0xf, Some(variant))
}

/// 0xc9760: SET object of type 5, variant 1, `+0x24` zero.
pub fn set_5_pure(b: &mut Cps2Bus) -> Result<Obj, Obj> {
    let set = object(b, &pools::SET, 5, Some(1))?;
    set.set_u8(b, 0x24, 0);
    Ok(set)
}

/// 0xdfe26: SOBJ object of type 14 (its variant is not set).
pub fn sobj_14_pure(b: &mut Cps2Bus) -> Result<Obj, Obj> {
    object(b, &pools::SOBJ, 0xe, None)
}

/// 0xdfc78: SOBJ object of type 12, of variant `variant`, zero speed.
pub fn sobj_12_pure(b: &mut Cps2Bus, variant: u8) -> Result<Obj, Obj> {
    let o = object(b, &pools::SOBJ, 0xc, Some(variant))?;
    o.set_u16(b, 0x14, 0);
    Ok(o)
}

/// 0xc8312: SET object of variant `variant`, whose type depends on the scene (`A5+0x73 / 2`, table 0xc834c); counts the
/// SETs created (`A5+0x51`, `A5-0x5b8e`). The processor receives D0 = scene index, Z = (D0.b == 0).
pub fn scene_set(b: &mut Cps2Bus, variant: u8) -> Option<Obj> {
    scene_set_via(&mut Pure, b, variant)
}

/// `scene_set` whose failure is not tested: returns the A1 that the original leaves (`Pool::alloc_or_leftover`).
pub fn scene_set_or_rest(b: &mut Cps2Bus, variant: u8) -> Obj {
    scene_set(b, variant).unwrap_or_else(|| pools::SET.alloc_or_leftover(b).expect_err("the SET pool was just found full"))
}

/// 0x4cfbc: four 0x13 effects (variant 1, `+0x2f` = 3 down to 0, `+0x5a` = 2) linked to the creator, then the red
/// screen flash (0x1ac68).
pub fn effects_13(b: &mut Cps2Bus, creator: Obj) {
    for rank in (0..4u8).rev() {
        let Some(fx) = pools::PEFF.alloc(b) else { break };
        fx.set_u8(b, 0x0, 1);
        fx.set_u8(b, 0x2, 0x13);
        fx.set_u8(b, 0x3, 1);
        fx.set_u8(b, 0x2f, rank);
        fx.set_u8(b, 0x5a, 2);
        fx.set_u16(b, 0x50, creator.0 as u16);
    }
    crate::palette::set_flash_color(b, crate::palette::FLASH_COLOR_RED);
}

pub(crate) fn scene_set_via(r: &mut impl Regs, b: &mut Cps2Bus, variant: u8) -> Option<Obj> {
    let set = r.allocated(b, &pools::SET)?;
    for at in [A5 + 0x51, A5 - 0x5b8e] {
        let n = rd8(b, at).wrapping_add(1);
        wr8(b, at, n);
    }
    set.set_u8(b, Obj::ACTIVE, 1);
    set.set_u8(b, Obj::VARIANT, variant);
    set.set_u8(b, 0x22, 0);
    set.set_u8(b, 0x38, 0);
    set.set_u8(b, 0x37, 0xff);
    let scene = (rd8(b, A5 + 0x73) >> 1) as u32;
    r.d(0, scene);
    let kind = prog8(b, 0xc834c + scene);
    set.set_u8(b, Obj::KIND, kind);
    r.flags(scene, Size::Byte);
    Some(set)
}

/// Only one SET object 0xa at a time (`A5-0x5b8c`), and none during `A5-0x5fbe`.
const SET_A_ACTIVE: u32 = A5 - 0x5b8c;
const SET_A_BLOCKED: u32 = A5 - 0x5fbe;

/// 0xcc352 / 0xcc38e: SET object 0xa of variant `variant` (effect of a used item, wand), which holds the class `class`
/// (`+0x2f`); refused (the processor receives the carry alone, `move #1,CCR`) if one already exists, during
/// `SET_A_BLOCKED` or if the pool is full. The class is read after the allocation.
pub(crate) fn set_a_via(r: &mut impl Regs, b: &mut Cps2Bus, variant: u8, class: impl FnOnce(&mut Cps2Bus) -> u8) -> Option<Obj> {
    let created = (rd8(b, SET_A_ACTIVE) == 0 && rd8(b, SET_A_BLOCKED) == 0).then(|| r.allocated(b, &pools::SET)).flatten();
    let Some(set) = created else {
        r.carry_only();
        return None;
    };
    wr8(b, SET_A_ACTIVE, 1);
    set.set_u8(b, Obj::ACTIVE, 1);
    set.set_u8(b, Obj::KIND, 0xa);
    set.set_u8(b, Obj::VARIANT, variant);
    let v = class(b);
    set.set_u8(b, 0x2f, v);
    r.flags(v as u32, Size::Byte);
    Some(set)
}

/// 0xcc352: the class is that of the creator.
pub fn set_a(b: &mut Cps2Bus, creator: Obj, variant: u8) -> Option<Obj> {
    set_a_via(&mut Pure, b, variant, |b| creator.u8(b, Obj::VARIANT))
}

/// 0xcc38e: the class is that of the player linked to the creator (`+0x50`, left in A4).
pub(crate) fn set_a_player_via(r: &mut impl Regs, b: &mut Cps2Bus, creator: Obj, variant: u8) -> Option<Obj> {
    let mut player = None;
    let created = set_a_via(r, b, variant, |b| {
        let p = Obj(rd16(b, creator.0 + 0x50) as i16 as i32 as u32);
        player = Some(p);
        p.u8(b, Obj::VARIANT)
    });
    if let Some(p) = player {
        r.a(4, p.0);
    }
    created
}

/// 0xcc38e: `set_a`, the class being that of the player linked to the creator (`+0x50`, left in A4).
pub fn set_a_player(b: &mut Cps2Bus, creator: Obj, variant: u8) -> Option<Obj> {
    set_a_player_via(&mut Pure, b, creator, variant)
}

/// 0xdc5fe: ITEM 6 object (scene object) of variant D0, except when `A5-0x5ac6` is negative (the N flag of the test
/// then stays set); carry if the pool is full. 0xdc5fe: pickup object of the scene (ITEM 6) of variant `variant`,
/// except when the object level (`A5-0x5ac6`) is negative.
pub fn scene_object(b: &mut Cps2Bus, variant: u8) {
    if (rd8(b, A5 - 0x5ac6) as i8) < 0 {
        return;
    }
    if let Some(item) = pools::ITEM.alloc(b) {
        item.set_u8(b, 0x0, 1);
        item.set_u8(b, 0x2, 6);
        item.set_u8(b, 0x3, variant);
        item.set_u8(b, 0x2f, 0);
    }
}

/// 0xd9778 without registers: the music of the variant of scene object `scene` (nothing, 0xe0d8c or 0xe0c5c).
pub fn play_scene_music(b: &mut Cps2Bus, scene: Obj) {
    let variant = scene.u8(b, Obj::VARIANT) as u32;
    let i = 4 * prog8(b, 0xd9798 + variant) as u32;
    let routine = prog32(b, 0xd978c + i);
    scene_routine(b, scene, routine);
}

/// 0xd981a without registers: the scene exit routine of the variant of `scene`.
pub fn run_scene_exit(b: &mut Cps2Bus, scene: Obj) {
    let variant = scene.u8(b, Obj::VARIANT) as u32;
    let i = 4 * prog8(b, 0xd9836 + variant) as u32;
    let routine = prog32(b, 0xd982e + i);
    scene_routine(b, scene, routine);
}

/// A routine of the scene tables: nothing (0xd99f8) or a music (0xe0d8c, 0xe0c5c, played on `scene`).
fn scene_routine(b: &mut Cps2Bus, scene: Obj, routine: u32) {
    match routine {
        0xd99f8 => {}
        0xe0c5c | 0xe0d8c => crate::sound::play(b, routine, scene),
        r => unreachable!("scene routine {r:#x}: tables 0xd978c and 0xd982e only hold 0xd99f8, 0xe0d8c and 0xe0c5c"),
    }
}

/// 0xdf688: SOBJ object 8 linked to the creator (`+0x14`), variant 1 if the creator itself has a link.
pub fn sobj_8(b: &mut Cps2Bus, creator: Obj) -> Option<Obj> {
    sobj_8_via(&mut Pure, b, creator)
}

pub(crate) fn sobj_8_via(r: &mut impl Regs, b: &mut Cps2Bus, creator: Obj) -> Option<Obj> {
    let o = r.allocated(b, &pools::SOBJ)?;
    o.set_u8(b, Obj::ACTIVE, 1);
    o.set_u8(b, Obj::KIND, 8);
    o.set_u16(b, 0x14, creator.word());
    let linked = creator.u16(b, 0x14);
    r.flags(linked as u32, Size::Word);
    if linked != 0 {
        o.set_u8(b, Obj::VARIANT, 1);
        r.flags(1, Size::Byte);
    }
    Some(o)
}

/// Boss health bar (SET object 0x14) held by 0xd5f30 (low word of its address).
const LIFE_BAR_SET: u32 = A5 - 0x5b8a;

/// 0xd5f30: in game, SET object 0x14 (variant 0) linked to the creator `creator` (`+0x20`), held in `A5-0x5b8a`. Core.
pub fn set_14(b: &mut Cps2Bus, creator: Obj) -> Option<Obj> {
    set_14_via(&mut Pure, b, creator)
}

fn set_14_via(r: &mut impl Regs, b: &mut Cps2Bus, creator: Obj) -> Option<Obj> {
    let game = crate::ram::GAME.get(b);
    r.flags(game as u32, Size::Byte);
    if game == 0 {
        return None;
    }
    let set = r.allocated(b, &pools::SET)?;
    set.set_u8(b, Obj::ACTIVE, 1);
    set.set_u8(b, Obj::KIND, 0x14);
    set.set_u16(b, 0x20, creator.word());
    set.set_u8(b, Obj::VARIANT, 0);
    wr16(b, LIFE_BAR_SET, set.word());
    r.flags(set.word() as u32, Size::Word);
    Some(set)
}

/// Value of `+0x42` of the SET object 0x15 of 0xd6952, by creator type.
const SET_15_PARAMS: u32 = 0xd697c;

/// 0xd6952: SET object 0x15 (variant 2) whose `+0x42` comes from table 0xd697c according to the creator's type. Core.
pub fn set_15_b(b: &mut Cps2Bus, creator: Obj) -> Option<Obj> {
    set_15_b_via(&mut Pure, b, creator)
}

fn set_15_b_via(r: &mut impl Regs, b: &mut Cps2Bus, creator: Obj) -> Option<Obj> {
    let kind = creator.u8(b, Obj::KIND) as u32;
    let set = r.allocated(b, &pools::SET)?;
    set.set_u8(b, Obj::ACTIVE, 1);
    set.set_u8(b, Obj::KIND, 0x15);
    set.set_u8(b, Obj::VARIANT, 2);
    r.d(0, 2 * kind);
    let v = crate::mem::prog16(b, SET_15_PARAMS + 2 * kind);
    set.set_u16(b, 0x42, v);
    r.flags(v as u32, Size::Word);
    Some(set)
}

/// Player shot created: resistances `+0x45`/`+0x46` read from two tables (in the clear) according to the player's level
/// `+0x10c`.
fn shot_strength_core(b: &mut Cps2Bus, shot: Obj, level: u8, tables: (u32, u32)) {
    let (a, d) = (rd8(b, tables.0 + level as u32), rd8(b, tables.1 + level as u32));
    shot.set_u8(b, 0x45, a);
    shot.set_u8(b, 0x46, d);
}

/// 0x53e48: lightning of player `me` (PSHL 0x13) with speeds `vx`, `vz` (D0.w, D1.w of the original), while some remain
/// (`A5-0x5fb4`). Called by the wizard's storm (PEFF 25).
pub(crate) fn bolt_shot(b: &mut Cps2Bus, me: Obj, vx: u16, vz: u16) {
    let left = rd16(b, A5 - 0x5fb4);
    if left == 0 {
        return;
    }
    wr16(b, A5 - 0x5fb4, left - 1);
    if let Some(shot) = pools::PSHL.alloc(b) {
        shot.set_u8(b, 0x0, 1);
        shot.set_u8(b, 0x2, 0x13);
        shot.set_u16(b, 0x50, me.0 as u16);
        shot.set_u16(b, 0x14, vx);
        shot.set_u16(b, 0x18, vz);
        let level = me.u8(b, 0x10c);
        shot_strength_core(b, shot, level, (0x1042a6, 0x1042c4));
    }
}

/// 0x52578: type 5 shot at the position and orientation of `me`, same player (pool full: nothing). Called by the ray
/// impact (PEFF 10).
pub(crate) fn shot_5(b: &mut Cps2Bus, me: Obj) {
    if let Some(shot) = pools::PSHL.alloc(b) {
        shot.set_u8(b, 0x0, 1);
        shot.set_u8(b, 0x2, 5);
        for off in [0x8, 0xc, 0x10] {
            let v = me.u16(b, off);
            shot.set_u16(b, off, v);
        }
        let flip = me.u8(b, 0x2c);
        shot.set_u8(b, 0x2c, flip);
        let owner = me.u16(b, 0x50);
        shot.set_u16(b, 0x50, owner);
        let level = me.u8(b, 0x10c);
        shot_strength_core(b, shot, level, (0x10426a, 0x104288));
    }
}

/// Player effect (PEFF) of type `kind`, variant `variant`, linked to the player `me` (`+0x50`).
pub(crate) fn peff_via(r: &mut impl Regs, b: &mut Cps2Bus, me: Obj, kind: u8, variant: Option<u8>) -> Option<Obj> {
    let fx = r.allocated(b, &pools::PEFF)?;
    fx.set_u8(b, 0x0, 1);
    fx.set_u8(b, 0x2, kind);
    if let Some(v) = variant {
        fx.set_u8(b, 0x3, v);
    }
    fx.set_u16(b, 0x50, me.word());
    r.flags(me.0 & 0xffff, Size::Word);
    Some(fx)
}

/// 0x51bb6: effect 0x2f (no link).
pub(crate) fn effect_2f_via(r: &mut impl Regs, b: &mut Cps2Bus) -> Option<Obj> {
    let fx = r.allocated(b, &pools::PEFF)?;
    fx.set_u8(b, 0x0, 1);
    fx.set_u8(b, 0x2, 0x2f);
    r.flags(0x2f, Size::Byte);
    Some(fx)
}

/// Player effects created by the player module (`player/`): cores on `r` (the player's leftovers D2-D5).
pub(crate) mod for_players {
    use super::*;

    /// 0x4b7a0: bolt (PEFF 10) of variant `variant`.
    pub(crate) fn bolt(r: &mut impl Regs, b: &mut Cps2Bus, me: Obj, variant: u8) -> Option<Obj> {
        bolt_via(r, b, me, variant)
    }
    /// 0x4dc9a: two sparks (PEFF 25).
    pub(crate) fn sparks(r: &mut impl Regs, b: &mut Cps2Bus, me: Obj) -> Option<Obj> {
        sparks_via(r, b, me)
    }
    /// 0x4b340: quake effects (see `quake`).
    pub(crate) fn quake(r: &mut impl Regs, b: &mut Cps2Bus, me: Obj, pair: bool) -> Option<Obj> {
        quake_via(r, b, me, pair)
    }
    /// 0xcc352: SET object 0xa of variant `variant`, of the class of `me` (see `set_a`).
    pub(crate) fn set_a(r: &mut impl Regs, b: &mut Cps2Bus, me: Obj, variant: u8) -> Option<Obj> {
        set_a_via(r, b, variant, |b| me.u8(b, Obj::VARIANT))
    }

    /// 0x4b518: effect 8 linked to `me`.
    pub(crate) fn effect_8(r: &mut impl Regs, b: &mut Cps2Bus, me: Obj) -> Option<Obj> {
        peff_via(r, b, me, 8, None)
    }
    /// 0x51bb6: effect 0x2f (not linked).
    pub(crate) fn effect_2f(r: &mut impl Regs, b: &mut Cps2Bus) -> Option<Obj> {
        effect_2f_via(r, b)
    }
    /// 0x4b8d0: effect 0xb (variant 0).
    pub(crate) fn whirl(r: &mut impl Regs, b: &mut Cps2Bus, me: Obj) -> Option<Obj> {
        peff_via(r, b, me, 0xb, Some(0))
    }
}

/// 0x4b340 (spell quake): `pair`, two 7 effects (variant 0, `+0x2f` = 0 and 1); otherwise one 7 effect of variant 1.
/// The player's step `+7` is copied to `+0x5d`. Returns the last effect created.
pub fn quake(b: &mut Cps2Bus, player: Obj, pair: bool) -> Option<Obj> {
    quake_via(&mut Pure, b, player, pair)
}

pub(crate) fn quake_via<R: Regs>(r: &mut R, b: &mut Cps2Bus, player: Obj, pair: bool) -> Option<Obj> {
    let step = player.u8(b, 7);
    let effect = |r: &mut R, b: &mut Cps2Bus, variant: u8, index: Option<u8>| {
        let fx = player_effect(r, b, 7)?;
        fx.set_u8(b, Obj::VARIANT, variant);
        if let Some(i) = index {
            fx.set_u8(b, 0x2f, i);
        }
        fx.set_u8(b, 0x5d, step);
        fx.set_u16(b, 0x50, player.word());
        r.flags(player.word() as u32, Size::Word);
        Some(fx)
    };
    if pair {
        effect(r, b, 0, Some(0))?;
        effect(r, b, 0, Some(1))
    } else {
        effect(r, b, 1, None)
    }
}

/// 0x63ae0 without registers: true if both type 7 enemies are created.
pub fn enemy_7_pair_created(b: &mut Cps2Bus, creator: Obj) -> bool {
    super::small::enemy_pair_kind_via(&mut Pure, b, creator, 7)
}

/// Core of `enemy_at`: the enemy created at `pos` = (x, z, y).
pub(crate) fn enemy_at_via(r: &mut impl Regs, b: &mut Cps2Bus, me: Obj, kind: u8, link: u32, extra: &[(u32, u8)], pos: (u16, u16, u16)) -> Option<Obj> {
    let o = r.allocated(b, &pools::ENEMY)?;
    o.set_u8(b, 0x0, 1);
    o.set_u8(b, 0x2, kind);
    o.set_u16(b, 0x8, pos.0);
    o.set_u16(b, 0xc, pos.1);
    o.set_u16(b, 0x10, pos.2);
    o.set_u16(b, link, me.word());
    o.set_u8(b, 0x3, 5);
    for &(off, v) in extra {
        o.set_u8(b, off, v);
    }
    Some(o)
}

/// 0x5f764 without registers: enemy of type 5 at `pos` = (x, z, y), linked by `+0xa4`.
pub(crate) fn enemy_5_at_pure(b: &mut Cps2Bus, me: Obj, pos: (u16, u16, u16)) -> Option<Obj> {
    enemy_at_via(&mut Pure, b, me, 5, 0xa4, &[], pos)
}

/// 0x57c8c without registers: enemy of type 1 at `pos` = (x, z, y), linked by `+0xaa`.
pub(crate) fn enemy_1_at_pure(b: &mut Cps2Bus, me: Obj, pos: (u16, u16, u16)) -> Option<Obj> {
    enemy_at_via(&mut Pure, b, me, 1, 0xaa, &[(0xac, 1), (0x2f, 0)], pos)
}

/// 0x57c66: is object `o` in the right half of the screen (x >= camera + 0xc0)? Core.
pub fn in_right_half(b: &mut Cps2Bus, o: Obj) -> bool {
    let (camera, _) = crate::ram::camera(b);
    (camera.wrapping_add(0xc0).wrapping_sub(o.u16(b, Obj::X)) as i16) <= 0
}

/// 0x5cdc6: random draw, true 10 times out of 16 (bit (random & 0xf) of mask 0x03ff). Core.
pub fn chance_10_16(b: &mut Cps2Bus) -> bool {
    let bit = crate::rng::random(b) & 0xf;
    0x03ff >> bit & 1 != 0
}

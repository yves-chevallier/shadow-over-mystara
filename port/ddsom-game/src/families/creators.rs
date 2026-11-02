//! Small object creation routines called by the player, enemy and scene code: they allocate a slot in a pool, then set
//! the class, the type and the link to the creator.
//!
//! Common contract: A0 = the creator; on return A1 = the created object and the carry set if the pool is full (callers
//! test `bcs`); the other flags are those of the last write.

use cps2::Cps2Bus;
use m68k::decode::Size;
use m68k::Cpu;

use crate::mem::{prog32, prog8, rd16, rd8, rts, wr16, wr8, A5};
use crate::objects::Obj;
use crate::{call, regs, Native};
use crate::objects::pools;
use super::effects_creators::leftovers::{Pure, Regs};


/// Allocates in a pool; `None` (carry set) if the pool is full.
fn alloc(c: &mut Cpu, b: &mut Cps2Bus, pool: &pools::Pool) -> Option<Obj> {
    pool.alloc_68k(c, b)
}

/// Flags of a `move` of `v` (size `size`): N and Z according to the value, V and C cleared.
fn moved(c: &mut Cpu, v: u32, size: Size) {
    c.flags_logic(v, size);
}

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
    r.drapeaux(power as u32, Size::Byte);
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
    r.drapeaux(player.word() as u32, Size::Word);
}

/// 0x4b7a0: lightning of the spell (PEFF 10, variant `variant`, rank `+3` of the player in `+0x2f`).
pub fn bolt(b: &mut Cps2Bus, player: Obj, variant: u8) -> Option<Obj> {
    bolt_via(&mut Pure, b, player, variant)
}

/// `bolt` with the original's registers (callers by address: `player/`): A1, carry and flags.
#[track_caller]
pub fn bolt_68k(c: &mut Cpu, b: &mut Cps2Bus, player: Obj, variant: u8) -> Option<Obj> {
    c.a[0] = player.0;
    let r = bolt_via(c, b, player, variant);
    crate::diag::dead_regs(c, "effects", CREATED);
    r
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

/// `sparks` with the original's registers (callers by address: `player/`).
#[track_caller]
pub fn sparks_68k(c: &mut Cpu, b: &mut Cps2Bus, player: Obj) -> Option<Obj> {
    c.a[0] = player.0;
    let r = sparks_via(c, b, player);
    crate::diag::dead_regs(c, "effects", CREATED);
    r
}

pub(crate) fn sparks_via(r: &mut impl Regs, b: &mut Cps2Bus, player: Obj) -> Option<Obj> {
    let fx = player_effect(r, b, 25)?;
    link_effect(r, b, fx, player);
    let fx = player_effect(r, b, 25)?;
    fx.set_u8(b, 0x2f, 1);
    link_effect(r, b, fx, player);
    Some(fx)
}

/// 0x9ef20: ESHL shot 9 linked to enemy A0, one frame out of four (counter `A5+0x6e`).
fn enemy_shot_9(c: &mut Cpu, b: &mut Cps2Bus) {
    let me = Obj(c.a[0]);
    enemy_shot_9_via(c, b, me);
    rts(c, b);
}

pub(crate) fn enemy_shot_9_via(r: &mut impl Regs, b: &mut Cps2Bus, me: Obj) -> Option<Obj> {
    let phase = rd16(b, A5 + 0x6e) & 3;
    r.d_word(0, phase);
    if phase != 0 {
        r.drapeaux(phase as u32, Size::Word);
        return None;
    }
    let shot = r.allocated(b, &pools::ESHL)?;
    shot.set_u8(b, 0x0, 1);
    shot.set_u8(b, 0x2, 9);
    shot.set_u8(b, 0x3, 0);
    wr16(b, shot.0 + 0x50, me.word());
    r.drapeaux(me.word() as u32, Size::Word);
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

fn set_13(c: &mut Cpu, b: &mut Cps2Bus) {
    if let Some(set) = alloc(c, b, &pools::SET) {
        set.set_u8(b, 0x0, 1);
        set.set_u8(b, 0x2, 13);
        let v = c.d[0] as u8;
        set.set_u8(b, 0x3, v);
        moved(c, v as u32, Size::Byte);
    }
    rts(c, b);
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
    r.drapeaux(scene, Size::Byte);
    Some(set)
}

fn scene_set_native(c: &mut Cpu, b: &mut Cps2Bus) {
    let v = c.d[0] as u8;
    scene_set_via(c, b, v);
    rts(c, b);
}

/// 0xd981a: end of scene specific to the variant (`+3`, index table 0xd9836, routines 0xd982e: in practice always
/// 0xd99f8, a plain return).
fn scene_exit(c: &mut Cpu, b: &mut Cps2Bus) {
    let variant = rd8(b, c.a[0] + 3) as u32;
    let i = 4 * prog8(b, 0xd9836 + variant) as u32;
    c.d[0] = i;
    c.a[1] = prog32(b, 0xd982e + i);
    let target = c.a[1];
    call(c, b, target);
    rts(c, b);
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
    r.drapeaux(v as u32, Size::Byte);
    Some(set)
}

/// 0xcc352: the class is that of the creator.
pub fn set_a(b: &mut Cps2Bus, creator: Obj, variant: u8) -> Option<Obj> {
    set_a_via(&mut Pure, b, variant, |b| creator.u8(b, Obj::VARIANT))
}

/// `set_a` with the original's registers (callers by address: `player/`): A0 = the creator, D0.b = the variant; A1,
/// carry and flags.
#[track_caller]
pub fn set_a_68k(c: &mut Cpu, b: &mut Cps2Bus, creator: Obj, variant: u8) -> Option<Obj> {
    c.a[0] = creator.0;
    let r = set_a_via(c, b, variant, |b| creator.u8(b, Obj::VARIANT));
    crate::diag::dead_regs(c, "effects", CREATED);
    r
}

fn set_a_creator(c: &mut Cpu, b: &mut Cps2Bus) {
    let (me, v) = (Obj(c.a[0]), c.d[0] as u8);
    set_a_via(c, b, v, |b| me.u8(b, Obj::VARIANT));
    rts(c, b);
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

fn set_a_player_native(c: &mut Cpu, b: &mut Cps2Bus) {
    let (me, v) = (Obj(c.a[0]), c.d[0] as u8);
    set_a_player_via(c, b, me, v);
    rts(c, b);
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

fn scene_item(c: &mut Cpu, b: &mut Cps2Bus) {
    let level = rd8(b, A5 - 0x5ac6);
    if (level as i8) < 0 {
        moved(c, level as u32, Size::Byte);
        return rts(c, b);
    }
    if let Some(item) = alloc(c, b, &pools::ITEM) {
        item.set_u8(b, 0x0, 1);
        item.set_u8(b, 0x2, 6);
        item.set_u8(b, 0x3, c.d[0] as u8);
        item.set_u8(b, 0x2f, 0);
        moved(c, 0, Size::Byte);
    }
    rts(c, b);
}

/// 0xd9778: music specific to the scene variant (`+3`, index table 0xd9798, routines 0xd978c: nothing (0xd99f8) or the
/// music of scene 0xe0c5c). D0 = offset in the table, A1 = the routine.
fn scene_music(c: &mut Cpu, b: &mut Cps2Bus) {
    let variant = rd8(b, c.a[0] + 3) as u32;
    let i = 4 * prog8(b, 0xd9798 + variant) as u32;
    c.d[0] = i;
    c.a[1] = prog32(b, 0xd978c + i);
    let target = c.a[1];
    call(c, b, target);
    rts(c, b);
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

/// 0xd99f8: empty routine of the scene tables.
fn nothing(c: &mut Cpu, b: &mut Cps2Bus) {
    rts(c, b);
}

/// 0xce00a: SET object 15 of variant D0.
fn set_15(c: &mut Cpu, b: &mut Cps2Bus) {
    if let Some(set) = alloc(c, b, &pools::SET) {
        set.set_u8(b, 0x0, 1);
        set.set_u8(b, 0x2, 0xf);
        let v = c.d[0] as u8;
        set.set_u8(b, 0x3, v);
        moved(c, v as u32, Size::Byte);
    }
    rts(c, b);
}

/// 0xdfe26: SOBJ object 14.
fn sobj_14(c: &mut Cpu, b: &mut Cps2Bus) {
    if let Some(o) = alloc(c, b, &pools::SOBJ) {
        o.set_u8(b, 0x0, 1);
        o.set_u8(b, 0x2, 0xe);
        moved(c, 0xe, Size::Byte);
    }
    rts(c, b);
}

/// 0xdfc78: SOBJ object 12 of variant D0, zero speed; flags of D0.b.
fn sobj_12(c: &mut Cpu, b: &mut Cps2Bus) {
    if let Some(o) = alloc(c, b, &pools::SOBJ) {
        o.set_u8(b, 0x0, 1);
        o.set_u8(b, 0x2, 0xc);
        let v = c.d[0] as u8;
        o.set_u8(b, 0x3, v);
        o.set_u16(b, 0x14, 0);
        moved(c, v as u32, Size::Byte);
    }
    rts(c, b);
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
    r.drapeaux(linked as u32, Size::Word);
    if linked != 0 {
        o.set_u8(b, Obj::VARIANT, 1);
        r.drapeaux(1, Size::Byte);
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
    r.drapeaux(game as u32, Size::Byte);
    if game == 0 {
        return None;
    }
    let set = r.allocated(b, &pools::SET)?;
    set.set_u8(b, Obj::ACTIVE, 1);
    set.set_u8(b, Obj::KIND, 0x14);
    set.set_u16(b, 0x20, creator.word());
    set.set_u8(b, Obj::VARIANT, 0);
    wr16(b, LIFE_BAR_SET, set.word());
    r.drapeaux(set.word() as u32, Size::Word);
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
    r.drapeaux(v as u32, Size::Word);
    Some(set)
}

/// 0xc9760: SET object 5 (variant 1), `+0x24` zero.
fn set_5(c: &mut Cpu, b: &mut Cps2Bus) {
    if let Some(set) = alloc(c, b, &pools::SET) {
        set.set_u8(b, 0x0, 1);
        set.set_u8(b, 0x2, 5);
        set.set_u8(b, 0x3, 1);
        set.set_u8(b, 0x24, 0);
        moved(c, 0, Size::Byte);
    }
    rts(c, b);
}

/// 0xd6182: 44 cells of a tile column (A1, step 0x80) filled with 0x00200000 (empty tile).
fn blank_column(c: &mut Cpu, b: &mut Cps2Bus) {
    for _ in 0..0x2c {
        crate::mem::wr32(b, c.a[1], 0x20_0000);
        c.a[1] = c.a[1].wrapping_add(0x80);
    }
    c.d[0] = 0xffff;
    c.d[4] = 0x20_0000;
    rts(c, b);
}

/// 0xceb46: bonus sheet in A4 (guard byte, then 9 bytes): if the guard is positive, each characteristic `+0x151..` of
/// player A0 takes the maximum; A4 passes the sheet.
fn bonus_sheet(c: &mut Cpu, b: &mut Cps2Bus) {
    let guard = rd8(b, c.a[4]);
    c.a[4] = c.a[4].wrapping_add(1);
    if (guard as i8) < 0 {
        c.a[4] = c.a[4].wrapping_add(9);
        return rts(c, b);
    }
    let mut at = c.a[0] + 0x151;
    for _ in 0..9 {
        let v = rd8(b, c.a[4]);
        c.a[4] = c.a[4].wrapping_add(1);
        c.d[1] = c.d[1] & !0xff | v as u32;
        if v as i8 > rd8(b, at) as i8 {
            wr8(b, at, v);
        }
        at += 1;
    }
    c.a[1] = at;
    c.d[0] = 0xffff;
    rts(c, b);
}

/// The scenery of the scene is restored: cameras (`cameras`: 0x33aa6 at the start of the scene, 0x33b24 on return from
/// a side zone), scene script (0xf2e56), palette pages (0x1a360, 0x1a678, 0x1a8ac), all the pages to copy back, full
/// brightness (0x1686). Common start of 0xcc0ca, of the side zone doors (SET 7) and of the passage (EFFCT 14). A0 and
/// D7 preserved.
pub(crate) fn resume_scenery(c: &mut Cpu, b: &mut Cps2Bus, cameras: u32) {
    let (a0, d7) = (c.a[0], c.d[7]);
    call(c, b, cameras);
    (c.a[0], c.d[7]) = (a0, d7);
    call(c, b, crate::scene::script::RESET_ENTRY);
    for routine in [0x1a360, 0x1a678, 0x1a8ac] {
        call(c, b, routine);
    }
    (c.a[0], c.d[7]) = (a0, d7);
    wr8(b, A5 + 0x48e, 0xf);
    crate::palette::full_brightness(b);
}

/// 0xcc0ca: the scene restarts from its beginning (h): scenery restored (`resume_scenery`); each living player returns
/// to normal play (hidden and in scripted entry if down or dead), is repositioned (0x20de6) then sorted. A0 and D7
/// preserved.
pub(crate) fn restart_scene(c: &mut Cpu, b: &mut Cps2Bus) {
    resume_scenery(c, b, 0x33aa6);
    wr16(b, A5 + 0x52, 0);
    let alive = |b: &mut Cps2Bus, p: u32| rd8(b, p) != 0 && rd16(b, p + 0x62) != 0;
    for k in 0..4 {
        let p = A5 + 0x5de + 0x200 * k;
        if !alive(b, p) {
            continue;
        }
        wr8(b, p, 1);
        let state = rd8(b, p + 4);
        c.d[0] = c.d[0] & !0xff | state as u32;
        crate::mem::wr32(b, p + 4, 0);
        let flags = rd8(b, p + 1) & !2;
        wr8(b, p + 1, flags);
        for off in [0x66, 0x15c, 0x8a, 0x10f] {
            wr8(b, p + off, 0);
        }
        if state as i8 > 2 {
            wr8(b, p + 1, flags | 2);
            wr16(b, p + 4, 0x402);
        }
    }
    c.a[1] = A5 + 0x5de + 0x800;
    call(c, b, 0x20de6);
    let a0 = c.a[0];
    for k in 0..4 {
        let p = A5 + 0x5de + 0x200 * k;
        if alive(b, p) {
            c.a[0] = p;
            crate::objects::lists::ACTORS.reinsert_68k(c, b, Obj(c.a[0]));
        }
    }
    c.a[0] = a0;
    c.d[6] = 0xffff;
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

/// Player effect (PEFF) of type `kind`, variant `variant`, linked to the player (`+0x50`).
fn peff(c: &mut Cpu, b: &mut Cps2Bus, kind: u8, variant: Option<u8>) -> Option<Obj> {
    let me = c.a[0];
    let fx = alloc(c, b, &pools::PEFF)?;
    fx.set_u8(b, 0x0, 1);
    fx.set_u8(b, 0x2, kind);
    if let Some(v) = variant {
        fx.set_u8(b, 0x3, v);
    }
    fx.set_u16(b, 0x50, me as u16);
    moved(c, me & 0xffff, Size::Word);
    Some(fx)
}

/// Player effects created by the player module, still on the processor: the arguments are passed, A0 = the player,
/// and the registers the original leaves (A1, carry, flags) are written like the native writes them.
pub(crate) mod for_players {
    use super::*;

    /// 0x4b7a0: bolt (PEFF 10) of variant `variant`.
    pub(crate) fn bolt_68k(c: &mut Cpu, b: &mut Cps2Bus, me: Obj, variant: u8) -> Option<Obj> {
        c.a[0] = me.0;
        bolt_via(c, b, me, variant)
    }
    /// 0x4dc9a: two sparks (PEFF 25).
    pub(crate) fn sparks_68k(c: &mut Cpu, b: &mut Cps2Bus, me: Obj) -> Option<Obj> {
        c.a[0] = me.0;
        sparks_via(c, b, me)
    }
    /// 0x4b518: effect 8 linked to `me`.
    pub(crate) fn effect_8_68k(c: &mut Cpu, b: &mut Cps2Bus, me: Obj) -> Option<Obj> {
        c.a[0] = me.0;
        peff(c, b, 8, None)
    }
    /// 0x51bb6: effect 0x2f (not linked).
    pub(crate) fn effect_2f_68k(c: &mut Cpu, b: &mut Cps2Bus, me: Obj) -> Option<Obj> {
        c.a[0] = me.0;
        let fx = alloc(c, b, &pools::PEFF)?;
        fx.set_u8(b, 0x0, 1);
        fx.set_u8(b, 0x2, 0x2f);
        moved(c, 0x2f, Size::Byte);
        Some(fx)
    }
    /// 0x4b8d0: effect 0xb (variant 0).
    pub(crate) fn whirl_68k(c: &mut Cpu, b: &mut Cps2Bus, me: Obj) -> Option<Obj> {
        c.a[0] = me.0;
        peff(c, b, 0xb, Some(0))
    }
}

/// 0x4b8d0: effect 0xb (variant 0).
fn peff_b(c: &mut Cpu, b: &mut Cps2Bus) {
    peff(c, b, 0xb, Some(0));
    rts(c, b);
}

/// 0x4b518: effect 8.
fn peff_8(c: &mut Cpu, b: &mut Cps2Bus) {
    peff(c, b, 8, None);
    rts(c, b);
}

/// 0x51bb6: effect 0x2f (no link).
fn peff_2f(c: &mut Cpu, b: &mut Cps2Bus) {
    if let Some(fx) = alloc(c, b, &pools::PEFF) {
        fx.set_u8(b, 0x0, 1);
        fx.set_u8(b, 0x2, 0x2f);
        moved(c, 0x2f, Size::Byte);
    }
    rts(c, b);
}

/// 0x4cfbc: four 0x13 effects (variant 1, `+0x2f` = 3 down to 0, `+0x5a` = 2), then the screen flash (0x1ac68).
fn peff_13_x4(c: &mut Cpu, b: &mut Cps2Bus) {
    let me = c.a[0];
    c.d[5] = 3;
    loop {
        let Some(fx) = alloc(c, b, &pools::PEFF) else { break };
        fx.set_u8(b, 0x0, 1);
        fx.set_u8(b, 0x2, 0x13);
        fx.set_u8(b, 0x3, 1);
        fx.set_u8(b, 0x2f, c.d[5] as u8);
        fx.set_u8(b, 0x5a, 2);
        fx.set_u16(b, 0x50, me as u16);
        let n = (c.d[5] as u16).wrapping_sub(1);
        c.d[5] = c.d[5] & !0xffff | n as u32;
        if n == 0xffff {
            break;
        }
    }
    crate::palette::set_flash_color(b, crate::palette::FLASH_COLOR_RED);
    rts(c, b);
}

/// 0x4b340 (spell quake): `pair`, two 7 effects (variant 0, `+0x2f` = 0 and 1); otherwise one 7 effect of variant 1.
/// The player's step `+7` is copied to `+0x5d`. Returns the last effect created.
pub fn quake(b: &mut Cps2Bus, player: Obj, pair: bool) -> Option<Obj> {
    quake_via(&mut Pure, b, player, pair)
}

/// `quake` with the original's registers (callers by address: `player/`): A0 = the player, D0.b zero for the pair; A1,
/// carry and flags.
#[track_caller]
pub fn quake_68k(c: &mut Cpu, b: &mut Cps2Bus, player: Obj, pair: bool) -> Option<Obj> {
    c.a[0] = player.0;
    let r = quake_via(c, b, player, pair);
    crate::diag::dead_regs(c, "effects", CREATED);
    r
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
        r.drapeaux(player.word() as u32, Size::Word);
        Some(fx)
    };
    if pair {
        effect(r, b, 0, Some(0))?;
        effect(r, b, 0, Some(1))
    } else {
        effect(r, b, 1, None)
    }
}

fn quake_native(c: &mut Cpu, b: &mut Cps2Bus) {
    let (me, pair) = (Obj(c.a[0]), c.d[0] as u8 == 0);
    quake_via(c, b, me, pair);
    rts(c, b);
}

/// 0x63ae0: if more than two enemy slots remain (`A5+0x7e62`), two enemies of type 7 (variants 0 and 1) linked to the
/// creator through `+0x46`; D0 = 1, otherwise 0 (the allocation is not checked).
fn enemy_7_pair(c: &mut Cpu, b: &mut Cps2Bus) {
    let me = Obj(c.a[0]);
    super::small::enemy_pair_kind_via(c, b, me, 7);
    rts(c, b);
}

/// 0x63ae0 without registers: true if both type 7 enemies are created.
pub fn enemy_7_pair_created(b: &mut Cps2Bus, creator: Obj) -> bool {
    super::small::enemy_pair_kind_via(&mut Pure, b, creator, 7)
}

/// Enemy of type `kind` (variant 5) placed at (D0, D1, D2) = (x, z, y), linked to the creator by `link`; flags of D0.b
/// (`or.b D0b,D0b`).
fn enemy_at(c: &mut Cpu, b: &mut Cps2Bus, kind: u8, link: u32, extra: &[(u32, u8)]) {
    let (me, pos) = (Obj(c.a[0]), (c.d[0] as u16, c.d[1] as u16, c.d[2] as u16));
    let d0 = c.d[0];
    if enemy_at_via(c, b, me, kind, link, extra, pos).is_some() {
        moved(c, d0 & 0xff, Size::Byte);
    }
    rts(c, b);
}

/// Core of `enemy_at`: the enemy created at `pos` = (x, z, y).
fn enemy_at_via(r: &mut impl Regs, b: &mut Cps2Bus, me: Obj, kind: u8, link: u32, extra: &[(u32, u8)], pos: (u16, u16, u16)) -> Option<Obj> {
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

/// 0x5f764: enemy of type 5 at (D0, D1, D2), linked by `+0xa4`.
fn enemy_5_at(c: &mut Cpu, b: &mut Cps2Bus) {
    enemy_at(c, b, 5, 0xa4, &[]);
}

/// 0x5f764 without registers: enemy of type 5 at `pos` = (x, z, y), linked by `+0xa4`.
pub(crate) fn enemy_5_at_pure(b: &mut Cps2Bus, me: Obj, pos: (u16, u16, u16)) -> Option<Obj> {
    enemy_at_via(&mut Pure, b, me, 5, 0xa4, &[], pos)
}

/// 0x57c8c: enemy of type 1 at (D0, D1, D2), linked by `+0xaa` (`+0xac` = 1, `+0x2f` = 0).
fn enemy_1_at(c: &mut Cpu, b: &mut Cps2Bus) {
    enemy_at(c, b, 1, 0xaa, &[(0xac, 1), (0x2f, 0)]);
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

/// 0x97218: boss: `+0x67` = 1, `+0x79` = 0x31, state 0x02020400.
fn boss_state_0202(c: &mut Cpu, b: &mut Cps2Bus) {
    let me = Obj(c.a[0]);
    me.set_u8(b, 0x67, 1);
    me.set_u8(b, 0x79, 0x31);
    me.set_u32(b, 0x4, 0x0202_0400);
    moved(c, 0x0202_0400, Size::Long);
    rts(c, b);
}

const fn native(entry: u32, name: &'static str, run: fn(&mut Cpu, &mut Cps2Bus), outputs: u32) -> Native {
    Native { entry, name, run: crate::Body::Processeur(run), touches_sound: false, outputs }
}

const CREATED: u32 = regs::a(1) | regs::CCR;

pub const NATIVES: &[Native] = &[
    native(0x51c06, "cree_tir_arme", |c, b| { let me = Obj(c.a[0]); weapon_shot(c, b, me); rts(c, b) }, regs::d(0) | regs::d(1) | CREATED),
    native(0x4b7a0, "cree_eclair", |c, b| { let (me, v) = (Obj(c.a[0]), c.d[0] as u8); bolt_via(c, b, me, v); rts(c, b) }, CREATED),
    native(0x4dc9a, "cree_etincelles", |c, b| { let me = Obj(c.a[0]); sparks_via(c, b, me); rts(c, b) }, CREATED),
    native(0x9ef20, "cree_tir_9", enemy_shot_9, regs::d(0) | CREATED),
    native(0xcdedc, "cree_set_13", set_13, CREATED),
    native(0xc8312, "cree_set_scene", scene_set_native, regs::d(0) | CREATED),
    native(0xd981a, "fin_de_scene_variante", scene_exit, regs::d(0) | regs::a(1)),
    native(0xcc352, "cree_set_a", set_a_creator, CREATED),
    native(0xdc5fe, "cree_item_scene", scene_item, CREATED),
    native(0xd9778, "musique_variante", scene_music, regs::d(0) | regs::a(1)),
    native(0xd99f8, "scene_rien", nothing, regs::ALL),
    native(0xce00a, "cree_set_15", set_15, CREATED),
    native(0xcc0ca, "scene_repart", |c, b| { restart_scene(c, b); rts(c, b) }, regs::a(0) | regs::d(7)),
    native(0x4b8d0, "cree_effet_b", peff_b, CREATED),
    native(0x4b518, "cree_effet_8", peff_8, CREATED),
    native(0x51bb6, "cree_effet_2f", peff_2f, CREATED),
    native(0x4cfbc, "cree_effets_13", peff_13_x4, CREATED | regs::d(5)),
    native(0x4b340, "cree_effets_7", quake_native, CREATED),
    native(0x63ae0, "cree_ennemis_7", enemy_7_pair, CREATED | regs::d(0)),
    native(0x5f764, "cree_ennemi_5", enemy_5_at, CREATED),
    native(0x57c8c, "cree_ennemi_1", enemy_1_at, CREATED),
    native(0x97218, "boss_etat_0202", boss_state_0202, regs::CCR),
    native(0xc9760, "cree_set_5", set_5, CREATED),
    native(0xd6182, "colonne_vide", blank_column, regs::a(1) | regs::d(0) | regs::d(4)),
    native(0xceb46, "fiche_bonus", bonus_sheet, regs::a(1) | regs::a(4) | regs::d(0) | regs::d(1)),
    native(0xdfe26, "cree_sobj_14", sobj_14, CREATED),
    native(0xdfc78, "cree_sobj_12", sobj_12, CREATED),
    native(0xdf688, "cree_sobj_8", |c, b| { let me = Obj(c.a[0]); sobj_8_via(c, b, me); rts(c, b) }, CREATED),
    native(0xcc38e, "cree_set_a_joueur", set_a_player_native, CREATED | regs::a(4)),
];

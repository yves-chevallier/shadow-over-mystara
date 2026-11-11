//! AI helpers for enemies and bosses (continuation of `ai`): stance draws from tables, test of the
//! box in front of an opponent.
//!
//! These routines keep the contract of the originals: a result in D0 (1: yes, 0: no) and the
//! flags of a `moveq`, which the handlers test right after the call (`beq`).

use crate::world::World;

use crate::services::ai::RANK;
use crate::world::objects::{pools, Enemy, Obj};
use crate::actors::player::Player;
use crate::world::rng;
use crate::world::tables::{engine, Table};

mod draws;
mod grabs;
mod hitbox;
mod loot;
mod walk;
pub use draws::*;
pub use grabs::*;
pub use hitbox::*;
pub use loot::*;
pub use walk::*;

/// Byte offset `index` into a table, sign-extended as a `Dn.w` index does on the 68000.
fn offset(index: u16) -> u32 {
    index as i16 as i32 as u32
}

/// The engine table `name` (`data/engine.json`, or the enemy tables it reads).
fn table(name: &str) -> &'static Table {
    engine().get(name)
}

/// Replaces the low byte / low word of a register (the other bits remain).
pub(crate) fn set_b(r: &mut u32, v: u8) {
    *r = *r & !0xff | v as u32;
}
pub(crate) fn set_w(r: &mut u32, v: u16) {
    *r = *r & !0xffff | v as u32;
}

/// Screen origin in x, depending on the camera in use (`ram::CAMERA_ALT_ACTIVE`).
fn screen_origin(b: &mut World) -> u16 {
    if crate::world::ram::CAMERA_ALT_ACTIVE.is_set(b) { crate::world::ram::CAMERA2_X.get(b) } else { crate::world::ram::CAMERA1_X.get(b) }
}

/// First entry (16 bytes) of set `set` whose byte `+3` (the low byte of its flags) equals `key` under `mask`.
fn find_animation(b: &mut World, set: u32, key: u8, mask: u8) -> u32 {
    let mut at = set;
    while crate::world::objects::anim::entry_flags(b, at) as u8 & mask != key & mask {
        at = at.wrapping_add(16);
    }
    at
}

/// Copy of the animation mark (0x1046c).
fn copy_mark(b: &mut World, o: Enemy) {
    let v = o.strike_key(b);
    o.set_strike_kind(b, v);
}

/// 0x103e0: starts the animation of set `set` with the same key as the current variant (`+0x33`), keeping the duration
/// of the current entry.
pub fn animation_by_variant(b: &mut World, o: Obj, set: u32) {
    let timer = o.anim_timer(b);
    let key = Enemy(o).key_frame(b);
    let at = find_animation(b, set, key, 0x7f);
    crate::world::objects::anim::start_entry(b, o, at);
    o.set_anim_timer(b, timer);
    copy_mark(b, Enemy(o));
}

/// 0x10428: starts the animation of set `set` with the same key as the master's variant (`+0xb2`).
pub fn animation_of_master(b: &mut World, o: Obj, set: u32) {
    let master = Enemy(Obj::from_word(Enemy(o).anim_master(b)));
    let key = master.key_frame(b);
    let at = find_animation(b, set, key, 0x7f);
    crate::world::objects::anim::start_entry(b, o, at);
    copy_mark(b, Enemy(o));
}

/// 0x10450: starts the animation of set `set` whose mark (`+3`) is that of the master's current entry.
pub fn animation_of_mark(b: &mut World, o: Obj, set: u32) {
    let master = Enemy(Obj::from_word(Enemy(o).anim_master(b)));
    let key = master.strike_key(b);
    let at = find_animation(b, set, key, 0xff);
    crate::world::objects::anim::start_entry(b, o, at);
}

/// 0x10474: at least one player is alive.
pub fn players_alive(b: &mut World) -> bool {
    crate::world::ram::PLAYERS_ALIVE.get(b) != 0
}

/// 0x104a2: is `o` outside the strip `[screen + left, screen + right]` (limits excluded from the inside)? The screen
/// is the origin of the display camera (`A5+0x5a6`, or `A5+0x5ae` in two-camera mode).
pub fn outside_band(b: &mut World, o: Obj, left: u16, right: u16) -> bool {
    let origin = screen_origin(b);
    let x = o.u16(b, Obj::X) as i16;
    let (low, high) = (left.wrapping_add(origin) as i16, right.wrapping_add(origin) as i16);
    low >= x || high <= x
}

/// 0x105b6: passes the next HP tier (`+0xaa` < `+0xab`) if its threshold in block `block` of `ai/hp_thresholds`
/// (0x100 bytes per block: tier row of 128 bytes, column `+0xac`, words) reaches the HP (`+0x62`). True if a tier is
/// passed.
pub fn next_threshold(b: &mut World, o: Obj, block: u16) -> bool {
    let e = Enemy(o);
    let level = e.hp_tier(b) as i8;
    if level >= e.hp_tiers(b) as i8 {
        return false;
    }
    let column = e.hp_rank(b) as i8 as i16;
    let index = (level as i16).wrapping_shl(7).wrapping_add(column);
    let at = (block as u32 * 0x100).wrapping_add(offset(index as u16));
    let threshold = crate::world::tables::engine_table!("ai/hp_thresholds").at16(b, at) as i16;
    if threshold < e.hp(b) as i16 {
        return false;
    }
    e.set_hp_tier(b, level.wrapping_add(1) as u8);
    true
}

/// 0x103ba: red flash, 8-frame shake (`ram::SHAKE`) and impact spark (0xb141a, speeds `vx`, `ax`)
/// whose variant is reset to zero.
/// Original glitch: the original does not set the spark's speeds; it takes the D2 and D3 left by the
/// routines called before (the callers pass them in the clear).
pub fn flash_and_shake(b: &mut World, o: Obj, vx: u16, ax: u16) {
    crate::services::palette::set_flash_color(b, crate::services::palette::FLASH_COLOR_RED);
    crate::world::ram::SHAKE.set(b, 8);
    if let Some(spark) = crate::actors::families::effects_creators::impact_spark(b, o, vx, ax) {
        spark.set_u8(b, Obj::VARIANT, 0);
    }
}

/// 0x3e480: the landing routine specific to the type of `o` (`ai/landing_routines`, longs of the code: enemies, class
/// 2, types 0..31; bosses afterwards) and
/// the index read in the table, which the original leaves in D0 when jumping to the routine (A1). `None` for the
/// types without a routine (a plain `rts`: 0x3e2aa, 0x3bb9a).
pub fn landing_routine(b: &mut World, o: Obj) -> Option<(u32, u16)> {
    let base: u8 = if o.u8(b, Obj::CLASS) == 2 { 0 } else { 32 };
    let index = base.wrapping_add(o.u8(b, Obj::KIND)) as u16 * 4;
    let routine = crate::world::tables::engine_table!("ai/landing_routines").at32(b, offset(index));
    (routine != 0x3e2aa && routine != 0x3bb9a).then_some((routine, index))
}

//! AI helpers for enemies and bosses (continuation of `ai`): stance draws from tables, test of the
//! box in front of an opponent.
//!
//! These routines keep the contract of the originals: a result in D0 (1: yes, 0: no) and the
//! flags of a `moveq`, which the handlers test right after the call (`beq`).

use cps2::Cps2Bus;

use crate::mem::{rd16, rd32, rd8, wr16, wr32, wr8, A5};

use crate::objects::Obj;
use crate::rng;
use crate::objects::pools;

mod hitbox;
mod walk;
mod grabs;
mod draws;
mod loot;
pub use hitbox::*;
pub use walk::*;
pub use grabs::*;
pub use draws::*;
pub use loot::*;

/// Counter of the current frame, used as an index in the stance tables.
const TICK: u32 = A5 + 0x7f92;

/// Fields of the enemy (A0).
pub(crate) mod f {
    pub const X: u32 = 0x08;
    pub const DEPTH: u32 = 0x10;
    pub const FACING: u32 = 0x2c;
    /// Targeted object (16-bit pointer).
    pub const TARGET: u32 = 0x50;
    pub const STATE: u32 = 0x05;
    pub const SUB_STATE: u32 = 0x06;
    pub const SUB_STEP: u32 = 0x07;
    /// Table numbers: stance (0x70), timing (0x72, 0x73), attack box (0x74, 0x75).
    pub const STANCE_TABLE: u32 = 0x70;
    pub const STANCE_VALUE: u32 = 0x71;
    pub const BOX_CHANCE: u32 = 0x74;
    pub const BOX_ANIMATION: u32 = 0x75;
}
/// Word of the pointer table `table` designated by `index`, sign-extended as a `Dn.w` index
/// does on the 68000.
fn offset(index: u16) -> u32 {
    index as i16 as i32 as u32
}

/// Replaces the low byte / low word of a register (the other bits remain).
pub(crate) fn set_b(r: &mut u32, v: u8) {
    *r = *r & !0xff | v as u32;
}
pub(crate) fn set_w(r: &mut u32, v: u16) {
    *r = *r & !0xffff | v as u32;
}

/// Address of the object designated by a 16-bit pointer (sign-extended).
pub(crate) fn object(pointer: u16) -> u32 {
    pointer as i16 as i32 as u32
}

/// Screen origin in x (A5 word), depending on the camera in use (`A5+0x36`).
fn screen_origin(b: &mut Cps2Bus) -> u16 {
    let offset = if rd8(b, A5 + 0x36) == 0 { 0x5a6 } else { 0x5ae };
    rd16(b, A5 + offset)
}

/// Landing routines by type (longs of the code): enemies (class 2) at indices 0..31, bosses afterwards.
const LANDING_BY_KIND: u32 = 0x3e49c;

/// Enemy fields specific to these helpers.
mod g {
    /// Animation key of the variant (7 bits) and duration of the current entry.
    pub const VARIANT_KEY: u32 = 0x33;
    pub const ANIM_TIMER: u32 = 0x30;
    /// Mark of the animation entry (hit type), copied to `+0x44` after each step.
    pub const ANIM_MARK: u32 = 0x3e;
    pub const ANIM_MARK_COPY: u32 = 0x44;
    /// Master (16-bit pointer) whose variant or mark chooses the animation.
    pub const MASTER: u32 = 0xb2;
    /// Targeted point (x, depth), point set and number of the last point retained.
    pub const GOAL_X: u32 = 0x7a;
    pub const GOAL_DEPTH: u32 = 0x7c;
    pub const POINT_SET: u32 = 0x79;
    pub const LAST_POINT: u32 = 0x99;
    /// HP tiers: current tier, number of tiers, column (HP rank).
    pub const LEVEL: u32 = 0xaa;
    pub const LEVELS: u32 = 0xab;
    /// Behaviour parameter (and countdown of 0xff54).
    pub const BEHAVIOUR: u32 = 0x98;
    /// Rows of the draw tables: `+0x6a` → `+0x6b`, `+0x6c` (masks) and `+0x6d` → `+0x6e`, `+0x8e` →
    /// `+0x85` (`+0x84`: off screen).
    pub const ROW_6A: u32 = 0x6a;
    pub const DRAW_6B: u32 = 0x6b;
    pub const ROW_6C: u32 = 0x6c;
    pub const ROW_6D: u32 = 0x6d;
    pub const DRAW_6E: u32 = 0x6e;
    pub const OFFSCREEN_84: u32 = 0x84;
    pub const DRAW_85: u32 = 0x85;
    pub const ROW_8E: u32 = 0x8e;
    /// Parry level (0x10112).
    pub const PARRY_LEVEL: u32 = 0x68;
    /// Loot: table row (`+0x80`), mask and flag of the object dropped only once.
    pub const LOOT_ROW: u32 = 0x80;
    pub const ONCE_MASK: u32 = 0x9c;
    pub const ONCE_DONE: u32 = 0x9d;
}

/// The object designated by the word `pointer` (sign-extended, like `movea.w`).
fn linked(pointer: u16) -> Obj {
    Obj(object(pointer))
}

/// First entry (16 bytes) of set `set` whose byte `+3` equals `key` under `mask`.
fn find_animation(b: &mut Cps2Bus, set: u32, key: u8, mask: u8) -> u32 {
    let mut at = set;
    while rd8(b, at + 3) & mask != key & mask {
        at = at.wrapping_add(16);
    }
    at
}

/// Copy of the animation mark (0x1046c).
fn copy_mark(b: &mut Cps2Bus, o: Obj) {
    let v = o.u8(b, g::ANIM_MARK);
    o.set_u8(b, g::ANIM_MARK_COPY, v);
}

/// 0x103e0: starts the animation of set `set` with the same key as the current variant (`+0x33`), keeping the duration
/// of the current entry.
pub fn animation_by_variant(b: &mut Cps2Bus, o: Obj, set: u32) {
    let timer = o.u16(b, g::ANIM_TIMER);
    let key = o.u8(b, g::VARIANT_KEY);
    let at = find_animation(b, set, key, 0x7f);
    crate::objects::anim::start_entry(b, o, at);
    o.set_u16(b, g::ANIM_TIMER, timer);
    copy_mark(b, o);
}

/// 0x10428: starts the animation of set `set` with the same key as the master's variant (`+0xb2`).
pub fn animation_of_master(b: &mut Cps2Bus, o: Obj, set: u32) {
    let master = linked(o.u16(b, g::MASTER));
    let key = master.u8(b, g::VARIANT_KEY);
    let at = find_animation(b, set, key, 0x7f);
    crate::objects::anim::start_entry(b, o, at);
    copy_mark(b, o);
}

/// 0x10450: starts the animation of set `set` whose mark (`+3`) is that of the master's current entry.
pub fn animation_of_mark(b: &mut Cps2Bus, o: Obj, set: u32) {
    let master = linked(o.u16(b, g::MASTER));
    let key = master.u8(b, g::ANIM_MARK);
    let at = find_animation(b, set, key, 0xff);
    crate::objects::anim::start_entry(b, o, at);
}

/// 0x10474: at least one player is alive.
pub fn players_alive(b: &mut Cps2Bus) -> bool {
    crate::ram::PLAYERS_ALIVE.get(b) != 0
}

/// 0x104a2: is `o` outside the strip `[screen + left, screen + right]` (limits excluded from the inside)? The screen
/// is the origin of the display camera (`A5+0x5a6`, or `A5+0x5ae` in two-camera mode).
pub fn outside_band(b: &mut Cps2Bus, o: Obj, left: u16, right: u16) -> bool {
    let origin = screen_origin(b);
    let x = o.u16(b, Obj::X) as i16;
    let (low, high) = (left.wrapping_add(origin) as i16, right.wrapping_add(origin) as i16);
    low >= x || high <= x
}

/// 0x105b6: passes the next HP tier (`+0xaa` < `+0xab`) if its threshold in `table` (tier row,
/// column `+0xac`, words) reaches the HP (`+0x62`). True if a tier is passed.
pub fn next_threshold(b: &mut Cps2Bus, o: Obj, table: u32) -> bool {
    let level = o.u8(b, g::LEVEL) as i8;
    if level >= o.u8(b, g::LEVELS) as i8 {
        return false;
    }
    let column = o.u8(b, crate::ai::f::HP_RANK) as i8 as i16;
    let index = (level as i16).wrapping_shl(7).wrapping_add(column);
    let threshold = rd16(b, table.wrapping_add(offset(index as u16))) as i16;
    if threshold < o.u16(b, crate::ai::f::HP) as i16 {
        return false;
    }
    o.set_u8(b, g::LEVEL, level.wrapping_add(1) as u8);
    true
}

/// 0x103ba: red flash, 8-frame shake (`A5+0x4be`) and impact spark (0xb141a, speeds `vx`, `ax`)
/// whose variant is reset to zero.
/// Original glitch: the original does not set the spark's speeds; it takes the D2 and D3 left by the
/// routines called before (the callers pass them in the clear).
pub fn flash_and_shake(b: &mut Cps2Bus, o: Obj, vx: u16, ax: u16) {
    crate::palette::set_flash_color(b, crate::palette::FLASH_COLOR_RED);
    wr16(b, A5 + 0x4be, 8);
    if let Some(spark) = crate::families::effects_creators::impact_spark(b, o, vx, ax) {
        spark.set_u8(b, Obj::VARIANT, 0);
    }
}

/// 0x3e480: the landing routine specific to the type of `o` (enemies: class 2, types 0..31; bosses afterwards) and
/// the index read in the table, which the original leaves in D0 when jumping to the routine (A1). `None` for the
/// types without a routine (a plain `rts`: 0x3e2aa, 0x3bb9a).
pub fn landing_routine(b: &mut Cps2Bus, o: Obj) -> Option<(u32, u16)> {
    let base: u8 = if o.u8(b, Obj::CLASS) == 2 { 0 } else { 32 };
    let index = base.wrapping_add(o.u8(b, Obj::KIND)) as u16 * 4;
    let routine = crate::mem::prog32(b, LANDING_BY_KIND.wrapping_add(offset(index)));
    (routine != 0x3e2aa && routine != 0x3bb9a).then_some((routine, index))
}

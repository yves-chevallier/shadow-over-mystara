//! Boss of type 21 (0x9947a), stage 44. See `docs/families/boss.md`.
//!
//! Cousin of boss 17 (`boss_17`): same skeleton (`+4` state, `+5` group, `+6` and `+7` steps, states set in one go
//! as longs), same AI libraries, same slow motion (`+0xbb`); the ROM tables, the attacks and the death are its own.
//! It is linked to a host (`+0x46`): when the host dies (state 0x0406), the boss leaves the stage.
//!
//! | `+5` | File | Role |
//! |---|---|---|
//! | 0 | `intro` | arrival from the right (`+6` = 0), then entry into combat (2) |
//! | 2 | `fight` | combat: wait, about-face, walk toward a player (`+6` = 4, 6), charge (8, 10) |
//! | 4 | `attacks` | nine attacks (`+6` = 0 to 16) |
//! | 6, 8, 10 | here | wait while no player is alive |
//! | state 4 | `hit` | reactions to hits (common ROM table), stun, death |
//!
//! Animations: 0x1836ec (movements), 0x18371c (hits), 0x18373a (knockback). `+0xa0` is the current set
//! (0x30 and 0x31: sequences with no shadow or zone test) and `+0xa1` the current animation.

use crate::families::cx::Bench;
use super::boss_common::*;
use super::cx::handlers;
use crate::objects::Obj;

mod attacks;
mod decide;
mod fight;
pub(crate) mod hit;
mod intro;

/// Fields specific to this type.
mod g {
    /// Stun value (and copy into `+0x53`: freeze after a hit).
    pub const STUN: u32 = 0x56;
    /// Vulnerable (1) or not (0).
    pub const VULNERABLE: u32 = 0x67;
    /// Walk steps remaining (`+0x6b`), frames before the next step (`+0x71`), approach timer (`+0x6e`).
    pub const WALK_STEPS: u32 = 0x6b;
    pub const APPROACH: u32 = 0x6e;
    pub const WALK_DELAY: u32 = 0x71;
    /// Type of the shot launched by `0xe76c` (table index).
    pub const SHOT: u32 = 0x79;
    /// Step timer (word).
    pub const TIMER: u32 = 0xa6;
    /// Current animation set and current animation.
    pub const ANIM_KIND: u32 = 0xa0;
    pub const ANIM_LAST: u32 = 0xa1;
    /// Saved state (long) to resume after an interruption.
    pub const SAVED_STATE: u32 = 0xa2;
    pub const PHASE: u32 = 0xaa;
    pub const PHASES: u32 = 0xab;
    /// Non-zero byte: the boss is invulnerable (`+0x38` reset to zero every frame).
    pub const INVULNERABLE: u32 = 0xad;
    /// Number of dropped objects still in play.
    pub const SPAWNED: u32 = 0xaf;
    /// Linked object (word): shot.
    pub const CHILD: u32 = 0xb0;
    /// Attacks remaining of each kind (`0xb2`, `0xb3`, `0xb4`).
    pub const BUDGET_A: u32 = 0xb2;
    pub const BUDGET_B: u32 = 0xb3;
    pub const BUDGET_C: u32 = 0xb4;
    /// Slow motion (`0xbb`: 0 normal, 1 or 2 mode) and its counters.
    pub const SLOW_TICK: u32 = 0xb5;
    pub const STRIKES: u32 = 0xb6;
    pub const STRIKE_LIMIT: u32 = 0xb7;
    pub const SLOW_TIME: u32 = 0xb8;
    pub const SLOW_COUNT: u32 = 0xba;
    pub const SLOW: u32 = 0xbb;
    /// Saved position of the target.
    pub const MARK_X: u32 = 0xbc;
    pub const MARK_Y: u32 = 0xbe;
    /// Counter of hits taken during the stun.
    pub const HITS: u32 = 0x84;
}
use g::*;
use crate::objects::pools;
use crate::objects::Current;

/// Jeux d'animations.
const SET_BODY: u32 = 0x1836ec;
const SET_STRIKE: u32 = 0x18371c;
const SET_HURT: u32 = 0x18373a;

/// Starts animation `n` of the movements / hits set via the AI (`0x103d6`).
fn body<R: Bench>(x: &mut Cx<'_, R>, n: u32) {
    lib::start(x, SET_BODY, n);
}

fn strike<R: Bench>(x: &mut Cx<'_, R>, n: u32) {
    lib::start(x, SET_STRIKE, n);
}

pub fn run<R: Bench>(x: &mut Cx<'_, R>) {
    if suspended(x) {
        return;
    }
    loop {
        x.tick_counters();
        match x.u8(f::STATE) {
            st::INIT => init(x),
            st::ACTIVE => active(x),
            st::HIT => hit::run(x),
            _ => x.free(&pools::BOSS),
        }
        if x.u8(INVULNERABLE) != 0 {
            x.set8(0x38, 0);
        }
        if x.u8(0) == 0 {
            return;
        }
        // slow motion: as long as `+0xba` stays positive, the boss replays its logic within the same frame
        if x.u32(f::STATE) & 0xffff_ff00 != decide::WAIT && x.u8(SLOW) != 0 {
            if sub8(x, SLOW_COUNT, 1).pl() {
                continue;
            }
            if sub16(x, SLOW_TIME, 1).gt() {
                let tick = x.u8(SLOW);
                x.set8(SLOW_COUNT, tick);
                slow_tick(x);
            } else {
                x.set8(SLOW, 0);
                decide::pick_strike_limit(x);
            }
        }
        break;
    }
    // the dead host takes the boss down with it (state 0x0406 = death)
    if decide::host_dead(x) {
        x.set8(0, 2);
        x.set32(f::IMMUNITY, 0xffff_ffff);
    }
    x.reinsert();
}

// --- state 0: creation --------------------------------------------------------------------------

fn init<R: Bench>(x: &mut Cx<'_, R>) {
    x.add8(f::STATE, 2);
    pick_target(x);
    x.set8(PHASES, 4);
    x.set8(f::BOX_SET, 0x31);
    lib::initial_hp_boss(x);
    lib::spread_index(x);
    decide::pick_strike_limit(x);
    x.set8(0, 2);
    x.set_bit(0x1, 1, true);
    for off in [0x76, 0x70, 0x6a, 0x75] {
        x.set8(off, 0);
    }
    for (off, v) in [(0x72, 1), (0x73, 1), (0x6c, 6), (0x6d, 6), (0x74, 7), (0x78, 0xff), (0x8e, 3), (0x68, 0xb), (0x69, 1), (VULNERABLE, 0)] {
        x.set8(off, v);
    }
    x.set16(f::IMMUNITY, 0xffff);
    x.set8(ANIM_KIND, 0);
    x.set8(ANIM_LAST, 0xff);
    x.set8(BUDGET_A, 8);
    x.set8(BUDGET_B, 8);
    x.set8(BUDGET_C, 3);
    for (off, v) in [(0x7e, 0x4d), (0x7f, 0), (0x80, 0), (0x81, 0x18)] {
        x.set8(off, v);
    }
    decide::start_intro(x);
    body(x, 0);
}

// --- state 2: active -----------------------------------------------------------------------------

fn active<R: Bench>(x: &mut Cx<'_, R>) {
    let freeze = tst8(x, f::FREEZE);
    if freeze.gt() {
        sub8(x, f::FREEZE, 1);
    } else if var::PAUSE.get(x.b) == 0 {
        match x.u8(f::SUB) {
            0 => intro::run(x),
            2 => fight::run(x),
            4 => attacks::run(x),
            _ => wait_for_players(x),
        }
    }
    // the special sequences (set 0x30 and beyond) have neither play area nor shadow
    if !cmp8(x, ANIM_KIND, 0x30).ge() {
        lib::keep_in_zone(x);
        lib::collide_terrain(x);
    }
}

/// `+5` = 6, 8, 10 (0x9a36a): waits for players to come back.
fn wait_for_players<R: Bench>(x: &mut Cx<'_, R>) {
    if var::PLAYERS_ALIVE.get(x.b) as i8 > 0 {
        return decide::chase(x);
    }
    match x.u8(f::SUB2) {
        0 => {
            x.add8(f::SUB2, 2);
            x.set8(ANIM_LAST, 0xff);
            body(x, 0);
        }
        2 => {
            lib::step(x);
        }
        _ => {}
    }
}

// --- effects and slow motion ---------------------------------------------------------------------------

/// An effect (EFFCT) linked to the boss (`+0x42`): active byte, type, variant, orientation of the boss.
/// `retry`: insists as long as the pool is full.
fn effect<R: Bench>(x: &mut Cx<'_, R>, retry: bool, kind: u8, variant: Option<u8>, flip: bool) -> Option<Obj> {
    let e = if retry { alloc_retry(x, &pools::EFFCT)? } else { x.alloc(&pools::EFFCT)? };
    e.set_u8(x.b, 0, 1);
    e.set_u8(x.b, 2, kind);
    if let Some(v) = variant {
        e.set_u8(x.b, 3, v);
    }
    if flip {
        let side = x.u8(f::FLIP);
        e.set_u8(x.b, f::FLIP, side);
    }
    e.set_u16(x.b, 0x42, x.o.0 as u16);
    Some(e)
}

/// 0x9ae52: a dust cloud (effect 0x32, variant 4).
fn dust<R: Bench>(x: &mut Cx<'_, R>) {
    effect(x, false, 0x32, Some(4), true);
}

/// 0x9ae4c: same, except during the slow motion.
fn dust_unless_slow<R: Bench>(x: &mut Cx<'_, R>) {
    if x.u8(SLOW) == 0 {
        dust(x);
    }
}

/// 0x9ae2a: slow-motion step: the dust falls every 11 steps (mode 1) or 7 steps (mode 2).
fn slow_tick<R: Bench>(x: &mut Cx<'_, R>) {
    if x.u8(SLOW) == 0 {
        return;
    }
    x.add8(SLOW_TICK, 1);
    let period = if x.u8(SLOW) == 1 { 0xb } else { 7 };
    if (period as i8) > x.u8(SLOW_TICK) as i8 {
        return;
    }
    x.set8(SLOW_TICK, 0);
    dust(x);
}

handlers! {
    0x9947a "boss_21" run,
    // getting up after a common reaction (table 0x3e000 of `reactions.rs`)
    0x9a6d4 "boss_21_getup" hit::recovered,
}

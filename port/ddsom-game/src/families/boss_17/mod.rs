//! Boss of type 17 (0x952b4), stages 13 and 44 (h: "Tel'Arin" in the atlas's debug names). See
//! `docs/families/boss.md`.
//!
//! Fighter with three variants (`+3`): 0 is the boss of stage 13 (stage arrival, combat, death and ending
//! cutscene), 1 and 2 are side appearances (stage 44). Enemy skeleton (`+4` state, `+5` group, `+6` and `+7` steps);
//! active states (`+5`):
//!
//! | `+5` | File | Role |
//! |---|---|---|
//! | 0 | `intro` | cutscenes: arrival (`+6` = 0 and 2), death and end of stage (4), variants 1 and 2 (6, 8) |
//! | 2 | `fight` | combat: walk toward a player (`+6` = 4, 6), charge (8, 10), everything is decided by `decide` |
//! | 4 | `attacks` | nine attacks (`+6` = 0 to 16) |
//! | 6, 8, 10 | here | wait while no player is alive |
//! | state 4 | `hit` | reactions to hits, death |
//!
//! Animations: 0x18076e (movements), 0x1807a2 (hits), 0x1807c0 (knockback). `+0xa0` is the current set
//! (0x30 to 0x34: special sequences, with no shadow or zone test) and `+0xa1` the current animation.

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
pub(self) mod g {
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
    /// Step timer (word) and counter (word).
    pub const TIMER: u32 = 0xa6;
    pub const COUNT: u32 = 0xa8;
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
    /// Linked object (word): shot or message.
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
}
use g::*;
use crate::objects::pools;
use crate::objects::Current;

/// Jeux d'animations.
pub(self) const SET_BODY: u32 = 0x18076e;
pub(self) const SET_STRIKE: u32 = 0x1807a2;
pub(self) const SET_HURT: u32 = 0x1807c0;

/// Starts animation `n` of the movements / hits set via the AI (`0x103d6`).
pub(self) fn body<R: Bench>(x: &mut Cx<'_, R>, n: u32) {
    lib::start(x, SET_BODY, n);
}

pub(self) fn strike<R: Bench>(x: &mut Cx<'_, R>, n: u32) {
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
            let t = sub8(x, SLOW_COUNT, 1);
            if t.pl() {
                continue;
            }
            if sub16(x, SLOW_TIME, 1).gt() {
                let tick = x.u8(SLOW);
                x.set8(SLOW_COUNT, tick);
                attacks::slow_tick(x);
            } else {
                x.set8(SLOW, 0);
                decide::pick_strike_limit(x);
            }
        }
        break;
    }
    if cmp8(x, ANIM_KIND, 0x30).eq() {
        x.reinsert_foreground();
    } else {
        x.reinsert();
    }
}

// --- state 0: creation --------------------------------------------------------------------------

fn init<R: Bench>(x: &mut Cx<'_, R>) {
    x.add8(f::STATE, 2);
    x.set_a5_u8(0x50, 1);
    x.set_a5_u32(0x38, 0x0459_5900);
    pick_target(x);
    x.set8(PHASES, 4);
    x.set8(f::BOX_SET, 0x31);
    lib::initial_hp_boss(x);
    lib::spread_index(x);
    var::SCENE_LEVEL.set(x.b, 0);
    lib::clear_popup(x);
    lib::empty_popups(x);
    lib::clear_all_popups(x);
    decide::pick_strike_limit(x);
    x.set8(0, 2);
    x.set_bit(0x1, 1, true);
    if x.u8(Obj::VARIANT) != 0 {
        x.set8(0x93, 1);
        x.set8(0x9a, 0xff);
    }
    for off in [0x76, 0x70, 0x6a, 0x75] {
        x.set8(off, 0);
    }
    for (off, v) in [(0x72, 1), (0x73, 1), (0x6c, 6), (0x6d, 6), (0x74, 7), (0x78, 0xff), (0x8e, 3), (0x68, 0xb), (0x69, 1), (VULNERABLE, 0)] {
        x.set8(off, v);
    }
    x.set32(f::IMMUNITY, 0xffff_ffff);
    x.set8(ANIM_KIND, 0);
    x.set8(ANIM_LAST, 0xff);
    x.set8(BUDGET_A, 0xc);
    x.set8(BUDGET_B, 0xc);
    x.set8(BUDGET_C, 3);
    body(x, 0);
    match x.u8(Obj::VARIANT) {
        0 => decide::start_intro(x),
        1 => decide::start_variant1(x),
        _ => decide::start_variant2(x),
    }
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

/// `+5` = 6, 8, 10 (0x96958): waits for players to come back.
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

handlers! {
    0x952b4 "boss_17" #[without] run,
    // getting up after a common reaction (table 0x3e000 of `reactions.rs`)
    0x96ce0 "boss_17_getup" hit::recovered_tail,
}

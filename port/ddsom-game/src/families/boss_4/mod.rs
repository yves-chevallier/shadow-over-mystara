//! Boss of type 4 (0x81b3a): the Chimera (stages 104 and 105, and in `cov_2p_c`, `cov_tour4_c3`). See
//! `docs/families/boss.md`.
//!
//! Skeleton of the AI bosses (`+4` state, `+5` group, `+6` and `+7` steps, states set in one go as longs,
//! AI library 0x103d6..0x105b6). The body is accompanied by three graphic doubles (ENEMY 22, 23, 24 of
//! `enemy_double`), created with it and linked by `+0xb4`, `+0xb6`, `+0xb8`; the first (`+0xb4`) replays on request
//! (`+0xb6` of the double) the breath gestures and says through its `+0x33` when to breathe.
//!
//! | `+5` | File | Role |
//! |---|---|---|
//! | 0 | `fight` | arrival, descending from the top of the screen |
//! | 2 | `fight` | combat: wait, about-face, walk (`+6` = 4, 6), charge (8), leap (10) |
//! | 4 | `attacks` | seven attacks (`+6` = 0 to 12): cry, takeoff and dive, crossing, leap, breaths |
//! | 6 to 12 | `fight` | wait while no player is alive |
//! | 14 | `hit` | return to combat |
//! | state 4 | `hit` | reactions to hits (ROM table), knockback, fall, death |
//!
//! The attack choices (draws in the ROM tables according to phase `+0xaa`) and the states they set are
//! in `decide`. Animations: 0x17557e (body), 0x175596 (flight and attacks), 0x1755ae (hits received).

use crate::families::cx::Bench;
use super::boss_common::*;
use super::cx::handlers;
use crate::objects::Obj;

mod attacks;
mod decide;
mod fight;
pub(crate) mod hit;

/// Fields specific to this type.
pub(self) mod g {
    /// Walk steps remaining (`+0x6b`), countdown before the next step (`+0x71`), standing wait (`+0x6e`).
    pub const WALK_STEPS: u32 = 0x6b;
    pub const IDLE_COUNT: u32 = 0x6e;
    pub const WALK_DELAY: u32 = 0x71;
    pub const STUN: u32 = 0x56;
    pub const VULNERABLE: u32 = 0x67;
    pub const SHOT: u32 = 0x79;
    /// Shaking counter (reaction 0x4c).
    pub const SHAKE: u32 = 0x90;
    /// Current animation set and current animation.
    pub const ANIM_KIND: u32 = 0xa0;
    pub const ANIM_LAST: u32 = 0xa1;
    /// Saved state (long) to resume after an about-face.
    pub const SAVED_STATE: u32 = 0xa2;
    /// Step timer and dust rhythm (or oscillation rhythm in flight).
    pub const TIMER: u32 = 0xa6;
    pub const DUST: u32 = 0xa8;
    pub const PHASE: u32 = 0xaa;
    pub const PHASES: u32 = 0xab;
    /// Non-zero byte: the boss is invulnerable (`+0x38` reset to zero every frame).
    pub const INVULNERABLE: u32 = 0xad;
    /// Countdown between two jets of the breath, and rank of the jet in the angle table (`+0x42`).
    pub const BREATH: u32 = 0xaf;
    pub const BREATH_STEP: u32 = 0x42;
    /// Charge: 0 toward the middle of the screen, 1 toward the target's retained X position (`+0xbc`).
    pub const CHARGE_AT_TARGET: u32 = 0xb2;
    pub const MARK_X: u32 = 0xbc;
    /// Wing beat: alternated at each mark (0x11a36).
    pub const WING: u32 = 0xb3;
    /// The three doubles (words).
    pub const DOUBLE_A: u32 = 0xb4;
    pub const DOUBLE_B: u32 = 0xb6;
    pub const DOUBLE_C: u32 = 0xb8;
    /// Hits taken on the ground (word).
    pub const HITS: u32 = 0x84;
}
use g::*;
use crate::objects::pools;
use crate::objects::Current;

/// Animation sets: body, flight and attacks, hits received.
pub(self) const SET_BODY: u32 = 0x17557e;
pub(self) const SET_AIR: u32 = 0x175596;
pub(self) const SET_HURT: u32 = 0x1755ae;

/// Starts animation `n` of a set via the AI (0x103d6).
pub(self) fn body<R: Bench>(x: &mut Cx<'_, R>, n: u32) {
    lib::start(x, SET_BODY, n);
}

pub(self) fn air<R: Bench>(x: &mut Cx<'_, R>, n: u32) {
    lib::start(x, SET_AIR, n);
}

/// `moveq #k,D0 ; move.b D0,(0xa0) ; move.b D0,(0xa1)` : sorte d'animation `k`.
pub(self) fn kind<R: Bench>(x: &mut Cx<'_, R>, k: u8) {
    x.set8(ANIM_KIND, k);
    x.set8(ANIM_LAST, k);
}

/// Low-order word of a data register: only D2 to D5 are kept (leftovers re-read by the following handlers);
/// D0, D1 and D6 are re-read by no one.
pub(self) fn setw<R: Bench>(x: &mut Cx<'_, R>, n: usize, v: u16) {
    let r = match n {
        2 => x.c.d2(),
        3 => x.c.d3(),
        4 => x.c.d4(),
        5 => x.c.d5(),
        _ => return,
    };
    *r = *r & 0xffff_0000 | v as u32;
}

/// `subq.w #1,(off,A0)`: true when the countdown has elapsed (`ble`).
pub(self) fn expired<R: Bench>(x: &mut Cx<'_, R>, off: u32) -> bool {
    sub16(x, off, 1).le()
}

/// At least one player alive (0x10474).
pub(self) fn alive<R: Bench>(x: &mut Cx<'_, R>) -> bool {
    lib::players_alive(x)
}

/// The double `off` (`movea.w (off,A0),A1`).
pub(self) fn double<R: Bench>(x: &mut Cx<'_, R>, off: u32) -> Obj {
    x.linked(off)
}

/// Asks the first double to replay its gesture (`+0xb6` of the double).
pub(self) fn cue_double<R: Bench>(x: &mut Cx<'_, R>) {
    let d = double(x, DOUBLE_A);
    d.set_u8(x.b, 0xb6, 1);
}

/// Every 4 frames, a puff of dust (0x1172e, kind 3).
pub(self) fn dust_tick<R: Bench>(x: &mut Cx<'_, R>) {
    if !sub16(x, DUST, 1).gt() {
        x.set16(DUST, 4);
        lib::debris(x, 3);
    }
}

/// Marked key frame (bit 7 of `+0x33`): a wing beat, on alternating sides.
pub(self) fn flap<R: Bench>(x: &mut Cx<'_, R>) {
    if (x.u8(f::KEY_FRAME) as i8) < 0 {
        let k = x.u8(f::KEY_FRAME) & 0x7f;
        x.set8(f::KEY_FRAME, k);
        let w = x.u8(WING) ^ 1;
        x.set8(WING, w);
        lib::ground_debris(x, w);
    }
}

/// `moveq #0,D0 ; move.w (off,A0),D0 ; swap D0 ; asr.l #8,D0`: speed (word) as a long with 8 fractional bits.
pub(self) fn speed_long<R: Bench>(x: &mut Cx<'_, R>, off: u32) -> u32 {
    let v = ((x.u16(off) as i16 as i32) << 8) as u32;
    v
}

pub fn run<R: Bench>(x: &mut Cx<'_, R>) {
    if suspended(x) {
        return;
    }
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
    if x.u8(0) != 0 {
        x.reinsert();
    }
}

// --- state 0: creation --------------------------------------------------------------------------

/// 0x81b74: boss setup, creation of the three doubles; in game, it enters from the top of the screen.
fn init<R: Bench>(x: &mut Cx<'_, R>) {
    x.add8(f::STATE, 2);
    x.set_a5_u8(0x50, 1);
    x.set_a5_u32(0x38, 0x0459_5900);
    pick_target(x);
    x.set8(PHASES, 2);
    x.set8(f::BOX_SET, 0x24);
    lib::initial_hp_boss(x);
    var::SCENE_LEVEL.set(x.b, 0);
    lib::spread_index(x);
    lib::clear_popup(x);
    lib::empty_popups(x);
    lib::clear_all_popups(x);
    for (off, v) in [(0x76, 0), (0x70, 0), (0x6a, 2), (0x75, 0)] {
        x.set8(off, v);
    }
    x.set32(f::IMMUNITY, 0xffff_ffff);
    x.set8(0, 2);
    x.set_bit(0x1, 1, true);
    for (off, v) in [
        (0x93, 1),
        (0x9a, 0xff),
        (0x72, 1),
        (0x73, 1),
        (0x6c, 6),
        (0x6d, 1),
        (0x74, 7),
        (0x78, 0xff),
        (0x8e, 3),
        (0x68, 8),
        (0x69, 1),
        (VULNERABLE, 0),
        (ANIM_KIND, 0),
        (ANIM_LAST, 0xff),
        (0x7e, 0x4d),
        (0x7f, 0),
        (0x80, 0x25),
        (0x81, 0x1e),
    ] {
        x.set8(off, v);
    }
    // two doubles with their own HP (the boss's minus 0xfa or 0x12c, at least 0x96 or 0x64), a third without
    spawn_double(x, 0x16, Some((0xfa, 0x96, 0x43)), DOUBLE_A);
    spawn_double(x, 0x17, Some((0x12c, 0x64, 0x44)), DOUBLE_B);
    spawn_double(x, 0x18, None, DOUBLE_C);
    if var::GAME_ON.get(x.b) != 0 {
        body(x, 0);
        lib::sound(x, 0xe1330);
    } else {
        x.set8(0, 1);
        x.set_bit(0x1, 1, false);
        decide::wait(x);
        lib::set_immunities(x);
        body(x, 0);
    }
}

/// Creates an ENEMY double of type `kind` (the pool is awaited as long as it is full) and links it to the boss
/// (`link`). `life`: (deduction, minimum, box set) for a double that has its own HP.
fn spawn_double<R: Bench>(x: &mut Cx<'_, R>, kind: u8, life: Option<(u16, u16, u8)>, link: u32) {
    let Some(d) = alloc_retry(x, &pools::ENEMY) else { return };
    d.set_u8(x.b, 0, 1);
    d.set_u8(x.b, 2, kind);
    if let Some((cut, least, boxes)) = life {
        let hp = x.u16(0x60);
        let mut v = hp.wrapping_sub(cut);
        if (v as i16) < 0 {
            v = least;
        }
        for off in [0x60, 0x62, 0x64] {
            d.set_u16(x.b, off, v);
        }
        d.set_u8(x.b, f::BOX_SET, boxes);
    }
    d.set_u32(x.b, f::IMMUNITY, 0xffff_ffff);
    if life.is_some() {
        for (off, v) in [(ANIM_KIND, 0), (ANIM_LAST, 0xff), (0x7f, 0), (0x80, 0), (0x81, 0)] {
            d.set_u8(x.b, off, v);
        }
    }
    d.set_u16(x.b, 0xb2, x.o.0 as u16);
    x.set16(link, d.0 as u16);
}

// --- state 2: active -----------------------------------------------------------------------------

/// 0x81d6e: one group per frame (outside freeze and pause), then play area and shadow.
fn active<R: Bench>(x: &mut Cx<'_, R>) {
    if tst8(x, f::FREEZE).gt() {
        sub8(x, f::FREEZE, 1);
    } else if var::PAUSE.get(x.b) == 0 {
        match x.u8(f::SUB) {
            0 => fight::intro(x),
            2 => fight::run(x),
            4 => attacks::run(x),
            6..=12 => fight::wait_for_players(x),
            _ => hit::resume(x),
        }
    }
    lib::keep_in_zone(x);
    lib::collide_terrain(x);
}

handlers! {
    0x81b3a "boss_4" #[without] run,
    // return to combat after a common reaction (table 0x3e000 of `reactions.rs`)
    0x82ea0 "boss_4_releve" hit::resume,
}

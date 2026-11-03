//! Boss of type 0 (0x794ae): the boss of stage 20 (h), which splits in two (a second half, variant 1, is linked to
//! it by `+0xba`). See `docs/families/boss.md`.
//!
//! Same skeleton as the leaping beast (`boss_1`): `+4` state, `+5` group, `+6` and `+7` steps, states set in one go
//! as longs, AI library 0x103d6.. It runs and jumps toward its target, hits it when in range, and when it has only
//! half of its HP left it transforms: the second half (variant 1) enters the stage and the two fight together;
//! they only die together.
//!
//! | `+5` | File | Role |
//! |---|---|---|
//! | 0 | `moves` | stage entrances: arrival from an edge (`+6` = 0), transformation creating the second half (2), arrival of the second half (4) |
//! | 2 | `moves` | combat: wait (`+6` = 0), about-face (2), run (4, 6), charge (8) |
//! | 4 | `moves` | attacks: seven hits (`+6` = 0 to 12) |
//! | 6 to 10 | `moves` | wait while no player is alive |
//! | state 4 | `hit` | reactions to hits (ROM table), knockback, fall, death |
//!
//! The attack choices (draws in the ROM tables according to phase `+0xaa`) and the states they set are here
//! (`go`). Animations: 0x171a52 (body), 0x171a7e (stage entrances, attacks), 0x171a94 (hits received).

use crate::families::cx::Bench;
use super::boss_common::*;
use super::cx::handlers;

pub(crate) mod hit;
mod moves;

/// Fields specific to this type.
mod g {
    pub const VULNERABLE: u32 = 0x67;
    /// Number of run steps, delay between two steps, standing wait.
    pub const WALK_STEPS: u32 = 0x6b;
    pub const IDLE_COUNT: u32 = 0x6e;
    pub const WALK_DELAY: u32 = 0x71;
    /// Time on the ground (word) and shaking counter (reaction 0x4c).
    pub const DOWN_TIME: u32 = 0x84;
    pub const SHAKE: u32 = 0x90;
    /// X position saved during the shaking (word).
    pub const SAVED_X: u32 = 0xa;
    /// Kind of current animation and last one started.
    pub const ANIM_KIND: u32 = 0xa0;
    pub const ANIM_LAST: u32 = 0xa1;
    /// Saved state (long) to resume after an about-face.
    pub const SAVED_STATE: u32 = 0xa2;
    /// Step timers (words).
    pub const TIMER: u32 = 0xa6;
    pub const TIMER2: u32 = 0xa8;
    pub const PHASE: u32 = 0xaa;
    /// Non-zero byte: the boss is invulnerable (`+0x38` reset to zero every frame).
    pub const INVULNERABLE: u32 = 0xad;
    /// Hits remaining in the chain.
    pub const COMBO: u32 = 0xae;
    /// Role in the story: 1 = transformed main half, 2 = second half.
    pub const ROLE: u32 = 0xaf;
    /// Charges toward the saved target position (`+0xbc`) rather than toward the camera.
    pub const CHARGE_AIMED: u32 = 0xb2;
    /// The transformation has taken place.
    pub const TRANSFORMED: u32 = 0xb3;
    /// This half died first (the other dies right away).
    pub const DONE: u32 = 0xb4;
    /// The other half (word, RAM address; 0 = none).
    pub const PARTNER: u32 = 0xba;
    /// Saved target X position (word).
    pub const MARK_X: u32 = 0xbc;
}
use g::*;
use crate::objects::pools;
use crate::objects::Current;

/// Animation sets: body, stage entrances and attacks, hits received.
const SET_BODY: u32 = 0x171a52;
const SET_ENTRY: u32 = 0x171a7e;
const SET_HURT: u32 = 0x171a94;
/// Walk sets by direction (0x103e0).
const SET_RUN_FORWARD: u32 = 0x171b76;
const SET_RUN_BACK: u32 = 0x171c1a;

/// HP thresholds of the phases (0x105b6).
const THRESHOLDS: u32 = 0x10ae2;

/// Starts animation `n` of the body set via the AI (0x103d6).
fn body<R: Bench>(x: &mut Cx<'_, R>, n: u32) {
    lib::start(x, SET_BODY, n);
}

/// Starts animation `n` of the stage entrances set.
fn entry<R: Bench>(x: &mut Cx<'_, R>, n: u32) {
    lib::start(x, SET_ENTRY, n);
}

/// Next step (`addq.b #2,(+6)` or `(+7)`).
fn next6<R: Bench>(x: &mut Cx<'_, R>) {
    x.add8(f::SUB2, 2);
}
fn next7<R: Bench>(x: &mut Cx<'_, R>) {
    x.add8(0x7, 2);
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

/// 0x794e8: boss setup; the main half (variant 0) waits invulnerable in its pose, the second half
/// (variant 1) waits for its turn. Nothing is set if the stage is already started (`A5+0x52`).
fn init<R: Bench>(x: &mut Cx<'_, R>) {
    x.set_bit(0x1, 1, true);
    if x.a5_u16(0x52) == 0 {
        x.add8(f::STATE, 2);
        x.set_a5_u8(0x50, 1);
        pick_target(x);
        x.set8(0xab, 2);
        x.set8(f::BOX_SET, 0x20);
        lib::initial_hp_boss(x);
        var::SCENE_LEVEL.set(x.b, 0);
        lib::spread_index(x);
        lib::clear_popup(x);
        lib::empty_popups(x);
        lib::clear_all_popups(x);
        x.set32(f::IMMUNITY, 0xffff_ffff);
        for (off, v) in [
            (0x76, 0),
            (0x70, 0),
            (0x6a, 2),
            (0x75, 0),
            (0x93, 0),
            (0x72, 1),
            (0x73, 1),
            (0x6c, 6),
            (0x6d, 1),
            (0x74, 7),
            (0x78, 0xff),
            (0x8e, 3),
            (0x68, 0xb),
            (0x69, 1),
            (VULNERABLE, 0),
            (ANIM_KIND, 0),
            (ANIM_LAST, 0xff),
            (0x7f, 0),
        ] {
            x.set8(off, v);
        }
        if x.u8(3) == 0 {
            for (off, v) in [(0x80, 0x1e), (0x81, 0x19), (0x93, 1), (0x9a, 0xff), (0x7e, 0x23)] {
                x.set8(off, v);
            }
            wait_for_transformation(x);
        } else {
            for (off, v) in [(0x7e, 0x24), (0x80, 0x1f), (0x81, 0x1a), (0x2a, 1), (0x2b, 0x1c)] {
                x.set8(off, v);
            }
            partner_waits(x);
        }
    }
    body(x, 4);
}

/// 0x7aad0: the main half waits, invulnerable, for the stage to start it (stage entrance `+6` = 0).
fn wait_for_transformation<R: Bench>(x: &mut Cx<'_, R>) {
    x.set_bit(0x1, 1, false);
    x.set8(INVULNERABLE, 1);
    x.set8(ANIM_KIND, 0x30);
    x.set32(f::IMMUNITY, 0xffff_ffff);
    x.set32(f::STATE, 0x0200_0000);
}

/// 0x7ab4c: the second half waits for its creation to complete (stage entrance `+6` = 4).
fn partner_waits<R: Bench>(x: &mut Cx<'_, R>) {
    x.set_bit(0x1, 1, false);
    x.set8(ANIM_KIND, 0x32);
    x.set32(f::IMMUNITY, 0xffff_ffff);
    x.set8(0, 2);
    x.set8(ROLE, 2);
    x.set32(f::STATE, 0x0200_0400);
}

// --- state 2: active -----------------------------------------------------------------------------

/// 0x79600: one group per frame (outside freeze and pause), then play area and shadow.
fn active<R: Bench>(x: &mut Cx<'_, R>) {
    if tst8(x, f::FREEZE).gt() {
        sub8(x, f::FREEZE, 1);
    } else if var::PAUSE.get(x.b) == 0 {
        match x.u8(f::SUB) {
            0 => moves::intro(x),
            2 => moves::fight(x),
            4 => moves::attacks(x),
            6 | 8 | 10 => moves::wait_for_players(x),
            _ => {}
        }
    }
    lib::keep_in_zone(x);
    lib::collide_terrain(x);
}

// --- decisions: states set in one go (0x7ab74..0x7ac70) -----------------------------------------

/// Sets the state long `+4..+7` and makes the boss vulnerable or not (`+0x67`).
fn state<R: Bench>(x: &mut Cx<'_, R>, vulnerable: bool, s: u32) {
    x.set8(VULNERABLE, vulnerable as u8);
    x.set32(f::STATE, s);
}

/// 0x7ab74 : debout.
fn stand<R: Bench>(x: &mut Cx<'_, R>) {
    state(x, true, 0x0202_0000);
}

/// 0x7ab84: about-face, the current state will be resumed.
fn turn<R: Bench>(x: &mut Cx<'_, R>) {
    x.set8(VULNERABLE, 0);
    let s = x.u32(f::STATE);
    x.set32(SAVED_STATE, s);
    x.set32(f::STATE, 0x0202_0200);
}

/// 0x7ab98: start of the run (run animation depending on the role: 0x34 or, second half, 0x36).
fn run_start<R: Bench>(x: &mut Cx<'_, R>) {
    let kind = if x.u8(3) == 0 { 0x34 } else { 0x36 };
    x.set8(0x79, kind);
    state(x, true, 0x0202_0400);
}

/// 0x7abba: end of the run (0x35 or 0x37).
fn run_end<R: Bench>(x: &mut Cx<'_, R>) {
    let kind = if x.u8(3) == 0 { 0x35 } else { 0x37 };
    x.set8(0x79, kind);
    state(x, true, 0x0202_0600);
}

/// 0x7abdc and 0x7abee: charge, toward the camera (`aimed` false) or toward the saved target.
fn charge<R: Bench>(x: &mut Cx<'_, R>, aimed: bool) {
    x.set8(CHARGE_AIMED, aimed as u8);
    state(x, false, 0x0202_0800);
}

/// 0x7ac02, 0x7ac10, 0x7ac1e, 0x7ac32, 0x7ac40, 0x7ac4e, 0x7ac62: attack `n` (`+6` = 0 to 12).
fn strike<R: Bench>(x: &mut Cx<'_, R>, n: u32, shielded: bool) {
    if shielded {
        x.set8(INVULNERABLE, 1);
    }
    state(x, false, 0x0204_0000 | n << 8);
}

/// 0x7ac70: no player left, wait.
fn idle_alone<R: Bench>(x: &mut Cx<'_, R>) {
    x.set32(f::STATE, 0x020a_0000);
}

/// 0x7aaf4: the transformation (HP at half): the main half starts the cutscene.
fn transform<R: Bench>(x: &mut Cx<'_, R>) {
    x.set8(ANIM_KIND, 0x31);
    x.set32(f::IMMUNITY, 0xffff_ffff);
    x.set8(0x93, 0);
    x.set_a5_u16(0x51e, 0x30);
    let (cx, cy) = (var::SCROLL_X.get(x.b), var::SCROLL_Y.get(x.b));
    x.set_a5_u16(0x524, cx);
    x.set_a5_u16(0x528, cy);
    x.set8(0, 2);
    x.set8(TRANSFORMED, 1);
    x.set8(ROLE, 1);
    x.set8(INVULNERABLE, 0);
    x.set16(0xc, 0);
    if lib::outside_band(x, 0x30, 0x150) {
        return charge(x, false);
    }
    x.set32(f::STATE, 0x0200_0200);
}

/// Runs the decision whose original address is `at` (jumps through ROM tables).
fn go<R: Bench>(x: &mut Cx<'_, R>, at: u32) {
    match at {
        0x7ac02 => strike(x, 0, false),
        0x7ac10 => strike(x, 2, false),
        0x7ac1e => strike(x, 4, false),
        0x7ac32 => strike(x, 6, false),
        0x7ac2c => strike(x, 6, true),
        0x7ac40 => strike(x, 8, false),
        0x7ac4e => strike(x, 10, true),
        0x7ac62 => strike(x, 12, false),
        0x7abee => charge(x, true),
        0x7abdc => charge(x, false),
        _ => panic!("boss 0: unknown decision {at:#x}"),
    }
}

/// 0x7a8d2: target at short range: draw in table 0x7a8e8.
fn from_close<R: Bench>(x: &mut Cx<'_, R>) {
    let r = rand_and(x, 0xf);
    let at = decide(x, "boss_0/close_picks", r, "boss_0/close_jumps");
    go(x, at)
}

/// 0x7a8fa: target ahead (run box); the three routines of phase `+0xaa` read the same table 0x7a932.
fn from_front<R: Bench>(x: &mut Cx<'_, R>) {
    let r = rand_and(x, 0xf);
    let at = decide(x, "boss_0/front_picks", r, "boss_0/front_jumps");
    go(x, at)
}

/// 0x7a944: the AI chose gesture `g` (1 or 2) for the near box; draw in tables 0x7a9ae (main), 0x7a9be (second
/// half, gesture 1) or 0x7a9ce.
fn from_near_gesture<R: Bench>(x: &mut Cx<'_, R>, g: u32) {
    let r = rand_and(x, 0xf);
    let picks = if x.u8(3) == 0 {
        "boss_0/near_picks_0"
    } else if g == 1 {
        "boss_0/near_picks_1"
    } else {
        "boss_0/near_picks_2"
    };
    let at = decide(x, picks, r, "boss_0/near_jumps");
    go(x, at)
}

/// 0x7a9e4: the AI chose gesture `g` (1 or 2) for the far box: table selection according to the phase.
fn from_far_gesture<R: Bench>(x: &mut Cx<'_, R>, g: u32) {
    let r = rand_and(x, 0xf);
    let phase = x.u8(PHASE) as i8 as i32 as u32;
    let index = g.wrapping_sub(1).wrapping_mul(2).wrapping_add(phase.wrapping_mul(4));
    // the jump tables 0x7aa16 (main half) and 0x7aa22 (second half), six entries each (by gesture and phase), lead to
    // routines that each draw in one of four pick tables
    let picks = if x.u8(3) == 0 {
        [0, 0, 0, 0, 1, 1]
    } else {
        [2, 2, 2, 2, 3, 3]
    };
    let k = *picks.get(index as usize / 2).unwrap_or_else(|| bad_index(index));
    let at = decide(x, ["boss_0/far_picks_0", "boss_0/far_picks_1", "boss_0/far_picks_2", "boss_0/far_picks_3"][k], r,
        "boss_0/far_jumps");
    go(x, at)
}

/// 0x7a7a4 and 0x7a7b0: delay between two run steps, depending on difficulty.
fn walk_delay<R: Bench>(x: &mut Cx<'_, R>, kind: u8) {
    let d = per_rank(x, if kind == 3 { "boss_0/walk_delays_back" } else { "boss_0/walk_delays" });
    x.set8(WALK_DELAY, d);
}

/// 0x7a7fc and 0x7a80e: number of run steps, drawn from the direction table.
fn draw_steps<R: Bench>(x: &mut Cx<'_, R>, kind: u8) {
    let r = rand_and(x, 0x1f);
    let n = byte(x, if kind == 3 { "boss_0/walk_steps_back" } else { "boss_0/walk_steps" }, r);
    x.set8(WALK_STEPS, n);
}

/// 0x7a860: target ahead (narrow box) and gesture chosen by the AI.
fn close_gesture<R: Bench>(x: &mut Cx<'_, R>) -> Option<u32> {
    ai_box(x, -0x50, 0x30, 5)
}

/// 0x7a87e: if the boss is on screen, target in a deep box (narrower for the second half) and gesture chosen by
/// the AI.
fn far_gesture<R: Bench>(x: &mut Cx<'_, R>) -> Option<u32> {
    let cam = camera_x(x);
    let px = x.u16(0x8);
    if Fl::sub16(cam.wrapping_sub(0x10), px).ge() || Fl::sub16(cam.wrapping_add(0x190), px).le() {
        return None;
    }
    let (d0, d1, d2) = if x.u8(3) == 0 { (-0xe0, 0x60, 0xa) } else { (-0xe8, 0x48, 0xc) };
    ai_box(x, d0, d1, d2)
}

handlers! {
    0x794ae "boss_0" run,
    // return to combat after a common reaction (table 0x3e000 of `reactions.rs`)
    0x7a5ac "boss_0_getup" hit::get_up,
}

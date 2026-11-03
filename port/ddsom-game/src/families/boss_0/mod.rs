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
use crate::data::RomAddr;
use crate::objects::Obj;
use crate::record::record;
use super::boss_common::*;
use super::cx::handlers;

pub(crate) mod hit;
mod moves;

/// Fields of this boss beyond those of `Enemy` and `Boss` (`record!`). A meaning guessed from the code alone is
/// marked (h).
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub struct Boss0(pub Obj);

impl Boss0 {
    pub fn at(self, off: u32) -> u32 {
        self.0.at(off)
    }
}

record! {
    impl Boss0 {
        /// Hits remaining in the chain.
        combo, set_combo, COMBO: u8 @ 0xae;
        /// Role in the story: 1 = transformed main half, 2 = second half.
        role, set_role, ROLE: u8 @ 0xaf;
        /// Charges toward the saved target position (`+0xbc`) rather than toward the camera.
        charge_aimed, set_charge_aimed, CHARGE_AIMED: u8 @ 0xb2;
        /// The transformation has taken place.
        transformed, set_transformed, TRANSFORMED: u8 @ 0xb3;
        /// This half died first (the other dies right away).
        done, set_done, DONE: u8 @ 0xb4;
        /// The other half (word, RAM address; 0 = none).
        partner, set_partner, PARTNER: u16 @ 0xba;
    }
}
use crate::objects::pools;
use crate::objects::Current;

/// Animation sets: body, stage entrances and attacks, hits received.
const SET_BODY: RomAddr = RomAddr(0x171a52);
const SET_ENTRY: RomAddr = RomAddr(0x171a7e);
const SET_HURT: RomAddr = RomAddr(0x171a94);
/// Walk sets by direction (0x103e0).
const SET_RUN_FORWARD: RomAddr = RomAddr(0x171b76);
const SET_RUN_BACK: RomAddr = RomAddr(0x171c1a);

/// HP thresholds of the phases (0x105b6).
const THRESHOLDS: RomAddr = RomAddr(0x10ae2);

/// Starts animation `n` of the body set via the AI (0x103d6).
fn body<R: Bench>(x: &mut Cx<'_, R>, n: u32) {
    lib::start(x, SET_BODY.0, n);
}

/// Starts animation `n` of the stage entrances set.
fn entry<R: Bench>(x: &mut Cx<'_, R>, n: u32) {
    lib::start(x, SET_ENTRY.0, n);
}

/// Next step (`addq.b #2,(+6)` or `(+7)`).
fn next6<R: Bench>(x: &mut Cx<'_, R>) {
    x.add8(f::SUB2, 2);
}
fn next7<R: Bench>(x: &mut Cx<'_, R>) {
    x.add8(Enemy::SUB_STEP, 2);
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
    if x.u8(Boss::INVULNERABLE) != 0 {
        x.set8(Enemy::FRAME_TAG, 0);
    }
    if x.u8(Enemy::ACTIVE) != 0 {
        x.reinsert();
    }
}

// --- state 0: creation --------------------------------------------------------------------------

/// 0x794e8: boss setup; the main half (variant 0) waits invulnerable in its pose, the second half
/// (variant 1) waits for its turn. Nothing is set if the stage is already started (`A5+0x52`).
fn init<R: Bench>(x: &mut Cx<'_, R>) {
    x.set_bit(Enemy::DISPLAY, 1, true);
    if var::SCRIPT_BUSY.get(x.b) == 0 {
        x.add8(f::STATE, 2);
        var::BOSS_FIGHT.set(x.b, 1);
        pick_target(x);
        x.set8(Boss::PHASES, 2);
        x.set8(f::BOX_SET, 0x20);
        lib::initial_hp_boss(x);
        var::SCENE_LEVEL.set(x.b, 0);
        lib::spread_index(x);
        lib::clear_popup(x);
        lib::empty_popups(x);
        lib::clear_all_popups(x);
        x.set32(f::IMMUNITY, 0xffff_ffff);
        for (off, v) in [
            (Enemy::AI_BYTE_ROW, 0),
            (Enemy::AI_STANCE_ROW, 0),
            (Enemy::AI_WANDER_ROW, 2),
            (Enemy::AI_BOX_ANIM, 0),
            (Boss::DEATH_CREDIT, 0),
            (Enemy::AI_DECISION_A, 1),
            (Enemy::AI_DECISION_B, 1),
            (Enemy::AI_WAIT_MASKS, 6),
            (Enemy::AI_WAIT_ROW, 1),
            (Enemy::AI_BOX_CHANCE, 7),
            (Enemy::AI_SHOT, 0xff),
            (Enemy::AI_ROW_8E, 3),
            (Enemy::AI_68, 0xb),
            (Enemy::AI_69, 1),
            (Enemy::VULNERABLE, 0),
            (Boss::ANIM_KIND, 0),
            (Boss::ANIM_LAST, 0xff),
            (Enemy::EVENT_LOW, 0),
        ] {
            x.set8(off, v);
        }
        if x.u8(Enemy::VARIANT) == 0 {
            for (off, v) in [(Enemy::LOOT_ROW, 0x1e), (Enemy::LOOT, 0x19), (Boss::DEATH_CREDIT, 1), (Boss::DEATH_VALUE, 0xff), (Enemy::EVENT_KIND, 0x23)] {
                x.set8(off, v);
            }
            wait_for_transformation(x);
        } else {
            for (off, v) in [(Enemy::EVENT_KIND, 0x24), (Enemy::LOOT_ROW, 0x1f), (Enemy::LOOT, 0x1a), (Enemy::OWN_PALETTE, 1), (Enemy::PALETTE, 0x1c)] {
                x.set8(off, v);
            }
            partner_waits(x);
        }
    }
    body(x, 4);
}

/// 0x7aad0: the main half waits, invulnerable, for the stage to start it (stage entrance `+6` = 0).
fn wait_for_transformation<R: Bench>(x: &mut Cx<'_, R>) {
    x.set_bit(Enemy::DISPLAY, 1, false);
    x.set8(Boss::INVULNERABLE, 1);
    x.set8(Boss::ANIM_KIND, 0x30);
    x.set32(f::IMMUNITY, 0xffff_ffff);
    x.set32(f::STATE, 0x0200_0000);
}

/// 0x7ab4c: the second half waits for its creation to complete (stage entrance `+6` = 4).
fn partner_waits<R: Bench>(x: &mut Cx<'_, R>) {
    x.set_bit(Enemy::DISPLAY, 1, false);
    x.set8(Boss::ANIM_KIND, 0x32);
    x.set32(f::IMMUNITY, 0xffff_ffff);
    x.set8(Enemy::ACTIVE, 2);
    x.set8(Boss0::ROLE, 2);
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
    x.set8(Enemy::VULNERABLE, vulnerable as u8);
    x.set32(f::STATE, s);
}

/// 0x7ab74 : debout.
fn stand<R: Bench>(x: &mut Cx<'_, R>) {
    state(x, true, 0x0202_0000);
}

/// 0x7ab84: about-face, the current state will be resumed.
fn turn<R: Bench>(x: &mut Cx<'_, R>) {
    x.set8(Enemy::VULNERABLE, 0);
    let s = x.u32(f::STATE);
    x.set32(Boss::SAVED_STATE, s);
    x.set32(f::STATE, 0x0202_0200);
}

/// 0x7ab98: start of the run (run animation depending on the role: 0x34 or, second half, 0x36).
fn run_start<R: Bench>(x: &mut Cx<'_, R>) {
    let kind = if x.u8(Enemy::VARIANT) == 0 { 0x34 } else { 0x36 };
    x.set8(Boss::SHOT, kind);
    state(x, true, 0x0202_0400);
}

/// 0x7abba: end of the run (0x35 or 0x37).
fn run_end<R: Bench>(x: &mut Cx<'_, R>) {
    let kind = if x.u8(Enemy::VARIANT) == 0 { 0x35 } else { 0x37 };
    x.set8(Boss::SHOT, kind);
    state(x, true, 0x0202_0600);
}

/// 0x7abdc and 0x7abee: charge, toward the camera (`aimed` false) or toward the saved target.
fn charge<R: Bench>(x: &mut Cx<'_, R>, aimed: bool) {
    x.set8(Boss0::CHARGE_AIMED, aimed as u8);
    state(x, false, 0x0202_0800);
}

/// 0x7ac02, 0x7ac10, 0x7ac1e, 0x7ac32, 0x7ac40, 0x7ac4e, 0x7ac62: attack `n` (`+6` = 0 to 12).
fn strike<R: Bench>(x: &mut Cx<'_, R>, n: u32, shielded: bool) {
    if shielded {
        x.set8(Boss::INVULNERABLE, 1);
    }
    state(x, false, 0x0204_0000 | n << 8);
}

/// 0x7ac70: no player left, wait.
fn idle_alone<R: Bench>(x: &mut Cx<'_, R>) {
    x.set32(f::STATE, 0x020a_0000);
}

/// 0x7aaf4: the transformation (HP at half): the main half starts the cutscene.
fn transform<R: Bench>(x: &mut Cx<'_, R>) {
    x.set8(Boss::ANIM_KIND, 0x31);
    x.set32(f::IMMUNITY, 0xffff_ffff);
    x.set8(Boss::DEATH_CREDIT, 0);
    var::CAMERA_MODE.set(x.b, 0x30);
    let (cx, cy) = (var::SCROLL_X.get(x.b), var::SCROLL_Y.get(x.b));
    var::PREV_CAMERA_X.set(x.b, cx);
    var::PREV_CAMERA_Y.set(x.b, cy);
    x.set8(Enemy::ACTIVE, 2);
    x.set8(Boss0::TRANSFORMED, 1);
    x.set8(Boss0::ROLE, 1);
    x.set8(Boss::INVULNERABLE, 0);
    x.set16(Obj::Z, 0);
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
    let picks = if x.u8(Enemy::VARIANT) == 0 {
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
    let phase = x.u8(Boss::PHASE) as i8 as i32 as u32;
    let index = g.wrapping_sub(1).wrapping_mul(2).wrapping_add(phase.wrapping_mul(4));
    // the jump tables 0x7aa16 (main half) and 0x7aa22 (second half), six entries each (by gesture and phase), lead to
    // routines that each draw in one of four pick tables
    let picks = if x.u8(Enemy::VARIANT) == 0 {
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
    x.set8(Boss::WALK_DELAY, d);
}

/// 0x7a7fc and 0x7a80e: number of run steps, drawn from the direction table.
fn draw_steps<R: Bench>(x: &mut Cx<'_, R>, kind: u8) {
    let r = rand_and(x, 0x1f);
    let n = byte(x, if kind == 3 { "boss_0/walk_steps_back" } else { "boss_0/walk_steps" }, r);
    x.set8(Boss::WALK_STEPS, n);
}

/// 0x7a860: target ahead (narrow box) and gesture chosen by the AI.
fn close_gesture<R: Bench>(x: &mut Cx<'_, R>) -> Option<u32> {
    ai_box(x, -0x50, 0x30, 5)
}

/// 0x7a87e: if the boss is on screen, target in a deep box (narrower for the second half) and gesture chosen by
/// the AI.
fn far_gesture<R: Bench>(x: &mut Cx<'_, R>) -> Option<u32> {
    let cam = camera_x(x);
    let px = x.u16(Obj::X);
    if Fl::sub16(cam.wrapping_sub(0x10), px).ge() || Fl::sub16(cam.wrapping_add(0x190), px).le() {
        return None;
    }
    let (d0, d1, d2) = if x.u8(Enemy::VARIANT) == 0 { (-0xe0, 0x60, 0xa) } else { (-0xe8, 0x48, 0xc) };
    ai_box(x, d0, d1, d2)
}

handlers! {
    0x794ae "boss_0" run,
    // return to combat after a common reaction (table 0x3e000 of `reactions.rs`)
    0x7a5ac "boss_0_getup" hit::get_up,
}

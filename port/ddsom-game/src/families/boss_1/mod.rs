//! Boss of type 1 (0x7acea): the leaping beast of stage 27 (debug name "Displacer Beast", h). See
//! `docs/families/boss.md`.
//!
//! Same skeleton as the Chimera (`boss_4`): `+4` state, `+5` group, `+6` and `+7` steps, states set in one go as
//! longs, AI library 0x103d6..0x105b6. The boss moves by leaps (a walk is a series of jumps toward a point drawn
//! around its target) and grabs the players.
//!
//! | `+5` | File | Role |
//! |---|---|---|
//! | 0 | `moves` | arrival: crossing the screen by jumping, then falling from the top of the screen |
//! | 2 | `moves` | combat: wait, about-face, walk by leaps (`+6` = 4, 6), big leap (8), backward leap (10) |
//! | 4 | `moves` | attacks: four hits (`+6` = 0 to 6), grab (8), leap with a double (10) |
//! | 6 to 10 | `moves` | wait while no player is alive |
//! | 12 | `moves` | a grabbed player: bites that take HP from them |
//! | 14 | `hit` | return to combat |
//! | state 4 | `hit` | reactions to hits (ROM table), knockback, fall, death |
//!
//! The attack choices (draws in the ROM tables according to phase `+0xaa`) and the states they set are
//! here (`go`). Animations: 0x172794 (body), 0x1727d4 (attacks), 0x1727e4 (hits received).

use crate::families::cx::Bench;
use crate::data::RomAddr;
use crate::record::record;
use super::boss_common::*;
use super::cx::handlers;
use crate::objects::Obj;

pub(crate) mod hit;
mod moves;

/// Fields of this boss beyond those of `Enemy` and `Boss` (`record!`). A meaning guessed from the code alone is
/// marked (h).
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub struct Boss1(pub Obj);

impl Boss1 {
    pub fn at(self, off: u32) -> u32 {
        self.0.at(off)
    }
}

record! {
    impl Boss1 {
        /// Hits remaining in the attack chain.
        combo, set_combo, COMBO: u8 @ 0xae;
        /// Counter of the arrival's puffs.
        puffs, set_puffs, PUFFS: u8 @ 0xaf;
        /// Double (ENEMY 0x1a) created for the scissor leap (word, 0 = none).
        double, set_double, DOUBLE: u16 @ 0xb0;
        /// Grab attempts (index into the chance table).
        grab_tries, set_grab_tries, GRAB_TRIES: u8 @ 0xb2;
        /// Big leap: 0 toward the target, 1 toward the middle of the screen.
        leap_to_center, set_leap_to_center, LEAP_TO_CENTER: u8 @ 0xb3;
        /// First animation of the walking leap (7 or 15 depending on the direction).
        hop_anim, set_hop_anim, HOP_ANIM: u8 @ 0xb4;
        /// State of the previous frame (long).
        prev_state, set_prev_state, PREV_STATE: u32 @ 0xb8;
        mark_z, set_mark_z, MARK_Z: u16 @ 0xbe;
        /// Cleared at the start of each leap; read by no code of the port (h).
        leap_flag, set_leap_flag, LEAP_FLAG: u8 @ 0xbb;
    }
}
use crate::objects::pools;
use crate::objects::Current;

/// Animation sets: body, attacks, hits received.
const SET_BODY: RomAddr = RomAddr(0x172794);
const SET_ATTACK: RomAddr = RomAddr(0x1727d4);
const SET_HURT: RomAddr = RomAddr(0x1727e4);

/// HP thresholds of the phases (0x105b6).
const THRESHOLDS: RomAddr = RomAddr(0x10be2);

/// Starts animation `n` of a set via the AI (0x103d6).
fn body<R: Bench>(x: &mut Cx<'_, R>, n: u32) {
    lib::start(x, SET_BODY.0, n);
}

fn attack<R: Bench>(x: &mut Cx<'_, R>, n: u32) {
    lib::start(x, SET_ATTACK.0, n);
}

/// `moveq #k,D0 ; move.b D0,(0xa0) ; move.b D0,(0xa1)` : sorte d'animation `k`.
fn kind<R: Bench>(x: &mut Cx<'_, R>, k: u8) {
    x.set8(Boss::ANIM_KIND, k);
    x.set8(Boss::ANIM_LAST, k);
}

/// Next step (`addq.b #2,(+6)` or `(+7)`).
fn next6<R: Bench>(x: &mut Cx<'_, R>) {
    x.add8(f::SUB2, 2);
}
fn next7<R: Bench>(x: &mut Cx<'_, R>) {
    x.add8(Enemy::SUB_STEP, 2);
}

/// At least one player alive (0x10474).
fn alive<R: Bench>(x: &mut Cx<'_, R>) -> bool {
    lib::players_alive(x)
}

/// Is the boss outside the band `[camera + d0, camera + d1]` (0x104a2)?
fn outside<R: Bench>(x: &mut Cx<'_, R>, d0: i16, d1: u16) -> bool {
    lib::outside_band(x, d0, d1 as i16)
}

/// Must the boss turn around toward its target (0x1050c)?
fn turn_needed<R: Bench>(x: &mut Cx<'_, R>) -> bool {
    !lib::facing_target(x)
}

/// Wide box in front of the boss (0x7ef4).
fn in_wide_box<R: Bench>(x: &mut Cx<'_, R>, d0: i16, d1: u16, d2: u16) -> bool {
    lib::guard_in_front(x, d0, d1, d2)
}

/// Puff of dust of kind `k` (0x1172e).
fn dust<R: Bench>(x: &mut Cx<'_, R>, k: u32) {
    lib::debris(x, k);
}

/// The boss's target (`movea.w (0x50,A0),A1`).
fn target_obj<R: Bench>(x: &mut Cx<'_, R>) -> crate::objects::Obj {
    load_target(x)
}

pub fn run<R: Bench>(x: &mut Cx<'_, R>) {
    if suspended(x) {
        return;
    }
    x.tick_counters();
    let s = x.u32(f::STATE);
    x.set32(Boss1::PREV_STATE, s);
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

/// 0x7ad2a: boss setup; it waits, in its pose, for the start of the arrival cutscene.
fn init<R: Bench>(x: &mut Cx<'_, R>) {
    x.add8(f::STATE, 2);
    var::BOSS_FIGHT.set(x.b, 1);
    var::STAGE_CLOCK.set(x.b, 0x0459_5900);
    pick_target(x);
    x.set8(Boss::PHASES, 2);
    x.set8(f::BOX_SET, 0x21);
    lib::initial_hp_boss(x);
    var::SCENE_LEVEL.set(x.b, 0);
    lib::spread_index(x);
    lib::empty_popups(x);
    lib::clear_popup(x);
    lib::clear_all_popups(x);
    x.set32(f::IMMUNITY, 0xffff_ffff);
    for (off, v) in [(Enemy::AI_BYTE_ROW, 0), (Enemy::AI_STANCE_ROW, 0), (Enemy::AI_WANDER_ROW, 2), (Enemy::AI_BOX_ANIM, 0)] {
        x.set8(off, v);
    }
    x.set_bit(Enemy::DISPLAY, 1, true);
    for (off, v) in [
        (Boss::DEATH_CREDIT, 1),
        (Boss::DEATH_VALUE, 0xff),
        (Enemy::AI_DECISION_A, 1),
        (Enemy::AI_DECISION_B, 1),
        (Enemy::AI_WAIT_MASKS, 6),
        (Enemy::AI_WAIT_ROW, 1),
        (Enemy::AI_BOX_CHANCE, 7),
        (Enemy::AI_SHOT, 0xff),
        (Enemy::AI_ROW_8E, 3),
        (Enemy::AI_68, 8),
        (Enemy::AI_69, 1),
        (Enemy::VULNERABLE, 0),
        (Boss::ANIM_KIND, 0),
        (Boss::ANIM_LAST, 0xff),
        (Enemy::EVENT_KIND, 0x25),
        (Enemy::EVENT_LOW, 0),
        (Enemy::LOOT_ROW, 0x23),
        (Enemy::LOOT, 0x1d),
    ] {
        x.set8(off, v);
    }
    body(x, 4);
    lib::sound(x, crate::sound::CMD_21);
}

// --- state 2: active -----------------------------------------------------------------------------

/// 0x7ae16: one group per frame (outside freeze and pause), then play area and shadow once arrived.
fn active<R: Bench>(x: &mut Cx<'_, R>) {
    if tst8(x, f::FREEZE).gt() {
        sub8(x, f::FREEZE, 1);
    } else if var::PAUSE.get(x.b) == 0 {
        match x.u8(f::SUB) {
            0 => moves::intro(x),
            2 => moves::fight(x),
            4 => moves::attacks(x),
            6..=10 => moves::wait_for_players(x),
            12 => moves::holding(x),
            _ => hit::get_up(x),
        }
    }
    if x.u8(f::SUB) != 0 {
        lib::keep_in_zone(x);
        lib::collide_terrain(x);
    }
}

// --- decisions: states set in one go (0x7c24a..0x7c372) -----------------------------------------

/// Sets the state long `+4..+7` and makes the boss vulnerable or not (`+0x67`).
fn state<R: Bench>(x: &mut Cx<'_, R>, vulnerable: bool, s: u32) {
    x.set8(Enemy::VULNERABLE, vulnerable as u8);
    x.set32(f::STATE, s);
}

/// 0x7c24a : debout.
fn stand<R: Bench>(x: &mut Cx<'_, R>) {
    state(x, true, 0x0202_0000);
}

/// 0x7c25a: about-face, the current state will be resumed.
fn turn<R: Bench>(x: &mut Cx<'_, R>) {
    x.set8(Enemy::VULNERABLE, 0);
    let s = x.u32(f::STATE);
    x.set32(Boss::SAVED_STATE, s);
    x.set32(f::STATE, 0x0202_0200);
}

/// 0x7c26e and 0x7c27c: walk by leaps (two draws of the number of leaps).
fn walk<R: Bench>(x: &mut Cx<'_, R>) {
    state(x, false, 0x0202_0400);
}
fn walk_b<R: Bench>(x: &mut Cx<'_, R>) {
    state(x, false, 0x0202_0600);
}

/// 0x7c2a2: big leap, toward the target or (`center`) toward the middle of the screen.
fn leap<R: Bench>(x: &mut Cx<'_, R>, center: bool) {
    x.set8(Boss1::LEAP_TO_CENTER, center as u8);
    state(x, false, 0x0202_0800);
}

/// 0x7c28a: big leap toward the target after a counterattack (invulnerable).
fn leap_guarded<R: Bench>(x: &mut Cx<'_, R>) {
    x.set8(Boss::INVULNERABLE, 1);
    leap(x, false);
}

/// 0x7c2b0: backward leap, except off screen (big leap).
fn back_step<R: Bench>(x: &mut Cx<'_, R>) {
    if outside(x, 0x18, 0x168) {
        return leap_guarded(x);
    }
    x.set8(Boss::INVULNERABLE, 1);
    state(x, false, 0x0202_0a00);
}

/// 0x7c2de: first hit (a grabbable target receives hit 4).
fn strike_a<R: Bench>(x: &mut Cx<'_, R>) {
    let t = target_obj(x);
    let s = if Prey(t).grounded(x.b) != 0 { 0x0204_0400 } else { 0x0204_0000 };
    x.set8(Boss::HIT_FLAGS, 0);
    state(x, false, s);
}

/// 0x7c304: second hit (or hit 6 against a grabbable target).
fn strike_b<R: Bench>(x: &mut Cx<'_, R>) {
    let t = target_obj(x);
    let s = if Prey(t).grounded(x.b) != 0 { 0x0204_0600 } else { 0x0204_0200 };
    x.set8(Boss::HIT_FLAGS, 0);
    state(x, false, s);
}

/// 0x7c300: chain (number of hits drawn according to difficulty) that starts with the second hit.
fn combo<R: Bench>(x: &mut Cx<'_, R>) {
    combo_count(x);
    strike_b(x);
}

/// 0x7c34a : saisie.
fn grab<R: Bench>(x: &mut Cx<'_, R>) {
    state(x, false, 0x0204_0800);
}

/// 0x7c358: scissor leap with a double.
fn split_leap<R: Bench>(x: &mut Cx<'_, R>) {
    x.set8(Enemy::ACTIVE, 2);
    x.set32(f::IMMUNITY, 0xffff_ffff);
    x.set8(Boss1::GRAB_TRIES, 0);
    x.set32(f::STATE, 0x0204_0a00);
}

/// 0x7c372: no player left, wait.
fn idle_alone<R: Bench>(x: &mut Cx<'_, R>) {
    x.set32(f::STATE, 0x020a_0000);
}

/// Runs the decision whose original address is `at` (jumps through ROM tables).
fn go<R: Bench>(x: &mut Cx<'_, R>, at: u32) {
    match at {
        0x7c24a => stand(x),
        0x7c25a => turn(x),
        0x7c26e => walk(x),
        0x7c27c => walk_b(x),
        0x7c28a => leap_guarded(x),
        0x7c296 => leap(x, true),
        0x7c29e => leap(x, false),
        0x7c2b0 => back_step(x),
        0x7c2da => {
            x.set8(Boss1::COMBO, 0);
            strike_a(x)
        }
        0x7c2de => strike_a(x),
        0x7c2fa => {
            x.set8(Boss::INVULNERABLE, 1);
            combo(x)
        }
        0x7c300 => combo(x),
        0x7c304 => strike_b(x),
        0x7c344 => {
            x.set8(Boss::INVULNERABLE, 1);
            grab(x)
        }
        0x7c34a => grab(x),
        0x7c358 => split_leap(x),
        0x7c372 => idle_alone(x),
        _ => panic!("boss 1: unknown decision {at:#x}"),
    }
}

/// 0x7c0ba: target in the wide box, draw in table 0x7c0d0.
fn from_wide_box<R: Bench>(x: &mut Cx<'_, R>) {
    let r = rand_and(x, 0xf);
    let at = decide(x, "boss_1/wide_picks", r, "boss_1/wide_jumps");
    go(x, at)
}

/// 0x7c0e2: target very close; draw in the table of phase `+0xaa`.
fn from_close<R: Bench>(x: &mut Cx<'_, R>) {
    let r = rand_and(x, 0xf);
    let phase = x.u8(Boss::PHASE) as i8 as i32 as u32;
    let picks = ["boss_1/close_picks_0", "boss_1/close_picks_1", "boss_1/close_picks_2"][phase as usize % 3];
    let at = decide(x, picks, r, "boss_1/close_jumps");
    go(x, at)
}

/// 0x7c14e and 0x7c1cc: the AI chose gesture `g` (1 or 2); draw in the phase's table.
fn from_gesture<R: Bench>(x: &mut Cx<'_, R>, g: u32, picks: [&str; 6], jumps: &str) {
    let r = rand_and(x, 0xf);
    let phase = x.u8(Boss::PHASE) as i8 as i32;
    let i = (g as i32 - 1 + 2 * phase) as usize;
    let at = decide(x, picks[i.min(5)], r, jumps);
    go(x, at)
}

/// 0x7c14e: near box (-0x70).
fn from_near_gesture<R: Bench>(x: &mut Cx<'_, R>, g: u32) {
    let (a, b) = ("boss_1/near_picks_0", "boss_1/near_picks_1");
    from_gesture(x, g, [a, a, b, b, b, b], "boss_1/near_jumps")
}

/// 0x7c1cc: far box (-0xe0).
fn from_far_gesture<R: Bench>(x: &mut Cx<'_, R>, g: u32) {
    let (a, b) = ("boss_1/far_picks_0", "boss_1/far_picks_1");
    from_gesture(x, g, [a, a, a, a, b, b], "boss_1/far_jumps")
}

/// 0x7c47c: number of hits in the chain, according to difficulty and number of players.
fn combo_count<R: Bench>(x: &mut Cx<'_, R>) {
    let level = (var::RANK.get(x.b) & 0x1e) * 2;
    let mut players = var::PLAYERS_ALIVE.get(x.b) as u16;
    players = players.saturating_sub(1);
    let i = level.wrapping_add(players);
    let n = byte(x, "boss_1/combo_counts", i as u32);
    x.set8(Boss1::COMBO, n);
}

/// 0x7c50c: scissor leap attempt (with no double in progress): a chance that grows with each try.
fn try_split_leap<R: Bench>(x: &mut Cx<'_, R>) -> bool {
    if x.u16(Boss1::DOUBLE) != 0 {
        return false;
    }
    x.add8(Boss1::GRAB_TRIES, 1);
    let r = rand_and(x, 0xf);
    let tries = x.u8(Boss1::GRAB_TRIES) as u32;
    let mask = word(x, "boss_1/split_leap_masks", 2 * tries);
    mask >> r & 1 != 0
}

/// 0x7c01c / 0x7c02e: number of leaps in the walk, drawn from table `table`.
fn draw_steps<R: Bench>(x: &mut Cx<'_, R>, table: &str) {
    let r = rand_and(x, 0x1f);
    let n = byte(x, table, r);
    x.set8(Boss::WALK_STEPS, n);
}

/// 0x7c396 / 0x7c3b0: aimed point near the target (offsets drawn from `table`, `mask` + 1 entries), on the
/// boss's side; the angle toward that point (0x137e) becomes the direction.
fn aim_near_target<R: Bench>(x: &mut Cx<'_, R>, table: &str, mask: u32) {
    let i = rand_and(x, mask) * 4;
    let dx = word(x, table, i);
    let dz = word(x, table, i + 2);
    *x.c.d2() = *x.c.d2() & 0xffff_0000 | dz as u32;
    let t = target_obj(x);
    let tx = t.u16(x.b, Obj::X);
    let mut px = dx;
    if tx as i16 >= x.u16(Obj::X) as i16 {
        px = px.wrapping_neg();
    }
    let mx = px.wrapping_add(tx);
    let mz = dz.wrapping_add(t.u16(x.b, Obj::Y));
    *x.c.d2() = *x.c.d2() & 0xffff_0000 | mz as u32;
    x.set16(Boss::MARK_X, mx);
    x.set16(Boss1::MARK_Z, mz);
    aim_at_mark(x, (mx, mz));
}

/// 0x137e toward point `mark` (D1, D2 of the original) then `+0x2d` = the angle.
fn aim_at_mark<R: Bench>(x: &mut Cx<'_, R>, mark: (u16, u16)) {
    let a = lib::aim_from(x, mark);
    x.set8(Obj::SPEED_INDEX, a);
}

/// 0x7c430: vertical speed of the leap according to the distance to the aimed point: 10 more per slice of the
/// horizontal speed (the larger of the two distances, with the corresponding speed).
fn hop_height<R: Bench>(x: &mut Cx<'_, R>) {
    let abs = |v: u16| if (v as i16) < 0 { v.wrapping_neg() } else { v };
    // `move.w (0x1c,A0),D3`: the high word of D3 is the one left by the direction 0x137e called just before by
    // both callers (`moveq #0,D3` then the step): zero
    let d3 = abs(x.u16(Enemy::VY)) as u32;
    let mut d4 = abs(x.u16(Enemy::VX)) as u32;
    let dz = abs(x.u16(Boss1::MARK_Z).wrapping_sub(x.u16(Obj::Y)));
    let mut d2 = abs(x.u16(Boss::MARK_X).wrapping_sub(x.u16(Obj::X))) as u32;
    if (d2 as u16 as i16) < dz as i16 {
        d2 = dz as u32;
        d4 = d3;
    }
    d2 = d2.rotate_left(16);
    d4 = (d4.rotate_left(16) as i32 >> 8) as u32;
    let mut d1: u16 = 0;
    loop {
        let f = Fl::sub32(d2, d4);
        d2 = d2.wrapping_sub(d4);
        if f.le() {
            break;
        }
        d1 = d1.wrapping_add(10);
    }
    x.add16(Enemy::VZ, d1);
    *x.c.d2() = d2;
    *x.c.d3() = d3;
    *x.c.d4() = d4;
}

/// 0x7c4da: a puff (EFFCT 0x32, variant 4) every `n` + 1 frames.
fn puff<R: Bench>(x: &mut Cx<'_, R>, n: u8) {
    x.add8(Boss1::PUFFS, 1);
    if (n as i8) > x.u8(Boss1::PUFFS) as i8 {
        return;
    }
    x.set8(Boss1::PUFFS, 0);
    let Some(e) = x.alloc(&pools::EFFCT) else { return };
    e.set_u8(x.b, Enemy::ACTIVE, 1);
    e.set_u8(x.b, Enemy::KIND, 0x32);
    e.set_u8(x.b, Enemy::VARIANT, 4);
    e.set_u16(x.b, Enemy::CREATOR, x.o.0 as u16);
    let side = x.u8(f::FLIP);
    e.set_u8(x.b, Enemy::FLIP, side);
}

/// 0x7c542: creates the double (ENEMY 0x1a) of the scissor leap, linked to the boss by `+0xb0`.
fn spawn_double<R: Bench>(x: &mut Cx<'_, R>) {
    let Some(d) = x.alloc(&pools::ENEMY) else {
        x.set16(Boss1::DOUBLE, 0);
        return;
    };
    let active = x.u8(Enemy::ACTIVE);
    d.set_u8(x.b, Enemy::ACTIVE, active);
    d.set_u8(x.b, Enemy::KIND, 0x1a);
    d.set_u32(x.b, f::IMMUNITY, 0xffff_ffff);
    d.set_u8(x.b, f::BOX_SET, 0x21);
    for off in [0x8, 0xc, 0x10] {
        let v = x.u16(off);
        d.set_u16(x.b, off, v);
    }
    let side = x.u8(f::FLIP);
    d.set_u8(x.b, f::FLIP, side);
    d.set_u16(x.b, Boss1::DOUBLE, x.o.0 as u16);
    x.set16(Boss1::DOUBLE, d.0 as u16);
}

handlers! {
    0x7acea "boss_1" run,
    // return to combat after a common reaction (table 0x3e000 of `reactions.rs`)
    0x7bece "boss_1_getup" hit::get_up,
}

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
use super::boss_common::*;
use super::cx::handlers;
use crate::objects::Obj;

pub(crate) mod hit;
mod moves;

/// Fields specific to this type.
mod g {
    /// Leaps remaining in the walk, standing wait.
    pub const WALK_STEPS: u32 = 0x6b;
    pub const IDLE_COUNT: u32 = 0x6e;
    pub const VULNERABLE: u32 = 0x67;
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
    pub const PHASES: u32 = 0xab;
    /// Non-zero byte: the boss is invulnerable (`+0x38` reset to zero every frame).
    pub const INVULNERABLE: u32 = 0xad;
    /// Hits remaining in the attack chain.
    pub const COMBO: u32 = 0xae;
    /// Counter of the arrival's puffs.
    pub const PUFFS: u32 = 0xaf;
    /// Double (ENEMY 0x1a) created for the scissor leap (word, 0 = none).
    pub const DOUBLE: u32 = 0xb0;
    /// Grab attempts (index into the chance table).
    pub const GRAB_TRIES: u32 = 0xb2;
    /// Big leap: 0 toward the target, 1 toward the middle of the screen.
    pub const LEAP_TO_CENTER: u32 = 0xb3;
    /// First animation of the walking leap (7 or 15 depending on the direction).
    pub const HOP_ANIM: u32 = 0xb4;
    /// State of the previous frame (long).
    pub const PREV_STATE: u32 = 0xb8;
    /// Point aimed at by a leap (words).
    pub const MARK_X: u32 = 0xbc;
    pub const MARK_Z: u32 = 0xbe;
}
use g::*;
use crate::objects::pools;
use crate::objects::Current;

/// Animation sets: body, attacks, hits received.
const SET_BODY: u32 = 0x172794;
const SET_ATTACK: u32 = 0x1727d4;
const SET_HURT: u32 = 0x1727e4;

/// HP thresholds of the phases (0x105b6).
const THRESHOLDS: u32 = 0x10be2;

/// Starts animation `n` of a set via the AI (0x103d6).
fn body<R: Bench>(x: &mut Cx<'_, R>, n: u32) {
    lib::start(x, SET_BODY, n);
}

fn attack<R: Bench>(x: &mut Cx<'_, R>, n: u32) {
    lib::start(x, SET_ATTACK, n);
}

/// `moveq #k,D0 ; move.b D0,(0xa0) ; move.b D0,(0xa1)` : sorte d'animation `k`.
fn kind<R: Bench>(x: &mut Cx<'_, R>, k: u8) {
    x.set8(ANIM_KIND, k);
    x.set8(ANIM_LAST, k);
}

/// Next step (`addq.b #2,(+6)` or `(+7)`).
fn next6<R: Bench>(x: &mut Cx<'_, R>) {
    x.add8(f::SUB2, 2);
}
fn next7<R: Bench>(x: &mut Cx<'_, R>) {
    x.add8(0x7, 2);
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

/// `subq.w #1,(off,A0)`: true when the countdown has elapsed (`ble`).
fn expired<R: Bench>(x: &mut Cx<'_, R>, off: u32) -> bool {
    sub16(x, off, 1).le()
}

/// The boss's target (`movea.w (0x50,A0),A1`).
fn target_obj<R: Bench>(x: &mut Cx<'_, R>) -> crate::objects::Obj {
    load_target(x)
}

/// Word of the PC-relative jump table `base` for index `i`: address of the chosen routine.
fn jump_target<R: Bench>(x: &mut Cx<'_, R>, base: u32, i: u32) -> u32 {
    base.wrapping_add(x.rom16(base + i) as i16 as u32)
}

pub fn run<R: Bench>(x: &mut Cx<'_, R>) {
    if suspended(x) {
        return;
    }
    x.tick_counters();
    let s = x.u32(f::STATE);
    x.set32(PREV_STATE, s);
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

/// 0x7ad2a: boss setup; it waits, in its pose, for the start of the arrival cutscene.
fn init<R: Bench>(x: &mut Cx<'_, R>) {
    x.add8(f::STATE, 2);
    x.set_a5_u8(0x50, 1);
    x.set_a5_u32(0x38, 0x0459_5900);
    pick_target(x);
    x.set8(PHASES, 2);
    x.set8(f::BOX_SET, 0x21);
    lib::initial_hp_boss(x);
    var::SCENE_LEVEL.set(x.b, 0);
    lib::spread_index(x);
    lib::empty_popups(x);
    lib::clear_popup(x);
    lib::clear_all_popups(x);
    x.set32(f::IMMUNITY, 0xffff_ffff);
    for (off, v) in [(0x76, 0), (0x70, 0), (0x6a, 2), (0x75, 0)] {
        x.set8(off, v);
    }
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
        (0x7e, 0x25),
        (0x7f, 0),
        (0x80, 0x23),
        (0x81, 0x1d),
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
    x.set8(VULNERABLE, vulnerable as u8);
    x.set32(f::STATE, s);
}

/// 0x7c24a : debout.
fn stand<R: Bench>(x: &mut Cx<'_, R>) {
    state(x, true, 0x0202_0000);
}

/// 0x7c25a: about-face, the current state will be resumed.
fn turn<R: Bench>(x: &mut Cx<'_, R>) {
    x.set8(VULNERABLE, 0);
    let s = x.u32(f::STATE);
    x.set32(SAVED_STATE, s);
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
    x.set8(LEAP_TO_CENTER, center as u8);
    state(x, false, 0x0202_0800);
}

/// 0x7c28a: big leap toward the target after a counterattack (invulnerable).
fn leap_guarded<R: Bench>(x: &mut Cx<'_, R>) {
    x.set8(INVULNERABLE, 1);
    leap(x, false);
}

/// 0x7c2b0: backward leap, except off screen (big leap).
fn back_step<R: Bench>(x: &mut Cx<'_, R>) {
    if outside(x, 0x18, 0x168) {
        return leap_guarded(x);
    }
    x.set8(INVULNERABLE, 1);
    state(x, false, 0x0202_0a00);
}

/// 0x7c2de: first hit (a grabbable target receives hit 4).
fn strike_a<R: Bench>(x: &mut Cx<'_, R>) {
    let t = target_obj(x);
    let s = if t.u8(x.b, 0x66) != 0 { 0x0204_0400 } else { 0x0204_0000 };
    x.set8(0x57, 0);
    state(x, false, s);
}

/// 0x7c304: second hit (or hit 6 against a grabbable target).
fn strike_b<R: Bench>(x: &mut Cx<'_, R>) {
    let t = target_obj(x);
    let s = if t.u8(x.b, 0x66) != 0 { 0x0204_0600 } else { 0x0204_0200 };
    x.set8(0x57, 0);
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
    x.set8(0, 2);
    x.set32(f::IMMUNITY, 0xffff_ffff);
    x.set8(GRAB_TRIES, 0);
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
            x.set8(COMBO, 0);
            strike_a(x)
        }
        0x7c2de => strike_a(x),
        0x7c2fa => {
            x.set8(INVULNERABLE, 1);
            combo(x)
        }
        0x7c300 => combo(x),
        0x7c304 => strike_b(x),
        0x7c344 => {
            x.set8(INVULNERABLE, 1);
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
    let i = x.rom8(0x7c0d0 + r) as u32;
    let at = jump_target(x, 0x7c0e0, i);
    go(x, at)
}

/// 0x7c0e2: target very close; draw in the table of phase `+0xaa`.
fn from_close<R: Bench>(x: &mut Cx<'_, R>) {
    let r = rand_and(x, 0xf);
    let phase = x.u8(PHASE) as i8 as i32 as u32;
    let table = [0x7c11a, 0x7c12a, 0x7c13a][phase as usize % 3];
    let i = x.rom8(table + r) as u32;
    let at = jump_target(x, 0x7c14a, i);
    go(x, at)
}

/// 0x7c14e and 0x7c1cc: the AI chose gesture `g` (1 or 2); draw in the phase's table.
fn from_gesture<R: Bench>(x: &mut Cx<'_, R>, g: u32, tables: [u32; 6], jumps: u32) {
    let r = rand_and(x, 0xf);
    let phase = x.u8(PHASE) as i8 as i32;
    let i = (g as i32 - 1 + 2 * phase) as usize;
    let k = x.rom8(tables[i.min(5)] + r) as u32;
    let at = jump_target(x, jumps, k);
    go(x, at)
}

/// 0x7c14e: near box (-0x70).
fn from_near_gesture<R: Bench>(x: &mut Cx<'_, R>, g: u32) {
    from_gesture(x, g, [0x7c1a8, 0x7c1a8, 0x7c1b8, 0x7c1b8, 0x7c1b8, 0x7c1b8], 0x7c1c8)
}

/// 0x7c1cc: far box (-0xe0).
fn from_far_gesture<R: Bench>(x: &mut Cx<'_, R>, g: u32) {
    from_gesture(x, g, [0x7c226, 0x7c226, 0x7c226, 0x7c226, 0x7c236, 0x7c236], 0x7c246)
}

/// 0x7c080 / 0x7c09e: target in the box and gesture chosen by the AI (0x1047a), or nothing.
fn ai_box<R: Bench>(x: &mut Cx<'_, R>, d0: i16, d1: u16, d2: u16) -> Option<u32> {
    if !lib::target_in_box(x, d0, d1, d2) {
        return None;
    }
    let turn = lib::turn_decision(x);
    (turn != lib::Turn::None).then_some(turn as u32)
}

/// 0x7c47c: number of hits in the chain, according to difficulty and number of players.
fn combo_count<R: Bench>(x: &mut Cx<'_, R>) {
    let level = (var::RANK.get(x.b) & 0x1e) * 2;
    let mut players = var::PLAYERS_ALIVE.get(x.b) as u16;
    players = players.saturating_sub(1);
    let i = level.wrapping_add(players);
    let n = x.rom8(0x7c49a + i as u32);
    x.set8(COMBO, n);
}

/// 0x7c50c: scissor leap attempt (with no double in progress): a chance that grows with each try.
fn try_split_leap<R: Bench>(x: &mut Cx<'_, R>) -> bool {
    if x.u16(DOUBLE) != 0 {
        return false;
    }
    x.add8(GRAB_TRIES, 1);
    let r = rand_and(x, 0xf);
    let tries = x.u8(GRAB_TRIES) as u32;
    let mask = x.rom16(0x7c534 + 2 * tries);
    mask >> r & 1 != 0
}

/// 0x7c01c / 0x7c02e: number of leaps in the walk, drawn from table `table`.
fn draw_steps<R: Bench>(x: &mut Cx<'_, R>, table: u32) {
    let r = rand_and(x, 0x1f);
    let n = x.rom8(table + r);
    x.set8(WALK_STEPS, n);
}

/// 0x7c396 / 0x7c3b0: aimed point near the target (offsets drawn from `table`, `mask` + 1 entries), on the
/// boss's side; the angle toward that point (0x137e) becomes the direction.
fn aim_near_target<R: Bench>(x: &mut Cx<'_, R>, table: u32, mask: u32) {
    let i = rand_and(x, mask) * 4;
    let dx = x.rom16(table + i);
    let dz = x.rom16(table + i + 2);
    *x.c.d2() = *x.c.d2() & 0xffff_0000 | dz as u32;
    let t = target_obj(x);
    let tx = t.u16(x.b, 0x8);
    let mut px = dx;
    if tx as i16 >= x.u16(0x8) as i16 {
        px = px.wrapping_neg();
    }
    let mx = px.wrapping_add(tx);
    let mz = dz.wrapping_add(t.u16(x.b, 0x10));
    *x.c.d2() = *x.c.d2() & 0xffff_0000 | mz as u32;
    x.set16(MARK_X, mx);
    x.set16(MARK_Z, mz);
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
    let d3 = abs(x.u16(0x1c)) as u32;
    let mut d4 = abs(x.u16(0x14)) as u32;
    let dz = abs(x.u16(MARK_Z).wrapping_sub(x.u16(0x10)));
    let mut d2 = abs(x.u16(MARK_X).wrapping_sub(x.u16(0x8))) as u32;
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
    x.add16(0x18, d1);
    *x.c.d2() = d2;
    *x.c.d3() = d3;
    *x.c.d4() = d4;
}

/// 0x7c4da: a puff (EFFCT 0x32, variant 4) every `n` + 1 frames.
fn puff<R: Bench>(x: &mut Cx<'_, R>, n: u8) {
    x.add8(PUFFS, 1);
    if (n as i8) > x.u8(PUFFS) as i8 {
        return;
    }
    x.set8(PUFFS, 0);
    let Some(e) = x.alloc(&pools::EFFCT) else { return };
    e.set_u8(x.b, 0, 1);
    e.set_u8(x.b, 2, 0x32);
    e.set_u8(x.b, 3, 4);
    e.set_u16(x.b, 0x42, x.o.0 as u16);
    let side = x.u8(f::FLIP);
    e.set_u8(x.b, 0x2c, side);
}

/// 0x7c542: creates the double (ENEMY 0x1a) of the scissor leap, linked to the boss by `+0xb0`.
fn spawn_double<R: Bench>(x: &mut Cx<'_, R>) {
    let Some(d) = x.alloc(&pools::ENEMY) else {
        x.set16(DOUBLE, 0);
        return;
    };
    let active = x.u8(0);
    d.set_u8(x.b, 0, active);
    d.set_u8(x.b, 2, 0x1a);
    d.set_u32(x.b, f::IMMUNITY, 0xffff_ffff);
    d.set_u8(x.b, f::BOX_SET, 0x21);
    for off in [0x8, 0xc, 0x10] {
        let v = x.u16(off);
        d.set_u16(x.b, off, v);
    }
    let side = x.u8(f::FLIP);
    d.set_u8(x.b, f::FLIP, side);
    d.set_u16(x.b, DOUBLE, x.o.0 as u16);
    x.set16(DOUBLE, d.0 as u16);
}

handlers! {
    0x7acea "boss_1" run,
    // return to combat after a common reaction (table 0x3e000 of `reactions.rs`)
    0x7bece "boss_1_getup" hit::get_up,
}

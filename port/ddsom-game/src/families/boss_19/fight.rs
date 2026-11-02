//! Boss 14 / 19, group `+5` = 2: the combat. The dragon walks toward a player; depending on the distance, the
//! direction and the state of its HP it turns around, breathes or attacks (`attacks`).
//!
//! | `+6` | Role |
//! |---|---|
//! | 0, 8 | walk (shot 0x20, 0x21): draws randomly, probes the range boxes |
//! | 2 | about-face (animation 1) |
//! | 4 | motionless for a moment, then about-face if needed |
//! | 6 | slow approach |
//!
//! The range probes return true when the dragon must react; several have quirks of the original (result always
//! false, return with the "wait" state written) reproduced as is.

use crate::families::cx::Bench;
use super::*;
use crate::objects::Current;

/// Table of threshold bytes for the special counterattacks (0x901e0), indexed by the quarter of HP remaining.
const SPECIAL_LEVELS: u32 = 0x901e0;
/// Attack choice (0x91096, 32 bytes) and short-range attack choice (0x9110c, 32 bytes per difficulty level).
const PICK_A: u32 = 0x91096;
const PICK_B: u32 = 0x9110c;

pub(super) fn run<R: Bench>(x: &mut Cx<'_, R>) {
    match x.u8(f::SUB2) {
        0 => walk(x, 0x20),
        2 => turning(x),
        4 => hold(x),
        6 => close_in(x),
        8 => walk(x, 0x21),
        _ => {}
    }
    if var::PLAYERS_ALIVE.get(x.b) == 0 {
        wait(x);
    }
}

// --- marche ---------------------------------------------------------------------------------------

fn walk<R: Bench>(x: &mut Cx<'_, R>, shot: u8) {
    x.set8(SHOT, shot);
    x.set8(HOVER, 1);
    if x.u8(0x7) == 0 {
        x.add8(0x7, 2);
        x.set8(VULNERABLE, 1);
        speed(x, 0x11);
        lib::draw_stance(x);
        return;
    }
    lib::pick_target(x);
    lib::approach_target(x);
    if !sub8(x, WALK_DELAY, 1).gt() {
        lib::draw_stance(x);
        if !sub8(x, WALK_STEPS, 1).pl() {
            if lib::maybe_draw_6e(x) {
                return chase(x);
            }
            lib::draw_6b(x);
        }
    }
    if special_due(x) {
        return attack_4(x);
    }
    // probe_near (0x9132e) never returns true: the probe is launched for nothing
    lib::target_in_front(x, -0x20, 0x40, 0x10);
    if probe_front(x) || probe_back(x) {
        return pick_attack_a(x);
    }
    if probe_far(x) {
        return;
    }
    if facing_away(x) {
        return turn(x);
    }
    x.move_by_table_xy();
    x.anim_step();
}

/// 0x901c2: is a special counterattack due? The remaining HP (in quarters of `+0x60`) trigger it.
fn special_due<R: Bench>(x: &mut Cx<'_, R>) -> bool {
    let d1 = x.u16(0x60) as u32;
    let d3 = (x.u16(0x62) as u32) << 2;
    let (mut d0, mut d2) = (-1i32, 0u32);
    // the original loops forever if `+0x60` is zero
    for _ in 0..0x20000 {
        d2 = d2.wrapping_add(d1);
        d0 += 1;
        if (d2 as i32) >= (d3 as i32) {
            break;
        }
    }
    let level = x.rom8(SPECIAL_LEVELS + (d0 as u16 as u32));
    let left = x.u8(SPECIALS);
    if left == 0 || (level as i8) > (left as i8) {
        return false;
    }
    x.set8(SPECIALS, left - 1);
    true
}

/// 0x91024: is the dragon turning its back on its target? (with no target: waits, and returns true like the original).
fn facing_away<R: Bench>(x: &mut Cx<'_, R>) -> bool {
    if (x.u16(f::TARGET) as i16) >= 0 {
        wait(x);
        return true;
    }
    let target = x.linked(f::TARGET);
    let mut d = target.u16(x.b, 0x8).wrapping_sub(x.u16(0x8)).wrapping_sub(0x20) as i16;
    if x.u8(f::FLIP) == 0 {
        d = d.wrapping_neg();
    }
    d < 0
}

/// 0x9130c: a player is very close behind (box 0x7f0c): the dragon reacts (0xac3a).
fn probe_far<R: Bench>(x: &mut Cx<'_, R>) -> bool {
    if !lib::guard_in_front_strict(x, -0x60, 0x60, 0x10) {
        return false;
    }
    lib::pick_byte(x);
    true
}

/// 0x91348, 0x91390: player in the box `(d0, d1, d2)` and favorable draw (0xa4c2 if the target faces the same
/// way as the dragon, 0xa4b4 otherwise). With no target, the dragon waits and the probe returns true like the original.
fn probe_target<R: Bench>(x: &mut Cx<'_, R>, d0: i16, d1: u16, d2: u16, no_target: impl Fn(u16) -> bool) -> bool {
    if !lib::target_in_box(x, d0, d1, d2) {
        return false;
    }
    if no_target(x.u16(f::TARGET)) {
        wait(x);
        return true;
    }
    let target = x.linked(f::TARGET);
    let same_side = target.u8(x.b, f::FLIP) == x.u8(f::FLIP);
    lib::decision(x, same_side)
}

fn probe_front<R: Bench>(x: &mut Cx<'_, R>) -> bool {
    probe_target(x, -0x50, 0x80, 0x10, |t| (t as i16) >= 0)
}

fn probe_back<R: Bench>(x: &mut Cx<'_, R>) -> bool {
    probe_target(x, -0xc0, 0x30, 0x10, |t| t == 0)
}

/// 0x91050: chooses an attack (`+6` = 0 to 10) from the table.
fn pick_attack_a<R: Bench>(x: &mut Cx<'_, R>) {
    let r = (x.random() as u32) & 0x1f;
    let which = x.data8(PICK_A + r);
    x.set8(f::SUB2, which);
    x.set16(f::STATE, 0x0204);
    x.set8(0x7, 0);
}

// --- +6 = 2 : demi-tour -----------------------------------------------------------------------------

fn turning<R: Bench>(x: &mut Cx<'_, R>) {
    if x.u16(ANIM_SET) != 1 {
        return stand(x);
    }
    if step(x) {
        return stand(x);
    }
    lib::pick_target(x);
    lib::approach_target(x);
    x.move_by_table_xy();
}

// --- +6 = 4 : immobile -------------------------------------------------------------------------------

fn hold<R: Bench>(x: &mut Cx<'_, R>) {
    if x.u8(0x7) == 0 {
        x.add8(0x7, 2);
        x.set8(VULNERABLE, 1);
        return rest(x);
    }
    if sub8(x, APPROACH, 1).pl() {
        if facing_away(x) {
            return turn(x);
        }
        x.anim_step();
        return;
    }
    x.set32(f::STATE, STAND);
}

// --- +6 = 6 : approche ---------------------------------------------------------------------------------

fn close_in<R: Bench>(x: &mut Cx<'_, R>) {
    if x.u8(0x7) == 0 {
        x.add8(0x7, 2);
        x.set16(TIMER, 0x28);
        x.set8(VULNERABLE, 1);
        return speed(x, 0x21);
    }
    if !sub16(x, TIMER, 1).gt() {
        return stand(x);
    }
    x.set8(SHOT, 0x20);
    lib::approach_target(x);
    if facing_away(x) {
        return turn(x);
    }
    x.anim_step();
    x.move_by_table_xy();
}

// --- choice of an attack after a trial shot (0x910b6) ---------------------------------------------------------

/// 0x910b6: is a player within breath range? The draw (table per level) is made, then the original overwrites
/// the result with the "standing" state: only the consumption of the draw remains.
pub(super) fn pick_attack_b<R: Bench>(x: &mut Cx<'_, R>) {
    if (x.u16(f::TARGET) as i16) >= 0 {
        return wait(x);
    }
    if !lib::target_in_box(x, -0x40, 0x40, 0x20) {
        return stand(x);
    }
    let level = (var::RANK.get(x.b) >> 1) as u32;
    let base = PICK_B + (level << 5);
    let r = (x.random() as u32) & 0x1f;
    let which = x.data8(base + r);
    x.set8(f::SUB2, which);
    x.set16(f::STATE, 0x0208);
    x.set8(0x7, 0);
    x.set32(f::STATE, STAND);
}

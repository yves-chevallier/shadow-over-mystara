//! Boss 18: choice of the next action (state changes) and draws in the ROM tables.
//!
//! The states are set in one go as longs (`+4..+7`): `0x0202_0400` = active, group 2 (combat), step 4.
//! The tables are read from the ROM (opcode space: PC-relative addressing). The two-level choices
//! (byte of table 1, then word of table 2) give the address of one of the state routines below;
//! `go` finds the routine from that address.

use crate::families::cx::Bench;
use super::*;

pub(super) const WAIT: u32 = 0x020a_0000;
const STAND: u32 = 0x0202_0000;
const INTERRUPTED: u32 = 0x0202_0200;
const CHASE: u32 = 0x0202_0400;
const CHASE_B: u32 = 0x0202_0600;
const DASH: u32 = 0x0202_0800;

/// 0x9917a: waits (no player alive).
pub(super) fn wait<R: Bench>(x: &mut Cx<'_, R>) {
    x.set32(f::STATE, WAIT);
}

/// 0x98f9c: standing, vulnerable.
pub(super) fn stand<R: Bench>(x: &mut Cx<'_, R>) {
    x.set8(Enemy::VULNERABLE, 1);
    x.set32(f::STATE, STAND);
}

/// 0x98fac: interrupts the action in progress (the state is saved in `+0xa2`).
pub(super) fn interrupt<R: Bench>(x: &mut Cx<'_, R>) {
    x.set8(Enemy::VULNERABLE, 0);
    let state = x.u32(f::STATE);
    x.set32(Boss::SAVED_STATE, state);
    x.set32(f::STATE, INTERRUPTED);
}

/// 0x98fc0: walk toward a player (shot 0x29).
pub(super) fn chase<R: Bench>(x: &mut Cx<'_, R>) {
    x.set8(Enemy::VULNERABLE, 1);
    x.set8(Boss::SHOT, 0x29);
    x.set32(f::STATE, CHASE);
}

/// 0x98fd6: same (shot 0x2a), slow walk.
pub(super) fn chase_b<R: Bench>(x: &mut Cx<'_, R>) {
    x.set8(Enemy::VULNERABLE, 1);
    x.set8(Boss::SHOT, 0x2a);
    x.set32(f::STATE, CHASE_B);
}

/// 0x98fec: the charge if the boss is in the useful band of the screen, otherwise attack 16.
pub(super) fn dash_or_attack<R: Bench>(x: &mut Cx<'_, R>) {
    let cam = var::CAMERA1_X.get(x.b);
    let here = x.u16(Obj::X);
    if Fl::sub16(cam.wrapping_add(0x20), here).ge() || Fl::sub16(cam.wrapping_add(0x160), here).lt() {
        return attack_16(x);
    }
    x.set8(Enemy::VULNERABLE, 0);
    x.set8(Boss::SHOT, 0x2a);
    x.set32(f::STATE, DASH);
}

fn attack<R: Bench>(x: &mut Cx<'_, R>, state: u32) {
    x.set8(Enemy::VULNERABLE, 0);
    x.set32(f::STATE, state);
}

/// 0x99032: attack 0.
pub(super) fn attack_0<R: Bench>(x: &mut Cx<'_, R>) {
    attack(x, 0x0204_0000);
}

/// 0x99040: attack 2.
pub(super) fn attack_2<R: Bench>(x: &mut Cx<'_, R>) {
    attack(x, 0x0204_0200);
}

/// 0x9904e: attack 4 (once).
pub(super) fn attack_4<R: Bench>(x: &mut Cx<'_, R>) {
    x.set8(Enemy::VULNERABLE, 0);
    x.set8(Boss18::BUDGET_4, 1);
    x.set32(f::STATE, 0x0204_0400);
}

/// 0x99062: attack 6 (magic pursuit) if a target exists.
pub(super) fn attack_6<R: Bench>(x: &mut Cx<'_, R>) {
    if !lib::pick_target_any(x) {
        return wait(x);
    }
    x.set8(Enemy::VULNERABLE, 0);
    x.set16(Boss18::CHASE_TIME, 0x168);
    x.set8(Boss18::AIM_TICK, 2);
    x.set8(Boss18::AIM_ON, 1);
    x.set32(f::STATE, 0x0204_0600);
}

/// 0x9908c: attack 8.
pub(super) fn attack_8<R: Bench>(x: &mut Cx<'_, R>) {
    attack(x, 0x0204_0800);
}

/// 0x9909a: attack 10 as long as some remain; otherwise attack 20.
pub(super) fn attack_10<R: Bench>(x: &mut Cx<'_, R>) {
    if (x.u8(Boss18::BUDGET_A) as i8) <= 0 {
        return attack_20(x);
    }
    attack(x, 0x0204_0a00);
}

/// 0x990b0: attack 12 as long as some remain; otherwise attack 8.
pub(super) fn attack_12<R: Bench>(x: &mut Cx<'_, R>) {
    if (x.u8(Boss18::BUDGET_B) as i8) <= 0 {
        return attack_8(x);
    }
    attack(x, 0x0204_0c00);
}

/// 0x990c4: attack 14 (dropping objects) if nothing is already dropped and some remain.
pub(super) fn attack_14<R: Bench>(x: &mut Cx<'_, R>) {
    x.set8(Boss18::AIM_ON, 0);
    if (x.u8(Boss18::SPAWNED) as i8) > 0 || (x.u8(Boss18::BUDGET_C) as i8) <= 0 {
        return attack_8(x);
    }
    x.set8(Boss::INVULNERABLE, 1);
    attack(x, 0x0204_0e00);
}

/// 0x990e8: attack 16 as long as some remain; otherwise attack 2.
pub(super) fn attack_16<R: Bench>(x: &mut Cx<'_, R>) {
    if (x.u8(Boss18::BUDGET_D) as i8) <= 0 {
        return attack_2(x);
    }
    sub8(x, Boss18::BUDGET_D, 1);
    attack(x, 0x0204_1000);
}

/// 0x99102: attack 18 (the boss is within the screen window) or 16, 0.
pub(super) fn attack_18<R: Bench>(x: &mut Cx<'_, R>) {
    if x.u8(Boss18::BUDGET_E) == 0 {
        return attack_0(x);
    }
    let cam = var::CAMERA1_X.get(x.b);
    let here = x.u16(Obj::X);
    if Fl::sub16(cam.wrapping_add(0x30), here).ge() || Fl::sub16(cam.wrapping_add(0x140), here).lt() {
        return attack_16(x);
    }
    let camy = var::CAMERA1_Y.get(x.b);
    let y = x.u16(Obj::Y);
    if Fl::sub16(camy.wrapping_add(0x38), y).ge() || Fl::sub16(camy.wrapping_add(0x70), y).lt() {
        return attack_0(x);
    }
    x.set8(Boss::INVULNERABLE, 1);
    attack(x, 0x0204_1200);
}

/// 0x99156: attack 20 (if the phase allows it); otherwise attack 6 or 8.
pub(super) fn attack_20<R: Bench>(x: &mut Cx<'_, R>) {
    if (x.u8(Boss18::BUDGET_F) as i8) <= 0 {
        return attack_6(x);
    }
    if (x.u8(Boss::PHASE) as i8) <= 0 {
        return attack_8(x);
    }
    x.set8(Boss::INVULNERABLE, 1);
    attack(x, 0x0204_1400);
}

// --- draws -------------------------------------------------------------------------------------


/// HP thresholds per phase (words, 128 bytes per row).
const THRESHOLDS: RomAddr = RomAddr(0x114e2);

/// 0x98cae: delay before the next step, fast walk; 0x98cba: slow walk.
pub(super) fn walk_delay_fast<R: Bench>(x: &mut Cx<'_, R>) {
    let v = per_rank(x, "boss_18/walk_delays");
    x.set8(Boss::WALK_DELAY, v);
}

pub(super) fn walk_delay_slow<R: Bench>(x: &mut Cx<'_, R>) {
    let v = per_rank(x, "boss_18/walk_delays_slow");
    x.set8(Boss::WALK_DELAY, v);
}

/// 0x98d06, 0x98d18: number of steps of a walk.
pub(super) fn walk_steps_fast<R: Bench>(x: &mut Cx<'_, R>) {
    let r = (x.random() as u32) & 0x1f;
    let v = byte(x, "boss_18/walk_steps", r);
    x.set8(Boss::WALK_STEPS, v);
}

pub(super) fn walk_steps_slow<R: Bench>(x: &mut Cx<'_, R>) {
    let r = (x.random() as u32) & 0x1f;
    let v = byte(x, "boss_18/walk_steps_slow", r);
    x.set8(Boss::WALK_STEPS, v);
}

/// 0x98d6a, 0x98d86: is a player in the box? Returns 0, or 1 / 2: the side (0x1047a).
pub(super) fn probe<R: Bench>(x: &mut Cx<'_, R>, d0: i16, d1: u16, d2: u16) -> u8 {
    if !lib::target_in_box(x, d0, d1, d2) {
        return 0;
    }
    lib::turn_decision(x) as u8
}

/// Runs the state routine whose address a table gives.
fn go<R: Bench>(x: &mut Cx<'_, R>, routine: u32) {
    match routine {
        0x98fec => dash_or_attack(x),
        0x99032 => attack_0(x),
        0x99040 => attack_2(x),
        0x99102 => attack_18(x),
        0x9904e => attack_4(x),
        0x99062 => attack_6(x),
        0x9908c => attack_8(x),
        0x9909a => attack_10(x),
        0x990b0 => attack_12(x),
        0x990c4 => attack_14(x),
        t => unreachable!("boss 18: state routine {t:x}"),
    }
}

/// 0x98da2: a player is very close.
pub(super) fn choose_close<R: Bench>(x: &mut Cx<'_, R>) {
    let r = (x.random() as u32) & 0xf;
    let routine = decide(x, "boss_18/close_picks", r, "boss_18/close_jumps");
    go(x, routine);
}

/// 0x98dcc: a player is ahead (box 0 / 0x2c / 0xc); the phase sets the choice.
pub(super) fn choose_front<R: Bench>(x: &mut Cx<'_, R>) {
    let r = (x.random() as u32) & 0xf;
    let phase = x.u8(Boss::PHASE) as i8 as i32;
    let picks = match jump_target(x, "boss_18/front_phases", (2 * phase) as u32) {
        0x98dec => "boss_18/front_picks_0",
        0x98df2 => "boss_18/front_picks_1",
        0x98df8 => "boss_18/front_picks_2",
        t => unreachable!("0x98dcc: target {t:x}"),
    };
    let routine = decide(x, picks, r, "boss_18/front_jumps");
    go(x, routine);
}

/// 0x98e3c: a player is at mid-distance; `side` = 1 or 2.
pub(super) fn choose_mid<R: Bench>(x: &mut Cx<'_, R>, side: u8) {
    let r = (x.random() as u32) & 0xf;
    let picks = if side == 1 { "boss_18/mid_picks_0" } else { "boss_18/mid_picks_1" };
    let routine = decide(x, picks, r, "boss_18/mid_jumps");
    go(x, routine);
}

/// 0x98e96: a player is far; `side` = 1 or 2.
pub(super) fn choose_far<R: Bench>(x: &mut Cx<'_, R>, side: u8) {
    let r = (x.random() as u32) & 0xf;
    let phase = x.u8(Boss::PHASE) as i8 as i32;
    let entry = (side as i32 - 1) * 2 + 4 * phase;
    let picks = match jump_target(x, "boss_18/far_phases", entry as u32) {
        0x98ec6 => "boss_18/far_picks_0",
        0x98ed2 | 0x98ed8 => "boss_18/far_picks_1",
        0x98eec => {
            let slot = (var::SCENE_LEVEL.get(x.b) as u32).min(4);
            match jump_target(x, "boss_18/far_levels", 2 * slot) {
                0x98f0e => "boss_18/far_picks_2",
                0x98f14 => "boss_18/far_picks_3",
                0x98f1a => "boss_18/far_picks_4",
                0x98f20 => "boss_18/far_picks_5",
                0x98f26 => "boss_18/far_picks_6",
                t => unreachable!("0x98eec: row {t:x}"),
            }
        }
        t => unreachable!("0x98e96: target {t:x}"),
    };
    let routine = decide(x, picks, r, "boss_18/far_jumps");
    match routine {
        0x9908c => attack_8(x),
        0x9909a => attack_10(x),
        0x990b0 => attack_12(x),
        0x990c4 => attack_14(x),
        t => unreachable!("0x98edc: target {t:x}"),
    }
}

// --- budgets and draws of the phase ---------------------------------------------------------------------

/// 0x9946e..: the boss is standing, chooses what follows according to its phase (0x99428).
pub(super) fn back_on_feet<R: Bench>(x: &mut Cx<'_, R>) {
    x.set8(Enemy::REACTION, 0);
    x.set32(Enemy::HIT_LOCK, 0);
    x.set8(Enemy::ACTIVE, 1);
    x.set8(Boss::ANIM_LAST, 0xff);
    if var::GAME_ON.get(x.b) == 0 {
        return wait(x);
    }
    if !lib::pick_target_any(x) {
        return wait(x);
    }
    if lib::next_threshold(x, THRESHOLDS.0) {
        if cmp8(x, Boss::PHASE, 2).eq() {
            return attack_20(x);
        }
        return attack_14(x);
    }
    if x.u8(Boss18::AIM_ON) != 0 {
        // the magic pursuit resumes without redrawing its duration
        return x.set32(f::STATE, 0x0204_0600);
    }
    chase(x);
}

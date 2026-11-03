//! Boss 6: choice of the next action (state changes) and draws in the ROM tables.
//! Same principle as `boss_17::decide`; the decision tables are read from the ROM (PC-relative addressing).

use crate::families::cx::Bench;
use super::*;

pub(super) const WAIT: u32 = 0x020a_0000;
const STAND: u32 = 0x0202_0000;
const INTERRUPTED: u32 = 0x0202_0200;
const CHASE: u32 = 0x0202_0400;
const CHASE_B: u32 = 0x0202_0600;
const CHARGE: u32 = 0x0202_0800;
const SHOOT: u32 = 0x0202_0a00;
const ATTACK_0: u32 = 0x0204_0000;
const ATTACK_2: u32 = 0x0204_0200;
const ATTACK_4: u32 = 0x0204_0400;
const ATTACK_6: u32 = 0x0204_0600;
const ATTACK_8: u32 = 0x0204_0800;

fn set_state<R: Bench>(x: &mut Cx<'_, R>, vulnerable: u8, state: u32) {
    x.set8(Enemy::VULNERABLE, vulnerable);
    x.set32(f::STATE, state);
}

/// 0x85962: waits (no player alive).
pub(super) fn wait<R: Bench>(x: &mut Cx<'_, R>) {
    x.set32(f::STATE, WAIT);
}

/// 0x8588a: standing and vulnerable.
pub(super) fn stand<R: Bench>(x: &mut Cx<'_, R>) {
    set_state(x, 1, STAND);
}

/// 0x8589a: interrupts the action (state saved in `+0xa2`).
pub(super) fn interrupt<R: Bench>(x: &mut Cx<'_, R>) {
    x.set8(Enemy::VULNERABLE, 0);
    let state = x.u32(f::STATE);
    x.set32(Boss::SAVED_STATE, state);
    x.set32(f::STATE, INTERRUPTED);
}

/// 0x858ae: fast walk toward a player.
pub(super) fn chase<R: Bench>(x: &mut Cx<'_, R>) {
    x.set8(Boss::SHOT, 0x2c);
    set_state(x, 1, CHASE);
}

/// 0x858c4 : marche lente.
pub(super) fn chase_b<R: Bench>(x: &mut Cx<'_, R>) {
    x.set8(Boss::SHOT, 0x2d);
    set_state(x, 1, CHASE_B);
}

/// 0x858da (`at_target` false) and 0x858ec: charge toward the camera or toward the target.
pub(super) fn charge<R: Bench>(x: &mut Cx<'_, R>, at_target: bool) {
    x.set8(Enemy::VULNERABLE, 0);
    x.set8(Boss6::CHARGE_AT_TARGET, at_target as u8);
    x.set32(f::STATE, CHARGE);
}

/// 0x85900: shot in a leap.
pub(super) fn shoot<R: Bench>(x: &mut Cx<'_, R>) {
    set_state(x, 0, SHOOT);
}

pub(super) fn attack_0<R: Bench>(x: &mut Cx<'_, R>) {
    set_state(x, 1, ATTACK_0);
}

pub(super) fn attack_2<R: Bench>(x: &mut Cx<'_, R>) {
    set_state(x, 0, ATTACK_2);
}

pub(super) fn attack_4<R: Bench>(x: &mut Cx<'_, R>) {
    set_state(x, 0, ATTACK_4);
}

/// 0x8593a: attack 6 while invulnerable.
pub(super) fn attack_6_shielded<R: Bench>(x: &mut Cx<'_, R>) {
    x.set8(Boss::INVULNERABLE, 1);
    set_state(x, 0, ATTACK_6);
}

/// 0x8594e: attack 8 while invulnerable.
pub(super) fn attack_8_shielded<R: Bench>(x: &mut Cx<'_, R>) {
    x.set8(Boss::INVULNERABLE, 1);
    set_state(x, 0, ATTACK_8);
}

/// Applies the state change whose original address is `routine`.
fn apply<R: Bench>(x: &mut Cx<'_, R>, routine: u32) {
    match routine {
        0x858ec => charge(x, true),
        0x85900 => shoot(x),
        0x8590e => attack_0(x),
        0x8591e => attack_2(x),
        0x8592c => attack_4(x),
        0x85940 => {
            x.set8(Enemy::VULNERABLE, 0);
            x.set32(f::STATE, ATTACK_6);
        }
        0x8594e => attack_8_shielded(x),
        0x85954 => {
            x.set8(Enemy::VULNERABLE, 0);
            x.set32(f::STATE, ATTACK_8);
        }
        t => unreachable!("boss 6: target {t:x}"),
    }
}

// --- tirages ------------------------------------------------------------------------------------

pub(super) fn walk_delay_fast<R: Bench>(x: &mut Cx<'_, R>) {
    let v = per_rank(x, "boss_6/walk_delays");
    x.set8(Boss::WALK_DELAY, v);
}

pub(super) fn walk_delay_slow<R: Bench>(x: &mut Cx<'_, R>) {
    let v = per_rank(x, "boss_6/walk_delays_slow");
    x.set8(Boss::WALK_DELAY, v);
}

pub(super) fn walk_steps_fast<R: Bench>(x: &mut Cx<'_, R>) {
    let r = (x.random() as u32) & 0x1f;
    let v = byte(x, "boss_6/walk_steps", r);
    x.set8(Boss::WALK_STEPS, v);
}

pub(super) fn walk_steps_slow<R: Bench>(x: &mut Cx<'_, R>) {
    let r = (x.random() as u32) & 0x1f;
    let v = byte(x, "boss_6/walk_steps_slow", r);
    x.set8(Boss::WALK_STEPS, v);
}

/// Is a player in the box `(d0, d1, d2)`? 0 if not, or 1 / 2: the side (0x1047a).
pub(super) fn probe<R: Bench>(x: &mut Cx<'_, R>, d0: i16, d1: u16, d2: u16) -> u8 {
    if !lib::target_in_box(x, d0, d1, d2) {
        return 0;
    }
    lib::turn_decision(x) as u8
}

/// 0x856f2: a player is very close ahead.
pub(super) fn choose_close<R: Bench>(x: &mut Cx<'_, R>) {
    let r = (x.random() as u32) & 0xf;
    let t = decide(x, "boss_6/close_picks", r, "boss_6/close_jumps");
    apply(x, t);
}

/// 0x8571a: a player is ahead; the table depends on the phase.
pub(super) fn choose_front<R: Bench>(x: &mut Cx<'_, R>) {
    let r = (x.random() as u32) & 0xf;
    let phase = x.u8(Boss::PHASE) as i8 as i32;
    // jump table 0x85734 (three entries, by phase): each routine draws in its own pick table
    let picks = match phase {
        0 => "boss_6/front_picks_0",
        1 => "boss_6/front_picks_1",
        2 => "boss_6/front_picks_2",
        _ => bad_index(2 * phase as u32),
    };
    let t = decide(x, picks, r, "boss_6/front_jumps");
    apply(x, t);
}

/// 0x85788: a player is at mid-distance (side `side`).
pub(super) fn choose_mid<R: Bench>(x: &mut Cx<'_, R>, side: u8) {
    let r = (x.random() as u32) & 0xf;
    let index = (side as u32 - 1) * 2 + 4 * (x.u8(Boss::PHASE) as i8 as i32 as u32);
    // the jump table 0x857ac (six entries, by side and phase) leads to six routines that draw in the same pick table
    if index >= 12 {
        bad_index(index);
    }
    let t = decide(x, "boss_6/mid_picks", r, "boss_6/mid_jumps");
    apply(x, t);
}

/// 0x857f6: a player is far (side `side`).
pub(super) fn choose_far<R: Bench>(x: &mut Cx<'_, R>, side: u8) {
    let r = (x.random() as u32) & 0xf;
    let index = (side as u32 - 1) * 2 + 4 * (x.u8(Boss::PHASE) as i8 as i32 as u32);
    // the jump table 0x8581a (six entries, by side and phase) leads to six routines: the first two draw in the first
    // pick table, the next two in the second, the last two in the third
    let picks = match index / 2 {
        0 | 1 => "boss_6/far_picks_0",
        2 | 3 => "boss_6/far_picks_1",
        4 | 5 => "boss_6/far_picks_2",
        _ => bad_index(index),
    };
    let t = decide(x, picks, r, "boss_6/far_jumps");
    apply(x, t);
}

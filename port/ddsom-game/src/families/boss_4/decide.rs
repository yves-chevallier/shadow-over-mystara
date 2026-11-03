//! Boss 4: states set in one go (0x8328e..0x833ac) and attack choice during the walk.
//!
//! During the walk, four boxes in front of the boss decide what follows; each decision draws a number from 0 to 15,
//! reads from a ROM byte table (chosen according to phase `+0xaa` and, for the two AI boxes, the gesture
//! 1 or 2 returned by 0x1047a) an index into a jump table to one of the states below.

use crate::families::cx::Bench;
use super::*;

/// 0x8328e : combat, debout.
pub(super) fn idle<R: Bench>(x: &mut Cx<'_, R>) {
    x.set8(VULNERABLE, 1);
    x.set32(f::STATE, 0x0202_0000);
}

/// 0x8329e: about-face, then resumption of the current state.
pub(super) fn turn<R: Bench>(x: &mut Cx<'_, R>) {
    x.set8(VULNERABLE, 0);
    let s = x.u32(f::STATE);
    x.set32(SAVED_STATE, s);
    x.set32(f::STATE, 0x0202_0200);
}

/// 0x832b2: walk toward the target, fast steps (shot 0x38).
pub(super) fn walk<R: Bench>(x: &mut Cx<'_, R>) {
    x.set8(VULNERABLE, 1);
    x.set8(SHOT, 0x38);
    x.set32(f::STATE, 0x0202_0400);
}

/// 0x832c8 : marche lente (tir 0x39).
pub(super) fn walk_slow<R: Bench>(x: &mut Cx<'_, R>) {
    x.set8(VULNERABLE, 1);
    x.set8(SHOT, 0x39);
    x.set32(f::STATE, 0x0202_0600);
}

/// 0x832de / 0x832f0: charge toward the middle of the screen, or toward the target.
pub(super) fn charge<R: Bench>(x: &mut Cx<'_, R>, at_target: bool) {
    x.set8(VULNERABLE, 0);
    x.set8(CHARGE_AT_TARGET, at_target as u8);
    x.set32(f::STATE, 0x0202_0800);
}

/// 0x83304: backward leap.
fn hop<R: Bench>(x: &mut Cx<'_, R>) {
    x.set8(VULNERABLE, 0);
    x.set32(f::STATE, 0x0202_0a00);
}

/// Attack `n` (`+6` = 2n of group 4), vulnerable or not.
fn attack<R: Bench>(x: &mut Cx<'_, R>, n: u8, vulnerable: bool) {
    x.set8(VULNERABLE, vulnerable as u8);
    x.set32(f::STATE, 0x0204_0000 | (n as u32 * 2) << 8);
}

/// 0x8333e: crossing the screen in flight (attack 3), invulnerable.
pub(super) fn swoop<R: Bench>(x: &mut Cx<'_, R>) {
    x.set8(INVULNERABLE, 1);
    attack(x, 3, false);
}

/// Is the first double busy (`+0xaf`)? The breaths then give way to another attack.
fn double_busy<R: Bench>(x: &mut Cx<'_, R>) -> bool {
    let d = double(x, DOUBLE_A);
    d.u8(x.b, 0xaf) != 0
}

/// 0x833ac: waiting for a player.
pub(super) fn wait<R: Bench>(x: &mut Cx<'_, R>) {
    x.set32(f::STATE, 0x020a_0000);
}

/// Sets the state of routine `target` from a decision jump table.
fn enter<R: Bench>(x: &mut Cx<'_, R>, target: u32) {
    match target {
        0x8328e => idle(x),
        0x832c8 => walk_slow(x),
        0x832f0 => charge(x, true),
        0x83304 => hop(x),
        0x83312 => attack(x, 0, true),
        0x83322 => attack(x, 1, false),
        0x83330 => attack(x, 2, false),
        0x83344 => attack(x, 3, false),
        // 0x83352: breath in flight (invulnerable), 0x83364: breath on the ground, otherwise takeoff (attack 2)
        0x83352 | 0x83364 => {
            if double_busy(x) {
                return attack(x, 2, false);
            }
            if target == 0x83352 {
                x.set8(INVULNERABLE, 1);
            }
            attack(x, 4, false);
        }
        // 0x8337c, 0x83394: long breaths, otherwise dive (1) or takeoff (2)
        0x8337c => {
            if double_busy(x) {
                return attack(x, 1, false);
            }
            attack(x, 5, false);
        }
        0x83394 => {
            if double_busy(x) {
                return attack(x, 2, false);
            }
            attack(x, 6, false);
        }
        _ => unreachable!("boss 4: state {target:x}"),
    }
}

/// Draw `r` (0 to 15) in the pick table `picks`, then enters the state designated by the jump table `jumps`.
fn roll<R: Bench>(x: &mut Cx<'_, R>, r: u32, picks: &str, jumps: &str) {
    let t = decide(x, picks, r, jumps);
    enter(x, t)
}

/// Phase `+0xaa` (byte extended to a word).
fn phase<R: Bench>(x: &mut Cx<'_, R>) -> u16 {
    x.u8(PHASE) as i8 as i16 as u16
}

/// 0x830ce: the target is in the wide box (0x7ef4): table 0x830e4 (a single choice, the backward leap).
pub(super) fn from_wide_box<R: Bench>(x: &mut Cx<'_, R>) {
    let r = rand_and(x, 0xf);
    roll(x, r, "boss_4/wide_picks", "boss_4/wide_jumps");
}

/// 0x830f6: the target is very close (0xb5e0): depending on the phase, tables 0x8312e, 0x8313e, 0x8314e.
pub(super) fn from_close<R: Bench>(x: &mut Cx<'_, R>) {
    let r = rand_and(x, 0xf);
    let p = phase(x).wrapping_add(phase(x));
    setw(x, 1, p);
    let picks = ["boss_4/close_picks_0", "boss_4/close_picks_1", "boss_4/close_picks_2"][p as usize / 2];
    roll(x, r, picks, "boss_4/close_jumps");
}

/// 0x83164 (near box, `near`) and 0x831f6 (far box): depending on the phase and the AI's gesture `gesture` (1 or 2).
pub(super) fn from_ai<R: Bench>(x: &mut Cx<'_, R>, gesture: u16, near: bool) {
    setw(x, 2, gesture);
    let r = rand_and(x, 0xf);
    let p = phase(x);
    setw(x, 1, p.wrapping_mul(4));
    let row = (gesture.wrapping_sub(1).wrapping_mul(2)).wrapping_add(p.wrapping_mul(4));
    setw(x, 2, row);
    let (picks, jumps): ([&str; 6], &str) = if near {
        let (a, b, c) = ("boss_4/near_picks_0", "boss_4/near_picks_1", "boss_4/near_picks_2");
        ([a, c, a, c, b, c], "boss_4/near_jumps")
    } else {
        let (a, b, c) = ("boss_4/far_picks_0", "boss_4/far_picks_1", "boss_4/far_picks_2");
        ([a, a, b, b, c, c], "boss_4/far_jumps")
    };
    roll(x, r, picks[row as usize / 2], jumps);
}

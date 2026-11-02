//! Boss 16, state 4 "hit" (0x94050). `+5` groups:
//!
//! | `+5` | Role |
//! |---|---|
//! | 0 | reaction to the hit: specific routine chosen by code `+0x49` (table 0x940a8) |
//! | 2 | knockback, sliding |
//! | 4 | on the ground, getting up |
//! | 6 | death (two sequences depending on the hit code) |
//! | 8 | nothing |
//! | 10 | return to combat |

use crate::families::cx::Bench;
use super::*;
use crate::objects::Current;

/// Table of reaction routines, indexed by the hit code `+0x49` (longs).
const REACTIONS: u32 = 0x940a8;
/// Reaction routines specific to the boss.
const KNOCKBACK: u32 = 0x9416e;
const KNOCKBACK_HARD: u32 = 0x941f8;
const BURST: u32 = 0x942da;
const KILL: u32 = 0x94154;
/// Table of death sequences, indexed by the hit code.
const DEATHS: u32 = 0x94644;
const DEATH_HEAVY: u32 = 0x947c6;

pub(super) fn run<R: Bench>(x: &mut Cx<'_, R>) {
    x.set_bit(0x1, 1, false);
    if tst8(x, f::FREEZE).gt() {
        sub8(x, f::FREEZE, 1);
    } else {
        match x.u8(f::SUB) {
            0 => reaction(x),
            2 => pushed_back(x),
            4 => down(x),
            6 => death(x),
            10 => next_attack(x),
            _ => {}
        }
    }
    lib::keep_on_screen(x, 0x50);
    lib::collide_terrain(x);
    x.reinsert();
}

/// Immediate death: out of HP, death sequence in the same frame.
fn die<R: Bench>(x: &mut Cx<'_, R>) {
    x.set8(f::SUB, 6);
    x.set16(f::SUB2, 0);
    death_normal(x);
}

/// 0x94092: first pass: damage (0xb648); then the hit code's routine.
fn reaction<R: Bench>(x: &mut Cx<'_, R>) {
    if x.u8(f::SUB2) == 0 {
        lib::drop_once(x);
    }
    let code = x.u8(0x49) as u32;
    match x.prog32(REACTIONS + code) {
        KNOCKBACK => knockback(x, false),
        KNOCKBACK_HARD => knockback(x, true),
        BURST => burst(x),
        KILL => {
            if var::PAUSE.get(x.b) == 0 {
                x.set16(f::HP, 0);
                die(x);
            }
        }
        other => panic!("boss 16: unknown reaction {other:#x}"),
    }
}

/// 0x9453a: counts the hits taken in a row (`+0xa5`, 5 at the start); on the last one, the boss counterattacks
/// (state 0x0204_0c00).
fn counter<R: Bench>(x: &mut Cx<'_, R>) -> bool {
    if sub8(x, HIT_BUDGET, 1).gt() {
        return false;
    }
    x.set32(f::STATE, 0x0204_0c00);
    x.set8(TIMER, 6);
    x.set8(0x38, 0);
    x.set32(0x58, 0);
    x.set8(0x49, 0);
    true
}

/// End of the knockbacks: the boss gets up away from the edges (attack 0xe to come back toward the screen) or walks.
fn recover_position<R: Bench>(x: &mut Cx<'_, R>) {
    x.set8(0, 1);
    x.set8(0x49, 0);
    x.set32(0x58, 0);
    let mut edge = var::SCROLL_X.get(x.b);
    if x.a5_u8(0xa) != 0 {
        edge = var::CAMERA_ALT_X.get(x.b);
    }
    let mut side = 0x20u8;
    let left = edge.wrapping_add(0x30);
    if Fl::sub16(left, x.u16(0x8)).ge() {
        // near the left edge: looks right
    } else {
        side = 0;
        let right = edge.wrapping_add(0x30).wrapping_add(0x120);
        if !Fl::sub16(right, x.u16(0x8)).le() {
            return x.set32(f::STATE, state::WALK);
        }
    }
    x.set8(TIMER, 8);
    x.set8(f::FLIP, side);
    x.set32(f::STATE, 0x0204_0e00);
}

/// 0x9416e and 0x941f8: thrown into the air (`hard`) or pushed back on the ground; at the end the boss gets up.
fn knockback<R: Bench>(x: &mut Cx<'_, R>, hard: bool) {
    if x.u8(f::SUB2) == 0 {
        return knockback_start(x, hard);
    }
    if hard {
        return knockback_hard_hold(x);
    }
    knockback_hold(x);
}

/// 0x94174 and 0x9421e...: start (HP still positive, otherwise death).
fn knockback_start<R: Bench>(x: &mut Cx<'_, R>, hard: bool) {
    if !tst16(x, f::HP).gt() {
        return die(x);
    }
    next6(x);
    let freeze = x.u8(0x56);
    x.set8(f::FREEZE, freeze);
    x.set8(SHAKE, 0x10);
    stop(x);
    if hard {
        let v = if x.u8(f::FLIP) == 0 { 0x600u16 } else { (-0x600i16) as u16 };
        x.set16(0x14, v);
        x.set16(0x16, 0);
        x.set16(0x18, 0x400);
        x.set16(0x1a, (-0x80i16) as u16);
    } else {
        let (vx, vz) = if x.u8(f::FLIP) == 0 { (0x400u16, (-0x48i16) as u16) } else { ((-0x400i16) as u16, 0x48) };
        x.set16(0x14, vx);
        x.set16(0x16, vz);
    }
    if counter(x) {
        return;
    }
    anim_c(x, 0);
}

/// 0x941c4: trembling then slide.
fn knockback_hold<R: Bench>(x: &mut Cx<'_, R>) {
    if x.u8(SHAKE) != 0 {
        let n = x.u8(SHAKE).wrapping_sub(1);
        x.set8(SHAKE, n);
        let d: u16 = if n & 1 != 0 { 0xfffe } else { 2 };
        x.add16(0x8, d);
        return;
    }
    if x.u8(0x14) == 0 {
        return recover_position(x);
    }
    x.move_x();
    lib::step(x);
}

/// 0x94254: thrown: landing back on the ground.
fn knockback_hard_hold<R: Bench>(x: &mut Cx<'_, R>) {
    if x.u8(SHAKE) != 0 {
        sub8(x, SHAKE, 1);
        x.set8(SHAKE, 0x10);
    }
    x.move_xz();
    if (x.u16(0xc) as i16) > 0 {
        lib::step(x);
        return;
    }
    x.set16(0xc, 0);
    recover_position(x);
}

/// 0x942da: special hits (codes 0x60, 0x70, 0x78, 0x90, 0x94): bursts of effects, then the knockback.
fn burst<R: Bench>(x: &mut Cx<'_, R>) {
    match x.u8(f::SUB2) {
        0 => {
            next6(x);
            x.set8(0, 2);
            let (kind, flag) = match x.u8(0x49) {
                0x60 => (7, 0),
                0x70 | 0x90 => (8, 0),
                0x78 => (1, 0),
                0x94 => (9, 0),
                _ => (0xff, 0),
            };
            if kind != 0xff {
                lib::attack_effect(x, kind, flag);
            }
            let r = rand_and(x, 3);
            let t = x.rom8(0x94368 + r);
            x.set8(SHAKE, t);
            anim_c(x, 0);
            if !tst16(x, f::HP).gt() {
                die(x);
            }
        }
        2 => {
            if sub8(x, SHAKE, 1).le() {
                if x.u8(0x49) != 0x60 {
                    let r = rand_and(x, 3);
                    if r != 0 {
                        lib::attack_effect(x, 1, 1);
                    }
                }
                let r = rand_and(x, 3);
                let t = x.rom8(0x94368 + r);
                x.set8(SHAKE, t);
            }
            let mut d = 1u16;
            if x.u8(SHAKE) & 1 != 0 {
                d = d.wrapping_neg();
            }
            if x.u8(f::FLIP) != 0 {
                d = d.wrapping_neg();
            }
            x.add16(0x8, d);
            if var::PAUSE.get(x.b) == 0 {
                next6(x);
            }
        }
        4 => knockback_start(x, false),
        6 => knockback_hold(x),
        _ => {}
    }
}

/// 0x94564: knockback sliding, then the rest through `next_attack`.
fn pushed_back<R: Bench>(x: &mut Cx<'_, R>) {
    match x.u8(f::SUB2) {
        0 => {
            next6(x);
            x.set8(0x68, 8);
            x.set8(0x69, 8);
            let (vx, vz) = if x.u8(f::FLIP) == 0 { (0x300u16, (-0x28i16) as u16) } else { ((-0x300i16) as u16, 0x28) };
            x.set16(0x14, vx);
            x.set16(0x16, vz);
            anim_a(x, 5);
        }
        2 => {
            x.move_x();
            if (x.u8(0x14) as i8) < 0 {
                return stop(x);
            }
            if lib::step(x) {
                x.set32(0x58, 0);
                next_attack(x);
            }
        }
        _ => {}
    }
}

/// 0x945c8: on the ground (`+0x84` frames outside pause), then getting up and counterattack.
fn down<R: Bench>(x: &mut Cx<'_, R>) {
    match x.u8(f::SUB2) {
        0 => {
            next6(x);
            anim_a(x, 0x11);
        }
        2 => {
            if var::PAUSE.get(x.b) != 0 || sub16(x, DOWN_TIME, 1).gt() {
                return;
            }
            x.set16(DOWN_TIME, 0);
            next6(x);
            x.set8(0x86, 0);
            x.set8(0x49, 0);
        }
        4 => {
            if var::PAUSE.get(x.b) != 0 || !lib::step(x) {
                return;
            }
            x.set8(VULNERABLE, 1);
            x.set8(0, 1);
            x.set8(TIMER, 6);
            x.set8(0x38, 0);
            x.set32(f::STATE, state::WALK);
            anim_a(x, 0);
        }
        _ => {}
    }
}

/// 0x94638: death; the sequence depends on the hit code.
fn death<R: Bench>(x: &mut Cx<'_, R>) {
    let code = x.u8(0x49) as u32;
    if x.prog32(DEATHS + code) == DEATH_HEAVY {
        death_heavy(x);
    } else {
        death_normal(x);
    }
}

/// Notifies the stage: boss defeated (type 0x10, variant 0), score `+0x7e`; death effects.
fn announce<R: Bench>(x: &mut Cx<'_, R>) {
    x.set_a5_u8(0x42, 1);
    x.set_a5_u8(0x50, 0);
    let score = x.u8(0x7e);
    set_abs8(x, A5_DEATH, 0x10);
    set_abs8(x, A5_DEATH + 1, 0);
    set_abs8(x, A5_DEATH + 2, score);
    lib::drop_rewards(x);
    lib::flash_and_shake(x);
    crate::sound::service(x.b, crate::sound::Service::Ff05);
    lib::sound(x, crate::sound::CMD_FF);
}

/// 0x946fc: ordinary death sequence (animations 0xb, 0xc).
fn death_normal<R: Bench>(x: &mut Cx<'_, R>) {
    death_sequence(x, 0xb, 0xc);
}

/// 0x947c6: death sequence for heavy hits (animations 3, 4).
fn death_heavy<R: Bench>(x: &mut Cx<'_, R>) {
    death_sequence(x, 3, 4);
}

/// Throw (`first`), landing (`second`) then a delay of 0x28 frames.
fn death_sequence<R: Bench>(x: &mut Cx<'_, R>, first: u32, second: u32) {
    match x.u8(f::SUB2) {
        0 => {
            next6(x);
            x.set8(0, 2);
            stop(x);
            x.set16(0x18, 0x400);
            x.set16(0x1a, (-0x40i16) as u16);
            let v = if x.u8(f::FLIP) == 0 { 0x200u16 } else { (-0x200i16) as u16 };
            x.set16(0x14, v);
            anim_a(x, first);
            announce(x);
        }
        2 => {
            x.move_xz();
            if (x.u16(0xc) as i16) >= 0 {
                return;
            }
            x.set16(0xc, 0);
            next6(x);
            anim_a(x, second);
            lib::sound(x, crate::sound::CMD_FE);
        }
        4 => {
            if !lib::step(x) {
                return;
            }
            next6(x);
            x.set8(SHAKE, 0x28);
            death_wait(x);
        }
        6 => death_wait(x),
        _ => {}
    }
}

/// 0x947b8: waits for the end of the delay.
fn death_wait<R: Bench>(x: &mut Cx<'_, R>) {
    if sub8(x, SHAKE, 1).pl() {
        return;
    }
    next6(x);
}

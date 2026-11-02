//! Riyakaa, hit state (`+4 = 4`): reactions, knockback, get-up, grab by a player.

use super::*;
use crate::families::cx::Bench;
use crate::objects::Current;

// --- state 4: hit ----------------------------------------------------------------------------

pub(super) fn hit<R: Bench>(x: &mut Cx<'_, R>) {
    hit_loop(x, |x| match x.u8(f::SUB) {
        0 => react(x, REACT_STANDING.0),
        2 => knockback(x),
        4 => recover(x),
        6 => react(x, REACT_ACTING.0),
        8 => grab(x),
        _ => {}
    });
}

/// 0x6fece: thrown backward (speed +/- 0x500), then counter-attacks or resumes walking.
pub(super) fn knockback<R: Bench>(x: &mut Cx<'_, R>) {
    if paused(x) {
        return;
    }
    if x.u8(f::SUB2) == 0 {
        x.add8(f::SUB2, 2);
        x.set8(f::FREEZE, 4);
        x.set16(Enemy::VX, 0x500);
        x.set16(Enemy::AX, 0xffc8);
        lib::sound(x, crate::sound::CMD_127_227_B);
        let side = x.u8(Enemy::ATTACKER_FLIP) ^ 0x20;
        x.set8(Enemy::ATTACKER_FLIP, side);
        if side != 0 {
            let a = (x.u16(Enemy::VX) as i16).wrapping_neg();
            x.set16(Enemy::VX, a as u16);
            let b = (x.u16(Enemy::AX) as i16).wrapping_neg();
            x.set16(Enemy::AX, b as u16);
        }
        return body(x, 5);
    }
    if x.u8(Enemy::VX) != 0 {
        x.move_x();
        return;
    }
    x.set8(Enemy::ACTIVE, 1);
    x.set32(Enemy::HIT_LOCK, 0);
    let level = ((var::RANK.get(x.b) >> 2) << 2) as u32;
    let mask = tables::table(MASKS).at32(x.b, level);
    let bit = rand_and(x, 0x1f);
    if mask >> bit & 1 != 0 {
        x.set32(f::STATE, 0x0204_0600);
        lunge_start(x);
    } else {
        walk(x);
        body(x, 0);
    }
}

/// 0x6ff90: gets up after the hit (`+6`: 0 animation, 2 wait, 4 end).
pub(super) fn recover<R: Bench>(x: &mut Cx<'_, R>) {
    match x.u8(f::SUB2) {
        0 => {
            x.add8(f::SUB2, 2);
            body(x, 8);
        }
        2 => {
            if paused(x) {
                return;
            }
            let n = (x.u16(Enemy::RECOVER) as i16).wrapping_sub(1);
            x.set16(Enemy::RECOVER, n as u16);
            if n > 0 {
                return;
            }
            x.set8(Enemy::DOWNED, 0);
            x.set8(Enemy::REACTION, 0);
            x.set8(Enemy::ACTIVE, 2);
            x.add8(f::SUB2, 2);
            body(x, 8);
        }
        4 => {
            if paused(x) {
                return;
            }
            if x.anim_step() {
                x.set8(Enemy::DOWNED, 0);
                x.set8(Enemy::ACTIVE, 1);
                x.set8(Enemy::VULNERABLE, 1);
                x.set32(f::STATE, WALK);
                x.set8(Riyakaa::GAZE, 0xc0);
            }
        }
        _ => {}
    }
}

/// 0x6fd22: grabbed by a player; the escape movement depends on the scene and the orientation.
pub(super) fn grab<R: Bench>(x: &mut Cx<'_, R>) {
    if paused(x) {
        return;
    }
    match x.u8(f::SUB2) {
        0 => grab_start(x),
        2 => {
            if x.anim_step() {
                x.add8(f::SUB2, 2);
                toggle_flip(x);
                grab_run(x);
            }
        }
        4 => grab_flee(x),
        _ => {}
    }
}

pub(super) fn grab_start<R: Bench>(x: &mut Cx<'_, R>) {
    x.set8(Riyakaa::GRABBED, 1);
    let flipped = x.u8(f::FLIP) != 0;
    let turn = if var::SCENE.get(x.b) == 4 {
        flipped
    } else {
        if !lib::in_right_half(x) { flipped } else { !flipped }
    };
    if turn {
        x.add8(f::SUB2, 2);
        x.set8(Enemy::VULNERABLE, 1);
        x.set8(Riyakaa::GAZE, 0x7f);
        x.set8(Riyakaa::POSE, 0x7f);
        body(x, 4);
    } else {
        x.add8(f::SUB2, 4);
        grab_run(x);
    }
}

/// 0x6fdc0: flees running, facing the direction of the escape.
pub(super) fn grab_run<R: Bench>(x: &mut Cx<'_, R>) {
    x.set8(Enemy::VULNERABLE, 1);
    x.set8(Enemy::HEADING, 8);
    if x.u8(f::FLIP) == 0 {
        x.set8(Enemy::HEADING, 0x18);
    }
    body(x, 2);
    speed(x, 0x21);
}

/// 0x6fdf2: flees until it leaves the screen, then disappears.
pub(super) fn grab_flee<R: Bench>(x: &mut Cx<'_, R>) {
    x.anim_step();
    x.move_by_table_xy();
    if x.in_arena_default() {
        return;
    }
    x.set32(f::STATE, st::DEAD_LONG);
    detach_from_host(x);
}

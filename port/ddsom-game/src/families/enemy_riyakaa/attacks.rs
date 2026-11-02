//! Riyakaa, active state: substates 4 (paw strikes), 6 (special hits) and 8 (throws itself at a player).

use super::*;
use super::decide::needs_turn;
use crate::families::cx::Bench;
use crate::objects::Current;

// --- substate 4: paw strikes -----------------------------------------------------------------

pub(super) fn swipe<R: Bench>(x: &mut Cx<'_, R>) {
    match x.u8(f::SUB2) {
        0 | 2 | 4 => swing(x),
        6 => lunge(x),
        8 | 10 => dash(x),
        12 => wait_target(x),
        14 => turn_around(x),
        _ => {}
    }
}

/// Setting up a paw strike: attack identifier `id`, strike animation `anim`, gaze shift `shift` (`+0xa0`).
pub(super) fn swing_pose<R: Bench>(x: &mut Cx<'_, R>, id: u8, anim: u32, shift: u8) {
    x.set8(Enemy::STRIKE_KIND, id);
    x.set8(0xa0, shift);
    x.add8(Enemy::SUB_STEP, 2);
    x.anim_start(STRIKE.0, anim as u16);
}

/// 0x6f6f0: paw strike (`+6` = 0 high, 2 middle, 4 low).
pub(super) fn swing<R: Bench>(x: &mut Cx<'_, R>) {
    if x.u8(Enemy::SUB_STEP) != 0 {
        if !x.anim_step() {
            return;
        }
        if (countdown(x, 0xa2) as i8) < 0 {
            return retreat(x);
        }
        x.add8(f::SUB2, 0xfe);
        return swing_pose(x, 0xb, 1, 0xf0);
    }
    x.set8(Enemy::VULNERABLE, 0);
    x.set8(0xa2, 0);
    x.set8(Enemy::STRIKE_POWER, 5);
    let v = x.u8(0xa0);
    x.set8(0xa1, v);
    match x.u8(f::SUB2) {
        2 => swing_pose(x, 0xc, 0, 0xf2),
        0 => swing_pose(x, 0xb, 1, 0xf0),
        _ => {
            x.add8(0xa2, 1);
            swing_pose(x, 0xc, 0, 0xf4);
        }
    }
}

/// 0x6f770: leap forward then landing (`+7`: 0 run-up, 2 flight, 4 fall back, 6 rebound).
pub(super) fn lunge<R: Bench>(x: &mut Cx<'_, R>) {
    match x.u8(Enemy::SUB_STEP) {
        0 => lunge_start(x),
        2 => {
            x.move_xy();
            if x.anim_step() {
                x.add8(Enemy::SUB_STEP, 2);
                x.set16(Obj::Z, 0xc);
                x.set16(Enemy::VX, 0);
                x.set16(Enemy::AX, 0);
                strike(x, 4);
            }
        }
        4 => {
            x.move_xz();
            x.anim_step();
            if x.u16(Obj::Z) < 0xc {
                x.add8(Enemy::SUB_STEP, 2);
                body(x, 0x10);
            }
        }
        6 => {
            x.move_xz();
            x.anim_step();
            if (x.u16(Obj::Z) as i16) < 0 {
                x.set16(Obj::Z, 0);
                retreat(x);
            }
        }
        _ => {}
    }
}

/// 0x6f784: run-up of the leap (also used as a counter-attack, hit state).
pub(super) fn lunge_start<R: Bench>(x: &mut Cx<'_, R>) {
    x.add8(Enemy::SUB_STEP, 2);
    x.set8(0xac, 0x1a);
    x.set8(Enemy::STRIKE_POWER, 8);
    x.set8(Enemy::AI_SHOT, 2);
    x.set8(Enemy::VULNERABLE, 0);
    let v = x.u8(0xa0);
    x.set8(0xa1, v);
    x.set8(0xa0, 0xf6);
    x.set16(Enemy::VX, 0);
    x.set16(Enemy::VY, 0);
    x.set16(Enemy::AY, 0);
    x.set16(Enemy::VZ, 0x600);
    x.set16(Enemy::GRAVITY, 0xffa0);
    let mut d = 0xffd0u16;
    if x.u8(f::FLIP) != 0 {
        d = d.wrapping_neg();
    }
    x.set32(Enemy::RESUME_STATE, 0x0204_0600);
    x.set16(Enemy::AX, d);
    strike(x, 2);
}

/// 0x6f85e: straight-line run (`+6` = 8: then strike, 10: then walk).
pub(super) fn dash<R: Bench>(x: &mut Cx<'_, R>) {
    if x.u8(Enemy::SUB_STEP) == 0 {
        x.add8(Enemy::SUB_STEP, 2);
        x.set8(Enemy::VULNERABLE, 0);
        x.set8(0xa1, 0);
        x.set32(0xa8, 0x0204_0200);
        if x.u8(f::SUB2) != 8 {
            x.set32(0xa8, WALK);
        }
        x.set8(0xa0, 0);
        x.set8(0xa2, 0x20);
        x.set8(Enemy::HEADING, 8);
        if x.u8(f::FLIP) != 0 {
            x.set8(Enemy::HEADING, 0x18);
        }
        speed(x, 0xd);
        return body(x, 2);
    }
    x.move_by_table_xy();
    x.anim_step();
    if (countdown(x, 0xa2) as i8) < 0 {
        let s = x.u32(0xa8);
        x.set32(f::STATE, s);
    }
}

/// 0x6f8d6: waits to know where to go (`+0xa3` = substate to resume).
pub(super) fn wait_target<R: Bench>(x: &mut Cx<'_, R>) {
    if x.u8(Enemy::SUB_STEP) == 0 {
        x.add8(Enemy::SUB_STEP, 2);
        x.set8(Enemy::VULNERABLE, 1);
        lib::pick_byte(x);
        if x.u8(Enemy::AI_BYTE) == 0 {
            x.set8(Enemy::SUB_STEP, 0);
            let v = x.u8(0xa3);
            x.set8(f::SUB2, v);
        } else if x.u8(0xa1) != 1 {
            x.set8(0xa1, 1);
        } else {
            return wait_target_follow(x);
        }
        return body(x, 0);
    }
    wait_target_follow(x);
}

pub(super) fn wait_target_follow<R: Bench>(x: &mut Cx<'_, R>) {
    x.anim_step();
    if (countdown(x, 0x77) as i8) >= 0 {
        return;
    }
    x.set8(Enemy::SUB_STEP, 0);
    if needs_turn(x) {
        x.add8(f::SUB2, 2);
    } else {
        let v = x.u8(0xa3);
        x.set8(f::SUB2, v);
    }
}

/// 0x6f93e: about-turn.
pub(super) fn turn_around<R: Bench>(x: &mut Cx<'_, R>) {
    if x.u8(Enemy::SUB_STEP) == 0 {
        x.add8(Enemy::SUB_STEP, 2);
        x.set8(Enemy::VULNERABLE, 1);
        return body(x, 4);
    }
    if !x.anim_step() {
        return;
    }
    toggle_flip(x);
    x.set16(f::SUB2, 0xc00);
    body(x, 0);
}

// --- substate 6: special hits ----------------------------------------------------------------

pub(super) fn special<R: Bench>(x: &mut Cx<'_, R>) {
    match x.u8(f::SUB2) {
        0 | 10 => roar(x),
        2 | 8 => circle(x),
        4 | 6 => jump_strike(x),
        12 => wait_target(x),
        14 => turn_around(x),
        _ => {}
    }
}

/// 0x6f990: cry (if the game forbids it, a simple look).
pub(super) fn roar<R: Bench>(x: &mut Cx<'_, R>) {
    match x.u8(Enemy::SUB_STEP) {
        0 => {
            x.add8(Enemy::SUB_STEP, 2);
            x.set8(0xa1, 0x1e);
            x.set8(Enemy::VULNERABLE, 1);
            if lib::chance_10_16(x) {
                return body(x, 1);
            }
            x.add8(Enemy::SUB_STEP, 2);
            x.set8(0xa2, 0x21);
            body(x, 0xe);
        }
        2 => {
            if x.anim_step() {
                stand_up(x);
            }
        }
        4 => {
            if (countdown(x, 0xa2) as i8) < 0 {
                return stand_up(x);
            }
            x.anim_step();
            if x.u8(Enemy::KEY_FRAME) != 0 {
                toggle_flip(x);
            }
        }
        _ => {}
    }
}

/// 0x6fa0c: circles around its target (`+6` = 2: run then end; 8: run then strike).
pub(super) fn circle<R: Bench>(x: &mut Cx<'_, R>) {
    match x.u8(Enemy::SUB_STEP) {
        0 => {
            x.add8(Enemy::SUB_STEP, 2);
            x.set8(Enemy::VULNERABLE, 1);
            x.set8(Enemy::AI_WANDER, 0x18);
            x.set8(Enemy::HEADING, 0);
            if (x.random() as u32) & 1 != 0 {
                x.add8(Enemy::HEADING, 0x10);
            }
            speed(x, 0xd);
            x.set8(0xa1, 0);
            body(x, 2);
        }
        2 => {
            x.move_by_table_xy();
            x.anim_step();
            if (countdown(x, 0x6b) as i8) >= 0 {
                return;
            }
            if x.u8(f::SUB2) == 8 {
                x.set8(0xa1, 0x1d);
                x.add8(Enemy::SUB_STEP, 2);
            } else {
                stand_up(x);
            }
        }
        4 => {
            x.add8(Enemy::SUB_STEP, 2);
            x.set8(Enemy::STRIKE_KIND, 0xc);
            x.set8(Enemy::STRIKE_POWER, 5);
            strike(x, 0);
        }
        6 => {
            if x.anim_step() {
                stand_up(x);
            }
        }
        _ => {}
    }
}

/// 0x6faba: jump with a strike (`+6` = 4: lands and ends; 6: lands and chains a strike).
pub(super) fn jump_strike<R: Bench>(x: &mut Cx<'_, R>) {
    match x.u8(Enemy::SUB_STEP) {
        0 => {
            x.add8(Enemy::SUB_STEP, 2);
            x.set8(Enemy::VULNERABLE, 1);
            x.set8(0xa1, 0x15);
            x.set16(Enemy::GRAVITY, 0xffc8);
            x.set16(Enemy::VZ, 0x200);
            x.set16(Enemy::VX, 0x400);
            x.set16(Enemy::AX, 0);
            if x.u8(f::FLIP) != 0 {
                let v = (x.u16(Enemy::VX) as i16).wrapping_neg();
                x.set16(Enemy::VX, v as u16);
            }
            body(x, 9);
        }
        2 => {
            x.move_xz();
            x.anim_step();
            if (x.u32(Obj::Z) as i32) >= 0 {
                return;
            }
            x.set32(Obj::Z, 0);
            if x.u8(f::SUB2) == 6 {
                x.add8(Enemy::SUB_STEP, 2);
            } else {
                stand_up(x);
            }
        }
        4 => {
            x.add8(Enemy::SUB_STEP, 2);
            x.set8(Enemy::STRIKE_KIND, 0xb);
            x.set8(Enemy::STRIKE_POWER, 5);
            strike(x, 1);
        }
        6 => {
            if x.anim_step() {
                stand_up(x);
            }
        }
        _ => {}
    }
}

// --- substate 8: throws itself at a player --------------------------------------------------

pub(super) fn chase<R: Bench>(x: &mut Cx<'_, R>) {
    match x.u8(f::SUB2) {
        0 => chase_wait(x),
        2 | 4 => chase_run(x),
        6 => chase_pounce(x),
        _ => {}
    }
}

/// 0x6fbfc: wait of 16 frames.
pub(super) fn chase_wait<R: Bench>(x: &mut Cx<'_, R>) {
    if x.u8(Enemy::SUB_STEP) == 0 {
        x.add8(Enemy::SUB_STEP, 2);
        x.set8(Enemy::VULNERABLE, 1);
        x.set8(0xa2, 0x10);
        x.set8(0xa1, 1);
        x.set8(0xa0, 1);
        x.add8(Enemy::SUB_STEP, 2);
        return body(x, 0);
    }
    if countdown(x, 0xa2) == 0 {
        walk(x);
    }
    x.anim_step();
}

/// 0x6fc44: runs toward its target (`+6` = 4 in X, 2 in depth).
pub(super) fn chase_run<R: Bench>(x: &mut Cx<'_, R>) {
    if x.u8(Enemy::SUB_STEP) == 0 {
        x.add8(Enemy::SUB_STEP, 2);
        x.set8(Enemy::VULNERABLE, 1);
        let target = x.linked(f::OWNER);
        x.set8(0xa1, 0);
        if x.u8(f::SUB2) == 4 {
            x.set8(0xa2, 0x18);
            let d = x.u16(Obj::X).wrapping_sub(target.u16(x.b, Obj::X));
            x.set8(Enemy::HEADING, 8);
            if (d as i16) < 0 {
                x.set8(Enemy::HEADING, 0x18);
            }
        } else {
            x.set8(0xa2, 0x10);
            let d = x.u16(Obj::Y).wrapping_sub(target.u16(x.b, Obj::Y));
            x.set8(Enemy::HEADING, 0x10);
            if (d as i16) < 0 {
                x.set8(Enemy::HEADING, 0);
            }
        }
        x.set8(0xa0, 0);
        x.set8(Enemy::VULNERABLE, 1);
        x.set8(0xa1, 0);
        speed(x, 0xd);
        body(x, 2);
    }
    if countdown(x, 0xa2) == 0 {
        walk(x);
    }
    x.move_by_table_xy();
    x.anim_step();
}

/// 0x6fb78: jumps on the player it aims at (three gaze tries).
pub(super) fn chase_pounce<R: Bench>(x: &mut Cx<'_, R>) {
    x.set8(Enemy::VULNERABLE, 1);
    let v = x.u8(0xa0);
    x.set8(0xa1, v);
    let shift = if lib::target_in_box(x, -0x28, 0x28, 0xc) {
        x.set32(f::STATE, 0x0204_0c00);
        x.set8(0xa3, 6);
        0xf6u8
    } else {
        if !want_attack(x) {
            walk(x);
            x.set8(0xac, 0);
            return look_begin(x);
        }
        choose_attack(x);
        x.u8(0xa3).wrapping_add(0xf0)
    };
    x.set8(0xa0, shift);
    if shift != x.u8(0xa1) {
        x.set8(0xac, 0xff);
    }
    x.add8(0xac, 1);
    if x.u8(0xac) == 3 {
        walk(x);
        look_begin(x);
    }
}

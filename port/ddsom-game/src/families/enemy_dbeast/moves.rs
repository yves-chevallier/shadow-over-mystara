//! Type 19, the beast's movements: charges and leaps (`+5 = 8`, 0x73c92), entrance (`+5 = 10`, 0x73efe) and exit (`+5 =
//! 12`, 0x73f98).

use crate::families::cx::Bench;
use super::parts::{bound_land, bound_wait, bound_y_land, bound_y_wait, finish_to_stance};
use super::stance::engage;
use super::*;
use crate::objects::Current;

// --- substate 8: charges and leaps -------------------------------------------------------------

/// Resume after a turn-around: active state, substate 8 (charge), `+7 = 4`.
const RESUME_CHARGE: u32 = 0x0208_0004;

pub(super) fn charge<R: Bench>(x: &mut Cx<'_, R>) {
    match x.u8(f::SUB2) {
        0 | 6 => stand(x),
        2 => leap_to_player(x),
        4 => leap_over_player(x),
        _ => {}
    }
}

/// Standing rest for 16 frames before moving off again (0x73ca6).
fn stand<R: Bench>(x: &mut Cx<'_, R>) {
    match x.u8(Enemy::SUB_STEP) {
        0 => {
            x.add8(Enemy::SUB_STEP, 2);
            x.set8(TIMER, 0x10);
            stand_pose(x);
        }
        4 => {
            x.add8(Enemy::SUB_STEP, 0xfe);
            stand_pose(x);
        }
        2 => {
            let t = x.u8(TIMER).wrapping_sub(1);
            x.set8(TIMER, t);
            if (t as i8) < 0 {
                return set_state_long(x, ACTIVE_STANCE);
            }
            if player_ahead(x) {
                x.set32(RESUME_STATE, RESUME_CHARGE);
                let sub2 = x.u8(f::SUB2);
                x.set8(RESUME_SUB2, sub2);
                return set_state_long(x, 0x0202_0200);
            }
            step(x);
        }
        _ => {}
    }
}

fn stand_pose<R: Bench>(x: &mut Cx<'_, R>) {
    x.set8(WANT_ANIM, 0);
    x.set8(CUR_ANIM, 0);
    x.set8(VULNERABLE, 1);
    play(x, 0);
}

/// 0x73d3c: leap toward a player, head-on (walk) or over.
pub(super) fn jump_toward<R: Bench>(x: &mut Cx<'_, R>) {
    x.set8(VULNERABLE, 1);
    if (x.random() as u32) & 1 == 0 {
        x.add8(Enemy::SUB_STEP, 4);
        let vx = if x.u8(f::FLIP) != 0 { -0x180 } else { 0x180 };
        set_velocity(x, vx, 0x3c0, -0x38);
        x.set8(WANT_ANIM, 0x1c);
        x.set8(CUR_ANIM, 0x1c);
        return play(x, 0x11);
    }
    x.add8(Enemy::SUB_STEP, 2);
    // to the right (8) if the target is on the right, otherwise to the left (0x18)
    let right = target(x).is_some_and(|t| (t.u16(x.b, Obj::X).wrapping_sub(x.u16(Obj::X)) as i16) >= 0);
    x.set8(Obj::SPEED_INDEX, if right { 8 } else { 0x18 });
    x.set8(WANT_ANIM, 0x1d);
    x.set8(CUR_ANIM, 0x1d);
    x.set8(TIMER, 0x24);
    play(x, 0x12);
    speed(x, 0xd);
}

/// 0x73402: the leap unfolds; at the end of the delay the beast gets back on guard.
pub(super) fn jump_wait<R: Bench>(x: &mut Cx<'_, R>) {
    let t = x.u8(TIMER).wrapping_sub(1);
    x.set8(TIMER, t);
    if (t as i8) >= 0 {
        move_xy(x);
        return step(x);
    }
    back_on_guard(x);
}

/// 0x7343a: end of the leap.
pub(super) fn jump_end<R: Bench>(x: &mut Cx<'_, R>) {
    if x.anim_step() {
        back_on_guard(x);
    }
}

/// The beast lands on guard and immediately tests the range of its target (0x72c96).
fn back_on_guard<R: Bench>(x: &mut Cx<'_, R>) {
    set_state_long(x, 0x0202_0002);
    x.set8(WANT_ANIM, 0xc);
    play(x, 8);
    engage(x);
}

/// +6 = 2 (0x73d26): leap towards a player, facing.
fn leap_to_player<R: Bench>(x: &mut Cx<'_, R>) {
    match x.u8(Enemy::SUB_STEP) {
        0 => jump_toward(x),
        2 => {
            let t = x.u8(TIMER).wrapping_sub(1);
            x.set8(TIMER, t);
            if (t as i8) < 0 {
                return set_state_long(x, ACTIVE_STANCE);
            }
            move_xy(x);
            step(x);
        }
        4 => bound_wait(x),
        6 => bound_land(x),
        8 => finish_to_stance(x),
        _ => {}
    }
}

/// +6 = 4 (0x73e08): leap over a player, in depth.
fn leap_over_player<R: Bench>(x: &mut Cx<'_, R>) {
    match x.u8(Enemy::SUB_STEP) {
        0 => {
            x.set8(VULNERABLE, 1);
            if (x.random() as u32) & 1 == 0 {
                x.set8(Enemy::SUB_STEP, 4);
                let mut vy = -0x80i16;
                if let Some(t) = target(x) {
                    if (t.u16(x.b, Obj::Y).wrapping_sub(x.u16(Obj::Y)) as i16) < 0 {
                        vy = 0x80;
                    }
                }
                x.set16(Enemy::VY, vy as u16);
                x.set16(Enemy::AY, 0);
                x.set16(Enemy::VZ, 0x3c0);
                x.set16(Enemy::GRAVITY, (-0x38i16) as u16);
                x.set8(WANT_ANIM, 6);
                x.set8(CUR_ANIM, 6);
                return play(x, 6);
            }
            x.add8(Enemy::SUB_STEP, 2);
            let below = target(x).is_some_and(|t| (t.u16(x.b, Obj::Y).wrapping_sub(x.u16(Obj::Y)) as i16) >= 0);
            x.set8(Obj::SPEED_INDEX, if below { 0x10 } else { 0 });
            x.set8(WANT_ANIM, 0xc);
            x.set8(CUR_ANIM, 0xc);
            x.set8(TIMER, 0x24);
            play(x, 8);
            speed(x, 0x11);
        }
        2 => {
            let t = x.u8(TIMER).wrapping_sub(1);
            x.set8(TIMER, t);
            if (t as i8) < 0 {
                return set_state_long(x, ACTIVE_STANCE);
            }
            x.move_xy();
            step(x);
        }
        4 => bound_y_wait(x),
        6 => bound_y_land(x),
        8 => finish_to_stance(x),
        _ => {}
    }
}

// --- substate 10: entrance -------------------------------------------------------------

/// The beast falls from the top of the screen, gets up, then waits for a player to be alive.
pub(super) fn enter<R: Bench>(x: &mut Cx<'_, R>) {
    match x.u8(f::SUB2) {
        0 => {
            x.add8(f::SUB2, 2);
            x.set32(Enemy::VY_LONG, 0);
            x.set32(Enemy::VX_LONG, 0);
            x.set16(Enemy::VZ, 0x400);
            x.set16(Enemy::GRAVITY, (-0x40i16) as u16);
            play(x, 6);
        }
        2 => {
            step(x);
            x.move_xz();
            if (x.u16(Obj::Z) as i16) > 0 {
                return;
            }
            x.set32(Obj::Z, 0);
            x.set8(ENTRY_TIMER, 0xa);
            x.add8(f::SUB2, 2);
            play(x, 0);
        }
        4 => {
            if var::PLAYERS_ALIVE.get(x.b) != 0 {
                set_state_long(x, ACTIVE_STANCE);
                x.set8(WANT_ANIM, 0x7f);
                pick_target(x);
                return;
            }
            let t = x.u8(ENTRY_TIMER).wrapping_sub(1);
            x.set8(ENTRY_TIMER, t);
            if t == 0 {
                x.set16(f::SUB2, 0);
            }
        }
        _ => {}
    }
}

// --- substate 12: exit ----------------------------------------------------------------------

/// A part leaves the scene: exit animation, then an upward leap until it disappears.
pub(super) fn leave<R: Bench>(x: &mut Cx<'_, R>) {
    match x.u8(f::SUB2) {
        0 => {
            x.add8(f::SUB2, 2);
            play(x, 0x4d);
        }
        2 => {
            step(x);
            x.set8(LAYER, 3);
        }
        4 => {
            x.add8(f::SUB2, 2);
            x.set8(TIMER, 0x3c);
            let mut n = 0x14;
            if x.u8(PART) == 6 {
                x.set8(TIMER, 0x44);
                n = 0x4c;
            }
            play(x, n);
        }
        6 => {
            let t = x.u8(TIMER).wrapping_sub(1);
            x.set8(TIMER, t);
            if (t as i8) < 0 {
                x.add8(f::SUB2, 2);
                x.set16(Enemy::VZ, (-0x300i16) as u16);
                x.set16(Enemy::GRAVITY, (-0x10i16) as u16);
                return;
            }
            if t == 0xa && x.u8(PART) != 6 {
                return play(x, 0x17);
            }
            step(x);
            x.set8(LAYER, 3);
        }
        8 => {
            let (_, cam_y) = camera(x);
            let top = x.u16(Obj::Z).wrapping_add(0x80).wrapping_sub(cam_y) as i16;
            if top < 0 {
                set_state_long(x, 0x0600_0000);
            }
            x.move_z();
            step(x);
            x.set8(LAYER, 3);
        }
        _ => {}
    }
}

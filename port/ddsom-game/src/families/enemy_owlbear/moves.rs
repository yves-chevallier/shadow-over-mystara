//! Type 4, charges toward a player (substate `+5 = 8`, 0x5e7ea).

use crate::families::cx::Bench;
use super::stance::engage;
use super::*;
use crate::objects::Obj;

/// Back on guard, then test of the target's range (0x5e980).
fn back_on_guard<R: Bench>(x: &mut Cx<'_, R>) {
    set_state_long(x, ACTIVE_ENGAGE);
    body(x, 2);
    speed(x, 0xb);
    engage(x);
}

pub(super) fn run<R: Bench>(x: &mut Cx<'_, R>) {
    match x.u8(f::SUB2) {
        0 => stand(x),
        2 => dash_x(x),
        4 => dash_y(x),
        6 => back_on_guard(x),
        _ => {}
    }
}

/// Waits a moment standing (`+0xa0` frames), then the beast sets off again.
fn stand<R: Bench>(x: &mut Cx<'_, R>) {
    match x.u8(Enemy::SUB_STEP) {
        0 => {
            x.add8(Enemy::SUB_STEP, 2);
            x.set8(Owlbear::TIMER, 0x1e);
            x.set8(Enemy::VULNERABLE, 1);
            pose(x, 6, SET_BODY.0, 0);
        }
        2 => {
            let t = x.u8(Owlbear::TIMER).wrapping_sub(1);
            x.set8(Owlbear::TIMER, t);
            if t == 0 {
                set_state_long(x, ACTIVE_STANCE);
            }
            step(x);
        }
        _ => {}
    }
}

/// Run toward the target: `timer` frames, direction `angle`, animation `want`/`n`, speed `speed_n`.
fn dash_begin<R: Bench>(x: &mut Cx<'_, R>, angle: u8, timer: u8, want: u8, n: u32, speed_n: u32) {
    x.add8(Enemy::SUB_STEP, 2);
    x.set8(Obj::SPEED_INDEX, angle);
    x.set8(Owlbear::TIMER, timer);
    x.set8(Enemy::VULNERABLE, 1);
    pose(x, want, SET_BODY.0, n);
    speed(x, speed_n);
}

/// The run lasts as long as `+0xa0` does not drop to zero; then the beast gets back on its feet.
fn dash_run<'a, R: Bench>(x: &mut Cx<'a, R>, advance: fn(&mut Cx<'a, R>)) {
    let t = x.u8(Owlbear::TIMER).wrapping_sub(1);
    x.set8(Owlbear::TIMER, t);
    if t == 0 {
        return set_state_long(x, ACTIVE_STANCE);
    }
    step(x);
    advance(x);
}

/// +6 = 2 (0x5e85c): horizontal run toward the side of the target.
fn dash_x<R: Bench>(x: &mut Cx<'_, R>) {
    match x.u8(Enemy::SUB_STEP) {
        0 => {
            let right = target_dx(x).is_some_and(|dx| dx >= 0);
            dash_begin(x, if right { 8 } else { 0x18 }, 0x30, 0x16, 4, 8);
        }
        2 => dash_run(x, |x| { x.move_by_table_xy(); }),
        _ => {}
    }
}

/// +6 = 4 (0x5e8ee): run in depth toward the target, through the speed table then ballistic.
fn dash_y<R: Bench>(x: &mut Cx<'_, R>) {
    match x.u8(Enemy::SUB_STEP) {
        0 => {
            let below = target_dy(x).is_some_and(|dy| dy >= 0);
            dash_begin(x, if below { 0x10 } else { 0 }, 0x15, 4, 2, 0xb);
        }
        2 => dash_run(x, |x| { x.move_xy(); }),
        _ => {}
    }
}

//! Type 5, fallback movements (`+5 = 6`, `+6`: 0 and 2 side step, 4 and 6 leap, 8 two-stage attacks, 10 screen exit)
//! and turn-arounds chained onto an attack (`+5 = 8`, `+6`: 0 to 6).

use super::*;
use crate::families::cx::Bench;
use crate::objects::Current;
use crate::objects::Obj;

// --- substate 6 --------------------------------------------------------------------------------

pub(super) fn run_b<R: Bench>(x: &mut Cx<'_, R>) {
    if special_hit(x) {
        return;
    }
    match x.u8(f::SUB2) {
        0 => side_step(x, false),
        2 => side_step(x, true),
        4 | 6 => hop(x),
        8 => two_strikes(x),
        10 => exit_screen(x),
        _ => {}
    }
}

/// 0: step forward (animation 0 or attack 3); 2: step in depth toward a randomly drawn side.
fn side_step<R: Bench>(x: &mut Cx<'_, R>, depth: bool) {
    if x.u8(f::SUB3) == 0 {
        x.add8(f::SUB3, 2);
        look(x, 6);
        x.set8(Enemy::VULNERABLE, 1);
        if !depth {
            x.set8(Ghoul::TIMER, 0x10);
            if rand_and(x, 1) != 0 {
                return body(x, 0);
            }
            return strike(x, 3);
        }
        x.set8(Ghoul::TIMER, 0xc);
        load_target(x);
        x.set8(Obj::SPEED_INDEX, 0);
        if rand_and(x, 1) != 0 {
            x.set8(Obj::SPEED_INDEX, 0x10);
        }
        set_speed(x, 0xd);
        return body(x, 2);
    }
    let t = x.u8(Ghoul::TIMER).wrapping_sub(1);
    x.set8(Ghoul::TIMER, t);
    if t == 0 {
        set_state_long(x, STANCE);
    }
    if depth {
        move_xy(x);
    }
    step(x);
}

/// 4 and 6: small leap backward then landing (animation 3, 4, 5).
fn hop<R: Bench>(x: &mut Cx<'_, R>) {
    match x.u8(f::SUB3) {
        0 => {
            x.add8(f::SUB3, 2);
            look(x, 6);
            x.set8(Ghoul::TIMER, 4);
            x.set8(Enemy::VULNERABLE, 1);
            body(x, 3);
        }
        2 => {
            let t = x.u8(Ghoul::TIMER).wrapping_sub(1);
            x.set8(Ghoul::TIMER, t);
            if t != 0 {
                return;
            }
            x.add8(f::SUB3, 2);
            x.set16(Obj::VX, -0x200i16 as u16);
            if x.u8(f::FLIP) != 0 {
                negate(x, Obj::VX);
            }
            x.set16(Obj::VZ, 0x280);
            x.set16(Obj::AZ, -0x48i16 as u16);
            x.set16(Obj::AX, 0);
            body(x, 4);
        }
        4 => {
            if x.move_xz().gt() {
                return;
            }
            x.add8(f::SUB3, 2);
            x.set32(Obj::Z, 0);
            body(x, 5);
        }
        6 => {
            if step(x) {
                set_state_long(x, STANCE);
            }
        }
        _ => {}
    }
}

/// 8: approach, then two claw attacks in a row.
fn two_strikes<R: Bench>(x: &mut Cx<'_, R>) {
    match x.u8(f::SUB3) {
        0 => {
            x.add8(f::SUB3, 2);
            look(x, 6);
            x.set8(Enemy::VULNERABLE, 1);
            x.set8(Ghoul::TIMER, 0xc);
            load_target(x);
            x.set8(Obj::SPEED_INDEX, 0);
            if rand_and(x, 1) != 0 {
                x.set8(Obj::SPEED_INDEX, 0x10);
            }
            set_speed(x, 0xd);
            body(x, 2);
        }
        2 => {
            let t = x.u8(Ghoul::TIMER).wrapping_sub(1);
            x.set8(Ghoul::TIMER, t);
            if t == 0 {
                x.add8(f::SUB3, 2);
                x.set8(Enemy::STRIKE_KIND, 0x11);
                return strike(x, 0);
            }
            move_xy(x);
            step(x);
        }
        4 => {
            if step(x) {
                x.add8(f::SUB3, 2);
                x.set8(Enemy::STRIKE_KIND, 0x12);
                strike(x, 1);
            }
        }
        6 => {
            if step(x) {
                set_state_long(x, STANCE);
            }
        }
        _ => {}
    }
}

/// 10: the ghoul leaves the screen running toward the nearest edge.
fn exit_screen<R: Bench>(x: &mut Cx<'_, R>) {
    if x.u8(f::SUB3) == 0 {
        x.set16(Obj::VX, -0x230i16 as u16);
        x.set8(f::FLIP, 0);
        let d = var::SCROLL_X.get(x.b).wrapping_sub(x.u16(Obj::X));
        if (d as i16) >= 0xc0 {
            negate(x, Obj::VX);
            x.set8(f::FLIP, 0x20);
        }
        x.add8(f::SUB3, 2);
        return body(x, 2);
    }
    if (x.u8(f::KEY_FRAME) as i8) >= 0 {
        x.move_x();
    }
    step(x);
    let d = x.u16(Obj::X).wrapping_sub(var::SCROLL_X.get(x.b)).wrapping_add(0x10);
    if d > 0x1a0 {
        detach_from_host(x);
        set_state_long(x, st::DEAD_LONG);
    }
}

// --- substate 8 --------------------------------------------------------------------------------

pub(super) fn run_c<R: Bench>(x: &mut Cx<'_, R>) {
    if special_hit(x) {
        return;
    }
    match x.u8(f::SUB2) {
        0 => turn_then(x, Axis::None),
        2 => turn_then(x, Axis::Horizontal),
        4 => turn_then(x, Axis::Depth),
        6 => wake(x),
        _ => {}
    }
}

#[derive(Clone, Copy, PartialEq)]
enum Axis {
    None,
    Horizontal,
    Depth,
}

/// Turns toward the target if needed, then walks according to `axis` (nothing, toward it in X, in depth).
fn turn_then<R: Bench>(x: &mut Cx<'_, R>, axis: Axis) {
    match (x.u8(f::SUB3), axis) {
        (0, Axis::None) => {
            x.add8(f::SUB3, 2);
            look(x, 8);
            x.set8(Enemy::VULNERABLE, 1);
            x.set8(Ghoul::TIMER, 0x10);
            if needs_turn_around(x) {
                let flip = x.u8(f::FLIP) ^ 0x20;
                x.set8(f::FLIP, flip);
                return body(x, 1);
            }
            x.add8(f::SUB3, 2);
            body(x, 0);
        }
        (2, Axis::None) => {
            if step(x) {
                x.add8(f::SUB3, 2);
                body(x, 0);
            }
        }
        (4, Axis::None) => {
            let t = x.u8(Ghoul::TIMER).wrapping_sub(1);
            x.set8(Ghoul::TIMER, t);
            if t == 0 {
                set_state_long(x, STANCE);
            }
            step(x);
        }
        (0, _) => {
            x.add8(f::SUB3, 2);
            look(x, 8);
            x.set8(Enemy::VULNERABLE, 1);
            x.set8(Ghoul::TIMER, if axis == Axis::Horizontal { 0x18 } else { 0x10 });
            if needs_turn_around(x) {
                let flip = x.u8(f::FLIP) ^ 0x20;
                x.set8(f::FLIP, flip);
                return body(x, 1);
            }
            walk_toward(x, axis);
        }
        (2, _) => {
            if step(x) {
                walk_toward(x, axis);
            }
        }
        (4, _) => {
            let t = x.u8(Ghoul::TIMER).wrapping_sub(1);
            x.set8(Ghoul::TIMER, t);
            if t == 0 {
                set_state_long(x, STANCE);
            }
            move_xy(x);
            step(x);
        }
        _ => {}
    }
}

/// 0x60472 / 0x60512: walks toward the target, in X or in depth.
fn walk_toward<R: Bench>(x: &mut Cx<'_, R>, axis: Axis) {
    x.add8(f::SUB3, 2);
    let target = load_target(x);
    if axis == Axis::Horizontal {
        x.set8(Obj::SPEED_INDEX, 8);
        let dx = x.u16(Obj::X).wrapping_sub(target.u16(x.b, Obj::X));
        if (dx as i16) < 0 {
            x.set8(Obj::SPEED_INDEX, 0x18);
        }
    } else {
        x.set8(Obj::SPEED_INDEX, 0);
        let dy = x.u16(Obj::Y).wrapping_sub(target.u16(x.b, Obj::Y));
        if (dy as i16) < 0 {
            x.set8(Obj::SPEED_INDEX, 0x10);
        }
    }
    set_speed(x, 0xd);
    body(x, 2);
}

/// 6: turns around if needed then checks whether a player is within striking range.
fn wake<R: Bench>(x: &mut Cx<'_, R>) {
    if x.u8(f::SUB3) == 0 {
        x.add8(f::SUB3, 2);
        if needs_turn_around(x) {
            let flip = x.u8(f::FLIP) ^ 0x20;
            x.set8(f::FLIP, flip);
            return body(x, 1);
        }
    } else if !step(x) {
        return;
    }
    look(x, 8);
    x.set8(Enemy::VULNERABLE, 1);
    body(x, 0);
    if box_attack(x, 0, 0x50, 8, CLOSE_ATTACKS, 0) {
        return;
    }
    if !box_attack(x, -0x80, 0x30, 8, FAR_ATTACKS, 0xa) {
        set_state_long(x, STANCE);
    }
}

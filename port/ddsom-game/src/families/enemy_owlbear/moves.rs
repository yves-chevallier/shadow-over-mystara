//! Type 4, charges vers un joueur (sous-état `+5 = 8`, 0x5e7ea).

use super::stance::engage;
use super::*;

/// Retour en garde, puis test de la portée de la cible (0x5e980).
fn back_on_guard(x: &mut Cx) {
    set_state_long(x, ACTIVE_ENGAGE);
    body(x, 2);
    speed(x, 0xb);
    engage(x);
}

pub(super) fn run(x: &mut Cx) {
    match x.u8(f::SUB2) {
        0 => stand(x),
        2 => dash_x(x),
        4 => dash_y(x),
        6 => back_on_guard(x),
        _ => {}
    }
}

/// Attend un instant debout (`+0xa0` images), puis la bête repart.
fn stand(x: &mut Cx) {
    match x.u8(0x7) {
        0 => {
            x.add8(0x7, 2);
            x.set8(TIMER, 0x1e);
            x.set8(VULNERABLE, 1);
            pose(x, 6, SET_BODY, 0);
        }
        2 => {
            let t = x.u8(TIMER).wrapping_sub(1);
            x.set8(TIMER, t);
            if t == 0 {
                set_state_long(x, ACTIVE_STANCE);
            }
            step(x);
        }
        _ => {}
    }
}

/// Course vers la cible : `timer` images, direction `angle`, animation `want`/`n`, vitesse `speed_n`.
fn dash_begin(x: &mut Cx, angle: u8, timer: u8, want: u8, n: u32, speed_n: u32) {
    x.add8(0x7, 2);
    x.set8(ANGLE, angle);
    x.set8(TIMER, timer);
    x.set8(VULNERABLE, 1);
    pose(x, want, SET_BODY, n);
    speed(x, speed_n);
}

/// La course dure tant que `+0xa0` ne tombe pas à zéro ; ensuite la bête se remet debout.
fn dash_run(x: &mut Cx, advance: u32) {
    let t = x.u8(TIMER).wrapping_sub(1);
    x.set8(TIMER, t);
    if t == 0 {
        return set_state_long(x, ACTIVE_STANCE);
    }
    step(x);
    x.call(advance);
}

/// +6 = 2 (0x5e85c) : course horizontale vers le côté de la cible.
fn dash_x(x: &mut Cx) {
    match x.u8(0x7) {
        0 => {
            let right = target_dx(x).is_some_and(|dx| dx >= 0);
            dash_begin(x, if right { 8 } else { 0x18 }, 0x30, 0x16, 4, 8);
        }
        2 => dash_run(x, sub::MOVE_TABLE_XY),
        _ => {}
    }
}

/// +6 = 4 (0x5e8ee) : course en profondeur vers la cible, par la table de vitesses puis balistique.
fn dash_y(x: &mut Cx) {
    match x.u8(0x7) {
        0 => {
            let below = target_dy(x).is_some_and(|dy| dy >= 0);
            dash_begin(x, if below { 0x10 } else { 0 }, 0x15, 4, 2, 0xb);
        }
        2 => dash_run(x, sub::MOVE_XY),
        _ => {}
    }
}

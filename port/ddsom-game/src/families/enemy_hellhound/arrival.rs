//! Type 6, sous-état 0 : arrivée. `+6` = 0 hurlement puis course, 2 demi-tour de l'arrivée, 4 et 6 bond d'entrée,
//! 8 chute d'en haut, 10 apparition.

use super::*;

pub(super) fn run(x: &mut Cx) {
    x.c.d[0] = x.u8(f::SUB2) as u32;
    match x.u8(f::SUB2) {
        0 => howl(x),
        2 => turn_in(x),
        4 | 6 => leap_in(x),
        8 => drop_in(x),
        10 => appear(x),
        _ => {}
    }
}

/// Fin du geste d'arrivée : l'animation finie, le chien se met debout (0x616d8).
pub(super) fn stand_when_done(x: &mut Cx) {
    if !x.anim_step() {
        return;
    }
    set_state_long(x, STANCE);
    x.call(call::TURN_TO_TARGET);
    x.call(call::FACE_TARGET);
    mv(x, 0);
}

// --- +6 = 0 -----------------------------------------------------------------------------------------

fn howl(x: &mut Cx) {
    x.c.d[0] = x.u8(STEP) as u32;
    match x.u8(STEP) {
        0 => {
            x.add8(STEP, 2);
            look(x, 0x17);
            mv(x, 0x10);
        }
        2 => {
            if !pick_target(x) {
                return;
            }
            x.anim_step();
            if !target_in_box(x, 0, 0x80, 0x20) {
                return;
            }
            x.add8(STEP, 2);
            x.set8(VULNERABLE, 1);
            x.set8(LOOK, 1);
            mv(x, 1);
        }
        4 => stand_when_done(x),
        _ => {}
    }
}

// --- +6 = 2 : demi-tour ---------------------------------------------------------------------------------

fn turn_in(x: &mut Cx) {
    if grabbed(x) {
        return set_state_long(x, HELD);
    }
    x.c.d[0] = x.u8(STEP) as u32;
    match x.u8(STEP) {
        0 => {
            x.add8(STEP, 2);
            x.set8(TIMER, 0x24);
            x.set8(VULNERABLE, 1);
            x.set8(LOOK, 2);
            set_angle_run(x);
            if x.u8(LOOK) != x.u8(LOOK_PREV) {
                let l = x.u8(LOOK);
                x.set8(LOOK_PREV, l);
                mv(x, 2);
            }
            set_speed(x, 0xb);
        }
        2 => {
            if !pick_target(x) {
                return;
            }
            let n = x.u8(TIMER).wrapping_sub(1);
            x.set8(TIMER, n);
            if n != 0 {
                if target_in_box(x, 0, 0x80, 0x20) {
                    set_state_long(x, STANCE);
                    x.call(call::TURN_TO_TARGET);
                    x.call(call::FACE_TARGET);
                } else {
                    x.anim_step();
                    x.call(sub::MOVE_TABLE_XY);
                }
                return;
            }
            x.add8(STEP, 2);
            x.set8(VULNERABLE, 1);
            look(x, 0);
            mv(x, 4);
        }
        4 => {
            if !x.anim_step() {
                return;
            }
            x.set8(STEP, 0);
            let flip = x.u8(f::FLIP) ^ 0x20;
            x.set8(f::FLIP, flip);
            x.set8(VULNERABLE, 1);
            x.set8(TIMER, 0);
            mv(x, 0);
        }
        _ => {}
    }
}

/// Angle de la course d'arrivée : 0x18 vers la droite, 8 vers la gauche (0x6174c).
fn set_angle_run(x: &mut Cx) {
    set_angle_facing(x, 0x18, 8);
}

pub(super) fn set_angle_facing(x: &mut Cx, forward: u8, flipped: u8) {
    let angle = if x.u8(f::FLIP) != 0 { flipped } else { forward };
    x.set8(ANGLE, angle);
}

// --- +6 = 4 et 6 : bond d'entrée -----------------------------------------------------------------------

fn leap_in(x: &mut Cx) {
    x.c.d[0] = x.u8(STEP) as u32;
    match x.u8(STEP) {
        0 => {
            x.add8(STEP, 2);
            x.set8(VULNERABLE, 1);
            look(x, 6);
            x.set8(STRIKE_KIND, 0x2b);
            x.set16(SPEED_X, -0x280i16 as u16);
            if x.u8(f::FLIP) != 0 {
                negate(x, SPEED_X);
            }
            x.set16(SPEED_Y, 0);
            x.set16(SPEED_Z, 0x4c0);
            x.set16(GRAVITY, -0x38i16 as u16);
            mv(x, 5);
        }
        2 => wait_key(x),
        4 => land(x),
        6 => stand_when_done(x),
        _ => {}
    }
}

// --- +6 = 8 : chute d'en haut -----------------------------------------------------------------------------

fn drop_in(x: &mut Cx) {
    x.c.d[0] = x.u8(STEP) as u32;
    match x.u8(STEP) {
        0 => {
            x.add8(STEP, 2);
            x.set8(VULNERABLE, 1);
            x.set8(LOOK, 0x19);
            x.set16(SPEED_Z, -0x300i16 as u16);
            x.set16(GRAVITY, -0x80i16 as u16);
            let h = x.a5_u16(0x50a).wrapping_add(0x100).wrapping_sub(x.u16(DEPTH));
            x.set16(HEIGHT, h);
            mv(x, 0xf);
        }
        2 => {
            x.call(sub::MOVE_Z);
            if (x.u16(HEIGHT) as i16) >= 0 {
                return;
            }
            x.add8(STEP, 2);
            x.set32(HEIGHT, 0);
            mv(x, 0xc);
        }
        4 => stand_when_done(x),
        _ => {}
    }
}

// --- +6 = 10 : apparition ----------------------------------------------------------------------------------

fn appear(x: &mut Cx) {
    x.c.d[0] = x.u8(STEP) as u32;
    match x.u8(STEP) {
        0 => {
            x.add8(STEP, 2);
            x.set8(VULNERABLE, 1);
            x.set8(LOOK, 0x1a);
            x.set16(TIMER_W, 0x400);
            mv(x, 0xe);
        }
        2 => {
            let n = x.u8(TIMER_W).wrapping_sub(1);
            x.set8(TIMER_W, n);
            if n != 0 {
                x.anim_step();
                return;
            }
            x.add8(STEP, 2);
            x.set8(LOOK, 0x1b);
            mv(x, 9);
        }
        4 => stand_when_done(x),
        _ => {}
    }
}

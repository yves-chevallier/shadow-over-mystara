//! Type 18, sous-état 0 : arrivée. `+6` = 0 chute d'entrée par le bord droit, 2 apparition au sol (variante 0),
//! 4 attente d'un joueur dans la boîte (variante 1), 6 apparition cachée jusqu'à l'arrivée d'un joueur (variante 2).

use super::*;

pub(super) fn run(x: &mut Cx) {
    x.c.d[0] = x.u8(f::SUB2) as u32;
    match x.u8(f::SUB2) {
        0 => fall_in(x),
        2 => walk_in(x),
        4 => wait_for_player(x),
        6 => hidden(x),
        _ => {}
    }
}

/// Début commun d'une arrivée : plus invulnérable, immunités remises.
fn begin(x: &mut Cx) {
    x.add8(STEP, 2);
    x.set_bit(1, 1, false);
    x.set8(INVULNERABLE, 0);
    x.call(rom::IMMUNITIES);
}

// --- +6 = 0 : chute d'entrée ----------------------------------------------------------------------------

fn fall_in(x: &mut Cx) {
    x.c.d[0] = x.u8(STEP) as u32;
    match x.u8(STEP) {
        0 => {
            begin(x);
            let px = x.a5_u16(0x5a6).wrapping_add(0x1e0);
            let pz = x.a5_u16(0x5aa).wrapping_add(0x34);
            x.set16(X, px);
            x.set16(DEPTH, pz);
            x.set16(SPEED_X, -0x300i16 as u16);
            x.set16(SPEED_Y, 0);
            x.set16(SPEED_Z, 0x400);
            x.set16(GRAVITY, -0x40i16 as u16);
            x.call(sub::MOVE_XZ);
            act(x, 7);
        }
        2 => {
            lib::step(x);
            if x.u8(KEY) != 0 {
                return;
            }
            x.call(sub::MOVE_XZ);
            if (x.u16(SPEED_Z) as i16) <= 0x100 {
                x.add8(STEP, 2);
                act(x, 8);
            }
        }
        4 => {
            x.call(sub::MOVE_XZ);
            if (x.u16(SPEED_Z) as i16) <= -0x100 {
                x.add8(STEP, 2);
                act(x, 9);
            }
        }
        6 => {
            x.call(sub::MOVE_XZ);
            if (x.u16(HEIGHT) as i16) > 0 {
                return;
            }
            x.add8(STEP, 2);
            x.set16(HEIGHT, 0);
            x.set16(TIMER, 0x12);
            x.set16(TIMER2, 0);
            x.set16(SPEED_Y, 0x30);
            x.c.d[0] = 0x1e;
            x.call(0x2860);
            x.call(0xecc0a);
            act(x, 0xa);
        }
        8 => {
            if !count_down(x, TIMER2) {
                x.set16(TIMER2, 4);
                spark(x, 3);
            }
            if count_down(x, TIMER) {
                x.call(sub::MOVE_X);
                lib::step(x);
            } else {
                x.add8(STEP, 2);
                x.set16(TIMER, 0x14);
            }
        }
        10 => {
            if count_down(x, TIMER) {
                return;
            }
            x.add8(STEP, 2);
            x.call(0xe3da6);
            mv(x, 8);
        }
        12 => {
            if lib::step(x) {
                x.add8(STEP, 2);
                mv(x, 0);
            }
        }
        14 => {
            if x.a5_u8(0x2c) == 0xc {
                go::wander(x);
            } else {
                lib::step(x);
            }
        }
        _ => {}
    }
}

// --- +6 = 2 : apparition au sol ---------------------------------------------------------------------------

fn walk_in(x: &mut Cx) {
    x.c.d[0] = x.u8(STEP) as u32;
    match x.u8(STEP) {
        0 => {
            begin(x);
            x.set16(SPEED_Y, 0);
            let mut angle = 0x18;
            x.set8(f::FLIP, 0);
            if (x.u16(X) as i16) <= (x.a5_u16(0x5a6) as i16) {
                x.set8(f::FLIP, 0x20);
                angle = 8;
            }
            x.set8(ANGLE, angle);
            set_speed(x, 0x35);
            look(x, 0x31);
            mv(x, 4);
        }
        2 => {
            lib::step(x);
            if x.u8(KEY) != 2 {
                x.call(sub::MOVE_X);
                let cam = x.a5_u16(0x5a6);
                let leftward = (x.u16(SPEED_X) as i16) < 0;
                let (mut d0, mut d1) = (cam.wrapping_add(if leftward { 0xf0 } else { 0x90 }), x.u16(X));
                if leftward {
                    std::mem::swap(&mut d0, &mut d1);
                }
                if (d1 as i16) >= (d0 as i16) && x.u8(KEY) != 0 {
                    return begin_dash(x);
                }
            }
            thump(x);
        }
        4 => dash(x),
        _ => {}
    }
}

/// 0x70770 : la clé 3 de l'animation fait trembler le sol.
fn thump(x: &mut Cx) {
    if x.u8(KEY) != 3 {
        return;
    }
    x.set8(KEY, 0);
    x.call(0xecdba);
    spark(x, 4);
    x.c.d[0] = 0xa;
    x.call(call::SHOCK);
}

/// 0x70794 : début de la course (animation 5).
fn begin_dash(x: &mut Cx) {
    x.add8(STEP, 2);
    x.set16(TIMER, 0x14);
    x.set16(TIMER2, 0);
    let vy = if x.u8(f::FLIP) != 0 { -0x40i16 } else { 0x40 };
    x.set16(SPEED_Y, vy as u16);
    x.c.d[0] = 0x14;
    x.call(call::SHOCK);
    x.call(0xec976);
    x.call(0xecdba);
    mv(x, 5);
}

/// 0x707d2 : la course, puis le premier geste.
fn dash(x: &mut Cx) {
    if !count_down(x, TIMER2) {
        x.set16(TIMER2, 4);
        spark(x, 3);
    }
    if count_down(x, TIMER) {
        return x.call(sub::MOVE_X);
    }
    if lib::players_alive(x) {
        go::wander(x);
    } else {
        go::retire(x);
    }
}

// --- +6 = 4 : attente d'un joueur ------------------------------------------------------------------------

fn wait_for_player(x: &mut Cx) {
    x.c.d[0] = x.u8(STEP) as u32;
    match x.u8(STEP) {
        0 => {
            begin(x);
            look(x, 0x32);
            mv(x, 0);
        }
        2 | 4 => {
            if target_in_box(x, -0x40, 0x40, 0x40) {
                x.add8(STEP, 2);
                x.call(0xe3da6);
                mv(x, 8);
            } else {
                lib::step(x);
            }
        }
        6 => waiting_out(x),
        _ => {}
    }
}

/// 0x75c36 : fin d'attente, décompte de `+0xa0` puis retour au sous-état d'avant (`+0xb1`).
fn waiting_out(x: &mut Cx) {
    let n = x.u8(LOOK).wrapping_sub(1);
    x.set8(LOOK, n);
    if (n as i8) >= 0 {
        return;
    }
    set_state_long(x, 0x0202_0000);
    if rand_and(x, 1) != 0 {
        let m = x.u8(0xa4).wrapping_sub(1);
        x.set8(0xa4, m);
        if (m as i8) >= 0 {
            return;
        }
        x.set8(0xa4, 2);
    }
    set_state_long(x, 0x0200_0000);
    let back = x.u8(0xb1);
    x.set8(f::SUB2, back);
}

// --- +6 = 6 : apparition cachée -----------------------------------------------------------------------------

fn hidden(x: &mut Cx) {
    x.c.d[0] = x.u8(STEP) as u32;
    match x.u8(STEP) {
        0 => {
            x.add8(STEP, 2);
            x.set8(NO_GRAB, 1);
            x.set_bit(1, 1, false);
            x.set8(INVULNERABLE, 0);
            x.call(rom::IMMUNITIES);
            look(x, 0x33);
            mv(x, 0xa);
        }
        2 => {
            if target_in_box(x, -0x20, 0x60, 0x20) {
                x.add8(STEP, 2);
                x.set8(NO_GRAB, 0);
            }
        }
        4 => {
            if lib::step(x) {
                go::stand(x);
            }
        }
        _ => {}
    }
}

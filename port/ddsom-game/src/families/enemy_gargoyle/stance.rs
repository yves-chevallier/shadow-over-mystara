//! Type 8, sous-état 2 : déplacements. `+6` = 0 poursuite lente, 2 demi-tour, 4 et 6 vol de poursuite (deux
//! cadences), 8 plongeon vers la cible, 10 saut avec retombée.

use super::*;
use crate::objects::Courant;

pub(super) fn run(x: &mut Cx) {
    x.c.d[0] = x.u8(f::SUB2) as u32;
    match x.u8(f::SUB2) {
        0 => pursue(x),
        2 => turn(x),
        4 => glide(x, false),
        6 => glide(x, true),
        8 => plunge(x),
        10 => leap(x),
        _ => {}
    }
}

/// Arrête le geste si un joueur saisit l'ennemi ou si plus personne n'est en vie ; vrai si l'état a changé.
fn interrupted(x: &mut Cx) -> bool {
    if grabbed(x) {
        go::held(x);
        return true;
    }
    if !lib::players_alive(x) {
        go::retire(x);
        return true;
    }
    false
}

/// Animation de vol en avant (3) ou en arrière (4) selon le sens de la vitesse (0x644f4).
fn fly_forward_or_back(x: &mut Cx) {
    let n = if lib::moving_forward(x) { 3 } else { 4 };
    fly(x, n);
}

/// Le jeu d'animations de vol correspondant au sens de la vitesse, à la même image (0x645b8).
fn switch_flight_set(x: &mut Cx) {
    let table = if lib::moving_forward(x) { SET_FLY_ALONG } else { SET_FLY_AGAINST };
    lib::switch_keeping_time(x, table);
}

/// 0x6563c / 0x65648 : cadence de poursuite lue selon la difficulté.
fn charge_a(x: &mut Cx) {
    let level = x.a5_u16(0x7f92) as u32;
    x.c.d[0] = x.c.d[0] & !0xffff | level;
    let v = x.rom8(0x65654 + level);
    x.set8(CHARGE, v);
}

fn charge_b(x: &mut Cx) {
    let level = x.a5_u16(0x7f92) as u32;
    x.c.d[0] = x.c.d[0] & !0xffff | level;
    let v = x.rom8(0x65674 + level);
    x.set8(CHARGE, v);
}

/// 0x65694 / 0x656a6 : nombre de passages avant de changer de geste.
fn wander_a(x: &mut Cx) {
    let r = rand_and(x, 0x1f);
    let v = x.rom8(0x656b8 + r);
    x.set8(WANDER, v);
}

fn wander_b(x: &mut Cx) {
    let r = rand_and(x, 0x1f);
    let v = x.rom8(0x656d8 + r);
    x.set8(WANDER, v);
}

// --- +6 = 0 : poursuite lente ---------------------------------------------------------------------

fn pursue(x: &mut Cx) {
    if interrupted(x) {
        return;
    }
    if x.u8(STEP) == 0 {
        x.add8(STEP, 2);
        x.call(0x79f6);
        x.c.d[0] &= !0xff;
        x.set8(LOOK, 0);
        if x.u8(LOOK_PREV) != 0 {
            x.set8(LOOK_PREV, 0);
            fly(x, 0);
        }
        return;
    }
    if lib::outside(x, -0x40, 0x1c0) {
        return go::dive(x);
    }
    let n = x.u8(TURNS).wrapping_sub(1);
    x.set8(TURNS, n);
    if (n as i8) <= 0 {
        return go::wander(x);
    }
    if !lib::facing_target(x) {
        return go::turn_around(x);
    }
    lib::step(x);
}

// --- +6 = 2 : demi-tour ---------------------------------------------------------------------------

fn turn(x: &mut Cx) {
    if x.u8(STEP) == 0 {
        x.add8(STEP, 2);
        let flip = x.u8(f::FLIP) ^ 0x20;
        x.set8(f::FLIP, flip);
        x.set8(LOOK, 1);
        return fly(x, 2);
    }
    if !lib::step(x) {
        return;
    }
    if interrupted(x) {
        return;
    }
    let saved = x.u32(SAVED_STATE);
    x.set32(f::STATE, saved);
    let prev = x.u8(LOOK_PREV);
    if prev == 2 || prev == 3 {
        x.set8(LOOK, prev);
        return fly_forward_or_back(x);
    }
    fly(x, 0);
}

// --- +6 = 4 et 6 : vol de poursuite -----------------------------------------------------------------

/// `second` : la variante 6 (`+0xa0 = 3`, cadence B), sinon la variante 4 (`+0xa0 = 2`, cadence A).
fn glide(x: &mut Cx, second: bool) {
    if interrupted(x) {
        return;
    }
    if x.u8(STEP) == 0 {
        if second {
            wander_b(x);
            charge_b(x);
            x.set8(LOOK, 3);
        } else {
            wander_a(x);
            charge_a(x);
            x.set8(LOOK, 2);
        }
        return begin_glide(x);
    }
    glide_step(x);
}

/// 0x644b8 : début du vol.
fn begin_glide(x: &mut Cx) {
    x.add8(STEP, 2);
    x.set16(SPEED_Y, 0);
    x.set16(SPEED_Y2, 0);
    if !pick_target(x) {
        return go::retire(x);
    }
    x.call(call::FLAP);
    set_speed(x, 0xd);
    let prev = x.u8(LOOK_PREV);
    let look = x.u8(LOOK);
    x.set8(LOOK_PREV, look);
    if prev == 2 || prev == 3 {
        switch_flight_set(x);
    } else {
        fly_forward_or_back(x);
    }
}

/// 0x6452c : un pas de vol ; change de geste quand les compteurs sont épuisés ou qu'un joueur est à portée.
fn glide_step(x: &mut Cx) {
    let n = x.u8(CHARGE).wrapping_sub(1);
    x.set8(CHARGE, n);
    if (n as i8) < 0 {
        let w = x.u8(WANDER).wrapping_sub(1);
        x.set8(WANDER, w);
        if (w as i8) <= 0 {
            return go::wander_far(x);
        }
        x.call(call::FLAP);
        set_speed(x, 0xd);
        if x.u8(LOOK) == 3 {
            charge_b(x);
        } else {
            charge_a(x);
        }
        return switch_flight_set(x);
    }
    if lib::outside(x, -0x40, 0x1c0) {
        return go::dive(x);
    }
    let near = near_box(x, -0x90, 0x10, 6);
    if near != 0 {
        return pick_b(x, near);
    }
    if x.u8(LOOK) == 3 {
        let close = near_box(x, -0x3a, 0x12, 3);
        if close != 0 {
            return pick_a2(x, close);
        }
    }
    if target_in_box(x, -0x10, 0x14, 8) {
        return pick_a(x);
    }
    if !lib::facing_target(x) {
        return go::turn_around(x);
    }
    if (x.u8(KEY) as i8) >= 0 {
        x.move_by_table_xy();
    }
    lib::step(x);
}

/// 0x656f8 / 0x65716 : un joueur dans la boîte, puis la décision de l'IA (0, 1 ou 2).
fn near_box(x: &mut Cx, d0: i16, d1: u16, d2: u16) -> u32 {
    if !target_in_box(x, d0, d1, d2) {
        return 0;
    }
    lib::ai_gate(x)
}

/// 0x6575a : choisit le geste d'attaque suivant (table par palier de PV `+0xaa`).
fn pick_a(x: &mut Cx) {
    let r = rand_and(x, 0xf);
    let table = [0x65792, 0x657a2, 0x657b2][x.u8(PHASE).min(2) as usize];
    let v = x.rom8(table + r);
    x.c.d[0] = x.c.d[0] & !0xff | v as u32;
    match v {
        0 => go::strike_0(x, false),
        2 => go::strike_0(x, true),
        4 => go::strike_8(x),
        _ => {}
    }
}

/// 0x657c8 : comme `pick_a`, mais selon le résultat `gate` de la décision de l'IA (1 : devant, 2 : derrière).
fn pick_a2(x: &mut Cx, gate: u32) {
    x.c.d[2] = x.c.d[2] & !0xffff | gate;
    let r = rand_and(x, 0xf);
    let table = if gate == 1 { 0x65822 } else { 0x65832 };
    let v = x.rom8(table + r);
    x.c.d[0] = x.c.d[0] & !0xff | v as u32;
    match v {
        0 => go::strike_2(x, false),
        2 => go::strike_2(x, true),
        4 => go::strike_4(x, false),
        _ => {}
    }
}

/// 0x65848 : tire un geste de repli (table par palier `+0xaa`).
fn pick_b(x: &mut Cx, gate: u32) {
    x.c.d[2] = x.c.d[2] & !0xffff | gate;
    let r = rand_and(x, 0xf);
    let table = 0x658a2 + 0x10 * x.u8(PHASE).min(2) as u32;
    let v = x.rom8(table + r);
    x.c.d[0] = x.c.d[0] & !0xff | v as u32;
    match v {
        0 => go::strike_10(x),
        2 => go::strike_6(x),
        4 => go::strike_12(x),
        _ => {}
    }
}

// --- +6 = 8 : plongeon -------------------------------------------------------------------------------

fn plunge(x: &mut Cx) {
    x.c.d[0] = x.u8(STEP) as u32;
    match x.u8(STEP) {
        0 => {
            x.add8(STEP, 2);
            x.set16(SPEED_Y, 0);
            let mut angle = 0x18;
            if x.u8(SIDE_TURN) != 0 {
                let target = load_target(x);
                let px = target.u16(x.b, X);
                x.set16(GOAL_X, px);
                if x.u8(f::FLIP) != 0 {
                    angle = 8;
                }
            } else {
                x.set8(f::FLIP, 0);
                let cam = camera_x(x);
                set_dw(x, 2, cam);
                if (x.u16(X) as i16) <= (cam as i16) {
                    x.set8(f::FLIP, 0x20);
                    angle = 8;
                }
            }
            x.set8(ANGLE, angle);
            set_speed(x, 0x2b);
            look(x, 4);
            fly(x, 1);
        }
        2 => {
            if (x.u16(HEIGHT) as i16) >= 0x14 && x.u8(KEY) == 4 {
                x.add8(STEP, 2);
                x.set16(TIMER, 2);
            } else {
                rise(x);
            }
            lib::step(x);
        }
        4 => {
            lib::step(x);
            bob(x);
            if count_down(x, TIMER) {
                return;
            }
            x.add8(STEP, 2);
            x.call(0xe3776);
            fly(x, 5);
        }
        6 => {
            x.move_x();
            let (mut d0, mut d1);
            let leftward = (x.u16(SPEED_X) as i16) < 0;
            if x.u8(SIDE_TURN) == 0 {
                let cam = camera_x(x);
                d0 = cam.wrapping_add(if leftward { 0xf0 } else { 0x90 });
                d1 = x.u16(X);
            } else {
                d0 = x.u16(GOAL_X);
                d1 = x.u16(X);
            }
            if leftward {
                std::mem::swap(&mut d0, &mut d1);
            }
            set_dw(x, 0, d0);
            set_dw(x, 1, d1);
            if (d1 as i16) < (d0 as i16) {
                return;
            }
            x.add8(STEP, 2);
            let vy = if x.u8(f::FLIP) != 0 { -0x10i16 } else { 0x10 };
            x.set16(SPEED_Y, vy as u16);
            x.set16(SPEED_Z, 0);
            x.set16(GRAVITY, -0x40i16 as u16);
        }
        8 => {
            x.move_xz();
            if (x.u16(HEIGHT) as i16) > 0 {
                return;
            }
            x.add8(STEP, 2);
            x.set16(TIMER, 0x14);
            x.set16(TIMER2, 0);
            x.set16(HEIGHT, 0);
            let vy = if x.u8(f::FLIP) != 0 { -0x10i16 } else { 0x10 };
            x.set16(SPEED_Y, vy as u16);
            fly(x, 6);
        }
        10 => {
            if !count_down(x, TIMER2) {
                x.set16(TIMER2, 4);
                spark(x, 8);
            }
            x.move_x();
            if count_down(x, TIMER) {
                lib::step(x);
            } else if !interrupted(x) {
                go::wander(x);
            }
        }
        _ => {}
    }
}

// --- +6 = 10 : saut ----------------------------------------------------------------------------------

fn leap(x: &mut Cx) {
    x.c.d[0] = x.u8(STEP) as u32;
    match x.u8(STEP) {
        0 => {
            x.add8(STEP, 2);
            look(x, 5);
            fly(x, 9);
        }
        2 => {
            lib::step(x);
            if x.u8(KEY) == 0 {
                return;
            }
            x.add8(STEP, 2);
            let (angle, vy) = if x.u8(f::FLIP) != 0 { (0x18, 0x40i16) } else { (8, -0x40i16) };
            x.set8(ANGLE, angle);
            x.set16(SPEED_Y, vy as u16);
            lib::jitter_angle(x);
            set_speed(x, 0x3f);
            x.set16(SPEED_Z, 0x380);
            x.set16(GRAVITY, -0x28i16 as u16);
            x.move_xz();
        }
        4 => {
            x.move_xz();
            if (x.u16(SPEED_Z) as i16) > 0xc0 {
                return;
            }
            x.add8(STEP, 2);
            x.set8(INVULNERABLE, 0);
            x.set16(SPEED_Y, 0);
        }
        6 => {
            x.move_xz();
            if (x.u16(HEIGHT) as i16) > 0 {
                lib::step(x);
                return;
            }
            x.add8(STEP, 2);
            x.set16(TIMER, 0x12);
            x.set16(TIMER2, 0);
            x.set16(HEIGHT, 0);
            let vy = if x.u8(f::FLIP) != 0 { 0x20i16 } else { -0x20 };
            x.set16(SPEED_Y, vy as u16);
            x.move_x();
            fly(x, 0xa);
        }
        8 => {
            if !count_down(x, TIMER2) {
                x.set16(TIMER2, 4);
                spark(x, 8);
            }
            if count_down(x, TIMER) {
                x.move_x();
                lib::step(x);
            } else if !interrupted(x) {
                go::wander(x);
            }
        }
        _ => {}
    }
}

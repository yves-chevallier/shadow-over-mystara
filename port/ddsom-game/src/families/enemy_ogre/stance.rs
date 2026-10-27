//! Type 18, sous-état 2 : déplacements. `+6` : 0 poursuite lente, 2 demi-tour, 4 et 6 vol de poursuite,
//! 8 plongeon vers la cible, 10 bond qui lance un bras (objet de type 0x2a, variante 0).

use super::*;
use crate::objects::pools;
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

/// Un joueur qui saisit l'ogre ou la fin de la partie interrompt le geste ; vrai si l'état a changé.
fn interrupted(x: &mut Cx) -> bool {
    held_by_player(x) || nobody_left(x)
}

/// Animation de vol 2 ou 3 selon le sens de la vitesse (0x70a94).
fn fly_forward_or_back(x: &mut Cx) {
    let n = if lib::moving_forward(x) { 2 } else { 3 };
    play(x, SET_A, n);
}

/// Le jeu de vol correspondant au sens de la vitesse, à la même image (0x70b6e).
fn switch_flight_set(x: &mut Cx) {
    let table = if lib::moving_forward(x) { SET_FLY_ALONG } else { SET_FLY_AGAINST };
    lib::switch_keeping_time(x, table);
}

fn charge_a(x: &mut Cx) {
    let level = x.a5_u16(0x7f92) as u32;
    let v = x.rom8(0x719f6 + level);
    x.set8(CHARGE, v);
}

fn charge_b(x: &mut Cx) {
    let level = x.a5_u16(0x7f92) as u32;
    let v = x.rom8(0x71a16 + level);
    x.set8(CHARGE, v);
}

fn wander_a(x: &mut Cx) {
    let r = rand_and(x, 0x1f);
    let v = x.rom8(0x71a5a + r);
    x.set8(WANDER, v);
}

fn wander_b(x: &mut Cx) {
    let r = rand_and(x, 0x1f);
    let v = x.rom8(0x71a7a + r);
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
            mv(x, 0);
        }
        return;
    }
    if lib::outside(x, -0x50, 0x1d0) {
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
        return mv(x, 1);
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
    mv(x, 0);
}

// --- +6 = 4 et 6 : vol de poursuite -----------------------------------------------------------------

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
    if lib::outside(x, -0x50, 0x1d0) {
        return go::dive(x);
    }
    if x.u8(LOOK) == 3 {
        let g = near_box(x, 0x71a9a);
        if g != 0 {
            return pick_b(x, g);
        }
        let g = near_box(x, 0x71ab8);
        if g != 0 {
            return pick_c(x);
        }
    }
    if target_in_box(x, -0x19, 0x18, 0xa) {
        return pick_a2(x);
    }
    if target_in_box_close(x, -0x80, 0x10, 8) {
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

/// 0x7ef4 : comme `target_in_box`, pour l'ogre.
fn target_in_box_close(x: &mut Cx, d0: i16, d1: u16, d2: u16) -> bool {
    set_dw(x, 0, d0 as u16);
    set_dw(x, 1, d1);
    set_dw(x, 2, d2);
    x.call(0x7ef4);
    !x.c.cond(7)
}

/// 0x71a9a / 0x71ab8 : un joueur dans la boîte `(−0x60, 0x30, 6)` ou `(−0xc0, 0x20, 8)`, puis la décision de l'IA.
fn near_box(x: &mut Cx, which: u32) -> u32 {
    let found = if which == 0x71a9a { target_in_box(x, -0x60, 0x30, 6) } else { target_in_box(x, -0xc0, 0x20, 8) };
    if !found {
        return 0;
    }
    lib::ai_gate(x)
}

/// 0x71ad4 : cercle autour du joueur (la table de choix n'a qu'une entrée).
fn pick_a(x: &mut Cx) {
    let r = rand_and(x, 0xf);
    let _ = x.rom8(0x71aea + r);
    go::leap(x);
}

/// 0x71afc : choisit un geste de la série 0x71b34 (plongeon de côté, coup 6 ou 8).
fn pick_a2(x: &mut Cx) {
    let r = rand_and(x, 0xf);
    let v = x.rom8(0x71b34 + r);
    match v {
        0 => go::dive_side(x),
        2 => strike_6_quiet(x),
        4 => go::strike_8(x),
        _ => {}
    }
}

/// 0x71d62 : coup 6 sans lever l'invulnérabilité.
fn strike_6_quiet(x: &mut Cx) {
    x.set8(VULNERABLE, 0);
    set_state_long(x, 0x0204_0600);
}

/// 0x71b4a : geste de la série devant, par palier `+0xaa` (tables 0x71ba4, 0x71bb4, 0x71bc4).
fn pick_b(x: &mut Cx, gate: u32) {
    let r = rand_and(x, 0xf);
    let phase = x.u8(PHASE).min(2) as usize;
    // décision 1 (cible devant) : une table par palier ; décision 2 : toujours la première
    let table = if gate == 1 { [0x71ba4u32, 0x71bb4, 0x71bc4][phase] } else { 0x71ba4 };
    let v = x.rom8(table + r);
    match v {
        0 => go::strike_0(x),
        2 => go::dive_side(x),
        _ => {}
    }
}

/// 0x71bd8 : geste de la série derrière.
fn pick_c(x: &mut Cx) {
    let r = rand_and(x, 0xf);
    let phase = x.u8(PHASE).min(2) as usize;
    let table = [0x71c32u32, 0x71c42, 0x71c52][phase];
    let v = x.rom8(table + r);
    match v {
        0 => go::dive_side(x),
        2 => go::strike_2(x),
        4 => go::strike_4(x),
        6 => strike_6_quiet(x),
        8 => {
            x.set8(VULNERABLE, 0);
            set_state_long(x, 0x0204_0800);
        }
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
                let cam = x.a5_u16(0x5a6);
                if (x.u16(X) as i16) <= (cam as i16) {
                    x.set8(f::FLIP, 0x20);
                    angle = 8;
                }
            }
            x.set8(ANGLE, angle);
            set_speed(x, 0x35);
            look(x, 4);
            act_a(x, 4);
        }
        2 => {
            lib::step(x);
            if x.u8(KEY) != 2 {
                x.move_x();
                let leftward = (x.u16(SPEED_X) as i16) < 0;
                let (mut d0, mut d1);
                if x.u8(SIDE_TURN) == 0 {
                    d0 = camera_x(x).wrapping_add(if leftward { 0xf0 } else { 0x90 });
                    d1 = x.u16(X);
                } else {
                    d0 = x.u16(GOAL_X);
                    d1 = x.u16(X);
                }
                if leftward {
                    std::mem::swap(&mut d0, &mut d1);
                }
                if (d1 as i16) >= (d0 as i16) && x.u8(KEY) != 0 {
                    return plunge_dash(x);
                }
            }
            if x.u8(KEY) == 3 {
                x.set8(KEY, 0);
                x.call(0xecdba);
                spark(x, 4);
                x.c.d[0] = 0xa;
                x.call(call::SHOCK);
            }
        }
        4 => {
            if !count_down(x, TIMER2) {
                x.set16(TIMER2, 4);
                spark(x, 3);
            }
            if count_down(x, TIMER) {
                x.move_x();
                return;
            }
            if !interrupted(x) {
                go::wander(x);
            }
        }
        _ => {}
    }
}

fn act_a(x: &mut Cx, n: u32) {
    mv(x, n);
}

/// 0x70c6c : début de la course du plongeon.
fn plunge_dash(x: &mut Cx) {
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

// --- +6 = 10 : bond et lancer de bras ----------------------------------------------------------------------

fn leap(x: &mut Cx) {
    x.c.d[0] = x.u8(STEP) as u32;
    match x.u8(STEP) {
        0 => {
            x.add8(STEP, 2);
            look(x, 5);
            mv(x, 6);
        }
        2 => {
            lib::step(x);
            if x.u8(KEY) != 0 {
                return;
            }
            x.add8(STEP, 2);
            let Some(arm) = x.alloc(&pools::ESHL) else { return end_leap(x) };
            x.set16(ARM, arm.0 as u16);
            arm.set_u8(x.b, 0, 1);
            arm.set_u8(x.b, 2, 0x2a);
            arm.set_u16(x.b, f::CREATOR, x.o.0 as u16);
            let target = x.u16(f::TARGET);
            arm.set_u16(x.b, f::TARGET, target);
            let flip = x.u8(f::FLIP);
            arm.set_u8(x.b, f::FLIP, flip);
            let (angle, vy) = if flip != 0 { (0x18, 0x20i16) } else { (8, -0x20) };
            x.set8(ANGLE, angle);
            x.set16(SPEED_Y, vy as u16);
            lib::jitter_angle(x);
            set_speed(x, 0x35);
            x.set16(SPEED_Z, 0x300);
            x.set16(GRAVITY, -0x40i16 as u16);
            x.move_xz();
        }
        4 => {
            x.move_xz();
            if (x.u16(SPEED_Z) as i16) <= 0 {
                x.add8(STEP, 2);
                x.set16(SPEED_Y, 0);
            }
        }
        6 => {
            x.move_xz();
            if (x.u16(HEIGHT) as i16) > 0 {
                lib::step(x);
                return;
            }
            x.add8(STEP, 2);
            x.set16(TIMER, 0x14);
            x.set16(TIMER2, 0);
            x.set16(HEIGHT, 0);
            let vy = if x.u8(f::FLIP) != 0 { 0x40i16 } else { -0x40 };
            x.set16(SPEED_Y, vy as u16);
            x.move_x();
            let arm = x.linked(ARM);
            arm.set_u8(x.b, 0x58, 1);
            mv(x, 7);
        }
        8 => {
            if !count_down(x, TIMER2) {
                x.set16(TIMER2, 4);
                spark(x, 3);
            }
            if count_down(x, TIMER) {
                x.move_x();
                lib::step(x);
            } else {
                end_leap(x);
            }
        }
        _ => {}
    }
}

/// 0x70e1c : fin du bond : retraite, saisie ou nouveau geste.
fn end_leap(x: &mut Cx) {
    if !interrupted(x) {
        go::wander(x);
    }
}

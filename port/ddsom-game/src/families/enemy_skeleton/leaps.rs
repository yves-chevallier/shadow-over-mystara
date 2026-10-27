//! Type 1, bonds et gardes. `+5 = 6` : `+6` = 0 repos, 2 bond de côté, 4 et 6 bonds vers l'avant,
//! 8 garde (série 1), 10 sortie d'écran. `+5 = 8` : 0 et 6 pause, 2 glissade, 4 pas de côté ou
//! bond de profondeur. `+5 = 10` : chute d'arrivée.

use super::attacks::{fall, land_wait, ready, rise, restart_slide, settle};
use super::*;
use crate::objects::Courant;
use crate::objects::Obj;

// --- sous-état 6 --------------------------------------------------------------------------------

pub(super) fn run(x: &mut Cx) {
    if special_hit(x) {
        return;
    }
    match x.u8(f::SUB2) {
        0 => rest(x),
        2 => hop_sideways(x),
        4 => hop_forward(x, false),
        6 => hop_forward(x, true),
        8 => guard_leap(x),
        10 => exit_screen(x),
        _ => {}
    }
    zone_and_terrain(x);
}

/// Repos de 16 images, puis debout.
fn rest(x: &mut Cx) {
    if x.u8(f::SUB3) == 0 {
        x.add8(f::SUB3, 2);
        x.set16(TIMER, 0x10);
        return body(x, 0);
    }
    let t = x.u16(TIMER).wrapping_sub(1);
    x.set16(TIMER, t);
    if t == 0 {
        set_state_long(x, STANCE);
    }
}

/// Bond de côté (`+6` = 2) : vitesse latérale au hasard.
fn hop_sideways(x: &mut Cx) {
    match x.u8(f::SUB3) {
        0 => ready(x),
        2 => settle(x),
        4 => {
            if x.u8(SERIES) != 0 {
                return guard_windup(x);
            }
            x.add8(f::SUB3, 2);
            x.set16(Obj::VY, 0x200);
            if rand_and(x, 1) != 0 {
                negate(x, Obj::VY);
            }
            x.set16(Obj::AY, 0);
            x.set16(Obj::VZ, 0x400);
            x.set16(Obj::AZ, -0x80i16 as u16);
            body(x, 3);
        }
        6 => land_wait(x),
        8 => hop_rise(x),
        10 => hop_fall(x),
        12 => hop_end(x),
        _ => {}
    }
}

/// Bonds vers l'avant (`+6` = 4 avec animation de mouvement, 6 avec animation de coup).
fn hop_forward(x: &mut Cx, striking: bool) {
    match x.u8(f::SUB3) {
        0 => ready(x),
        2 => settle(x),
        4 => {
            if !striking && x.u8(SERIES) != 0 {
                x.set8(f::SUB2, 8);
                return guard_windup_random(x);
            }
            x.add8(f::SUB3, 2);
            x.set16(Obj::VX, 0x200);
            if x.u8(f::FLIP) != 0 {
                negate(x, Obj::VX);
            }
            x.set16(Obj::AX, 0);
            x.set16(Obj::VZ, 0x800);
            x.set16(Obj::AZ, -0x80i16 as u16);
            if striking {
                strike(x, 2)
            } else {
                body(x, 3)
            }
        }
        6 => land_wait(x),
        8 if striking => rise(x),
        8 => {
            step(x);
            x.move_xz();
            if (x.u16(Obj::VZ) as i16) < 0 {
                x.add8(f::SUB3, 2);
                x.set16(ANIM_COUNT, 1);
            }
        }
        10 if striking => fall(x),
        10 => {
            step(x);
            x.move_xz();
            if (x.u32(Obj::Z) as i32) <= 0 {
                x.add8(f::SUB3, 2);
                x.set32(Obj::Z, 0);
                x.set16(ANIM_COUNT, 1);
            }
        }
        12 if striking => hop_end_byte(x),
        12 => hop_end(x),
        _ => {}
    }
}

/// Montée d'un bond de côté (déplacement en profondeur et en hauteur).
fn hop_rise(x: &mut Cx) {
    step(x);
    x.move_yz();
    if (x.u16(Obj::VZ) as i16) < 0 {
        x.add8(f::SUB3, 2);
        x.set16(ANIM_COUNT, 1);
    }
}

fn hop_fall(x: &mut Cx) {
    step(x);
    x.move_yz();
    if (x.u32(Obj::Z) as i32) <= 0 {
        x.add8(f::SUB3, 2);
        x.set32(Obj::Z, 0);
        x.set16(ANIM_COUNT, 1);
    }
}

/// Fin d'un bond : l'animation de réception terminée, retour debout.
fn hop_end(x: &mut Cx) {
    step(x);
    if (x.u16(ANIM_STATE) as i16) < 0 {
        set_state_long(x, STANCE);
    }
}

/// Fin d'un bond avec animation de coup : le test porte sur l'octet fort de l'état.
fn hop_end_byte(x: &mut Cx) {
    step(x);
    if (x.u8(ANIM_STATE) as i8) < 0 {
        set_state_long(x, STANCE);
    }
}

// --- gardes -------------------------------------------------------------------------------------

/// Garde du coup 18/20 (`+5 = 4`) : prépare la riposte `+0xa9 = 2` ; termine par le choix d'un nouveau coup.
pub(super) fn guard(x: &mut Cx) {
    match x.u8(f::SUB3) {
        0 => ready(x),
        2 => settle(x),
        4 => {
            x.set16(COUNTER, 0x100);
            x.set8(COUNTER_MOVE, 2);
            guard_start(x);
        }
        6 => guard_wait(x),
        8 => {
            if step(x) {
                end_attack(x);
            }
        }
        _ => {}
    }
}

/// Garde du saut de la série 1 (`+5 = 6`, `+6` = 8).
fn guard_leap(x: &mut Cx) {
    match x.u8(f::SUB3) {
        0 => ready(x),
        2 => settle(x),
        4 => guard_windup_random(x),
        6 => guard_wait(x),
        8 => {
            step(x);
            if (x.u16(ANIM_STATE) as i16) < 0 {
                set_state_long(x, STANCE);
            }
        }
        _ => {}
    }
}

/// 0x58966 / 0x58974 : commence la garde depuis un bond (`+6` = 8, riposte aléatoire ou aucune).
fn guard_windup(x: &mut Cx) {
    x.set16(COUNTER, 0);
    x.set8(f::SUB2, 8);
    guard_start(x);
}

/// 0x5897a : riposte tirée au hasard (0 ou 2), puis la garde.
fn guard_windup_random(x: &mut Cx) {
    let n = rand_and(x, 1) * 2;
    x.set8(COUNTER_MOVE, n as u8);
    x.set8(COUNTER, 0xff);
    guard_start(x);
}

/// 0x58990 : la garde dure `0x3c` images, la posture passe à « en garde ».
fn guard_start(x: &mut Cx) {
    x.add8(f::SUB3, 2);
    x.set16(TIMER, 0x3c);
    x.set8(POSTURE, 9);
    body(x, 8);
}

/// 0x589ae : tient la garde, puis la relâche.
fn guard_wait(x: &mut Cx) {
    if x.u8(f::KEY_FRAME) != 0 {
        let t = x.u16(TIMER).wrapping_sub(1);
        x.set16(TIMER, t);
        if (t as i16) >= 0 {
            return;
        }
        x.add8(f::SUB3, 2);
        x.set16(ANIM_COUNT, 1);
        let series = x.u8(SERIES) as i8 as i16 as u16 as u32;
        let v = x.rom8(0x589da + series);
        x.set8(POSTURE, v);
    }
    step(x);
}

// --- sortie d'écran -----------------------------------------------------------------------------

/// Le squelette s'enfuit vers le bord le plus proche (`+6` = 10).
fn exit_screen(x: &mut Cx) {
    if x.u8(f::SUB3) == 0 {
        x.set16(Obj::VX, -0x230i16 as u16);
        x.set8(f::FLIP, 0);
        let d = var::SCROLL_X.get(x.b).wrapping_sub(x.u16(Obj::X));
        if d as i16 >= 0xc0 {
            negate(x, Obj::VX);
            x.set8(f::FLIP, 0x20);
        }
        x.add8(f::SUB3, 2);
        return body(x, 0);
    }
    if (x.u8(f::KEY_FRAME) as i8) >= 0 {
        x.move_x();
    }
    step(x);
    let d = x.u16(Obj::X).wrapping_sub(var::SCROLL_X.get(x.b)).wrapping_add(0x10);
    if d > 0x1a0 {
        detach_from_host(x);
        set_state_long(x, 0x0600_0000);
    }
}

// --- sous-état 8 : esquives ---------------------------------------------------------------------

pub(super) fn run_dash(x: &mut Cx) {
    if special_hit(x) {
        return;
    }
    match x.u8(f::SUB2) {
        0 | 6 => pause(x),
        2 => run_aside(x),
        4 => dodge(x),
        _ => {}
    }
    zone_and_terrain(x);
}

/// Pause de 30 images.
fn pause(x: &mut Cx) {
    if x.u8(f::SUB3) == 0 {
        x.add8(f::SUB3, 2);
        x.set16(TIMER, 0x1e);
        return body(x, 0);
    }
    let t = x.u16(TIMER).wrapping_sub(1);
    x.set16(TIMER, t);
    if t == 0 {
        set_state_long(x, STANCE);
    }
}

/// Course de côté : même animation que la glissade, pendant 0x20 images.
fn run_aside(x: &mut Cx) {
    if x.u8(f::SUB3) == 0 {
        x.add8(f::SUB3, 2);
        restart_slide(x);
        x.set16(TIMER, 0x20);
        return;
    }
    step(x);
    if (x.u8(f::KEY_FRAME) as i8) < 0 {
        return;
    }
    let t = x.u16(TIMER).wrapping_sub(1);
    x.set16(TIMER, t);
    if t != 0 {
        move_xy(x);
        let d = x.u16(SAVED_POS).wrapping_sub(x.u16(Obj::X)).wrapping_add(0x30);
        if d < 0x60 {
            return;
        }
    }
    set_state_long(x, STANCE);
}

/// Esquive en profondeur (`+6` = 4) : course vers l'avant ou bond selon un tirage.
fn dodge(x: &mut Cx) {
    match x.u8(f::SUB3) {
        0 => {
            x.add8(f::SUB3, 2);
            let mut angle = 0;
            x.c.d[1] = 0;
            if x.u16(f::TARGET) != 0 {
                let target = load_target(x);
                let depth = x.u16(Obj::Y);
                set_dw(x, 0, depth);
                if depth < target.u16(x.b, Obj::Y) {
                    angle = 0x10;
                    x.c.d[1] = 0x10;
                }
            }
            x.set8(Obj::SPEED_INDEX, angle);
            if random(x) & 1 != 0 {
                x.set8(f::SUB3, 4);
                x.set16(Obj::VY, 0x300);
                if angle != 0 {
                    negate(x, Obj::VY);
                }
                x.set16(Obj::AY, 0);
                x.set16(Obj::VZ, 0x400);
                x.set16(Obj::AZ, -0x80i16 as u16);
                body(x, 3);
            } else {
                x.set16(TIMER, 0x16);
                let y = x.u16(Obj::Y);
                x.set16(SAVED_POS, y);
                x.set32(MOVE_TABLE, 0x1c74b6);
                body(x, 0);
            }
        }
        2 => {
            step(x);
            if (x.u8(f::KEY_FRAME) as i8) < 0 {
                return;
            }
            let t = x.u16(TIMER).wrapping_sub(1);
            x.set16(TIMER, t);
            if t != 0 {
                move_xy(x);
                let d = x.u16(SAVED_POS).wrapping_sub(x.u16(Obj::Y)).wrapping_add(0x20);
                if d < 0x40 {
                    return;
                }
            }
            set_state_long(x, STANCE);
        }
        4 => land_wait(x),
        6 => hop_rise(x),
        8 => hop_fall(x),
        10 => hop_end(x),
        _ => {}
    }
}

// --- sous-état 10 : chute d'arrivée -------------------------------------------------------------

pub(super) fn run_land(x: &mut Cx) {
    land(x);
    zone_and_terrain(x);
}

fn land(x: &mut Cx) {
    match x.u8(f::SUB2) {
        0 => {
            x.add8(f::SUB2, 2);
            x.set32(Obj::VX, 0);
            x.set32(Obj::VY, 0);
            x.set16(Obj::VZ, 0x400);
            x.set16(Obj::AZ, -0x40i16 as u16);
            body(x, 3);
        }
        2 => {
            step(x);
            x.move_xz();
            if (x.u16(Obj::VZ) as i16) <= 0 {
                x.add8(f::SUB2, 2);
                x.set16(ANIM_COUNT, 1);
            }
        }
        4 => {
            step(x);
            x.move_xz();
            if (x.u16(Obj::Z) as i16) <= 0 {
                x.set32(Obj::Z, 0);
                x.add8(f::SUB2, 2);
                x.set16(ANIM_COUNT, 1);
            }
        }
        6 => {
            if step(x) {
                x.set16(f::SUB2, 0);
                if var::PLAYERS_ALIVE.get(x.b) != 0 {
                    set_state_long(x, STANCE);
                    pick_target(x);
                }
            }
        }
        _ => {}
    }
}

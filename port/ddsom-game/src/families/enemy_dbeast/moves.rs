//! Type 19, déplacements de la bête : charges et bonds (`+5 = 8`, 0x73c92), entrée en scène
//! (`+5 = 10`, 0x73efe) et sortie (`+5 = 12`, 0x73f98).

use super::parts::{bound_land, bound_wait, bound_y_land, bound_y_wait, finish_to_stance};
use super::stance::engage;
use super::*;
use crate::objects::Courant;

// --- sous-état 8 : charges et bonds -------------------------------------------------------------

/// Reprise après un demi-tour : état actif, sous-état 8 (charge), `+7 = 4`.
const RESUME_CHARGE: u32 = 0x0208_0004;

pub(super) fn charge(x: &mut Cx) {
    match x.u8(f::SUB2) {
        0 | 6 => stand(x),
        2 => leap_to_player(x),
        4 => leap_over_player(x),
        _ => {}
    }
}

/// Repos debout pendant 16 images avant de repartir (0x73ca6).
fn stand(x: &mut Cx) {
    match x.u8(0x7) {
        0 => {
            x.add8(0x7, 2);
            x.set8(TIMER, 0x10);
            stand_pose(x);
        }
        4 => {
            x.add8(0x7, 0xfe);
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

fn stand_pose(x: &mut Cx) {
    x.set8(WANT_ANIM, 0);
    x.set8(CUR_ANIM, 0);
    x.set8(VULNERABLE, 1);
    play(x, 0);
}

/// 0x73d3c : bond vers un joueur, de face (marche) ou par-dessus.
pub(super) fn jump_toward(x: &mut Cx) {
    x.set8(VULNERABLE, 1);
    if random(x) & 1 == 0 {
        x.add8(0x7, 4);
        let vx = if x.u8(f::FLIP) != 0 { -0x180 } else { 0x180 };
        set_velocity(x, vx, 0x3c0, -0x38);
        x.set8(WANT_ANIM, 0x1c);
        x.set8(CUR_ANIM, 0x1c);
        return play(x, 0x11);
    }
    x.add8(0x7, 2);
    // vers la droite (8) si la cible est à droite, sinon vers la gauche (0x18)
    let right = target(x).is_some_and(|t| (t.u16(x.b, 0x8).wrapping_sub(x.u16(0x8)) as i16) >= 0);
    x.set8(ANGLE, if right { 8 } else { 0x18 });
    x.set8(WANT_ANIM, 0x1d);
    x.set8(CUR_ANIM, 0x1d);
    x.set8(TIMER, 0x24);
    play(x, 0x12);
    speed(x, 0xd);
}

/// 0x73402 : le bond se déroule ; à la fin du délai la bête se remet en garde.
pub(super) fn jump_wait(x: &mut Cx) {
    let t = x.u8(TIMER).wrapping_sub(1);
    x.set8(TIMER, t);
    if (t as i8) >= 0 {
        move_xy(x);
        return step(x);
    }
    back_on_guard(x);
}

/// 0x7343a : fin du bond.
pub(super) fn jump_end(x: &mut Cx) {
    if x.anim_step() {
        back_on_guard(x);
    }
}

/// La bête retombe en garde et teste aussitôt la portée de sa cible (0x72c96).
fn back_on_guard(x: &mut Cx) {
    set_state_long(x, 0x0202_0002);
    x.set8(WANT_ANIM, 0xc);
    play(x, 8);
    engage(x);
}

/// +6 = 2 (0x73d26) : bond vers un joueur, de face.
fn leap_to_player(x: &mut Cx) {
    match x.u8(0x7) {
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

/// +6 = 4 (0x73e08) : bond vers un joueur par-dessus, en profondeur.
fn leap_over_player(x: &mut Cx) {
    match x.u8(0x7) {
        0 => {
            x.set8(VULNERABLE, 1);
            if random(x) & 1 == 0 {
                x.set8(0x7, 4);
                let mut vy = -0x80i16;
                if let Some(t) = target(x) {
                    if (t.u16(x.b, 0x10).wrapping_sub(x.u16(0x10)) as i16) < 0 {
                        vy = 0x80;
                    }
                }
                x.set16(0x1c, vy as u16);
                x.set16(0x1e, 0);
                x.set16(0x18, 0x3c0);
                x.set16(0x1a, (-0x38i16) as u16);
                x.set8(WANT_ANIM, 6);
                x.set8(CUR_ANIM, 6);
                return play(x, 6);
            }
            x.add8(0x7, 2);
            let below = target(x).is_some_and(|t| (t.u16(x.b, 0x10).wrapping_sub(x.u16(0x10)) as i16) >= 0);
            x.set8(ANGLE, if below { 0x10 } else { 0 });
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

// --- sous-état 10 : entrée en scène -------------------------------------------------------------

/// La bête tombe du haut de l'écran, se relève, puis attend qu'un joueur soit vivant.
pub(super) fn enter(x: &mut Cx) {
    match x.u8(f::SUB2) {
        0 => {
            x.add8(f::SUB2, 2);
            x.set32(0x1c, 0);
            x.set32(0x14, 0);
            x.set16(0x18, 0x400);
            x.set16(0x1a, (-0x40i16) as u16);
            play(x, 6);
        }
        2 => {
            step(x);
            x.move_xz();
            if (x.u16(0xc) as i16) > 0 {
                return;
            }
            x.set32(0xc, 0);
            x.set8(ENTRY_TIMER, 0xa);
            x.add8(f::SUB2, 2);
            play(x, 0);
        }
        4 => {
            if x.a5_u8(0x35) != 0 {
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

// --- sous-état 12 : sortie ----------------------------------------------------------------------

/// Une pièce quitte la scène : animation de sortie, puis bond vers le haut jusqu'à disparaître.
pub(super) fn leave(x: &mut Cx) {
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
                x.set16(0x18, (-0x300i16) as u16);
                x.set16(0x1a, (-0x10i16) as u16);
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
            let top = x.u16(0xc).wrapping_add(0x80).wrapping_sub(cam_y) as i16;
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

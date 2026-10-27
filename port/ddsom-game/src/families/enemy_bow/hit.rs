//! Type 3, état touché (`+4 = 4`, 0x5caa0). `+5` : 0 et 2 réaction par table (`+0x49`), 4 chute et relevé, 6 seconde
//! table de réactions, 8 saisie par un joueur (le Bow Gnoll s'enfuit à pied). Zone et ombre sont testées sauf si
//! `+0xac` est non nul.

use super::*;
use crate::objects::Courant;
use crate::objects::Obj;

/// Tables de réactions communes aux coups, indexées par le code `+0x49`.
const REACT_FIRST: u32 = 0x5cbe8;
const REACT_SECOND: u32 = 0x5cd14;

pub(super) fn run(x: &mut Cx) {
    let freeze = x.u8(f::FREEZE) as i8;
    if freeze > 0 {
        x.set8(f::FREEZE, (freeze - 1) as u8);
    } else {
        x.c.d[0] = x.u8(f::SUB) as u32;
        match x.u8(f::SUB) {
            0 | 2 => react(x, REACT_FIRST),
            4 => fall(x),
            6 => react(x, REACT_SECOND),
            8 => grab(x),
            _ => {}
        }
    }
    if x.u8(NO_ZONE) == 0 {
        zone_and_terrain(x);
    }
    x.reinsert();
}

/// Chute puis relevé (0x5cc94).
fn fall(x: &mut Cx) {
    x.c.d[0] = x.u8(f::SUB2) as u32;
    match x.u8(f::SUB2) {
        0 => {
            x.add8(f::SUB2, 2);
            mv(x, 0xe);
        }
        2 => {
            if paused(x) {
                return;
            }
            let n = x.u16(0x84).wrapping_sub(1);
            x.set16(0x84, n);
            if (n as i16) > 0 {
                return;
            }
            x.set8(0x86, 0);
            x.set8(0x49, 0);
            x.set8(0, 2);
            x.add8(f::SUB2, 2);
            mv(x, 7);
        }
        4 => {
            if paused(x) {
                return;
            }
            if x.anim_step() {
                x.set8(0, 1);
                set_state_long(x, STANCE);
                x.set8(0, 1);
                x.set8(0x86, 0);
            }
        }
        _ => {}
    }
}

/// 0x5cae0 : saisi par un joueur : le Bow Gnoll fait demi-tour vers le bord de l'écran le plus proche (selon la
/// caméra `A5+0x34` et la moitié de l'écran), s'enfuit puis disparaît.
pub(super) fn grab(x: &mut Cx) {
    if paused(x) {
        return;
    }
    x.c.d[0] = x.u8(f::SUB2) as u32;
    match x.u8(f::SUB2) {
        0 => {
            x.set8(NO_ZONE, 1);
            let flipped = x.u8(f::FLIP) != 0;
            // `turn` : tourne d'abord (animation 3) ; sinon part tout de suite
            let turn = if x.a5_u8(0x34) == 4 {
                flipped
            } else {
                x.call(call::ON_RIGHT_HALF);
                if x.c.cond(7) {
                    flipped
                } else {
                    !flipped
                }
            };
            if turn {
                x.add8(f::SUB2, 2);
                x.set8(VULNERABLE, 1);
                return mv(x, 3);
            }
            x.add8(f::SUB2, 4);
            run_away(x);
        }
        2 => {
            if !x.anim_step() {
                return;
            }
            x.add8(f::SUB2, 2);
            let flip = x.u8(f::FLIP) ^ 0x20;
            x.set8(f::FLIP, flip);
            run_away(x);
        }
        4 => {
            x.anim_step();
            x.move_by_table_xy();
            if x.in_arena_default() {
                return;
            }
            if (x.u8(TIMED) as i8) < 0 && x.u16(f::HOST) != 0 {
                let host = x.linked(f::HOST);
                host.set_u8(x.b, Obj::SPEED_INDEX, 1);
            }
            set_state_long(x, st::DEAD_LONG);
            detach_from_host(x);
        }
        _ => {}
    }
}

/// 0x5cb70 : part en courant vers le bord (animation 2, vitesse 0x21).
fn run_away(x: &mut Cx) {
    x.set8(VULNERABLE, 1);
    walk_angle(x);
    mv(x, 2);
    x.select_speed(&crate::objects::motion::SPEEDS_B, 0x21);
}

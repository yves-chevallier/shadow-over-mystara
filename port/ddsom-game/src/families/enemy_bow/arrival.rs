//! Type 3, sous-état 0 : arrivée selon la variante `+3` (valeurs paires). 0 et 2 marche d'entrée (de plus en plus
//! vite), 4 chute depuis le haut, 6 apparition sur place (saisie à la première image), 8 course.

use super::stance::stance_begin;
use super::*;
use crate::objects::Courant;
use crate::objects::Obj;

pub(super) fn run(x: &mut Cx) {
    match x.u8(Obj::VARIANT) {
        0 => walk_in(x, 0x40, 0xd),
        2 => walk_in(x, 0x20, 0x17),
        4 => fall_in(x),
        6 => appear(x),
        8 => run_in(x),
        _ => {}
    }
}

/// Fin d'une marche d'entrée : à la fin de la minuterie `+0xa4`, debout et premier choix (0x5c17e).
fn end_of_walk(x: &mut Cx) {
    let n = x.u8(TIMER).wrapping_sub(1);
    x.set8(TIMER, n);
    if n == 0 {
        x.set8(f::SUB, 2);
        x.set16(f::SUB2, 0);
        return stance_begin(x);
    }
    x.move_by_table_xy();
    x.anim_step();
}

/// Variantes 0 (0x5ca68) et 2 (0x5c9ca) : marche `timer` images à la vitesse `speed`. Aussi appelée par le sous-état 4
/// quand le Bow Gnoll sort de l'écran (variante 2).
pub(super) fn walk_in(x: &mut Cx, timer: u8, speed: u32) {
    if x.u8(f::SUB2) != 0 {
        return end_of_walk(x);
    }
    x.add8(f::SUB2, 2);
    x.set8(TIMER, timer);
    walk_angle(x);
    set_speed(x, speed);
    mv(x, 2);
}

/// Variante 4 : chute depuis le haut (`+0x18` = −4 ajouté à la hauteur à chaque image).
fn fall_in(x: &mut Cx) {
    if x.u8(f::SUB2) == 0 {
        x.add8(f::SUB2, 2);
        x.set16(Obj::VZ, -4i16 as u16);
        x.set8(f::FLIP, 0);
        return mv(x, 0xf);
    }
    x.anim_step();
    let h = x.u16(Obj::Z).wrapping_add(x.u16(Obj::VZ));
    x.set16(Obj::Z, h);
    if (h as i16) < 0 {
        x.set8(f::SUB, 2);
        x.set16(f::SUB2, 0);
        x.set16(Obj::Z, 0);
    }
}

/// Variante 6 : apparition sur place, attend un joueur dans la boîte (0, 0x80, 0x20).
fn appear(x: &mut Cx) {
    if grabbed(x) {
        set_state_long(x, HELD);
        return;
    }
    match x.u8(f::SUB2) {
        0 => {
            x.add8(f::SUB2, 2);
            mv(x, 0xd);
        }
        2 => {
            x.anim_step();
            if !pick_target(x) {
                return;
            }
            if lib::target_in_box(x, 0, 0x80, 0x20) {
                x.add8(f::SUB2, 2);
            }
        }
        4 => {
            x.add8(f::SUB2, 2);
            mv(x, 0xc);
        }
        6 => {
            if x.anim_step() {
                set_state_long(x, STANCE);
            }
        }
        _ => {}
    }
}

/// Variante 8 : course d'entrée.
fn run_in(x: &mut Cx) {
    if x.u8(f::SUB2) == 0 {
        x.add8(f::SUB2, 2);
        x.set8(TIMER, 0x20);
        x.set8(Obj::SPEED_INDEX, 0xf);
        set_speed(x, 0xd);
        return mv(x, 5);
    }
    end_of_walk(x);
}

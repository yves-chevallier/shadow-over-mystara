//! Type 6, sous-états 4 à 12.
//!
//! * 4 : coups (`+6` = 0 course et morsure, 2 et 4 morsure en place, 6 coup de patte, 8 bond et morsure,
//!   10 à 14 coup de patte court, 16 souffle de feu) ;
//! * 6 : gestes d'affût (`+6` = 0 à 12) ;
//! * 8 : gestes de repli (`+6` = 0, 2, 4, 6) ;
//! * 10 : promenade tant qu'aucun joueur n'est en vie ;
//! * 12 : morsure d'un joueur terrassé.

use crate::families::cx::Banc;
use super::arrival::stand_when_done;
use super::*;
use crate::objects::Courant;
use crate::objects::Obj;

/// Table de la ROM : durée et clé de tir des flammes du souffle.
const FLAME_SCRIPT: u32 = 0x62284;

/// Effet d'une morsure ou d'un coup (0x9eba6 puis 0xebd30) ; la clé d'animation est remise à zéro.
fn bite_effect(x: &mut Cx, kind: u32) {
    x.set8(f::KEY_FRAME, 0);
    x.c.d[0] = kind;
    crate::families::eshl_creators::shot_6(&mut crate::families::effects_creators::restes::Pur, x.b, x.o, kind as u8);
    lib::sound(x, 0xebd30);
}

/// 0x61c88 : prépare l'attente (aspect 0, animation 0).
fn begin_wait<R: Banc>(x: &mut Cx<'_, R>) {
    x.add8(f::SUB3, 2);
    x.set8(VULNERABLE, 1);
    look(x, 0);
    mv(x, 0);
}

/// 0x61fa0 / 0x61f94 : poursuite en place jusqu'à la fin du compteur `+0x77`, avec demi-tour si la cible change
/// de côté ; `resume` est l'état à reprendre après le demi-tour.
fn hold<R: Banc>(x: &mut Cx<'_, R>, resume: u32) {
    x.set32(RESUME_STATE, resume);
    let n = x.u8(AI_LUNGE).wrapping_sub(1);
    x.set8(AI_LUNGE, n);
    if (n as i8) < 0 {
        return x.add8(f::SUB3, 2);
    }
    if must_turn(x) {
        let sub2 = x.u8(f::SUB2);
        x.set8(RESUME_SUB2, sub2);
        set_state_long(x, 0x0202_0200);
    }
    x.anim_step();
}

/// 0x61eb0 : fin de l'animation d'un coup → suite (0x63790).
fn step_then_after(x: &mut Cx) {
    if x.anim_step() {
        after_strike(x);
    }
}

/// Fin d'animation d'un coup : animation de marche, puis suite (0x63790).
fn done_then_after(x: &mut Cx) {
    if x.anim_step() {
        mv(x, 0);
        after_strike(x);
    }
}

/// Attend la clé d'animation puis passe à l'étape suivante, sans remettre la clé à zéro (0x61ec0 remet
/// `+0x33` à zéro, pas 0x6244a).
fn wait_key_clear<R: Banc>(x: &mut Cx<'_, R>) {
    if x.u8(f::KEY_FRAME) != 0 {
        x.set8(f::KEY_FRAME, 0);
        x.add8(f::SUB3, 2);
        x.set8(VULNERABLE, 0);
    }
    x.anim_step();
}

/// 0x61eda : en plein bond, la clé déclenche la morsure ; à terre, poussière et animation d'atterrissage.
fn bite_land(x: &mut Cx) {
    if x.u8(f::KEY_FRAME) != 0 {
        let Some(player) = lib::attack_hits_a_player(x, 4) else { return step_then_after(x) };
        x.set32(Obj::Z, 0);
        return lib::seize_player(x, player, 2);
    }
    if x.move_xz().gt() {
        x.anim_step();
        return;
    }
    x.set32(Obj::Z, 0);
    x.add8(f::SUB3, 2);
    x.set16(0x30, 1);
    x.set8(VULNERABLE, 1);
    mv(x, 0xc);
    let vy = if x.u8(f::FLIP) != 0 { -0x1c } else { 0x1c };
    x.set16(Obj::AX, vy as u16);
    x.anim_step();
}

/// 0x61f3c : le chien glisse en soufflant de la poussière jusqu'à l'arrêt.
fn slide(x: &mut Cx) {
    puff(x, 3, 1, 1, -0x18, -1);
    if x.u8(Obj::VX) == 0 {
        x.add8(f::SUB3, 2);
        x.set32(Obj::VX, 0);
    } else {
        x.move_x();
    }
}

/// Début d'un bond d'attaque : vitesse horizontale `vx` (inversée si l'ennemi regarde à gauche).
fn begin_jump<R: Banc>(x: &mut Cx<'_, R>, vx: i16, speed_z: u16, anim: u32, set: u32) {
    x.set16(Obj::VX, vx as u16);
    if x.u8(f::FLIP) != 0 {
        negate(x, Obj::VX);
    }
    x.set16(Obj::AX, 0);
    x.set16(Obj::VZ, speed_z);
    x.set16(Obj::AZ, -0x38i16 as u16);
    x.anim_start(set, anim as u16);
}

// --- sous-état 4 ----------------------------------------------------------------------------------------

pub(super) fn run(x: &mut Cx) {
    match x.u8(f::SUB2) {
        0 => lunge(x),
        2 | 4 => bite(x),
        6 => paw(x),
        8 => jump_bite(x),
        10 | 12 | 14 => short_paw(x),
        16 => breath(x),
        _ => {}
    }
}

/// `+6` = 0 : course, saut éventuel puis morsure.
fn lunge(x: &mut Cx) {
    x.c.d[0] = x.u8(f::SUB3) as u32;
    match x.u8(f::SUB3) {
        0 => begin_wait(x),
        2 => hold(x, 0x0204_0000),
        4 => {
            let r = rand_and(x, 1);
            if r == 0 {
                x.add8(f::SUB3, 2);
                strike(x, 0);
            } else {
                x.add8(f::SUB3, 4);
                x.set8(VULNERABLE, 1);
                x.set8(LOOK, 8);
                begin_jump(x, -0x280, 0x4c0, 7, SET_STRIKE);
            }
        }
        6 => {
            if x.u8(f::KEY_FRAME) != 0 {
                lib::sound(x, 0xe361a);
                if let Some(player) = lib::attack_hits_a_player(x, 3) {
                    return lib::seize_player(x, player, 0);
                }
            }
            step_then_after(x);
        }
        8 => wait_key_clear(x),
        10 => bite_land(x),
        12 => slide(x),
        14 => step_then_after(x),
        _ => {}
    }
}

/// `+6` = 2 et 4 : morsure en place.
fn bite(x: &mut Cx) {
    match x.u8(f::SUB3) {
        0 => begin_wait(x),
        2 => hold(x, 0x0204_0000),
        4 => {
            x.add8(f::SUB3, 2);
            strike(x, 3);
        }
        6 => {
            if x.u8(f::KEY_FRAME) != 0 {
                bite_effect(x, 0);
            }
            done_then_after(x);
        }
        _ => {}
    }
}

/// `+6` = 6 : coup de patte, avec élan ou en courant (deux variantes tirées au hasard).
fn paw(x: &mut Cx) {
    x.c.d[0] = x.u8(f::SUB3) as u32;
    match x.u8(f::SUB3) {
        0 => {
            x.set8(VULNERABLE, 1);
            let r = rand_and(x, 1);
            if r == 0 {
                x.add8(f::SUB3, 4);
                look(x, 0xff);
                x.set8(STRIKE_KIND, 0x2b);
                return begin_jump(x, 0x180, 0x3c0, 5, SET_MOVE);
            }
            x.add8(f::SUB3, 2);
            let mut angle = 0x18;
            if x.u16(f::TARGET) != 0 {
                let target = x.linked(f::TARGET);
                let d = target.u16(x.b, Obj::X).wrapping_sub(x.u16(Obj::X)) as i16;
                if d >= 0 {
                    angle = 8;
                }
            }
            x.set8(Obj::SPEED_INDEX, angle);
            look(x, 0x1d);
            x.set8(TIMER, 0x40);
            mv(x, 0xa);
            set_speed(x, 0xd);
        }
        2 => {
            let n = x.u8(TIMER).wrapping_sub(1);
            x.set8(TIMER, n);
            if (n as i8) >= 0 {
                x.move_by_table_xy();
                x.anim_step();
            } else {
                back_on_guard(x);
            }
        }
        4 => wait_key(x),
        6 => land(x),
        8 => {
            if x.anim_step() {
                back_on_guard(x);
            }
        }
        _ => {}
    }
}

/// Le geste fini, le chien reprend l'approche (état 2, `+6` = 0, étape 2) et revérifie les portées (0x61aa4).
fn back_on_guard(x: &mut Cx) {
    set_state_long(x, 0x0202_0002);
    mv(x, 2);
    stance::reach_boxes(x);
}

/// `+6` = 8 : bond en avant puis morsure.
fn jump_bite(x: &mut Cx) {
    match x.u8(f::SUB3) {
        0 => begin_wait(x),
        2 => hold(x, 0x0204_0000),
        4 => {
            x.add8(f::SUB3, 2);
            x.set8(VULNERABLE, 1);
            x.set8(LOOK, 8);
            begin_jump(x, -0x380, 0x4c0, 7, SET_STRIKE);
        }
        6 => wait_key_clear(x),
        8 => bite_land(x),
        10 => slide(x),
        12 => step_then_after(x),
        _ => {}
    }
}

/// `+6` = 10, 12, 14 : coup de patte court.
fn short_paw(x: &mut Cx) {
    match x.u8(f::SUB3) {
        0 => begin_wait(x),
        2 => hold(x, 0x0204_0000),
        4 => {
            x.add8(f::SUB3, 2);
            strike(x, 9);
        }
        6 => {
            if x.u8(f::KEY_FRAME) != 0 {
                bite_effect(x, 1);
            }
            done_then_after(x);
        }
        _ => {}
    }
}

/// `+6` = 16 : souffle de feu (animation 8 puis une série de tirs d'après la table).
fn breath(x: &mut Cx) {
    match x.u8(f::SUB3) {
        0 => begin_wait(x),
        2 => hold(x, 0x0204_0000),
        4 => {
            x.add8(f::SUB3, 2);
            strike(x, 8);
        }
        6 => flame_start(x),
        8 => {
            flame_tick(x, true);
            done_then_after(x);
        }
        _ => {}
    }
}

/// 0x6220e : la clé d'animation lance la série de flammes.
fn flame_start<R: Banc>(x: &mut Cx<'_, R>) {
    if x.u8(f::KEY_FRAME) != 0 {
        x.set8(TIMER, 1);
        x.set8(SCRIPT, 0);
        x.add8(f::SUB3, 2);
    } else {
        x.anim_step();
    }
}

/// Décompte de la série de flammes ; à zéro, tire la durée suivante dans la table ; `both` : deux effets par tir
/// (sous-état 4) au lieu d'un seul (sous-état 6). Rend faux quand la série est finie (0x62266 dans l'original).
fn flame_tick<R: Banc>(x: &mut Cx<'_, R>, both: bool) -> bool {
    let n = x.u8(TIMER).wrapping_sub(1);
    x.set8(TIMER, n);
    if n != 0 {
        return true;
    }
    let idx = 2 * x.u8(SCRIPT) as u32;
    let t = x.data8(FLAME_SCRIPT + idx);
    x.set8(TIMER, t);
    let v = x.data8(FLAME_SCRIPT + idx + 1);
    x.set8(0x43, v);
    x.add8(SCRIPT, 1);
    if (x.u8(SCRIPT) as i8) > 0xc {
        return false;
    }
    if both {
        lib::sound(x, 0xed398);
    }
    crate::families::eshl_creators::sweeping_8(&mut crate::families::effects_creators::restes::Pur, x.b, x.o);
    true
}

// --- sous-état 6 -------------------------------------------------------------------------------------------

pub(super) fn run_b(x: &mut Cx) {
    // étapes communes : 0 attente, 2 poursuite en place (retour à l'état 6)
    match x.u8(f::SUB3) {
        0 => return begin_wait(x),
        2 => return hold(x, 0x0206_0000),
        _ => {}
    }
    match x.u8(f::SUB2) {
        0 => b_charge(x),
        2 => b_stretch(x),
        4 => b_hop(x),
        6 => b_lunge(x),
        8 => b_slash(x),
        10 => b_claw(x),
        12 => b_flames(x),
        _ => {}
    }
}

/// 0x61a8c : le chien se met à l'affût (test aléatoire, puis boîtes de portée ou charge).
fn ready(x: &mut Cx) {
    if !lib::maybe_draw_6e(x) {
        stance::reach(x);
    } else {
        x.set16(f::SUB2, 0x400);
    }
}

/// 0x62332 : debout, aspect 0, puis à l'affût.
fn stand_and_ready(x: &mut Cx) {
    set_state_long(x, STANCE);
    x.set8(LOOK, 0);
    mv(x, 0);
    ready(x);
}

/// 0x6231e : l'animation finie, retour debout et à l'affût.
fn b_end(x: &mut Cx) {
    if x.anim_step() {
        stand_and_ready(x);
    }
}

fn b_charge(x: &mut Cx) {
    match x.u8(f::SUB3) {
        4 => {
            let r = rand_and(x, 1);
            if r != 0 {
                set_state_long(x, 0x0202_0002);
                x.set8(LOOK, 2);
                mv(x, 2);
                return ready(x);
            }
            x.add8(f::SUB3, 2);
            x.set8(TIMER, 0x2a);
            x.set8(VULNERABLE, 1);
            x.set8(LOOK, 0xa);
            mv(x, 0xa);
        }
        6 => {
            let n = x.u8(TIMER).wrapping_sub(1);
            x.set8(TIMER, n);
            if n == 0 {
                stand_and_ready(x);
            }
        }
        _ => {}
    }
}

fn b_stretch(x: &mut Cx) {
    match x.u8(f::SUB3) {
        4 => {
            x.add8(f::SUB3, 2);
            x.set8(VULNERABLE, 1);
            x.set8(LOOK, 0xb);
            mv(x, 0xb);
        }
        6 => b_end(x),
        _ => {}
    }
}

/// Saut de côté (0x6239e) : vitesse en profondeur tirée au hasard.
fn b_hop(x: &mut Cx) {
    match x.u8(f::SUB3) {
        4 => {
            x.add8(f::SUB3, 2);
            x.set16(Obj::VY, 0x100);
            let r = rand_and(x, 1);
            if r != 0 {
                x.set16(Obj::VY, -0x100i16 as u16);
            }
            x.set8(VULNERABLE, 1);
            look(x, 5);
            x.set16(Obj::AY, 0);
            x.set16(Obj::VZ, 0x400);
            x.set16(Obj::AZ, -0x38i16 as u16);
            mv(x, 5);
        }
        6 => wait_key(x),
        8 => {
            if x.move_yz().gt() {
                x.anim_step();
            } else {
                land_dust(x);
            }
        }
        10 => b_end(x),
        _ => {}
    }
}

/// `+6` = 6 : bond en avant, atterrissage, retour à l'affût.
fn b_lunge(x: &mut Cx) {
    match x.u8(f::SUB3) {
        4 => jump_forward(x),
        6 => wait_key(x),
        8 => land(x),
        10 => b_end(x),
        _ => {}
    }
}

/// `+6` = 8 : bond, atterrissage puis coup de queue.
fn b_slash(x: &mut Cx) {
    match x.u8(f::SUB3) {
        4 => jump_forward(x),
        6 => wait_key(x),
        8 => land(x),
        10 => {
            if x.anim_step() {
                x.add8(f::SUB3, 2);
                strike(x, 0);
            }
        }
        12 => b_end(x),
        _ => {}
    }
}

/// 0x62404 : élan du bond (animation 6).
fn jump_forward<R: Banc>(x: &mut Cx<'_, R>) {
    x.add8(f::SUB3, 2);
    x.set8(VULNERABLE, 1);
    look(x, 6);
    x.set16(Obj::VX, 0x300);
    if x.u8(f::FLIP) != 0 {
        negate(x, Obj::VX);
    }
    x.set16(Obj::AX, 0);
    x.set16(Obj::VZ, 0x380);
    x.set16(Obj::AZ, -0x38i16 as u16);
    mv(x, 6);
}

fn b_claw(x: &mut Cx) {
    match x.u8(f::SUB3) {
        4 => jump_forward(x),
        6 => wait_key(x),
        8 => land(x),
        10 => {
            if x.anim_step() {
                x.add8(f::SUB3, 2);
                x.set8(STRIKE_POWER, 5);
                strike(x, 9);
            }
        }
        12 => {
            if x.u8(f::KEY_FRAME) != 0 {
                x.set8(f::KEY_FRAME, 0);
                x.c.d[0] = 1;
                crate::families::eshl_creators::shot_6(&mut crate::families::effects_creators::restes::Pur, x.b, x.o, 1);
            }
            if x.anim_step() {
                stand_and_ready(x);
            }
        }
        _ => {}
    }
}

fn b_flames(x: &mut Cx) {
    match x.u8(f::SUB3) {
        4 => jump_forward(x),
        6 => wait_key(x),
        8 => land(x),
        10 => {
            if x.anim_step() {
                x.add8(f::SUB3, 2);
                x.set8(STRIKE_POWER, 5);
                strike(x, 8);
            }
        }
        12 => flame_start(x),
        14 => {
            if !flame_tick(x, false) {
                // série finie : fin de geste du sous-état 4
                return done_then_after(x);
            }
            if x.anim_step() {
                // contrairement aux autres gestes, l'aspect `+0xa1` n'est pas remis à zéro
                set_state_long(x, STANCE);
                mv(x, 0);
                ready(x);
            }
        }
        _ => {}
    }
}

// --- sous-état 8 ----------------------------------------------------------------------------------------------

pub(super) fn run_c(x: &mut Cx) {
    match x.u8(f::SUB2) {
        0 | 6 => c_wait(x),
        2 => c_leap(x),
        4 => c_side(x),
        _ => {}
    }
}

fn c_wait<R: Banc>(x: &mut Cx<'_, R>) {
    match x.u8(f::SUB3) {
        4 => {
            x.add8(f::SUB3, 0xfe);
            c_begin(x);
        }
        0 => {
            x.add8(f::SUB3, 2);
            x.set8(TIMER, 0x10);
            c_begin(x);
        }
        2 => {
            let n = x.u8(TIMER).wrapping_sub(1);
            x.set8(TIMER, n);
            if (n as i8) < 0 {
                return set_state_long(x, STANCE);
            }
            if must_turn(x) {
                x.set32(RESUME_STATE, 0x0208_0004);
                let sub2 = x.u8(f::SUB2);
                x.set8(RESUME_SUB2, sub2);
                return set_state_long(x, 0x0202_0200);
            }
            x.anim_step();
        }
        _ => {}
    }
}

fn c_begin<R: Banc>(x: &mut Cx<'_, R>) {
    look(x, 0);
    x.set8(VULNERABLE, 1);
    mv(x, 0);
}

/// `+6` = 2 : bond d'attaque, retombée, retour debout (0x616d8).
fn c_leap(x: &mut Cx) {
    match x.u8(f::SUB3) {
        0 => jump_forward(x),
        2 => wait_key(x),
        4 => land(x),
        6 => stand_when_done(x),
        _ => {}
    }
}

/// `+6` = 4 : pas de côté (en courant) ou saut de côté.
fn c_side(x: &mut Cx) {
    x.c.d[0] = x.u8(f::SUB3) as u32;
    match x.u8(f::SUB3) {
        0 => {
            x.set8(VULNERABLE, 1);
            let r = rand_and(x, 1);
            let mut above = false;
            if x.u16(f::TARGET) != 0 {
                let target = x.linked(f::TARGET);
                above = (target.u16(x.b, Obj::Y).wrapping_sub(x.u16(Obj::Y)) as i16) >= 0;
            }
            if r == 0 {
                x.set8(f::SUB3, 4);
                x.set16(Obj::VY, -0x80i16 as u16);
                if x.u16(f::TARGET) != 0 && !above {
                    negate(x, Obj::VY);
                }
                x.set16(Obj::AY, 0);
                x.set16(Obj::VZ, 0x3c0);
                x.set16(Obj::AZ, -0x38i16 as u16);
                look(x, 6);
                return mv(x, 5);
            }
            x.add8(f::SUB3, 2);
            x.set8(Obj::SPEED_INDEX, if above { 0x10 } else { 0 });
            look(x, 0xc);
            x.set8(TIMER, 0x24);
            mv(x, 2);
            set_speed(x, 0x11);
        }
        2 => {
            let n = x.u8(TIMER).wrapping_sub(1);
            x.set8(TIMER, n);
            if (n as i8) < 0 {
                return set_state_long(x, STANCE);
            }
            x.move_xy();
            x.anim_step();
        }
        4 => wait_key(x),
        6 => {
            if x.move_yz().gt() {
                x.anim_step();
            } else {
                land_dust(x);
            }
        }
        8 => stand_when_done(x),
        _ => {}
    }
}

// --- sous-état 10 : promenade ----------------------------------------------------------------------------

pub(super) fn idle(x: &mut Cx) {
    match x.u8(f::SUB2) {
        0 => {
            x.add8(f::SUB2, 2);
            set_angle(x);
            set_speed(x, 0xd);
            x.set8(0x90, 0x50);
            mv(x, 2);
        }
        2 => {
            x.move_by_table_xy();
            x.anim_step();
            let n = x.u8(0x90).wrapping_sub(1);
            x.set8(0x90, n);
            if n == 0 {
                x.add8(f::SUB2, 2);
                mv(x, 4);
            }
        }
        4 => {
            if x.anim_step() {
                let flip = x.u8(f::FLIP) ^ 0x20;
                x.set8(f::FLIP, flip);
                x.add8(Obj::SPEED_INDEX, 0x10);
                let a = x.u8(Obj::SPEED_INDEX) & 0x1f;
                x.set8(Obj::SPEED_INDEX, a);
                x.set8(f::SUB2, 2);
                x.set8(0x90, 0x50);
                mv(x, 2);
            }
        }
        _ => {}
    }
    if var::PLAYERS_ALIVE.get(x.b) != 0 {
        set_state_long(x, STANCE);
        pick_target(x);
    }
}

// --- sous-état 12 : morsure d'un joueur terrassé ------------------------------------------------------------

pub(super) fn grab(x: &mut Cx) {
    let which = x.u8(f::SUB2);
    if (x.u16(f::TARGET) as i16) >= 0 {
        return set_state_long(x, STANCE);
    }
    let victim = x.linked(f::TARGET);
    x.c.a[1] = victim.0 | 0xff00_0000; // relu : A1 relu par la suite (DDSOM_POISON : hellhound_scene49, scorpion_scene38)
    if victim.u8(x.b, 0) == 0 || victim.u8(x.b, 0x188) == 0 {
        return set_state_long(x, STANCE);
    }
    match x.u8(f::SUB3) {
        0 => {
            x.add8(f::SUB3, 2);
            let y = victim.u16(x.b, Obj::Y);
            x.set16(Obj::Y, y);
            if victim.u16(x.b, Obj::Z) != 0 {
                return lib::release_player(x, victim);
            }
            x.set16(Obj::Z, 0);
            x.set8(BITES, 8);
            strike(x, if which == 0 { 1 } else { 2 });
            lib::hold_seized(x);
        }
        2 => bite_victim(x, which),
        _ => {}
    }
}

/// 0x6293e : chaque morsure (clé `+0x33`) tire de la vie au joueur ; à la dernière il est libéré.
fn bite_victim(x: &mut Cx, which: u8) {
    x.anim_step();
    if x.u8(f::KEY_FRAME) == 0 {
        return lib::hold_seized(x);
    }
    x.set8(f::KEY_FRAME, 0);
    let n = x.u8(BITES).wrapping_sub(1);
    x.set8(BITES, n);
    if (n as i8) <= 0 {
        return release(x);
    }
    x.set8(f::FREEZE, 8);
    if (x.u16(f::TARGET) as i16) >= 0 {
        return set_state_long(x, STANCE);
    }
    let reach = if x.u8(f::FLIP) != 0 { 0x38i16 } else { -0x38 };
    let d3 = if which == 0 { 0x1c } else { 0x2c };
    x.c.d[1] = 2;
    let pos = (x.u16(Obj::X).wrapping_add(reach as u16), x.u16(Obj::Z).wrapping_add(d3), x.u16(Obj::Y).wrapping_sub(1));
    (x.c.d[2], x.c.d[3], x.c.d[4]) = (pos.0 as u32, pos.1 as u32, pos.2 as u32);
    // l'autre objet de l'étincelle est A1, la victime (posée par `grab`)
    let victim = x.linked(f::TARGET);
    crate::families::effects_creators::spark_at(x.b, x.o, victim.0, 2, pos);
    if (x.u16(f::TARGET) as i16) >= 0 {
        return set_state_long(x, STANCE);
    }
    x.c.a[1] = victim.0 | 0xff00_0000; // relu : A1 relu par la suite (DDSOM_POISON : hellhound_scene49, scorpion_scene38)
    if (victim.u16(x.b, f::HP) as i16) <= 1 {
        return release(x);
    }
    lib::sound(x, 0xedd96);
    let hp = victim.u16(x.b, f::HP).wrapping_sub(3);
    victim.set_u16(x.b, f::HP, hp);
    if (hp as i16) <= 0 {
        victim.set_u16(x.b, f::HP, 1);
        return release(x);
    }
    lib::hold_seized(x);
}

fn release(x: &mut Cx) {
    let victim = x.linked(f::TARGET);
    victim.set_u8(x.b, 0x6f, 1);
    lib::release_player(x, victim);
}

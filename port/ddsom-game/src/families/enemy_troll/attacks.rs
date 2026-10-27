//! Type 14, sous-états 4, 6 et 8 : les gestes. Le sous-état 4 contient le lancer, les sauts, la charge et le repos,
//! le 6 les pas en profondeur, sauts et téléportations, le 8 le choix qui suit un geste.

use super::stance::{approach_start, walk_off};
use crate::objects::Obj;
use super::*;
use crate::objects::Courant;

pub(super) fn run(x: &mut Cx) {
    x.c.d[0] = x.u8(f::SUB2) as u32;
    match (x.u8(f::SUB), x.u8(f::SUB2)) {
        (4, 0) | (6, 0) => windup(x),
        (4, 2) | (6, 2) => turn_back(x),
        (4, 4) => throw(x),
        (4, 6) | (4, 8) => hop(x),
        (4, 10) => maybe_throw(x),
        (4, 12) => charge(x),
        (4, 14) => again(x, 0x26),
        (4, 16) => again(x, 0x1c),
        (4, 18) => rest(x),
        (4, 20) => walk_off(x),
        (6, 4) => done_attack(x),
        (6, 6) => depth_step(x),
        (6, 8) => jump_here(x, false),
        (6, 10) => jump_here(x, true),
        (6, 12) => teleport(x),
        (8, 0) => pause(x),
        (8, 2) => jump_in_place(x),
        (8, 4) => lunge(x),
        (8, 6) => pick_again(x),
        _ => {}
    }
}

/// 0x6cd5c : l'étape suivante lance l'animation 4.
fn next_and_start(x: &mut Cx) {
    x.add8(f::SUB3, 2);
    mv(x, 4);
}

/// 0x6cd64 : étape suivante à la fin de l'animation.
fn next_when_done(x: &mut Cx) {
    if x.anim_step() {
        x.add8(f::SUB3, 2);
    }
}

/// 0x6cd90 : fin de l'animation, geste terminé.
fn finish(x: &mut Cx) {
    if x.anim_step() {
        done_attack(x);
    }
}

// --- geste 0 : provocation, puis geste choisi --------------------------------------------------------

fn windup(x: &mut Cx) {
    if x.u8(f::SUB3) == 0 {
        x.add8(f::SUB3, 2);
        x.set8(VULNERABLE, 1);
        x.call(call::ANNOUNCE);
        if x.u8(AI_LUNGE) == 0 {
            x.set8(f::SUB3, 0);
            let chosen = x.u8(CHOSEN);
            x.set8(f::SUB2, chosen);
        }
        return mv(x, 0);
    }
    x.anim_step();
    let n = x.u8(AI_LUNGE).wrapping_sub(1);
    x.set8(AI_LUNGE, n);
    if (n as i8) >= 0 {
        return;
    }
    x.set8(f::SUB3, 0);
    if faces_wrong(x) {
        x.add8(f::SUB2, 2);
    } else {
        let chosen = x.u8(CHOSEN);
        x.set8(f::SUB2, chosen);
    }
}

/// +6 = 2 : demi-tour puis geste 0xc (marche).
fn turn_back(x: &mut Cx) {
    if x.u8(f::SUB3) == 0 {
        x.add8(f::SUB3, 2);
        x.set8(VULNERABLE, 1);
        return mv(x, 2);
    }
    if x.anim_step() {
        let flip = x.u8(f::FLIP) ^ 0x20;
        x.set8(f::FLIP, flip);
        x.set16(f::SUB2, 0xc00);
        mv(x, 0);
    }
}

// --- lancer -------------------------------------------------------------------------------------------

/// 0x6ca46 : lance le geste de lancer (+6 = 4).
fn throw_begin(x: &mut Cx) {
    x.set16(f::SUB2, 0x400);
    throw(x);
}

/// +6 = 4 : le Troll lance un objet (tant qu'il lui reste des charges `+0xab`), l'attend, puis s'éloigne.
fn throw(x: &mut Cx) {
    match x.u8(f::SUB3) {
        0 => {
            x.add8(f::SUB3, 8);
            x.set8(TIMER, 0x30);
            if (x.u8(AMMO) as i8) > 0 {
                x.c.d[5] = 0;
                x.call(call::SPAWN_THROWN);
                if !alloc_failed(x) {
                    let obj = x.c.a[1];
                    x.set32(THROWN, obj);
                    x.add8(AMMO, 0xf0);
                    x.add8(f::SUB3, 0xfa);
                }
            }
            throw_anim(x, 0);
        }
        2 | 8 => {
            if x.u8(f::SUB3) == 8 {
                x.add8(NO_ZONE, 1);
            }
            let n = x.u8(TIMER).wrapping_sub(1);
            x.set8(TIMER, n);
            if (n as i8) >= 0 {
                return;
            }
            x.add8(f::SUB3, 2);
            x.set8(TIMER, 0x60);
            throw_anim(x, 4);
        }
        10 => {
            let n = x.u8(TIMER).wrapping_sub(1);
            x.set8(TIMER, n);
            if n == 0 {
                return throw_end(x);
            }
            throw_hold(x);
        }
        4 => throw_hold(x),
        6 => {
            x.anim_step();
            let n = x.u8(TIMER).wrapping_sub(1);
            x.set8(TIMER, n);
            if n != 0 {
                follow_up(x);
            }
        }
        12 => {
            x.anim_step();
            let n = x.u8(AFTER_THROW).wrapping_sub(1);
            x.set8(AFTER_THROW, n);
            if (n as i8) < 0 {
                set_state_long(x, 0x0202_0600);
            }
        }
        _ => {}
    }
}

/// 0x6cad2 : le lancer suit son animation ; à la clé, l'objet lancé passe à l'étape suivante ; on attend qu'il soit
/// parti (état `+4` de l'objet à 4 ou plus).
fn throw_hold(x: &mut Cx) {
    x.anim_step();
    let obj = Obj(x.u32(THROWN));
    if x.u8(f::KEY_FRAME) != 0 {
        x.set8(f::KEY_FRAME, 0);
        add8(x, obj, 5, 2);
    }
    if (obj.u8(x.b, 4) as i8) < 4 {
        return;
    }
    throw_end(x);
}

/// 0x6caf2 : fin du lancer.
fn throw_end(x: &mut Cx) {
    x.add8(f::SUB3, 2);
    x.set16(AFTER_THROW, 0xb4);
    mv(x, 0);
}

/// +6 = 10 : au hasard, retour à la marche (0x1c) ou lancer.
fn maybe_throw(x: &mut Cx) {
    if rand_and(x, 1) == 0 {
        return again(x, 0x1c);
    }
    throw_begin(x);
}

/// +6 = 14, 16 (et 10 au tirage) : retour debout et reprise de la marche, `kind` = type de tir mémorisé.
fn again(x: &mut Cx, kind: u8) {
    x.set8(SHOT_KIND, kind);
    set_state_long(x, STANCE);
    approach_start(x);
}

// --- sauts ---------------------------------------------------------------------------------------------

/// 0x6cdb6 : saut de côté (vitesse horizontale 0x300, élan vers le haut).
fn jump_start(x: &mut Cx) {
    x.add8(f::SUB3, 2);
    x.set32(Obj::VX, 0x0300_0000);
    let mut shift = 0x20i16;
    if x.u8(f::FLIP) != 0 {
        shift = -shift;
        negate(x, Obj::VX);
    }
    let at = x.u16(Obj::X).wrapping_add(shift as u16);
    x.set16(Obj::X, at);
    x.set16(Obj::VZ, 0);
    x.set16(Obj::AZ, -0x40i16 as u16);
    x.set16(Obj::Z, 0x50);
    mv(x, 3);
}

/// 0x6cdea : retombée du saut.
fn land(x: &mut Cx) {
    x.move_xz();
    if (x.u16(Obj::Z) as i16) >= 0 {
        return;
    }
    x.add8(f::SUB3, 2);
    x.set32(Obj::Z, 0);
    mv(x, 5);
}

/// +6 = 6 et 8 (sous-état 4) : saut de côté, puis lancer (6) ou retour à la marche (8).
fn hop(x: &mut Cx) {
    match x.u8(f::SUB3) {
        0 => next_and_start(x),
        2 | 6 => next_when_done(x),
        4 => jump_start(x),
        8 => land(x),
        10 => {
            if x.u8(f::SUB2) != 8 {
                return throw_begin(x);
            }
            set_state_long(x, STANCE);
            approach_start(x);
        }
        _ => {}
    }
}

/// +6 = 8 et 10 (sous-état 6) : saut de côté ; `then_throw` (10) : puis lancer.
fn jump_here(x: &mut Cx, then_throw: bool) {
    match x.u8(f::SUB3) {
        0 => next_and_start(x),
        2 | 6 => next_when_done(x),
        4 => jump_start(x),
        8 => land(x),
        10 => finish(x),
        12 if then_throw => {
            x.set8(f::SUB, 4);
            throw_begin(x);
        }
        _ => {}
    }
}

/// +6 = 12 : charge vers la cible pendant `+0xa2` images.
fn charge(x: &mut Cx) {
    if x.u8(f::SUB3) == 0 {
        x.add8(f::SUB3, 2);
        x.set8(TIMER, 0x40);
        let angle = if x.u8(f::FLIP) != 0 { 8 } else { 0x18 };
        x.set8(Obj::SPEED_INDEX, angle);
        set_speed(x, 0x12);
        return mv(x, 1);
    }
    x.anim_step();
    let n = x.u8(TIMER).wrapping_sub(1);
    x.set8(TIMER, n);
    if (n as i8) < 0 {
        x.set16(f::SUB2, 0x400);
    }
}

/// +6 = 18 : repos. L'original sort du repos dès la deuxième image (le test de fin de minuterie est inversé).
fn rest(x: &mut Cx) {
    if x.u8(f::SUB3) == 0 {
        x.add8(f::SUB3, 2);
        x.set8(TIMER, 0x1e);
        return mv(x, 0);
    }
    x.anim_step();
    let n = x.u8(TIMER).wrapping_sub(1);
    x.set8(TIMER, n);
    if n != 0 {
        follow_up(x);
    }
}

// --- sous-état 6 --------------------------------------------------------------------------------------

/// +6 = 6 : pas en profondeur.
fn depth_step(x: &mut Cx) {
    match x.u8(f::SUB3) {
        0 => next_and_start(x),
        2 => next_when_done(x),
        4 => {
            let r = rand_and(x, 1);
            let d: i16 = if r != 0 { -0x18 } else { 0x18 };
            let at = x.u16(Obj::Y).wrapping_add(d as u16);
            x.set16(Obj::Y, at);
            x.add8(f::SUB3, 2);
            mv(x, 3);
        }
        6 => finish(x),
        _ => {}
    }
}

/// Positions d'arrivée (X puis profondeur) de la téléportation, relatives à la cible.
const TELEPORT_X: u32 = 0x6ceaa;
const TELEPORT_Y: u32 = 0x6ce9c;

/// +6 = 12 : le Troll réapparaît près de sa cible.
fn teleport(x: &mut Cx) {
    match x.u8(f::SUB3) {
        0 => next_and_start(x),
        2 | 6 => next_when_done(x),
        4 => {
            x.add8(f::SUB3, 2);
            let r = rand_and(x, 3) * 2;
            let target = load_target(x);
            let off = x.rom16(TELEPORT_X + r);
            let at = off.wrapping_add(target.u16(x.b, Obj::X));
            x.set16(Obj::X, at);
            let r = rand_and(x, 7) * 2;
            let target = load_target(x);
            let off = x.rom16(TELEPORT_Y + r);
            let at = off.wrapping_add(target.u16(x.b, Obj::Y));
            x.set16(Obj::Y, at);
            x.set8(f::FLIP, 0x20);
            let dx = x.u16(Obj::X).wrapping_sub(target.u16(x.b, Obj::X));
            if (dx as i16) < 0 {
                x.set8(f::FLIP, 0);
            }
            mv(x, 3);
        }
        8 => finish(x),
        _ => {}
    }
}

// --- sous-état 8 --------------------------------------------------------------------------------------

/// +6 = 0 : courte pause debout.
fn pause(x: &mut Cx) {
    if x.u8(f::SUB3) == 0 {
        x.add8(f::SUB3, 2);
        x.set8(VULNERABLE, 1);
        x.set8(TIMER, 0x10);
        return mv(x, 0);
    }
    let n = x.u8(TIMER).wrapping_sub(1);
    x.set8(TIMER, n);
    if (n as i8) < 0 {
        set_state_long(x, STANCE);
    }
    x.anim_step();
}

/// +6 = 2 : saut sur place (sans déplacement horizontal).
fn jump_in_place(x: &mut Cx) {
    match x.u8(f::SUB3) {
        0 => next_and_start(x),
        2 | 6 => next_when_done(x),
        4 => {
            x.add8(f::SUB3, 2);
            x.set16(Obj::Z, 0x40);
            x.set16(Obj::VZ, 0);
            x.set16(Obj::AZ, -0x40i16 as u16);
            x.set32(Obj::VX, 0x0300_0000);
            mv(x, 3);
        }
        8 => land(x),
        10 => finish(x),
        _ => {}
    }
}

/// +6 = 4 : bond (ou course) vers la cible.
fn lunge(x: &mut Cx) {
    match x.u8(f::SUB3) {
        0 => {
            if rand_and(x, 1) != 0 {
                return next_and_start(x);
            }
            x.add8(f::SUB3, 0xc);
            x.set8(TIMER, 0x10);
            set_speed(x, 0xd);
            x.set8(Obj::SPEED_INDEX, 0);
            let target = load_target(x);
            let d = x.u16(Obj::Y).wrapping_sub(target.u16(x.b, Obj::Y));
            if (d as i16) < 0 {
                x.set8(Obj::SPEED_INDEX, 0x10);
            }
            mv(x, 1);
        }
        2 | 6 => next_when_done(x),
        4 => {
            x.add8(f::SUB3, 2);
            let target = load_target(x);
            let d = x.u16(Obj::Y).wrapping_sub(target.u16(x.b, Obj::Y));
            let step: i16 = if (d as i16) < 0 { -0x32 } else { 0x32 };
            let at = x.u16(Obj::Y).wrapping_add(step as u16);
            x.set16(Obj::Y, at);
            mv(x, 3);
        }
        8 => land(x),
        10 => finish(x),
        12 => {
            let n = x.u8(TIMER).wrapping_sub(1);
            x.set8(TIMER, n);
            if (n as i8) < 0 {
                set_state_long(x, STANCE);
            }
            x.move_by_table_xy();
            x.anim_step();
        }
        _ => {}
    }
}

/// +6 = 6 : rechoisit un geste ; ne change de geste que si le nouveau diffère du précédent ou après un délai.
fn pick_again(x: &mut Cx) {
    x.set8(VULNERABLE, 1);
    set_state_long(x, IDLE);
    let b = if target_in_box(x, -0x20, 0x20, 0xc) {
        set_state_long(x, 0x0204_0000);
        rand_and(x, 2);
        x.set16(f::SUB2, 0);
        x.u8(CHOSEN)
    } else if near_box(x) {
        pick_attack(x, NEAR_TABLE)
    } else if far_box(x) {
        pick_attack(x, FAR_TABLE)
    } else {
        return;
    };
    x.c.d[0] = x.c.d[0] & !0xff | b as u32;
    if b != x.u8(LOOK_PREV) {
        x.set8(LOOK_PREV, b);
        x.set8(COOLDOWN, 2);
        return;
    }
    let n = x.u8(COOLDOWN).wrapping_sub(1);
    x.set8(COOLDOWN, n);
    if (n as i8) < 0 {
        x.set8(COOLDOWN, 2);
    }
}

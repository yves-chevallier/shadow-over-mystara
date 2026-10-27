//! Type 13, sous-états 4, 6 et 8 : les gestes. Le sous-état 4 contient les coups de hache, les sauts sur la cible et
//! le repos, le 6 les pas, sauts et téléportations, le 8 le choix qui suit un geste.

use super::stance::{approach_restart, approach_start};
use super::*;
use crate::objects::Courant;
use crate::objects::Obj;

/// Les quatre sauts sur la cible du sous-état 4 (`+6` = 12, 16, 18 et 20) : les trois derniers retombent plus haut
/// et plus loin, les deux derniers enchaînent encore un coup (étapes `+7` = 12 et plus).
#[derive(Clone, Copy, PartialEq)]
enum Leap {
    Low,
    High,
    HighThenStrike,
    HighThenSeries,
}

pub(super) fn run(x: &mut Cx) {
    x.c.d[0] = x.u8(f::SUB2) as u32;
    match (x.u8(f::SUB), x.u8(f::SUB2)) {
        (4, 0) | (6, 0) => windup(x),
        (4, 2) | (6, 2) => turn_back(x),
        (4, 4) => swing(x, 0, 0x1b),
        (4, 6) => swing(x, 2, 0x1c),
        (4, 8) => combo_a(x),
        (4, 10) => combo_b(x),
        (4, 12) => leap_on_target(x, Leap::Low),
        (4, 14) | (6, 8) => hop(x),
        (4, 16) => leap_on_target(x, Leap::High),
        (4, 18) => leap_on_target(x, Leap::HighThenStrike),
        (4, 20) => leap_on_target(x, Leap::HighThenSeries),
        (4, 22) => {
            set_state_long(x, STANCE);
            approach_start(x);
        }
        (4, 24) => {
            set_state_long(x, STANCE);
            x.set8(SHOT_KIND, 0x1c);
            approach_restart(x);
        }
        (6, 4) => done_attack(x),
        (6, 6) => depth_step(x),
        (6, 10) => jump_up(x),
        (6, 12) => teleport(x),
        (8, 0) => pause(x),
        (8, 2) => back_step(x),
        (8, 4) => lunge(x),
        (8, 6) => pick_again(x),
        _ => {}
    }
}

/// 0x6b742 : l'étape suivante lance l'animation 4 (avec son).
fn next_and_start(x: &mut Cx) {
    x.add8(f::SUB3, 2);
    wind_up(x);
}

/// 0x6b74a : étape suivante à la fin de l'animation.
fn next_when_done(x: &mut Cx) {
    if x.anim_step() {
        x.add8(f::SUB3, 2);
    }
}

/// 0x6b776 : fin de l'animation, geste terminé.
fn finish(x: &mut Cx) {
    if x.anim_step() {
        done_attack(x);
    }
}

/// 0x6b4c6 : fin de l'animation, geste de repli.
fn finish_follow(x: &mut Cx) {
    if x.anim_step() {
        follow_up(x);
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
        let weapon = x.u8(WEAPON) as u32;
        return mv(x, weapon);
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

/// +6 = 2 : demi-tour puis geste 0xc.
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
        let weapon = x.u8(WEAPON) as u32;
        mv(x, weapon);
    }
}

// --- coups de hache -----------------------------------------------------------------------------------

/// +6 = 4 et 6 : un coup de hache (animation `anim`, type `kind`), ou, une fois sur dix à seize, retour à la marche.
fn swing(x: &mut Cx, anim: u32, kind: u8) {
    if x.u8(f::SUB3) != 0 {
        if x.anim_step() {
            follow_up(x);
        }
        return;
    }
    let r = rand_and(x, 0xf);
    // `btst D0,D1` avec D1 = 0xffc0 : les tirages de 6 à 15 renoncent au coup
    if (0xffc0u32 >> r) & 1 != 0 {
        x.set8(SHOT_KIND, 0x1b);
        set_state_long(x, STANCE);
        return approach_restart(x);
    }
    x.add8(f::SUB3, 2);
    strike_with(x, anim, kind, 5);
}

/// +6 = 8 : un coup, puis un coup bas répété `+0xa2` fois.
fn combo_a(x: &mut Cx) {
    match x.u8(f::SUB3) {
        0 => {
            x.add8(f::SUB3, 2);
            x.set8(AI_SHOT, 3);
            let r = rand_and(x, 1);
            x.set8(TIMER, r as u8);
            strike_with(x, 0, 0x1b, 5);
        }
        2 => {
            if x.anim_step() {
                x.add8(f::SUB3, 2);
                strike_with(x, 3, 0x1b, 4);
            }
        }
        4 => {
            if !x.anim_step() {
                return;
            }
            let n = x.u8(TIMER).wrapping_sub(1);
            x.set8(TIMER, n);
            if (n as i8) < 0 {
                return follow_up(x);
            }
            strike_with(x, 0, 0x1b, 4);
        }
        _ => {}
    }
}

/// +6 = 10 : un coup tiré au hasard entre deux, puis un deuxième.
fn combo_b(x: &mut Cx) {
    if x.u8(f::SUB3) == 0 {
        x.add8(f::SUB3, 2);
        x.set8(AI_SHOT, 3);
        x.set8(TIMER, 1);
        if rand_and(x, 1) != 0 {
            strike_with(x, 2, 0x1c, 5);
        } else {
            strike_with(x, 0, 0x1b, 5);
        }
        return;
    }
    if !x.anim_step() {
        return;
    }
    let n = x.u8(TIMER).wrapping_sub(1);
    x.set8(TIMER, n);
    if (n as i8) < 0 {
        return follow_up(x);
    }
    strike_with(x, 2, 0x1c, 4);
}

// --- sauts sur la cible -------------------------------------------------------------------------------

/// 0x6b3bc / 0x6b4fa : le Gnoll se place sur la cible (position copiée), à la hauteur `height`, et bondit de
/// `shift` en avant.
fn leap_start(x: &mut Cx, height: u32, shift: i16) {
    x.add8(f::SUB3, 2);
    let target = load_target(x);
    let at = target.u32(x.b, Obj::X);
    x.set32(Obj::X, at);
    let depth = target.u32(x.b, Obj::Y);
    x.set32(Obj::Y, depth);
    x.set32(Obj::Z, height << 16);
    x.set16(Obj::VZ, 0);
    x.set16(Obj::AZ, -0x40i16 as u16);
    let (mut d, mut v) = (shift, -0x400i16);
    if x.u8(f::FLIP) != 0 {
        d = -d;
        v = -v;
    }
    let pos = x.u16(Obj::X).wrapping_add(d as u16);
    x.set16(Obj::X, pos);
    x.set16(Obj::VX, v as u16);
    x.set16(Obj::AX, 0);
    x.set_bit(1, 1, false);
    mv(x, 3);
}

/// 0x6b8a8 : retombée du saut.
fn land(x: &mut Cx) {
    x.move_xz();
    if (x.u16(Obj::Z) as i16) > 0 {
        return;
    }
    x.add8(f::SUB3, 2);
    x.set32(Obj::Z, 0);
    mv(x, 5);
}

fn leap_on_target(x: &mut Cx, kind: Leap) {
    match x.u8(f::SUB3) {
        0 => next_and_start(x),
        2 => {
            if x.anim_step() {
                x.add8(f::SUB3, 2);
                x.set_bit(1, 1, true);
                x.set8(0x3b, 0);
                x.set8(TIMER, 0x1e);
            }
        }
        4 => {
            let n = x.u8(TIMER).wrapping_sub(1);
            x.set8(TIMER, n);
            if n != 0 {
                return;
            }
            if kind == Leap::Low {
                leap_start(x, 0x30, 0x40);
            } else {
                leap_start(x, 0x60, 0x60);
            }
        }
        6 => {
            if x.anim_step() {
                x.add8(f::SUB3, 2);
                strike_with(x, 1, 0x1d, 5);
            }
        }
        8 => {
            if (x.u16(Obj::Z) as i16) <= 0x30 {
                x.anim_step();
            }
            land(x);
        }
        10 => finish_follow(x),
        12 if kind == Leap::HighThenStrike => {
            x.add8(f::SUB3, 2);
            x.set8(STRIKE_KIND, 0x1b);
            x.set8(STRIKE_POWER, 5);
            let anim = if rand_and(x, 1) != 0 { 2 } else { 0 };
            strike(x, anim);
        }
        12 if kind == Leap::HighThenSeries => {
            x.add8(f::SUB3, 4);
            strike_with(x, 0, 0x1b, 5);
        }
        14 if kind == Leap::HighThenStrike => finish_follow(x),
        14 if kind == Leap::HighThenSeries => {
            if x.anim_step() {
                x.add8(f::SUB3, 4);
                strike_with(x, 2, 0x1b, 5);
            }
        }
        16 if kind == Leap::HighThenSeries => finish_follow(x),
        _ => {}
    }
}

/// +6 = 14 (sous-état 4) et 8 (sous-état 6) : saut sur place.
fn hop(x: &mut Cx) {
    match x.u8(f::SUB3) {
        0 => next_and_start(x),
        2 | 6 => next_when_done(x),
        4 => {
            x.add8(f::SUB3, 2);
            mv(x, 3);
        }
        8 => finish(x),
        _ => {}
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

/// +6 = 10 : bond en l'air, coup plongeant.
fn jump_up(x: &mut Cx) {
    match x.u8(f::SUB3) {
        0 => next_and_start(x),
        2 | 6 => next_when_done(x),
        4 => {
            x.add8(f::SUB3, 2);
            let h = x.u16(Obj::Z).wrapping_add(0x40);
            x.set16(Obj::Z, h);
            mv(x, 3);
        }
        8 => {
            x.add8(f::SUB3, 2);
            x.set8(STRIKE_KIND, 0x1d);
            x.set8(STRIKE_POWER, 5);
            x.set16(Obj::VZ, 0);
            x.set16(Obj::AX, 0);
            x.set16(Obj::AZ, -0x40i16 as u16);
            let v = if x.u8(f::FLIP) != 0 { 0x400 } else { -0x400i16 as u16 };
            x.set16(Obj::VX, v);
            strike(x, 1);
        }
        10 => {
            x.anim_step();
            land(x);
        }
        12 => finish(x),
        _ => {}
    }
}

/// Positions d'arrivée (X puis profondeur) de la téléportation, relatives à la cible.
const TELEPORT_X: u32 = 0x6b8a4;
const TELEPORT_Y: u32 = 0x6b89c;

/// +6 = 12 : le Gnoll réapparaît près de sa cible.
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
            let r = rand_and(x, 3) * 2;
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

/// +6 = 2 : pas en arrière (les étapes 8 et plus sont un saut sur place, jamais atteint depuis l'étape 6).
fn back_step(x: &mut Cx) {
    match x.u8(f::SUB3) {
        0 | 8 => next_and_start(x),
        2 => {
            if x.anim_step() {
                x.add8(f::SUB3, 2);
                x.set8(TIMER, 0x1e);
                x.set_bit(1, 1, true);
                x.set8(0x3b, 0);
            }
        }
        4 => {
            let n = x.u8(TIMER).wrapping_sub(1);
            x.set8(TIMER, n);
            if n != 0 {
                return;
            }
            let d: i16 = if x.u8(f::FLIP) != 0 { -0x30 } else { 0x30 };
            let at = x.u16(Obj::X).wrapping_add(d as u16);
            x.set16(Obj::X, at);
            x.set_bit(1, 1, false);
            x.add8(f::SUB3, 2);
            mv(x, 3);
        }
        6 | 18 => finish(x),
        10 | 14 => next_when_done(x),
        12 => {
            x.add8(f::SUB3, 2);
            x.set16(Obj::Z, 0x40);
            x.set16(Obj::VZ, 0);
            x.set16(Obj::AZ, -0x40i16 as u16);
            x.set32(Obj::VX, 0x0300_0000);
            if x.u8(f::FLIP) != 0 {
                negate(x, Obj::VX);
            }
            mv(x, 3);
        }
        16 => land(x),
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

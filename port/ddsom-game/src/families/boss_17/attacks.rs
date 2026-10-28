//! Boss 17, groupe `+5` = 4 : les attaques (`+6` = 0 à 16), et leurs effets de poussière.
//!
//! | `+6` | Attaque (h) |
//! |---|---|
//! | 0 | coup simple |
//! | 2 | coup avec onde |
//! | 4 | coup double avec élan |
//! | 6 | bond vers la cible |
//! | 8, 10 | tir ESHL 0x20 (lancer) |
//! | 12 | tir ESHL 0x21 (souffle) |
//! | 14 | cri qui lâche des objets autour de la caméra |
//! | 16 | cri final, mode ralenti |

use super::decide;
use super::*;
use crate::objects::pools;
use crate::objects::Courant;
use crate::objects::Obj;

pub(super) fn run(x: &mut Cx) {
    match x.u8(f::SUB2) {
        0 => strike_0(x),
        2 => strike_2(x),
        4 => strike_4(x),
        6 => leap(x),
        8 | 10 => throw(x),
        12 => breath(x),
        14 => roar(x),
        16 => final_roar(x),
        _ => {}
    }
}

fn players_alive<R: crate::families::cx::Banc>(x: &mut Cx<'_, R>) -> bool {
    var::PLAYERS_ALIVE.get(x.b) != 0
}

fn state7<R: crate::families::cx::Banc>(x: &mut Cx<'_, R>) -> u8 {
    x.u8(0x7)
}

/// Fin d'une attaque : retour au combat si l'animation est finie.
fn finish(x: &mut Cx, to: fn(&mut Cx)) {
    if lib::step(x) {
        if !players_alive(x) {
            return decide::wait(x);
        }
        to(x);
    }
}

fn strike_anim<R: crate::families::cx::Banc>(x: &mut Cx<'_, R>, kind: u8, n: u32) {
    x.set8(ANIM_KIND, kind);
    x.set8(ANIM_LAST, kind);
    lib::start(x, SET_STRIKE, n);
}

/// Poussière : un EFFCT 0x32 de variante `v` lié au boss, sans insister (0x2318).
fn puff<R: crate::families::cx::Banc>(x: &mut Cx<'_, R>, v: u8) {
    if let Some(e) = x.alloc(&pools::EFFCT) {
        e.set_u8(x.b, 0, 1);
        e.set_u8(x.b, 2, 0x32);
        e.set_u8(x.b, 3, v);
        e.set_u16(x.b, 0x42, x.o.0 as u16);
        let side = x.u8(f::FLIP);
        e.set_u8(x.b, f::FLIP, side);
    }
}

/// 0x97580 : poussière à intervalle régulier pendant le ralenti.
pub(super) fn slow_tick<R: crate::families::cx::Banc>(x: &mut Cx<'_, R>) {
    if x.u8(SLOW) == 0 {
        return;
    }
    x.add8(SLOW_TICK, 1);
    let every = if x.u8(SLOW) == 1 { 0xb } else { 7 };
    if Fl::sub8(every, x.u8(SLOW_TICK)).gt() {
        return;
    }
    x.set8(SLOW_TICK, 0);
    puff(x, 4);
}

/// 0x975a2 : poussière sauf pendant le ralenti.
pub(super) fn dust_unless_slow<R: crate::families::cx::Banc>(x: &mut Cx<'_, R>) {
    if x.u8(SLOW) == 0 {
        puff(x, 4);
    }
}

fn spark<R: crate::families::cx::Banc>(x: &mut Cx<'_, R>) {
    lib::sound(x, 0xe580a);
}

// --- +6 = 0 -----------------------------------------------------------------------------------

fn strike_0(x: &mut Cx) {
    match state7(x) {
        0 => {
            x.add8(0x7, 2);
            strike_anim(x, 0x10, 0);
        }
        2 => {
            if lib::step(x) {
                if !players_alive(x) {
                    return decide::wait(x);
                }
                return decide::chase(x);
            }
            if x.u8(f::KEY_FRAME) != 0 {
                x.set8(f::KEY_FRAME, 0);
                spark(x);
            }
        }
        _ => {}
    }
}

// --- +6 = 2 -----------------------------------------------------------------------------------

fn strike_2(x: &mut Cx) {
    match state7(x) {
        0 => {
            x.add8(0x7, 2);
            strike_anim(x, 0x11, 1);
        }
        2 => {
            if lib::step(x) {
                if !players_alive(x) {
                    return decide::wait(x);
                }
                return decide::chase(x);
            }
            if x.u8(f::KEY_FRAME) == 0 {
                return;
            }
            x.set8(f::KEY_FRAME, 0);
            puff(x, 0);
            spark(x);
        }
        _ => {}
    }
}

// --- +6 = 4 : coup double avec élan ---------------------------------------------------------------

fn strike_4(x: &mut Cx) {
    match state7(x) {
        0 => {
            x.add8(0x7, 2);
            strike_anim(x, 0x12, 2);
        }
        2 => {
            lib::step(x);
            if !tst8(x, f::KEY_FRAME).mi() {
                return;
            }
            x.add8(0x7, 2);
            x.set8(f::KEY_FRAME, 0);
            x.set16(TIMER, 0x1e);
            puff(x, 3);
            let (vx, vz) = if x.u8(f::FLIP) == 0 { (-0x900i16, 0x4ci16) } else { (0x900, -0x4c) };
            x.set16(0x14, vx as u16);
            x.set16(0x16, vz as u16);
            lib::sound(x, 0xece9e);
            x.move_x();
        }
        4 => {
            if sub16(x, TIMER, 1).le() {
                x.add8(0x7, 2);
                x.set16(TIMER, 8);
                x.set8(INVULNERABLE, 0);
                return;
            }
            x.move_x();
            lib::step(x);
            if x.u8(f::KEY_FRAME) == 0 {
                return;
            }
            x.set8(f::KEY_FRAME, 0);
            puff(x, 1);
            lib::sound(x, 0xe587e);
        }
        6 => {
            if sub16(x, TIMER, 1).le() {
                x.add8(0x7, 2);
                lib::start(x, SET_STRIKE, 3);
            }
        }
        8 => finish(x, decide::chase),
        _ => {}
    }
}

// --- +6 = 6 : bond ------------------------------------------------------------------------------

fn leap(x: &mut Cx) {
    match state7(x) {
        0 => {
            x.add8(0x7, 2);
            x.set16(0x16, 0);
            let angle = if x.u8(f::FLIP) == 0 { 0x18 } else { 8 };
            x.set8(Obj::SPEED_INDEX, angle);
            speed(x, 0x17);
            // vitesse en profondeur assez grande pour atteindre la cible en un bond
            let target = x.linked(f::TARGET);
            let mut gap = (x.u32(0x8) as i32).wrapping_sub(target.u32(x.b, 0x8) as i32);
            if gap < 0 {
                gap = gap.wrapping_neg();
            }
            let per_frame = ((x.u16(0x14) as i16 as i32) << 8).wrapping_abs();
            let mut vz: u16 = 0x100;
            // la boucle de l'original ne finit pas si la vitesse est nulle
            for _ in 0..0x10000 {
                gap = gap.wrapping_sub(per_frame);
                if gap <= 0 {
                    break;
                }
                vz = vz.wrapping_add(0xe);
            }
            x.set16(0x18, vz);
            x.set16(0x1a, (-0x20i16) as u16);
            x.set8(ANIM_KIND, 0x13);
            x.set8(ANIM_LAST, 0x13);
            body(x, 5);
        }
        2 => {
            x.anim_step();
            if x.u8(f::KEY_FRAME) != 0 {
                return;
            }
            x.move_xz();
            if cmp16(x, 0x18, 0x120).le() {
                x.add8(0x7, 2);
                strike(x, 4);
            }
        }
        4 => {
            x.move_xz();
            if cmp16(x, 0x18, 0xff60).le() {
                x.add8(0x7, 2);
                x.set16(0x1a, (-0x30i16) as u16);
                strike(x, 5);
            } else {
                lib::step(x);
            }
        }
        6 => {
            x.move_xz();
            if tst16(x, 0xc).le() {
                x.add8(0x7, 2);
                x.set16(0xc, 0);
                lib::sound(x, 0xecc7c);
                return body(x, 0xa);
            }
            lib::step(x);
            if x.u8(f::KEY_FRAME) == 0 {
                return;
            }
            x.set8(f::KEY_FRAME, 0);
            puff(x, 2);
            lib::sound(x, 0xebfdc);
            lib::sound(x, 0xe587e);
        }
        8 => finish(x, decide::chase),
        _ => {}
    }
}

// --- +6 = 8, 10 : lancer -------------------------------------------------------------------------

fn throw(x: &mut Cx) {
    match state7(x) {
        0 => {
            x.add8(0x7, 2);
            set_abs8(x, 0xff8043, 0x7f);
            x.set8(INVULNERABLE, 1);
            x.add8(BUDGET_A, 0xff);
            let target = x.linked(f::TARGET);
            let (tx, ty) = (target.u16(x.b, 0x8), target.u16(x.b, 0x10));
            x.set16(MARK_X, tx);
            x.set16(MARK_Y, ty);
            strike_anim(x, 0x15, 7);
        }
        2 => {
            lib::step(x);
            if x.u8(f::KEY_FRAME) == 0 {
                return;
            }
            x.add8(0x7, 2);
            x.set8(INVULNERABLE, 0);
            x.set16(TIMER, 0x14);
            let Some(shot) = x.alloc(&pools::ESHL) else { return throw_end(x) };
            x.set16(CHILD, shot.0 as u16);
            shot.set_u8(x.b, 0, 1);
            shot.set_u8(x.b, 2, 0x20);
            shot.set_u8(x.b, 3, 1);
            shot.set_u16(x.b, 0x42, x.o.0 as u16);
            let side = x.u8(f::FLIP);
            shot.set_u8(x.b, f::FLIP, side);
            for off in [0x8u32, 0xc, 0x10] {
                let v = x.u16(off);
                shot.set_u16(x.b, off, v);
            }
            let dx: i16 = if x.u8(f::FLIP) == 0 { -0x44 } else { 0x44 };
            let px = shot.u16(x.b, 0x8).wrapping_add(dx as u16);
            shot.set_u16(x.b, 0x8, px);
            shot.set_u16(x.b, 0xc, 0x30);
            lib::step(x);
        }
        4 => {
            if sub16(x, TIMER, 1).gt() {
                lib::step(x);
                return;
            }
            x.add8(0x7, 2);
            x.set16(TIMER, 0xa);
            let shot = x.linked(CHILD);
            shot.set_u8(x.b, 0x58, 1);
            lib::sound(x, 0xe59da);
            lib::step(x);
        }
        6 => {
            let shot = x.linked(CHILD);
            if shot.u8(x.b, 0) == 0 || shot.u8(x.b, 0x5a) != 0 {
                return throw_end(x);
            }
            lib::step(x);
        }
        8 => {
            if lib::step(x) {
                set_abs8(x, 0xff8043, 0xff);
                if !players_alive(x) {
                    return decide::wait(x);
                }
                decide::chase(x);
            }
        }
        _ => {}
    }
}

/// 0x96614 : le tir a disparu (ou n'a pas pu naître) : fin du geste.
fn throw_end<R: crate::families::cx::Banc>(x: &mut Cx<'_, R>) {
    x.add8(0x7, 2);
    lib::start(x, SET_STRIKE, 8);
}

// --- +6 = 12 : souffle ----------------------------------------------------------------------------

fn breath(x: &mut Cx) {
    match state7(x) {
        0 => {
            x.add8(0x7, 2);
            x.set_a5_u16(0x51e, 4);
            set_abs8(x, 0xff8043, 0x7f);
            x.set8(INVULNERABLE, 1);
            x.add8(BUDGET_B, 0xff);
            strike_anim(x, 0x16, 9);
        }
        2 => {
            x.set_a5_u16(0x51e, 4);
            lib::step(x);
            if x.u8(f::KEY_FRAME) == 0 {
                return;
            }
            x.add8(0x7, 2);
            x.set16(TIMER, 0xa);
            x.set8(f::KEY_FRAME, 0);
            let Some(shot) = x.alloc(&pools::ESHL) else { return breath_end(x) };
            x.set16(CHILD, shot.0 as u16);
            shot.set_u8(x.b, 0, 1);
            shot.set_u8(x.b, 2, 0x21);
            shot.set_u16(x.b, 0x42, x.o.0 as u16);
            let side = x.u8(f::FLIP) ^ 0x20;
            shot.set_u8(x.b, f::FLIP, side);
            for off in [0x8u32, 0xc, 0x10] {
                let v = x.u16(off);
                shot.set_u16(x.b, off, v);
            }
            let dx: i16 = if x.u8(f::FLIP) == 0 { -0x3b } else { 0x3b };
            let px = shot.u16(x.b, 0x8).wrapping_add(dx as u16);
            let pz = shot.u16(x.b, 0xc).wrapping_add(0x30);
            shot.set_u16(x.b, 0x8, px);
            shot.set_u16(x.b, 0xc, pz);
        }
        4 => {
            x.set_a5_u16(0x51e, 4);
            if sub16(x, TIMER, 1).gt() {
                return step_only(x);
            }
            x.add8(0x7, 2);
            x.set8(INVULNERABLE, 0);
            x.set16(TIMER, 0x5a);
            let shot = x.linked(CHILD);
            shot.set_u8(x.b, 0x58, 1);
            lib::sound(x, 0xe5a4e);
            lib::step(x);
        }
        6 => {
            x.set_a5_u16(0x51e, 4);
            if sub16(x, TIMER, 1).gt() {
                return step_only(x);
            }
            x.add8(0x7, 2);
            let shot = x.linked(CHILD);
            shot.set_u8(x.b, 0x59, 1);
            lib::step(x);
        }
        8 => {
            let shot = x.linked(CHILD);
            if shot.u8(x.b, 0) == 0 || shot.u8(x.b, 0x5a) != 0 {
                return breath_end(x);
            }
            x.set_a5_u16(0x51e, 4);
            lib::step(x);
        }
        10 => {
            if lib::step(x) {
                set_abs8(x, 0xff8043, 0xff);
                if !players_alive(x) {
                    return decide::wait(x);
                }
                decide::stand(x);
            }
        }
        _ => {}
    }
}

fn step_only<R: crate::families::cx::Banc>(x: &mut Cx<'_, R>) {
    lib::step(x);
}

/// 0x9676c.
fn breath_end<R: crate::families::cx::Banc>(x: &mut Cx<'_, R>) {
    x.add8(0x7, 2);
    x.set_a5_u16(0x51e, 0);
    lib::start(x, SET_STRIKE, 0xa);
}

// --- +6 = 14 : cri ---------------------------------------------------------------------------------

fn roar(x: &mut Cx) {
    match state7(x) {
        0 => {
            x.add8(0x7, 2);
            set_abs8(x, 0xff8043, 0x7f);
            x.set8(INVULNERABLE, 1);
            x.add8(BUDGET_C, 0xff);
            strike_anim(x, 0x17, 0xb);
        }
        2 => {
            lib::step(x);
            if x.u8(f::KEY_FRAME) != 0 {
                x.add8(0x7, 2);
                x.set16(TIMER, 0x1e);
                x.set8(f::KEY_FRAME, 0);
            }
        }
        4 => {
            if sub16(x, TIMER, 1).gt() {
                return step_only(x);
            }
            x.add8(0x7, 2);
            set_abs8(x, 0xff8043, 0xff);
            x.set8(INVULNERABLE, 0);
            decide::spawn_by_level(x);
            lib::sound(x, 0xe5ac2);
            x.set16(TIMER, 0x3c);
            lib::step(x);
        }
        6 => {
            if sub16(x, TIMER, 1).le() {
                x.add8(0x7, 2);
                lib::start(x, SET_STRIKE, 0xc);
            } else {
                lib::step(x);
            }
        }
        8 => finish(x, decide::stand),
        _ => {}
    }
}

// --- +6 = 16 : cri final ---------------------------------------------------------------------------

fn final_roar(x: &mut Cx) {
    match state7(x) {
        0 => {
            x.add8(0x7, 2);
            set_abs8(x, 0xff8043, 0x7f);
            x.set8(0, 2);
            strike_anim(x, 0x18, 0xd);
        }
        2 => {
            lib::step(x);
            if x.u8(f::KEY_FRAME) != 0 {
                x.add8(0x7, 2);
                x.set16(TIMER, 0xa);
                x.set8(f::KEY_FRAME, 0);
            }
        }
        4 => {
            if sub16(x, TIMER, 1).gt() {
                return step_only(x);
            }
            x.add8(0x7, 2);
            x.set16(TIMER, 0x3c);
            if let Some(e) = x.alloc(&pools::EFFCT) {
                e.set_u8(x.b, 0, 1);
                e.set_u8(x.b, 2, 0x25);
                e.set_u16(x.b, 0x42, x.o.0 as u16);
                let (px, py) = (x.u16(0x8), x.u16(0x10));
                e.set_u16(x.b, 0x8, px);
                e.set_u16(x.b, 0x10, py);
                e.set_u16(x.b, 0xc, 0xa0);
                let dx: i16 = if x.u8(f::FLIP) != 0 { -5 } else { 5 };
                e.set_u16(x.b, 0x8, px.wrapping_add(dx as u16));
            }
            lib::step(x);
        }
        6 => {
            if sub16(x, TIMER, 1).le() {
                x.add8(0x7, 2);
                lib::start(x, SET_STRIKE, 0xe);
            } else {
                lib::step(x);
            }
        }
        8 => {
            if !lib::step(x) {
                return;
            }
            set_abs8(x, 0xff8043, 0xff);
            x.set8(0, 1);
            x.set8(SLOW_TICK, 0);
            decide::pick_slow_time(x);
            decide::pick_slow_mode(x);
            let mode = x.u8(SLOW);
            x.set8(SLOW_COUNT, mode);
            if !players_alive(x) {
                return decide::wait(x);
            }
            decide::stand(x);
        }
        _ => {}
    }
}

//! Boss 6, groupe `+5` = 2 : le combat. Attente, demi-tour, marche vers un joueur, puis selon la distance une
//! attaque (`decide`), une charge ou un tir en bond.
//!
//! | `+6` | Rôle |
//! |---|---|
//! | 0 | debout, attend |
//! | 2 | demi-tour |
//! | 4, 6 | marche (rapide, lente) |
//! | 8 | charge (vers la caméra, ou vers la position mémorisée de la cible si `+0xb2`) |
//! | 10 | tir en bond : crée un ESHL 0x2a, saute et le lâche |

use crate::families::cx::Banc;
use super::decide;
use super::*;
use crate::objects::pools;
use crate::objects::Courant;
use crate::objects::Obj;

/// Jeux d'animations de marche (0x103e0) : le premier si le boss ne regarde pas dans le sens où il avance.
const WALK_SIDEWAYS: u32 = 0x177992;
const WALK_FORWARD: u32 = 0x177a36;

pub(super) fn run(x: &mut Cx) {
    match x.u8(f::SUB2) {
        0 => idle(x),
        2 => turn(x),
        4 => walk(x, false),
        6 => walk(x, true),
        8 => charge(x),
        10 => shoot(x),
        _ => {}
    }
}

fn players_alive<R: Banc>(x: &mut Cx<'_, R>) -> bool {
    var::PLAYERS_ALIVE.get(x.b) != 0
}

fn needs_turn<R: Banc>(x: &mut Cx<'_, R>) -> bool {
    !lib::facing_target(x)
}

fn facing_forward<R: Banc>(x: &mut Cx<'_, R>) -> bool {
    lib::moving_forward(x)
}

/// Le boss est-il hors de la fenêtre `[caméra - 0x50, caméra + 0x1d0]` (0x104a2) ?
fn off_screen<R: Banc>(x: &mut Cx<'_, R>) -> bool {
    lib::outside_band(x, -0x50, 0x1d0)
}

fn walk_anim<R: Banc>(x: &mut Cx<'_, R>) {
    let n = if facing_forward(x) { 2 } else { 3 };
    body(x, n);
}

fn walk_select<R: Banc>(x: &mut Cx<'_, R>) {
    let set = if facing_forward(x) { WALK_FORWARD } else { WALK_SIDEWAYS };
    lib::switch_keeping_time(x, set);
}

// --- +6 = 0 -----------------------------------------------------------------------------------

fn idle(x: &mut Cx) {
    if !players_alive(x) {
        return decide::wait(x);
    }
    if x.u8(0x7) == 0 {
        x.add8(0x7, 2);
        lib::draw_6e(x);
        x.set8(ANIM_KIND, 0);
        if x.u8(ANIM_LAST) != 0 {
            x.set8(ANIM_LAST, 0);
            body(x, 0);
        }
        return;
    }
    if off_screen(x) {
        return decide::charge(x, false);
    }
    if sub8(x, APPROACH, 1).le() {
        return decide::chase(x);
    }
    if needs_turn(x) {
        return decide::interrupt(x);
    }
    lib::step(x);
}

// --- +6 = 2 -----------------------------------------------------------------------------------

fn turn<R: Banc>(x: &mut Cx<'_, R>) {
    if x.u8(0x7) == 0 {
        x.add8(0x7, 2);
        let side = x.u8(f::FLIP) ^ 0x20;
        x.set8(f::FLIP, side);
        x.set8(ANIM_KIND, 1);
        return body(x, 1);
    }
    if !lib::step(x) {
        return;
    }
    if !players_alive(x) {
        return decide::wait(x);
    }
    let resume = x.u32(SAVED_STATE);
    x.set32(f::STATE, resume);
    let last = x.u8(ANIM_LAST);
    if last == 2 || last == 3 {
        x.set8(ANIM_KIND, last);
        walk_anim(x);
    } else {
        body(x, 0);
    }
}

// --- +6 = 4, 6 : marche --------------------------------------------------------------------------

fn walk(x: &mut Cx, slow: bool) {
    if !players_alive(x) {
        return decide::wait(x);
    }
    if x.u8(0x7) != 0 {
        return keep_walking(x);
    }
    if slow {
        decide::walk_steps_slow(x);
        decide::walk_delay_slow(x);
    } else {
        decide::walk_steps_fast(x);
        decide::walk_delay_fast(x);
    }
    x.set8(ANIM_KIND, if slow { 3 } else { 2 });
    x.add8(0x7, 2);
    x.set16(0x16, 0);
    x.set16(0x1e, 0);
    if !pick_target(x) {
        return decide::wait(x);
    }
    lib::approach_target(x);
    speed(x, 0xd);
    let last = x.u8(ANIM_LAST);
    let kind = x.u8(ANIM_KIND);
    x.set8(ANIM_LAST, kind);
    if last == 2 || last == 3 {
        walk_select(x);
    } else {
        walk_anim(x);
    }
}

fn keep_walking(x: &mut Cx) {
    if sub8(x, WALK_DELAY, 1).mi() {
        if sub8(x, WALK_STEPS, 1).le() {
            return decide::chase_b(x);
        }
        lib::approach_target(x);
        speed(x, 0xd);
        if x.u8(ANIM_KIND) == 3 {
            decide::walk_delay_slow(x);
        } else {
            decide::walk_delay_fast(x);
        }
        return walk_select(x);
    }
    if off_screen(x) {
        return decide::charge(x, false);
    }
    if x.u8(ANIM_KIND) == 3 {
        let side = decide::probe(x, -0x60, 0x40, 6);
        if side != 0 {
            return decide::choose_mid(x, side);
        }
    }
    let side = decide::probe(x, -0xc0, 0x20, 0xc);
    if side != 0 {
        return decide::choose_far(x, side);
    }
    if lib::target_in_box(x, -0x19, 0x18, 0xe) {
        return decide::choose_front(x);
    }
    if lib::guard_in_front(x, -0x80, 0x10, 8) {
        return decide::choose_close(x);
    }
    if needs_turn(x) {
        return decide::interrupt(x);
    }
    if !tst8(x, f::KEY_FRAME).mi() {
        x.move_by_table_xy();
    }
    lib::step(x);
}

// --- +6 = 8 : charge ----------------------------------------------------------------------------

fn charge(x: &mut Cx) {
    match x.u8(0x7) {
        0 => {
            x.add8(0x7, 2);
            x.set16(0x16, 0);
            let mut angle = 0x18;
            if x.u8(CHARGE_AT_TARGET) == 0 {
                x.set8(f::FLIP, 0);
                let cam = var::CAMERA1_X.get(x.b);
                if !Fl::sub16(x.u16(0x8), cam).gt() {
                    x.set8(f::FLIP, 0x20);
                    angle = 8;
                }
            } else {
                let target = x.linked(f::TARGET);
                let tx = target.u16(x.b, 0x8);
                x.set16(MARK_X, tx);
                if x.u8(f::FLIP) != 0 {
                    angle = 8;
                }
            }
            x.set8(Obj::SPEED_INDEX, angle);
            speed(x, 0x35);
            x.set8(ANIM_KIND, 4);
            x.set8(ANIM_LAST, 4);
            body(x, 4);
        }
        2 => {
            lib::step(x);
            if x.u8(f::KEY_FRAME) != 2 {
                x.move_x();
                let going_left = (x.u16(0x14) as i16) < 0;
                let here = x.u16(0x8);
                let limit = if x.u8(CHARGE_AT_TARGET) == 0 {
                    let margin = if going_left { 0xf0 } else { 0x90 };
                    var::CAMERA1_X.get(x.b).wrapping_add(margin)
                } else {
                    x.u16(MARK_X)
                };
                let (d0, d1) = if going_left { (here, limit) } else { (limit, here) };
                if !Fl::sub16(d1, d0).lt() && x.u8(f::KEY_FRAME) != 0 {
                    return charge_end(x);
                }
            }
            if x.u8(f::KEY_FRAME) == 3 {
                x.set8(f::KEY_FRAME, 0);
                lib::sound(x, 0xecdba);
                lib::debris(x, 4);
                lib::shake_54(x, 0xa);
            }
        }
        4 => {
            if !sub16(x, COUNT, 1).gt() {
                x.set16(COUNT, 4);
                lib::debris(x, 3);
            }
            if sub16(x, TIMER, 1).le() {
                if !players_alive(x) {
                    return decide::wait(x);
                }
                return decide::chase(x);
            }
            x.move_x();
        }
        _ => {}
    }
}

/// 0x84930 : le boss est arrivé : il freine.
fn charge_end<R: Banc>(x: &mut Cx<'_, R>) {
    x.add8(0x7, 2);
    x.set16(TIMER, 0x14);
    x.set16(COUNT, 0);
    let mut brake: i16 = 0x40;
    if x.u8(f::FLIP) != 0 {
        brake = -brake;
    }
    x.set16(0x16, brake as u16);
    lib::shake_54(x, 0x14);
    lib::sound(x, 0xec976);
    lib::sound(x, 0xecdba);
    body(x, 5);
}

// --- +6 = 10 : tir en bond ------------------------------------------------------------------------

fn shoot(x: &mut Cx) {
    match x.u8(0x7) {
        0 => {
            x.add8(0x7, 2);
            x.set8(ANIM_KIND, 5);
            x.set8(ANIM_LAST, 5);
            body(x, 6);
        }
        2 => {
            lib::step(x);
            if x.u8(f::KEY_FRAME) != 0 {
                return;
            }
            x.add8(0x7, 2);
            let Some(shot) = x.alloc(&pools::ESHL) else { return resume(x) };
            x.set16(CHILD, shot.0 as u16);
            shot.set_u8(x.b, 0, 1);
            shot.set_u8(x.b, 2, 0x2a);
            shot.set_u16(x.b, 0x42, x.o.0 as u16);
            let target = x.u16(f::TARGET);
            shot.set_u16(x.b, f::TARGET, target);
            let side = x.u8(f::FLIP);
            shot.set_u8(x.b, f::FLIP, side);
            let (angle, vz) = if side == 0 { (8, -0x20i16) } else { (0x18, 0x20) };
            x.set8(Obj::SPEED_INDEX, angle);
            x.set16(0x16, vz as u16);
            lib::drift_heading(x);
            speed(x, 0x35);
            x.set16(0x18, 0x300);
            x.set16(0x1a, (-0x40i16) as u16);
            x.move_xz();
        }
        4 => {
            x.move_xz();
            if !tst16(x, 0x18).gt() {
                x.add8(0x7, 2);
                x.set16(0x16, 0);
            }
        }
        6 => {
            x.move_xz();
            if tst16(x, 0xc).gt() {
                return step_only(x);
            }
            x.add8(0x7, 2);
            x.set16(TIMER, 0x14);
            x.set16(COUNT, 0);
            x.set16(0xc, 0);
            let vz: i16 = if x.u8(f::FLIP) == 0 { -0x40 } else { 0x40 };
            x.set16(0x16, vz as u16);
            x.move_x();
            let shot = x.linked(CHILD);
            shot.set_u8(x.b, 0x58, 1);
            body(x, 7);
        }
        8 => {
            if !sub16(x, COUNT, 1).gt() {
                x.set16(COUNT, 4);
                lib::debris(x, 3);
            }
            if sub16(x, TIMER, 1).le() {
                return resume(x);
            }
            x.move_x();
            lib::step(x);
        }
        _ => {}
    }
}

fn step_only<R: Banc>(x: &mut Cx<'_, R>) {
    lib::step(x);
}

/// 0x84ad0 : fin du tir : retour à la marche.
fn resume(x: &mut Cx) {
    if !players_alive(x) {
        return decide::wait(x);
    }
    decide::chase(x);
}

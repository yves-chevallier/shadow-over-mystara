//! Boss 17, groupe `+5` = 2 : le combat. Le boss attend, se retourne, marche vers un joueur et, selon la
//! distance, choisit une attaque (`decide`) ; ou fonce en ligne droite.
//!
//! | `+6` | Rôle |
//! |---|---|
//! | 0 | debout, attend (animation 0) |
//! | 2 | demi-tour |
//! | 4, 6 | marche vers un joueur (4 : pas rapides, 6 : lents) |
//! | 8, 10 | charge |

use super::decide;
use super::*;
use crate::objects::Courant;

/// Jeux d'animations de marche, choisis par la marque `+0x33` (0x103e0) : le premier quand le boss ne regarde pas
/// dans le sens où il avance, le second sinon.
const WALK_SIDEWAYS: u32 = 0x180a46;
const WALK_FORWARD: u32 = 0x180aea;

mod lib {
    pub const FACING: u32 = 0x10560;
    pub const TURN_NEEDED: u32 = 0x1050c;
    pub const RAND_TIMER: u32 = 0x79f6;
    pub const ASK_TARGET: u32 = 0xe76c;
    pub const RANDOM_TURN: u32 = 0x10574;
    pub const TARGET_BOX_FAR: u32 = 0x7ef4;
}

pub(super) fn run(x: &mut Cx) {
    match x.u8(f::SUB2) {
        0 => idle(x),
        2 => turn(x),
        4 => walk(x, false),
        6 => walk(x, true),
        8 | 10 => dash(x),
        _ => {}
    }
}

fn players_alive(x: &mut Cx) -> bool {
    x.a5_u8(var::PLAYERS_ALIVE) != 0
}

/// Le boss doit-il se retourner vers sa cible (0x1050c, Z levé = non) ?
fn needs_turn(x: &mut Cx) -> bool {
    x.call(lib::TURN_NEEDED);
    !zero(x)
}

/// Le boss regarde-t-il dans le sens où il avance (0x10560, Z levé = oui) ?
fn facing_forward(x: &mut Cx) -> bool {
    x.call(lib::FACING);
    zero(x)
}

/// Animation de marche d'après le sens (2 ou 3).
fn walk_anim(x: &mut Cx) {
    let n = if facing_forward(x) { 2 } else { 3 };
    body(x, n);
}

/// Animation de marche choisie par la marque `+0x33` dans le jeu du sens de marche.
fn walk_select(x: &mut Cx) {
    let set = if facing_forward(x) { WALK_FORWARD } else { WALK_SIDEWAYS };
    x.c.a[4] = set;
    x.call(ai::SELECT);
}

// --- +6 = 0 : debout --------------------------------------------------------------------------

fn idle(x: &mut Cx) {
    if !players_alive(x) {
        return decide::wait(x);
    }
    if x.u8(0x7) == 0 {
        x.add8(0x7, 2);
        x.call(lib::RAND_TIMER);
        x.set8(ANIM_KIND, 0);
        if x.u8(ANIM_LAST) != 0 {
            x.set8(ANIM_LAST, 0);
            body(x, 0);
        }
        return;
    }
    if sub8(x, APPROACH, 1).le() {
        return decide::chase(x);
    }
    if needs_turn(x) {
        return decide::interrupt(x);
    }
    step(x);
}

// --- +6 = 2 : demi-tour ----------------------------------------------------------------------

fn turn(x: &mut Cx) {
    if x.u8(0x7) == 0 {
        x.add8(0x7, 2);
        let side = x.u8(f::FLIP) ^ 0x20;
        x.set8(f::FLIP, side);
        x.set8(ANIM_KIND, 1);
        return body(x, 1);
    }
    if !step(x) {
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

// --- +6 = 4 et 6 : marche -----------------------------------------------------------------------

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
    x.call(lib::ASK_TARGET);
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

/// 0x95ff8 : un pas de marche de plus ; à la fin des pas, la marche s'arrête (`chase_b`) ; tant qu'une
/// attaque est possible, le boss la déclenche.
fn keep_walking(x: &mut Cx) {
    if sub8(x, WALK_DELAY, 1).mi() {
        if sub8(x, WALK_STEPS, 1).le() {
            return decide::chase_b(x);
        }
        x.call(lib::ASK_TARGET);
        speed(x, 0xd);
        if x.u8(ANIM_KIND) == 3 {
            decide::walk_delay_slow(x);
        } else {
            decide::walk_delay_fast(x);
        }
        return walk_select(x);
    }
    let side = decide::probe(x, -0xc0, 0x40, 0xc);
    if side != 0 {
        return decide::choose_far(x, side);
    }
    if decide::probe(x, -0x40, 0x30, 6) != 0 {
        return decide::choose_mid(x);
    }
    if target_in_box(x, -0x10, 0x10, 8) {
        return decide::choose_front(x);
    }
    set_dw(x, 0, (-0x40i16) as u16);
    set_dw(x, 1, 0x20);
    set_dw(x, 2, 8);
    x.call(lib::TARGET_BOX_FAR);
    if !zero(x) {
        return decide::choose_close(x);
    }
    if needs_turn(x) {
        return decide::interrupt(x);
    }
    x.move_by_table_xy();
    step(x);
}

// --- +6 = 8 et 10 : charge ----------------------------------------------------------------------

fn dash(x: &mut Cx) {
    match x.u8(0x7) {
        0 => {
            x.add8(0x7, 2);
            x.set8(INVULNERABLE, 1);
            x.set16(0x1e, 0);
            x.set16(0x18, 0x500);
            x.set16(0x1a, (-0x30i16) as u16);
            let (angle, vz) = if x.u8(f::FLIP) == 0 { (8, -0x28i16) } else { (0x18, 0x28) };
            x.set8(ANGLE, angle);
            x.set16(0x16, vz as u16);
            x.call(lib::RANDOM_TURN);
            speed(x, 0x35);
            x.set8(ANIM_KIND, 5);
            x.set8(ANIM_LAST, 5);
            body(x, 6);
        }
        2 => {
            x.move_xyz();
            if x.a5_u16(0x6e) & 3 == 0 {
                attacks::dust_unless_slow(x);
            }
            if cmp16(x, 0x18, 0x100).le() {
                x.add8(0x7, 2);
                x.set8(INVULNERABLE, 0);
                let vz = (x.u16(0x16) as i16) >> 1;
                x.set16(0x16, vz as u16);
                x.set16(0x1a, (-0x1ci16) as u16);
            }
            step(x);
        }
        4 => {
            x.move_xyz();
            if cmp16(x, 0x18, 0xff00).le() {
                x.add8(0x7, 2);
                x.set16(0x16, 0);
                body(x, 8);
            } else {
                step(x);
            }
        }
        6 => {
            x.move_xz();
            if tst16(x, 0xc).gt() {
                step(x);
            } else {
                x.add8(0x7, 2);
                x.set16(0xc, 0);
                x.call(0xecc7c);
                body(x, 0xa);
            }
        }
        8 => {
            if step(x) {
                if !players_alive(x) {
                    return decide::wait(x);
                }
                decide::chase(x);
            }
        }
        _ => {}
    }
}

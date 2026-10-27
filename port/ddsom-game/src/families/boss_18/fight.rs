//! Boss 18, groupe `+5` = 2 : le combat. Le Lich attend, se retourne, marche vers un joueur et, selon la distance,
//! choisit une attaque (`decide`) ; ou fonce en ligne droite.
//!
//! | `+6` | Rôle |
//! |---|---|
//! | 0 | debout, attend (animation 2) |
//! | 2 | demi-tour (continue d'avancer s'il marchait) |
//! | 4, 6 | marche vers un joueur (4 : pas rapides, 6 : lents) |
//! | 8 | charge |
//! | 10 | geste de reprise (animation 6) |

use super::decide;
use super::*;
use crate::objects::Courant;
use crate::objects::Obj;

pub(super) fn run(x: &mut Cx) {
    match x.u8(f::SUB2) {
        0 => idle(x),
        2 => turn(x),
        4 => walk(x, false),
        6 => walk(x, true),
        8 => dash(x),
        10 => resume(x),
        _ => {}
    }
}

fn players_alive(x: &mut Cx) -> bool {
    lib::any_player_alive(x)
}

fn needs_turn(x: &mut Cx) -> bool {
    !lib::faces_target(x)
}

// --- +6 = 0 : debout --------------------------------------------------------------------------

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
            body(x, 2);
        }
        return;
    }
    if sub8(x, APPROACH, 1).le() {
        return decide::chase(x);
    }
    if needs_turn(x) {
        return decide::interrupt(x);
    }
    lib::step(x);
}

// --- +6 = 2 : demi-tour ------------------------------------------------------------------------

/// Le boss marchait (état mémorisé `0x0202_04xx` ou `06xx`) : il avance pendant le demi-tour.
fn was_walking(x: &mut Cx) -> bool {
    x.u16(SAVED_STATE) == 0x0202 && matches!(x.u8(SAVED_STATE + 2), 4 | 6)
}

fn turn(x: &mut Cx) {
    if x.u8(0x7) == 0 {
        x.add8(0x7, 2);
        if was_walking(x) {
            x.move_by_table_xy();
        }
        let side = x.u8(f::FLIP) ^ 0x20;
        x.set8(f::FLIP, side);
        x.set8(ANIM_KIND, 1);
        x.set8(ANIM_LAST, 1);
        let n = if x.u8(AIM_ON) != 0 { 4 } else { 3 };
        return body(x, n);
    }
    if (x.u16(CHASE_TIME) as i16) > 0 {
        sub16(x, CHASE_TIME, 1);
    }
    if !lib::step(x) {
        if was_walking(x) {
            x.move_by_table_xy();
        }
        return;
    }
    if !players_alive(x) {
        return decide::wait(x);
    }
    let resume = x.u32(SAVED_STATE);
    x.set32(f::STATE, resume);
    body(x, 2);
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
    if !lib::pick_target_any(x) {
        return decide::wait(x);
    }
    lib::approach_target(x);
    speed(x, 0xd);
    x.move_by_table_xy();
    let kind = x.u8(ANIM_KIND);
    if kind != x.u8(ANIM_LAST) {
        x.set8(ANIM_LAST, kind);
        body(x, 5);
    }
}

/// 0x97a0a : un pas de marche de plus ; à la fin des pas, la marche s'arrête (`chase_b`) ; tant qu'une
/// attaque est possible, le boss la déclenche.
fn keep_walking(x: &mut Cx) {
    if !sub8(x, WALK_DELAY, 1).pl() {
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
    } else {
        let side = decide::probe(x, -0xe0, 0x68, 8);
        if side != 0 {
            return decide::choose_far(x, side);
        }
        if x.u8(ANIM_KIND) == 3 {
            let side = decide::probe(x, -0x60, 0x20, 6);
            if side != 0 {
                return decide::choose_mid(x, side);
            }
        }
        if lib::target_in_box(x, 0, 0x2c, 0xc) {
            return decide::choose_front(x);
        }
        if lib::guard_in_front(x, -0x20, 0x40, 8) {
            return decide::choose_close(x);
        }
        if needs_turn(x) {
            return decide::interrupt(x);
        }
    }
    x.move_by_table_xy();
    lib::step(x);
}

// --- +6 = 8 : charge ----------------------------------------------------------------------------

fn dash(x: &mut Cx) {
    match x.u8(0x7) {
        0 => {
            x.add8(0x7, 2);
            x.set16(0x1e, 0);
            x.set16(0x18, 0x300);
            x.set16(0x1a, (-0x30i16) as u16);
            let (angle, vz) = if x.u8(f::FLIP) == 0 { (8, -0x50i16) } else { (0x18, 0x50) };
            x.set8(Obj::SPEED_INDEX, angle);
            x.set16(0x16, vz as u16);
            lib::drift_heading(x);
            speed(x, 0x3f);
            x.set8(ANIM_KIND, 4);
            x.set8(ANIM_LAST, 4);
            body(x, 5);
        }
        2 => {
            x.move_xyz();
            if !tst16(x, 0x18).gt() {
                x.add8(0x7, 2);
                x.set16(0x16, 0);
            }
            lib::step(x);
        }
        4 => {
            x.move_xyz();
            if !tst16(x, 0xc).gt() {
                x.add8(0x7, 2);
                x.set16(TIMER, 0xc);
                let mut v: i16 = -0x1c;
                if x.u8(f::FLIP) != 0 {
                    v = -v;
                }
                x.set16(0x16, v as u16);
            }
            lib::step(x);
        }
        6 => {
            if sub16(x, TIMER, 1).gt() {
                x.move_xy();
                lib::step(x);
                return;
            }
            if !players_alive(x) {
                return decide::wait(x);
            }
            decide::chase(x);
        }
        _ => {}
    }
}

// --- +6 = 10 : reprise -------------------------------------------------------------------------------

fn resume(x: &mut Cx) {
    if x.u8(0x7) == 0 {
        x.add8(0x7, 2);
        x.set8(ANIM_KIND, 5);
        x.set8(ANIM_LAST, 5);
        return body(x, 6);
    }
    if !lib::step(x) {
        return;
    }
    if !players_alive(x) {
        return decide::wait(x);
    }
    let resume = x.u32(SAVED_STATE);
    x.set32(f::STATE, resume);
}

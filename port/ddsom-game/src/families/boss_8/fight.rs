//! Boss 8, groupes 0 (arrivée), 2 (combat), 10 (attente sans joueur) et 14 (relevé).

use super::aids;
use super::*;
use crate::objects::Courant;
use crate::machine::Machine;

/// Octet de poids faible d'un registre de données.
fn setb(x: &mut Cx, n: usize, v: u8) {
    x.c.d[n] = x.c.d[n] & !0xff | v as u32;
}

// --- groupe 0 : arrivée --------------------------------------------------------------------------------------

/// 0x882ae : pause, apparition, saut, atterrissage, premier combat.
pub(super) fn intro(x: &mut Cx) {
    match dispatch(x, 0x882ba, f::SUB2) {
        0x882c4 => {
            if sub16(x, TIMER, 1).pl() {
                return;
            }
            x.add8(f::SUB2, 2);
            x.set16(TIMER, 0x3c);
            lib::shake_54(x, 0x3e);
        }
        0x882e0 => {
            if sub16(x, TIMER, 1).pl() {
                return;
            }
            x.add8(f::SUB2, 2);
            x.set_bit(0x1, 1, false);
            lib::sound(x, 0xecf7a);
            body(x, 5);
        }
        0x88308 => {
            if x.anim_step() {
                x.add8(f::SUB2, 2);
                x.set16(0x18, (-0x80i16) as u16);
                x.set16(0x1a, (-0x58i16) as u16);
                x.set16(0x14, 0);
                x.set16(0x16, 0);
            }
            x.set8(0x38, 0);
        }
        0x88330 => {
            if x.move_xz().gt() {
                return;
            }
            lib::create_set_14(x);
            aids::burst_small(x);
            x.set16(0xc, 0);
            x.add8(f::SUB2, 2);
            body(x, 0xc);
            lib::sound(x, 0xecc0a);
            lib::shake_54(x, 0x1e);
        }
        _ => {
            if x.u8(f::KEY_FRAME) != 0 {
                x.set8(f::KEY_FRAME, 0);
                lib::sound(x, 0xe42c6);
            }
            if x.anim_step() {
                lib::set_immunities(x);
                x.set8(0, 1);
                x.set32(f::STATE, 0x0202_0000);
            }
        }
    }
}

// --- groupe 2 : combat -----------------------------------------------------------------------------------------

/// 0x8839a : tant qu'un joueur est en vie, marche, demi-tour, arrêt, approche.
pub(super) fn fight(x: &mut Cx) {
    if var::PLAYERS_ALIVE.get(x.b) == 0 {
        x.set32(f::STATE, super::WAIT);
        return;
    }
    match dispatch(x, 0x883cc, f::SUB2) {
        0x883d6 => walk(x),
        0x8863a => turn(x),
        0x88692 => stop(x),
        _ => approach(x),
    }
    if var::PLAYERS_ALIVE.get(x.b) == 0 {
        x.set32(f::STATE, super::WAIT);
    }
}

/// Animation `kind` (clé `+0xa2`) ; le script `anim` ne repart que si la clé change.
fn keyed_body(x: &mut Cx, kind: u8, anim: u32) {
    x.set8(ANIM_KIND, kind);
    setb(x, 2, kind);
    if kind != x.u8(ANIM_LAST) {
        x.set8(ANIM_LAST, kind);
        body(x, anim);
    }
}

/// 0x883d6 : marche vers l'adversaire, et regarde s'il peut attaquer.
fn walk(x: &mut Cx) {
    if x.u8(f::SUB3) == 0 {
        x.add8(f::SUB3, 2);
        x.set8(VULNERABLE, 1);
        keyed_body(x, 1, 2);
        speed(x, 0x10);
        return lib::draw_stance(x);
    }
    if sub8(x, 0x71, 1).mi() {
        lib::draw_stance(x);
        lib::pick_target(x);
        let shot = if x.u8(f::SUB2) == 0 { 0x20 } else { 0x21 };
        x.set8(SHOT, shot);
        lib::approach_target(x);
        if sub8(x, 0x6b, 1).eq() {
            lib::draw_6b(x);
            if lib::maybe_draw_6e(x) {
                x.set16(f::SUB2, 0x400);
                return;
            }
        }
    }
    near_checks(x)
}

/// 0x88470 : décide d'attaquer, de s'écarter ou de continuer la marche.
fn near_checks(x: &mut Cx) {
    if aids::player_near(x, false) {
        x.set32(f::STATE, 0x0204_0000);
        return lib::pick_byte(x);
    }
    if guard(x, -0x50, 0x20, 0x18, true) || guard(x, -0xb0, 0x40, 0x18, false) {
        return;
    }
    if aids::player_near(x, true) {
        let i = x.random() & 0xf;
        x.set32(f::STATE, if i >= 6 { 0x0204_1000 } else { 0x0204_1200 });
        return lib::pick_byte(x);
    }
    if lib::guard_in_front_strict(x, -0x40, 0x40, 0x20) {
        return lib::pick_byte(x);
    }
    x.anim_step();
    if (x.u8(f::KEY_FRAME) as i8) >= 0 {
        x.move_by_table_xy();
    }
    if x.u8(0x3e) != 0 {
        x.set8(0x3e, 0);
        lib::sound(x, 0xec502);
    }
    if x.u16(f::TARGET) == 0 {
        return;
    }
    let t = x.linked(f::TARGET);
    let d = x.u16(0x8).wrapping_sub(t.u16(x.b, 0x8));
    let side = if (d as i16) < 0 { 0x20 } else { 0 };
    setb(x, 1, side);
    if side != x.u8(f::FLIP) {
        x.set32(RESUME_STATE, 0x0202_0000);
        x.set8(RESUME_SUB, 0);
        x.set32(f::STATE, 0x0202_0200);
    }
}

/// 0x88546 (devant, `front`) et 0x885c0 (loin) : un adversaire à portée de coup. Rend vrai si une attaque est engagée.
fn guard(x: &mut Cx, d0: i16, d1: u16, d2: u16, front: bool) -> bool {
    if !lib::target_in_box(x, d0, d1, d2) || x.u16(f::TARGET) == 0 {
        x.c.d[0] = 0;
        return false;
    }
    let t = x.linked(f::TARGET);
    let side = t.u8(x.b, f::FLIP);
    setb(x, 0, side);
    let mine = x.u8(f::FLIP);
    if !lib::decision(x, side == mine) {
        x.c.d[0] = 0;
        return false;
    }
    let i = x.random() & 0xf;
    if i >= 6 {
        x.set32(f::STATE, 0x0202_0800);
        return true;
    }
    x.set32(f::STATE, 0x0204_0000);
    lib::pick_byte(x);
    aids::choose_attack(x, front);
    true
}

/// 0x8863a : demi-tour, puis reprise de l'état mémorisé (`+0xa6`, `+0xa5`).
fn turn<R: crate::families::cx::Banc>(x: &mut Cx<'_, R>) {
    if x.u8(f::SUB3) == 0 {
        x.add8(f::SUB3, 2);
        x.set8(VULNERABLE, 1);
        x.set8(ANIM_KIND, 4);
        x.set8(ANIM_LAST, 4);
        let flip = x.u8(f::FLIP) ^ 0x20;
        x.set8(f::FLIP, flip);
        return body(x, 1);
    }
    if x.anim_step() {
        let s = x.u32(RESUME_STATE);
        x.set32(f::STATE, s);
        let sub = x.u8(RESUME_SUB);
        x.set8(f::SUB2, sub);
        body(x, 0);
    }
}

/// 0x88692 : arrêt face à l'adversaire.
fn stop(x: &mut Cx) {
    if x.u8(f::SUB3) == 0 {
        x.add8(f::SUB3, 2);
        x.set8(VULNERABLE, 1);
        x.set8(ANIM_KIND, 0);
        setb(x, 2, 0);
        if x.u8(ANIM_LAST) != 0 {
            x.set8(ANIM_LAST, 0);
            body(x, 0);
        }
        return;
    }
    if sub8(x, 0x6e, 1).mi() {
        x.set32(f::STATE, 0x0202_0000);
        return;
    }
    if aids::must_turn(x) {
        x.set32(RESUME_STATE, 0x0202_0000);
        x.set8(RESUME_SUB, 4);
        x.set32(f::STATE, 0x0202_0200);
    }
    x.anim_step();
}

/// 0x88702 : approche lente de l'adversaire.
fn approach(x: &mut Cx) {
    match dispatch(x, 0x8870e, f::SUB3) {
        0x8875a => {
            if sub16(x, TIMER, 1).mi() {
                x.set32(f::STATE, 0x0202_0000);
                return;
            }
            x.set8(SHOT, 0x20);
            lib::approach_target(x);
            if aids::must_turn(x) {
                x.set32(RESUME_STATE, 0x0202_0004);
                let b6 = x.u8(f::SUB2);
                x.set8(RESUME_SUB, b6);
                x.set32(f::STATE, 0x0202_0200);
            }
            x.anim_step();
            if x.u8(f::KEY_FRAME) != 0xff {
                x.move_by_table_xy();
            }
        }
        t => {
            if t == 0x8871c {
                x.add8(f::SUB3, 2);
                x.set16(TIMER, 0x28);
            } else {
                x.add8(f::SUB3, 0xfe);
            }
            x.set8(VULNERABLE, 1);
            x.set8(ANIM_KIND, 3);
            setb(x, 2, 3);
            if x.u8(ANIM_LAST) != 3 {
                x.set8(ANIM_LAST, 3);
                body(x, 6);
            }
            speed(x, 0x17);
        }
    }
}

// --- groupe 10 : attente sans joueur ----------------------------------------------------------------------------

/// 0x891e2 : attend qu'un joueur soit en vie.
pub(super) fn wait(x: &mut Cx) {
    if var::PLAYERS_ALIVE.get(x.b) != 0 {
        x.set32(f::STATE, 0x0202_0000);
        x.set8(ANIM_KIND, 0x7f);
        lib::pick_target(x);
        return;
    }
    if x.u8(f::SUB2) == 0 {
        x.add8(f::SUB2, 2);
        body(x, 0xd);
    } else {
        x.anim_step();
    }
}

// --- groupe 14 : relevé -------------------------------------------------------------------------------------------

/// 0x8921e : le boss se relève après une chute.
pub(super) fn stand_up(x: &mut Cx) {
    match dispatch(x, 0x8922c, f::SUB2) {
        0x89232 => {
            x.add8(f::SUB2, 2);
            x.set16(HITS, 0);
            x.set8(0x86, 0);
            x.set8(REACTION, 0);
            x.set8(PHASE_TIMER, 0x10);
            x.set8(0, 1);
            x.set16(TIMER, 8);
            x.set_bit(0x1, 1, true);
            body(x, 5);
        }
        0x89266 => {
            if sub16(x, TIMER, 1).pl() {
                return;
            }
            x.add8(f::SUB2, 2);
            x.set_bit(0x1, 1, false);
        }
        _ => {
            if x.anim_step() {
                x.set32(f::STATE, 0x0204_0a00);
                x.set8(VULNERABLE, 1);
            }
        }
    }
}

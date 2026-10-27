//! Boss 7, groupes 0 (arrivée, pose de l'attract), 2 (combat), 10 (attente sans joueur) et 14 (relevé).

use super::aids;
use super::*;
use crate::objects::Courant;
use crate::machine::Machine;

// --- groupe 0 : arrivée --------------------------------------------------------------------------------------

/// 0x85d08 : `+6` = 0, chute et atterrissage ; `+6` = 2, pose de l'attract.
pub(super) fn intro(x: &mut Cx) {
    if dispatch(x, 0x85d16, f::SUB2) == 0x85d1a {
        arrival(x)
    } else {
        pose(x)
    }
}

/// 0x85d1a : le boss apparaît, tombe, atterrit dans un nuage de poussière, puis se met en garde.
fn arrival(x: &mut Cx) {
    match dispatch(x, 0x85d28, f::SUB3) {
        0x85d30 => {
            addq8(x, f::SUB3, 2);
            x.call(call::FADE);
            x.set_bit(0x1, 1, false);
            body(x, 0xe);
        }
        0x85d4e => {
            if x.anim_step() {
                addq8(x, f::SUB3, 2);
                x.set16(0x18, (-0x80i16) as u16);
                x.set16(0x1a, (-0x58i16) as u16);
                x.set16(0x14, 0);
                x.set16(0x16, 0);
            }
        }
        0x85d72 => {
            if x.move_xz().gt() {
                return;
            }
            aids::burst_small(x);
            x.set16(0xc, 0);
            addq8(x, f::SUB3, 2);
            body(x, 0xc);
            x.call(0xecc0a);
            x.c.d[0] = 0x1e;
            x.call(call::SOUND_EVENT);
        }
        _ => {
            if x.u8(f::KEY_FRAME) != 0 {
                x.set8(f::KEY_FRAME, 0);
                x.call(0xe42c6);
            }
            if x.anim_step() {
                x.call(rom::IMMUNITIES);
                x.set8(0, 1);
                x.set32(f::STATE, FIGHT);
            }
        }
    }
}

/// 0x85dd6 : pose de l'attract (animation 0, relancée seulement si la clé change).
fn pose(x: &mut Cx) {
    dispatch(x, 0x85de4, f::SUB3);
    x.set8(VULNERABLE, 0);
    x.set8(ANIM_KIND, 0);
    setb(x, 2, 0);
    if x.u8(ANIM_LAST) != 0 {
        x.set8(ANIM_LAST, 0);
        x.c.d[0] = 0;
        body(x, 0);
    } else {
        x.anim_step();
    }
}

// --- groupe 2 : combat -----------------------------------------------------------------------------------------

/// 0x85e18 : tant qu'un joueur est en vie, marche, demi-tour, arrêt, approche.
pub(super) fn fight(x: &mut Cx) {
    if x.a5_u8(var::PLAYERS_ALIVE) == 0 {
        x.set32(f::STATE, WAIT);
        return;
    }
    match dispatch(x, 0x85e4a, f::SUB2) {
        0x860b8 => turn(x),
        0x86110 => stop(x),
        0x86180 => approach(x),
        _ => walk(x),
    }
    if x.a5_u8(var::PLAYERS_ALIVE) == 0 {
        x.set32(f::STATE, WAIT);
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

/// 0x85e54 : marche vers l'adversaire, et regarde s'il peut attaquer.
fn walk(x: &mut Cx) {
    if x.u8(f::SUB3) == 0 {
        addq8(x, f::SUB3, 2);
        x.set8(VULNERABLE, 1);
        keyed_body(x, 1, 2);
        speed(x, 0x10);
        return x.call(0xa470);
    }
    if sub8(x, 0x71, 1).mi() {
        x.call(0xa470);
        x.call(call::PICK_TARGET);
        let shot = if x.u8(f::SUB2) == 0 { 0x20 } else { 0x21 };
        x.set8(SHOT, shot);
        x.call(call::ASK_TARGET);
        if sub8(x, 0x6b, 1).eq() {
            x.call(0x788c);
            x.call(0x79ce);
            if !zero(x) {
                x.set16(f::SUB2, 0x400);
                return;
            }
        }
    }
    near_checks(x)
}

/// 0x85eee : décide d'attaquer, de frapper ce qui est devant, ou de continuer la marche.
fn near_checks(x: &mut Cx) {
    if aids::player_near(x, false) {
        x.set32(f::STATE, 0x0204_0000);
        return x.call(call::ATTACK_START);
    }
    if guard(x, -0x50, 0x20, 0x18, true) || guard(x, -0xb0, 0x40, 0x18, false) {
        return;
    }
    if aids::player_near(x, true) {
        x.random_68k();
        setw(x, 1, 0xffc0);
        let i = x.c.d[0] as u16 & 0xf;
        setw(x, 0, i);
        x.set32(f::STATE, if i >= 6 { 0x0204_1000 } else { 0x0204_0600 });
        return x.call(call::ATTACK_START);
    }
    if target_in_box_far(x, -0x40, 0x40, 0x20) {
        return x.call(call::ATTACK_START);
    }
    x.anim_step();
    if (x.u8(f::KEY_FRAME) as i8) >= 0 {
        x.move_by_table_xy();
    }
    if x.u8(0x3e) != 0 {
        x.set8(0x3e, 0);
        x.call(0xec502);
    }
    if x.u16(f::TARGET) == 0 {
        return;
    }
    let t = load_target(x);
    let d = x.u16(0x8).wrapping_sub(t.u16(x.b, 0x8));
    setw(x, 0, d);
    let side = if (d as i16) < 0 { 0x20 } else { 0 };
    setb(x, 1, side);
    if side != x.u8(f::FLIP) {
        x.set32(RESUME_STATE, FIGHT);
        x.set8(RESUME_SUB, 0);
        x.set32(f::STATE, 0x0202_0200);
    }
}

/// 0x85fc4 (devant, `front`) et 0x8603e (plus loin) : un adversaire à portée de coup. Rend vrai si le boss
/// engage une attaque ou s'écarte.
fn guard(x: &mut Cx, d0: i16, d1: u16, d2: u16, front: bool) -> bool {
    if !target_in_box(x, d0, d1, d2) || x.u16(f::TARGET) == 0 {
        x.c.d[0] = 0;
        return false;
    }
    let t = load_target(x);
    let side = t.u8(x.b, f::FLIP);
    setb(x, 0, side);
    let mine = x.u8(f::FLIP);
    x.call(if side == mine { 0xa4c2 } else { 0xa4b4 });
    if zero(x) {
        x.c.d[0] = 0;
        return false;
    }
    x.random_68k();
    setw(x, 1, 0xffc0);
    let i = x.c.d[0] as u16 & 0xf;
    setw(x, 0, i);
    if i >= 6 {
        x.set32(f::STATE, 0x0202_0800);
        return true;
    }
    x.set32(f::STATE, 0x0204_0000);
    x.call(call::ATTACK_START);
    aids::choose_attack(x, front);
    x.c.d[0] = 1;
    true
}

/// 0x860b8 : demi-tour, puis reprise de l'état mémorisé (`+0xa6`, `+0xa5`).
fn turn(x: &mut Cx) {
    if x.u8(f::SUB3) == 0 {
        addq8(x, f::SUB3, 2);
        x.set8(VULNERABLE, 1);
        key(x, 4);
        let flip = x.u8(f::FLIP) ^ 0x20;
        x.set8(f::FLIP, flip);
        return body(x, 1);
    }
    if x.anim_step() {
        let s = x.u32(RESUME_STATE);
        x.set32(f::STATE, s);
        let sub = x.u8(RESUME_SUB);
        x.set8(f::SUB2, sub);
        x.c.d[0] = 0;
        body(x, 0);
    }
}

/// 0x86110 : arrêt face à l'adversaire.
fn stop(x: &mut Cx) {
    if x.u8(f::SUB3) == 0 {
        addq8(x, f::SUB3, 2);
        x.set8(VULNERABLE, 1);
        x.set8(ANIM_KIND, 0);
        setb(x, 2, 0);
        if x.u8(ANIM_LAST) != 0 {
            x.set8(ANIM_LAST, 0);
            x.c.d[0] = 0;
            body(x, 0);
        }
        return;
    }
    if sub8(x, 0x6e, 1).mi() {
        x.set32(f::STATE, FIGHT);
        return;
    }
    if aids::must_turn(x) {
        x.set32(RESUME_STATE, FIGHT);
        x.set8(RESUME_SUB, 4);
        x.set32(f::STATE, 0x0202_0200);
    }
    x.anim_step();
}

/// 0x86180 : approche lente de l'adversaire.
fn approach(x: &mut Cx) {
    match dispatch(x, 0x8618c, f::SUB3) {
        0x861d8 => {
            if timer_done(x) {
                x.set32(f::STATE, FIGHT);
                return;
            }
            x.set8(SHOT, 0x20);
            x.call(call::ASK_TARGET);
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
            if t == 0x8619a {
                addq8(x, f::SUB3, 2);
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

/// 0x86af6 : attend qu'un joueur soit en vie.
pub(super) fn wait(x: &mut Cx) {
    if x.a5_u8(var::PLAYERS_ALIVE) != 0 {
        x.set32(f::STATE, FIGHT);
        x.set8(ANIM_KIND, 0x7f);
        return x.call(call::PICK_TARGET);
    }
    if x.u8(f::SUB2) == 0 {
        x.add8(f::SUB2, 2);
        body(x, 0xd);
    } else {
        x.anim_step();
    }
}

// --- groupe 14 : relevé -------------------------------------------------------------------------------------------

/// 0x86b32 : le boss se relève, invisible un instant, puis riposte (attaque 10).
pub(super) fn stand_up(x: &mut Cx) {
    match dispatch(x, 0x86b40, f::SUB2) {
        0x86b46 => {
            x.add8(f::SUB2, 2);
            x.set16(HITS, 0);
            x.set8(0x86, 0);
            x.set8(REACTION, 0);
            x.set16(TIMER, 8);
            x.set8(0, 2);
            x.set_bit(0x1, 1, true);
            body(x, 5);
        }
        0x86b74 => {
            if timer_done(x) {
                x.add8(f::SUB2, 2);
                x.set_bit(0x1, 1, false);
            }
        }
        _ => {
            if x.anim_step() {
                x.set8(PHASE_TIMER, 0x1e);
                x.set8(0, 1);
                x.set32(f::STATE, 0x0204_0a00);
                x.set8(VULNERABLE, 1);
            }
        }
    }
}

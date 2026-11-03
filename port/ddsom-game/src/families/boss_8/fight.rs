//! Boss 8, groups 0 (arrival), 2 (combat), 10 (wait with no player) and 14 (getting up).

use crate::families::cx::Bench;
use super::aids;
use super::*;
use crate::objects::Current;
use crate::machine::Machine;

// --- group 0: arrival --------------------------------------------------------------------------------------

/// 0x882ae: pause, appearance, jump, landing, first fight.
pub(super) fn intro<R: Bench>(x: &mut Cx<'_, R>) {
    match x.u8(f::SUB2) {
        0 => {
            if sub16(x, Warrior::TIMER, 1).pl() {
                return;
            }
            x.add8(f::SUB2, 2);
            x.set16(Warrior::TIMER, 0x3c);
            lib::shake_54(x, 0x3e);
        }
        2 => {
            if sub16(x, Warrior::TIMER, 1).pl() {
                return;
            }
            x.add8(f::SUB2, 2);
            x.set_bit(Enemy::DISPLAY, 1, false);
            lib::sound(x, crate::sound::CMD_349);
            body(x, 5);
        }
        4 => {
            if x.anim_step() {
                x.add8(f::SUB2, 2);
                x.set16(Enemy::VZ, (-0x80i16) as u16);
                x.set16(Enemy::GRAVITY, (-0x58i16) as u16);
                x.set16(Enemy::VX, 0);
                x.set16(Enemy::AX, 0);
            }
            x.set8(Enemy::FRAME_TAG, 0);
        }
        6 => {
            if x.move_xz().gt() {
                return;
            }
            lib::create_set_14(x);
            aids::burst_small(x);
            x.set16(Obj::Z, 0);
            x.add8(f::SUB2, 2);
            body(x, 0xc);
            lib::sound(x, crate::sound::CMD_328_428_B);
            lib::shake_54(x, 0x1e);
        }
        _ => {
            if x.u8(f::KEY_FRAME) != 0 {
                x.set8(f::KEY_FRAME, 0);
                lib::sound(x, crate::sound::CMD_342_342);
            }
            if x.anim_step() {
                lib::set_immunities(x);
                x.set8(Enemy::ACTIVE, 1);
                x.set32(f::STATE, 0x0202_0000);
            }
        }
    }
}

// --- groupe 2 : combat -----------------------------------------------------------------------------------------

/// 0x8839a: as long as a player is alive, walk, about-face, stop, approach.
pub(super) fn fight<R: Bench>(x: &mut Cx<'_, R>) {
    if var::PLAYERS_ALIVE.get(x.b) == 0 {
        x.set32(f::STATE, super::WAIT);
        return;
    }
    match x.u8(f::SUB2) {
        0 | 8 => walk(x),
        2 => turn(x),
        4 => stop(x),
        _ => approach(x),
    }
    if var::PLAYERS_ALIVE.get(x.b) == 0 {
        x.set32(f::STATE, super::WAIT);
    }
}

/// Animation `kind` (key `+0xa2`); the `anim` script only restarts if the key changes.
fn keyed_body<R: Bench>(x: &mut Cx<'_, R>, kind: u8, anim: u32) {
    x.set8(Warrior::ANIM_KIND, kind);
    setb(x, 2, kind);
    if kind != x.u8(Warrior::ANIM_LAST) {
        x.set8(Warrior::ANIM_LAST, kind);
        body(x, anim);
    }
}

/// 0x883d6: walks toward the opponent, and checks whether it can attack.
fn walk<R: Bench>(x: &mut Cx<'_, R>) {
    if x.u8(f::SUB3) == 0 {
        x.add8(f::SUB3, 2);
        x.set8(Enemy::VULNERABLE, 1);
        keyed_body(x, 1, 2);
        speed(x, 0x10);
        return lib::draw_stance(x);
    }
    if sub8(x, 0x71, 1).mi() {
        lib::draw_stance(x);
        lib::pick_target(x);
        let shot = if x.u8(f::SUB2) == 0 { 0x20 } else { 0x21 };
        x.set8(Boss::SHOT, shot);
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

/// 0x88470: decides to attack, to step aside, or to keep walking.
fn near_checks<R: Bench>(x: &mut Cx<'_, R>) {
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
    if x.u8(Enemy::STRIKE_KEY) != 0 {
        x.set8(Enemy::STRIKE_KEY, 0);
        lib::sound(x, crate::sound::CMD_326_426);
    }
    if x.u16(f::TARGET) == 0 {
        return;
    }
    let t = x.linked(f::TARGET);
    let d = x.u16(Obj::X).wrapping_sub(t.u16(x.b, Obj::X));
    let side = if (d as i16) < 0 { 0x20 } else { 0 };
    if side != x.u8(f::FLIP) {
        x.set32(Warrior::RESUME_STATE, 0x0202_0000);
        x.set8(Warrior::RESUME_SUB, 0);
        x.set32(f::STATE, 0x0202_0200);
    }
}

/// 0x88546 (ahead, `front`) and 0x885c0 (far): an opponent within striking range. Returns true if an attack is started.
fn guard<R: Bench>(x: &mut Cx<'_, R>, d0: i16, d1: u16, d2: u16, front: bool) -> bool {
    if !lib::target_in_box(x, d0, d1, d2) || x.u16(f::TARGET) == 0 {
        return false;
    }
    let t = x.linked(f::TARGET);
    let side = t.u8(x.b, f::FLIP);
    let mine = x.u8(f::FLIP);
    if !lib::decision(x, side == mine) {
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

/// 0x8863a: about-face, then resumption of the saved state (`+0xa6`, `+0xa5`).
fn turn<R: Bench>(x: &mut Cx<'_, R>) {
    if x.u8(f::SUB3) == 0 {
        x.add8(f::SUB3, 2);
        x.set8(Enemy::VULNERABLE, 1);
        x.set8(Warrior::ANIM_KIND, 4);
        x.set8(Warrior::ANIM_LAST, 4);
        let flip = x.u8(f::FLIP) ^ 0x20;
        x.set8(f::FLIP, flip);
        return body(x, 1);
    }
    if x.anim_step() {
        let s = x.u32(Warrior::RESUME_STATE);
        x.set32(f::STATE, s);
        let sub = x.u8(Warrior::RESUME_SUB);
        x.set8(f::SUB2, sub);
        body(x, 0);
    }
}

/// 0x88692: stop facing the opponent.
fn stop<R: Bench>(x: &mut Cx<'_, R>) {
    if x.u8(f::SUB3) == 0 {
        x.add8(f::SUB3, 2);
        x.set8(Enemy::VULNERABLE, 1);
        x.set8(Warrior::ANIM_KIND, 0);
        setb(x, 2, 0);
        if x.u8(Warrior::ANIM_LAST) != 0 {
            x.set8(Warrior::ANIM_LAST, 0);
            body(x, 0);
        }
        return;
    }
    if sub8(x, 0x6e, 1).mi() {
        x.set32(f::STATE, 0x0202_0000);
        return;
    }
    if aids::must_turn(x) {
        x.set32(Warrior::RESUME_STATE, 0x0202_0000);
        x.set8(Warrior::RESUME_SUB, 4);
        x.set32(f::STATE, 0x0202_0200);
    }
    x.anim_step();
}

/// 0x88702 : approche lente de l'adversaire.
fn approach<R: Bench>(x: &mut Cx<'_, R>) {
    match x.u8(f::SUB3) {
        2 => {
            if sub16(x, Warrior::TIMER, 1).mi() {
                x.set32(f::STATE, 0x0202_0000);
                return;
            }
            x.set8(Boss::SHOT, 0x20);
            lib::approach_target(x);
            if aids::must_turn(x) {
                x.set32(Warrior::RESUME_STATE, 0x0202_0004);
                let b6 = x.u8(f::SUB2);
                x.set8(Warrior::RESUME_SUB, b6);
                x.set32(f::STATE, 0x0202_0200);
            }
            x.anim_step();
            if x.u8(f::KEY_FRAME) != 0xff {
                x.move_by_table_xy();
            }
        }
        t => {
            if t == 0 {
                x.add8(f::SUB3, 2);
                x.set16(Warrior::TIMER, 0x28);
            } else {
                x.add8(f::SUB3, 0xfe);
            }
            x.set8(Enemy::VULNERABLE, 1);
            x.set8(Warrior::ANIM_KIND, 3);
            setb(x, 2, 3);
            if x.u8(Warrior::ANIM_LAST) != 3 {
                x.set8(Warrior::ANIM_LAST, 3);
                body(x, 6);
            }
            speed(x, 0x17);
        }
    }
}

// --- group 10: wait with no player ----------------------------------------------------------------------------

/// 0x891e2: waits for a player to be alive.
pub(super) fn wait<R: Bench>(x: &mut Cx<'_, R>) {
    if var::PLAYERS_ALIVE.get(x.b) != 0 {
        x.set32(f::STATE, 0x0202_0000);
        x.set8(Warrior::ANIM_KIND, 0x7f);
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

// --- group 14: getting up -------------------------------------------------------------------------------------------

/// 0x8921e: the boss gets up after a fall.
pub(super) fn stand_up<R: Bench>(x: &mut Cx<'_, R>) {
    match x.u8(f::SUB2) {
        0 => {
            x.add8(f::SUB2, 2);
            x.set16(Warrior::HITS, 0);
            x.set8(Enemy::DOWNED, 0);
            x.set8(Enemy::REACTION, 0);
            x.set8(Warrior::PHASE_TIMER, 0x10);
            x.set8(Enemy::ACTIVE, 1);
            x.set16(Warrior::TIMER, 8);
            x.set_bit(Enemy::DISPLAY, 1, true);
            body(x, 5);
        }
        2 => {
            if sub16(x, Warrior::TIMER, 1).pl() {
                return;
            }
            x.add8(f::SUB2, 2);
            x.set_bit(Enemy::DISPLAY, 1, false);
        }
        _ => {
            if x.anim_step() {
                x.set32(f::STATE, 0x0204_0a00);
                x.set8(Enemy::VULNERABLE, 1);
            }
        }
    }
}

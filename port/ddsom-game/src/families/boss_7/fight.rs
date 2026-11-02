//! Boss 7, groups 0 (arrival, attract pose), 2 (combat), 10 (wait with no player) and 14 (getting up).

use super::aids;
use crate::families::cx::Bench;
use super::*;
use crate::objects::Current;
use crate::machine::Machine;

// --- group 0: arrival --------------------------------------------------------------------------------------

/// 0x85d08: `+6` = 0, fall and landing; `+6` = 2, attract pose.
pub(super) fn intro<R: Bench>(x: &mut Cx<'_, R>) {
    if dispatch(x, 0x85d16, f::SUB2) == 0x85d1a {
        arrival(x)
    } else {
        pose(x)
    }
}

/// 0x85d1a: the boss appears, falls, lands in a cloud of dust, then takes its guard.
fn arrival<R: Bench>(x: &mut Cx<'_, R>) {
    match dispatch(x, 0x85d28, f::SUB3) {
        0x85d30 => {
            addq8(x, f::SUB3, 2);
            lib::sound(x, 0xecf7a);
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
                x.set32(f::STATE, FIGHT);
            }
        }
    }
}

/// 0x85dd6: attract pose (animation 0, restarted only if the key changes).
fn pose<R: Bench>(x: &mut Cx<'_, R>) {
    dispatch(x, 0x85de4, f::SUB3);
    x.set8(VULNERABLE, 0);
    x.set8(ANIM_KIND, 0);
    setb(x, 2, 0);
    if x.u8(ANIM_LAST) != 0 {
        x.set8(ANIM_LAST, 0);
        body(x, 0);
    } else {
        x.anim_step();
    }
}

// --- groupe 2 : combat -----------------------------------------------------------------------------------------

/// 0x85e18: as long as a player is alive, walk, about-face, stop, approach.
pub(super) fn fight<R: Bench>(x: &mut Cx<'_, R>) {
    if var::PLAYERS_ALIVE.get(x.b) == 0 {
        x.set32(f::STATE, WAIT);
        return;
    }
    match dispatch(x, 0x85e4a, f::SUB2) {
        0x860b8 => turn(x),
        0x86110 => stop(x),
        0x86180 => approach(x),
        _ => walk(x),
    }
    if var::PLAYERS_ALIVE.get(x.b) == 0 {
        x.set32(f::STATE, WAIT);
    }
}

/// Animation `kind` (key `+0xa2`); the `anim` script only restarts if the key changes.
fn keyed_body<R: Bench>(x: &mut Cx<'_, R>, kind: u8, anim: u32) {
    x.set8(ANIM_KIND, kind);
    setb(x, 2, kind);
    if kind != x.u8(ANIM_LAST) {
        x.set8(ANIM_LAST, kind);
        body(x, anim);
    }
}

/// 0x85e54: walks toward the opponent, and checks whether it can attack.
fn walk<R: Bench>(x: &mut Cx<'_, R>) {
    if x.u8(f::SUB3) == 0 {
        addq8(x, f::SUB3, 2);
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

/// 0x85eee: decides to attack, to strike whatever is in front, or to keep walking.
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
        x.set32(f::STATE, if i >= 6 { 0x0204_1000 } else { 0x0204_0600 });
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
    let t = load_target(x);
    let d = x.u16(0x8).wrapping_sub(t.u16(x.b, 0x8));
    let side = if (d as i16) < 0 { 0x20 } else { 0 };
    setb(x, 1, side);
    if side != x.u8(f::FLIP) {
        x.set32(RESUME_STATE, FIGHT);
        x.set8(RESUME_SUB, 0);
        x.set32(f::STATE, 0x0202_0200);
    }
}

/// 0x85fc4 (ahead, `front`) and 0x8603e (farther): an opponent within striking range. Returns true if the boss
/// starts an attack or steps aside.
fn guard<R: Bench>(x: &mut Cx<'_, R>, d0: i16, d1: u16, d2: u16, front: bool) -> bool {
    if !lib::target_in_box(x, d0, d1, d2) || x.u16(f::TARGET) == 0 {
        return false;
    }
    let t = load_target(x);
    let side = t.u8(x.b, f::FLIP);
    setb(x, 0, side);
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

/// 0x860b8: about-face, then resumption of the saved state (`+0xa6`, `+0xa5`).
fn turn<R: Bench>(x: &mut Cx<'_, R>) {
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
        body(x, 0);
    }
}

/// 0x86110: stop facing the opponent.
fn stop<R: Bench>(x: &mut Cx<'_, R>) {
    if x.u8(f::SUB3) == 0 {
        addq8(x, f::SUB3, 2);
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
fn approach<R: Bench>(x: &mut Cx<'_, R>) {
    match dispatch(x, 0x8618c, f::SUB3) {
        0x861d8 => {
            if timer_done(x) {
                x.set32(f::STATE, FIGHT);
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

// --- group 10: wait with no player ----------------------------------------------------------------------------

/// 0x86af6: waits for a player to be alive.
pub(super) fn wait<R: Bench>(x: &mut Cx<'_, R>) {
    if var::PLAYERS_ALIVE.get(x.b) != 0 {
        x.set32(f::STATE, FIGHT);
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

// --- group 14: getting up -------------------------------------------------------------------------------------------

/// 0x86b32: the boss gets up, invisible for a moment, then counterattacks (attack 10).
pub(super) fn stand_up<R: Bench>(x: &mut Cx<'_, R>) {
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

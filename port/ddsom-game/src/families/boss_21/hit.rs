//! Boss 21, state 4 "hit" (0x9a3a2). `+5` groups:
//!
//! | `+5` | Role |
//! |---|---|
//! | 0 | reaction to the hit: death if out of HP, otherwise routine from table 0x9a40a (code `+0x49`) |
//! | 2 | knockback (pushed back, `+6` = 0 to 4) |
//! | 4 | getting up after a fall (`+6` = 0 to 6) |
//! | 6, 8 | death: fall, flight, end (`+6` = 0 to 8) |
//! | 10 | end of getting up |

use crate::families::cx::Bench;
use crate::data::RomAddr;
use super::decide;
use super::*;
use crate::objects::Current;

/// Table of reaction routines, indexed by the hit code `+0x49` (longs).
const REACTIONS: RomAddr = RomAddr(0x9a40a);
/// The only routine specific to the boss (code 0x4c): stun with trembling.
const STUN_REACTION: RomAddr = RomAddr(0x9a4b6);
/// HP thresholds per phase (words, 128 bytes per row).
const THRESHOLDS: RomAddr = RomAddr(0x112e2);

pub(super) fn run<R: Bench>(x: &mut Cx<'_, R>) {
    if tst8(x, f::FREEZE).gt() {
        sub8(x, f::FREEZE, 1);
    } else {
        match x.u8(f::SUB) {
            0 => reaction(x),
            2 => pushed_back(x),
            4 => get_up(x),
            6 | 8 => down(x),
            10 => recovered(x),
            _ => {}
        }
    }
    lib::collide_terrain(x);
}

/// 0x9ae8a: out of HP? Then moves to group 6 (death) and returns true.
fn out_of_hp<R: Bench>(x: &mut Cx<'_, R>) -> bool {
    if tst16(x, f::HP).gt() {
        return false;
    }
    x.set8(f::SUB, 6);
    x.set8(Enemy::ACTIVE, 2);
    true
}

fn reaction<R: Bench>(x: &mut Cx<'_, R>) {
    if x.u8(f::SUB2) == 0 {
        if x.u8(Boss21::SLOW) != 0 {
            let mut add = x.u8(Enemy::STUN);
            if x.u8(Boss21::SLOW) == 2 {
                add = add.wrapping_add(add);
            }
            let v = x.u8(Enemy::STUN).wrapping_add(add);
            x.set8(Enemy::STUN, v);
        }
        x.set8(Boss::INVULNERABLE, 0);
        var::CAMERA_MODE.set(x.b, 0);
        lib::set_68(x);
        if out_of_hp(x) {
            return down(x);
        }
    }
    if reaction_routine(x, REACTIONS.0) == STUN_REACTION.0 {
        return stun(x);
    }
    react(x, REACTIONS.0);
}

/// 0x9a4b6: stun: the boss trembles in place then sets off again.
fn stun<R: Bench>(x: &mut Cx<'_, R>) {
    match x.u8(f::SUB2) {
        0 => {
            x.add8(f::SUB2, 2);
            x.set8(Enemy::ACTIVE, 2);
            let px = x.u16(Obj::X);
            x.set16(Enemy::X_SAVED, px);
            x.set16(Boss21::HITS, 0);
            let stun = x.u8(Enemy::STUN);
            x.set8(f::FREEZE, stun);
            x.set8(Boss::SHAKE, 0x10);
            lib::start(x, SET_HURT.0, 8);
        }
        2 => {
            if tst8(x, Boss::SHAKE).le() {
                let px = x.u16(Enemy::X_SAVED);
                x.set16(Obj::X, px);
                x.set8(Enemy::ACTIVE, 1);
                x.set32(f::STATE, 0x0404_0200);
                x.set32(Enemy::HIT_LOCK, 0);
                lib::step(x);
                return;
            }
            sub8(x, Boss::SHAKE, 1);
            let mut d = 2i16;
            if x.u8(Boss::SHAKE) & 1 != 0 {
                d = -d;
            }
            if x.u8(f::FLIP) != 0 {
                d = -d;
            }
            x.add16(Obj::X, d as u16);
        }
        _ => {}
    }
}

// --- +5 = 2 : recul -----------------------------------------------------------------------------

fn pushed_back<R: Bench>(x: &mut Cx<'_, R>) {
    match x.u8(f::SUB2) {
        0 => {
            x.set16(Boss::TIMER, 0x14);
            let (vx, vz) = if x.u8(f::FLIP) == 0 { (0x400i16, -0x30i16) } else { (-0x400, 0x30) };
            x.set16(Enemy::VX, vx as u16);
            x.set16(Enemy::AX, vz as u16);
            x.set8(Boss::ANIM_KIND, 0x20);
            x.set8(Boss::ANIM_LAST, 0x20);
            x.add8(f::SUB2, 2);
            lib::sound(x, crate::sound::CMD_126_226_B);
            body(x, 0xb);
        }
        2 => {
            if sub16(x, Boss::TIMER, 1).le() {
                x.add8(f::SUB2, 2);
                x.set16(Enemy::AX, 0);
                let n = if cmp8(x, Boss::PHASE, 2).ge() { 0xd } else { 0xc };
                body(x, n);
            } else {
                x.move_x();
                lib::step(x);
            }
        }
        4 => {
            if !lib::step(x) {
                return;
            }
            x.set32(Enemy::HIT_LOCK, 0);
            if cmp8(x, Boss::PHASE, 2).ge() {
                decide::attack_4_shielded(x);
            } else {
                decide::chase(x);
            }
        }
        _ => {}
    }
}

// --- +5 = 4: getting up ----------------------------------------------------------------------------

fn get_up<R: Bench>(x: &mut Cx<'_, R>) {
    match x.u8(f::SUB2) {
        0 => {
            x.add8(f::SUB2, 2);
            x.set8(Boss::ANIM_LAST, 0xff);
            x.anim_start(SET_BODY.0, 0xe);
        }
        2 => {
            if var::PAUSE.get(x.b) as i8 > 0 {
                return;
            }
            if sub16(x, Boss21::HITS, 1).gt() {
                return;
            }
            x.add8(f::SUB2, 2);
            x.set8(Enemy::VULNERABLE, 1);
            x.set8(Enemy::DOWNED, 0);
            x.set8(Enemy::REACTION, 0);
            x.set16(Enemy::AX, 0);
            x.set16(Enemy::AY, 0);
            let mut edge = var::CAMERA1_X.get(x.b);
            let mut angle = 8;
            edge = edge.wrapping_add(0x30);
            if !Fl::sub16(edge, x.u16(Obj::X)).ge() {
                angle = 0x18;
                edge = edge.wrapping_add(0x120);
                if !Fl::sub16(edge, x.u16(Obj::X)).le() {
                    // between the two edges: no about-face
                    x.add8(f::SUB2, 2);
                    return;
                }
            }
            x.set8(Obj::SPEED_INDEX, angle);
            lib::drift_heading(x);
            speed(x, 0x2b);
            x.set16(Boss::TIMER, 0x1e);
            // the animation depends on the boss's direction relative to the chosen edge
            let wrong = (x.u8(f::FLIP) as i8 as i16 as u16).rotate_right(6) != 0;
            body(x, if wrong { 0xf } else { 0x10 });
        }
        4 => {
            if var::PAUSE.get(x.b) != 0 {
                return;
            }
            if sub16(x, Boss::TIMER, 1).le() {
                x.add8(f::SUB2, 2);
                let side = x.u8(f::FLIP) ^ 0x20;
                x.set8(f::FLIP, side);
                body(x, 0x11);
            } else {
                x.move_xy();
                lib::step(x);
            }
        }
        6 => {
            if var::PAUSE.get(x.b) != 0 {
                return;
            }
            if !x.anim_step() {
                return;
            }
            x.add8(Boss21::STRIKES, 1);
            recovered(x);
        }
        _ => {}
    }
}

/// 0x9a6d4: the boss is standing; chooses what follows according to its phase.
pub(crate) fn recovered<R: Bench>(x: &mut Cx<'_, R>) {
    x.set8(Enemy::ACTIVE, 1);
    x.set8(Boss::ANIM_LAST, 0xff);
    if !pick_target(x) {
        return decide::wait(x);
    }
    lib::next_threshold(x, THRESHOLDS.0);
    let mut limit = 2;
    if (var::PLAYERS_ALIVE.get(x.b) as i8) > 2 {
        limit = 0;
    }
    if !Fl::sub8(limit, x.u8(Boss::PHASE)).gt() {
        if x.u8(Boss21::SLOW) != 0 {
            return decide::chase(x);
        }
        if Fl::sub8(x.u8(Boss21::STRIKE_LIMIT), x.u8(Boss21::STRIKES)).lt() {
            return decide::attack_16(x);
        }
    }
    if lib::target_in_box(x, 0, 0x20, 0xc) {
        decide::attack_4_shielded(x);
    } else {
        decide::chase(x);
    }
}

// --- +5 = 6, 8 : mort ---------------------------------------------------------------------------

fn down<R: Bench>(x: &mut Cx<'_, R>) {
    match x.u8(f::SUB2) {
        0 => {
            x.add8(f::SUB2, 2);
            x.set32(f::IMMUNITY, 0xffff_ffff);
            lib::object_event(x);
            x.set8(Enemy::ACTIVE, 2);
            x.set8(Boss21::SLOW, 0);
            lib::sound(x, crate::sound::CMD_83_83);
            lib::drop_rewards(x);
            x.set16(Enemy::AX, 0);
            x.set16(Enemy::VZ, 0x500);
            x.set16(Enemy::GRAVITY, (-0x38i16) as u16);
            let mut vx: i16 = 0x200;
            let mut side = 0;
            let edge = var::CAMERA1_X.get(x.b).wrapping_add(0xc0);
            if !Fl::sub16(edge, x.u16(Obj::X)).ge() {
                vx = -vx;
                side = 0x20;
            }
            x.set16(Enemy::VX, vx as u16);
            x.set8(f::FLIP, side);
            x.move_xz();
            x.anim_start(SET_BODY.0, 0x12);
        }
        2 => {
            x.move_xz();
            if cmp16(x, Enemy::VZ, 0x100).gt() {
                x.anim_step();
            } else {
                x.add8(f::SUB2, 2);
                x.anim_start(SET_BODY.0, 0x13);
            }
        }
        4 => {
            x.move_xz();
            if cmp16(x, Enemy::VZ, 0xff00).gt() {
                x.anim_step();
            } else {
                x.add8(f::SUB2, 2);
                x.anim_start(SET_BODY.0, 0x14);
            }
        }
        6 => {
            x.move_xz();
            if tst16(x, Obj::Z).gt() {
                return;
            }
            x.add8(f::SUB2, 2);
            x.set16(Obj::Z, 0);
            x.anim_start(SET_BODY.0, 0x15);
        }
        8 => {
            if x.anim_step() {
                x.add8(f::SUB2, 2);
            }
        }
        _ => {}
    }
}

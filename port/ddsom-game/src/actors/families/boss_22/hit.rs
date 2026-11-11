//! Boss 22, state 4 "hit" (0x9cdc6). `+5` groups:
//!
//! | `+5` | Role |
//! |---|---|
//! | 0 | reaction to the hit: common routine chosen by the code `+0x49` (table 0x9ce66) |
//! | 2 | knockback, sliding |
//! | 4 | on the ground, getting up |
//! | 6 | death: leap, landing, handing over to the host |
//! | 8, 10 | return to combat |

use super::*;
use crate::actors::families::cx::Bench;
use crate::world::objects::Current;


pub(super) fn run<R: Bench>(x: &mut Cx<'_, R>) {
    if x.u8(Boss22::HIT_TICK) == 0 {
        var::CAMERA_MODE.set(x.b, 0);
        var::INPUT_MASK.set(x.b, 0xff);
        if two(x) {
            x.set8(Enemy::VULNERABLE, 0);
        }
        x.set8(Boss22::HIT_TICK, 1);
    }
    x.set8(Boss22::BUSY, 0);
    if tst8(x, f::FREEZE).gt() {
        sub8(x, f::FREEZE, 1);
    } else {
        match x.u8(f::SUB) {
            0 => reaction(x),
            2 => pushed_back(x),
            4 => down(x),
            6 => death(x),
            8 | 10 => x.set32(f::STATE, state::STAND),
            _ => {}
        }
    }
    if x.u8(f::SUB) != 6 {
        lib::keep_on_screen(x, 0x90);
        lib::collide_terrain(x);
    }
    x.reinsert();
}

/// 0x9ce28: damage (0x10112); with no HP, death, otherwise the common routine of the hit code.
fn reaction<R: Bench>(x: &mut Cx<'_, R>) {
    if !second_form(x) {
        x.set8(Boss22::SECOND_FX, 0);
    }
    lib::set_68(x);
    x.set8(Boss22::ARM_TIMER, 0);
    if !tst16(x, f::HP).gt() {
        x.set8(f::SUB, 6);
        x.set8(Enemy::ACTIVE, 2);
        return death(x);
    }
    x.add16(Boss22::GUARD, 0xffff);
    x.set8(Boss22::ANIM_LAST, 0x7f);
    let table = kit(x).reactions.0;
    react(x, table);
}

/// A dust puff (0xb10c8) without the counter of eight: type 2 drops one when it is pushed back to the right.
fn rubble_once<R: Bench>(x: &mut Cx<'_, R>) {
    lib::shock(x, 0, 0x201, 0x20, 0x20);
}

/// 0x9cf12: knockback sliding, then return to combat or counterattack.
fn pushed_back<R: Bench>(x: &mut Cx<'_, R>) {
    match x.u8(f::SUB2) {
        0 => {
            next6(x);
            x.set8(Boss22::ANIM_ID, 0x1e);
            x.set8(Boss22::ANIM_LAST, 0x1e);
            lib::set_68(x);
            lib::sound(x, crate::hw::sound::CMD_127_227_B);
            let side = x.u8(Enemy::ATTACKER_FLIP);
            x.set8(f::FLIP, side);
            let side = x.u8(f::FLIP) ^ 0x20;
            x.set8(f::FLIP, side);
            x.set16(Enemy::VX, 0x400);
            x.set16(Enemy::AX, (-0x28i16) as u16);
            if x.u8(f::FLIP) != 0 {
                if two(x) {
                    rubble_once(x);
                }
                x.set16(Enemy::VX, 0xfc00);
                x.set16(Enemy::AX, 0x28);
            }
            let n = if second_form(x) { 0x12 } else { 0xe };
            start(x, Set::A, n);
        }
        2 => {
            // `andi.w #3,(0x6e,A5)` also reduces the counter in the working RAM
            let tick = var::FRAME_COUNTER.get(x.b) & 3;
            var::FRAME_COUNTER.set(x.b, tick);
            if tick == 0 {
                lib::shock(x, 1, 2, 0x30, 0xffff);
            }
            if x.u8(Enemy::VX) != 0 {
                x.move_x();
                return;
            }
            if !x.anim_step() {
                return;
            }
            x.set32(Enemy::HIT_LOCK, 0);
            x.set32(Enemy::VX_LONG, 0);
            if second_form(x) {
                x.set32(f::STATE, 0x0204_1004);
                x.set8(Boss22::ANIM_LAST, 0x7f);
                return;
            }
            x.set8(Boss22::SECOND_FX, 0);
            let alive = var::PLAYERS_ALIVE.get(x.b) as u32;
            let k = kit(x);
            let chances = long(x, k.chances, alive << k.chances_shift);
            let r = rand_and(x, 0x1f);
            if chances >> r & 1 != 0 {
                let fuse = if two(x) { 0x50 } else { 0x46 };
                x.set8(Boss22::ARM_TIMER, fuse);
                return x.set32(f::STATE, 0x0204_1600);
            }
            x.set8(Enemy::ACTIVE, 1);
            x.set32(f::STATE, state::STAND);
        }
        _ => {}
    }
}

/// 0x9d026: on the ground (`+0x84` frames outside pause), then getting up.
fn down<R: Bench>(x: &mut Cx<'_, R>) {
    match x.u8(f::SUB2) {
        0 => {
            next6(x);
            lib::behaviour_by_kind(x);
            let n = if second_form(x) { 0x10 } else { 0xd };
            start(x, Set::A, n);
        }
        2 => {
            if var::PAUSE.get(x.b) != 0 || sub16(x, Boss::DOWN_TIME, 1).gt() {
                return;
            }
            next6(x);
            x.set8(Enemy::DOWNED, 0);
            x.set16(Boss::DOWN_TIME, 0);
            x.set8(Enemy::REACTION, 0);
        }
        4 => {
            if var::PAUSE.get(x.b) != 0 || !x.anim_step() {
                return;
            }
            x.set8(Enemy::ACTIVE, 1);
            x.set8(Enemy::VULNERABLE, 1);
            if second_form(x) {
                x.set32(f::STATE, 0x0204_1004);
                x.set8(Boss22::ANIM_LAST, 0x7f);
                return;
            }
            if two(x) {
                x.set8(Boss22::SECOND_FX, 0);
            }
            x.set32(f::STATE, state::STAND);
            x.set8(Boss22::ANIM_ID, 0);
            x.set8(Boss22::ANIM_LAST, 0);
            start(x, Set::A, 0);
        }
        _ => {}
    }
}

/// 0x9d0d6: death. If the host died before or at the same time, only a fall (0x19ad6); otherwise the stage is
/// notified: boss defeated (type 2).
fn death<R: Bench>(x: &mut Cx<'_, R>) {
    match x.u8(f::SUB2) {
        0 => {
            lib::sound(x, crate::hw::sound::CMD_FF);
            x.set32(f::IMMUNITY, 0xffff_ffff);
            x.set8(Enemy::ACTIVE, 2);
            let freeze = x.u8(Enemy::STUN);
            x.set8(f::FREEZE, freeze);
            let host = x.linked(Boss22::HOST);
            let mut last = true;
            if !two(x) || x.u8(Boss22::PARTNERED) != 0 {
                if host.u32(x.b, f::STATE) == x.u32(f::STATE) {
                    x.set8(Enemy::EVENT_LOW, 1);
                    last = false;
                } else if host.u8(x.b, Enemy::EVENT_LOW) == 0 && host.u16(x.b, f::STATE) != 0x406 {
                    last = false;
                }
            }
            if last {
                var::PLAYERS_FROZEN.set(x.b, 1);
                var::BOSS_FIGHT.set(x.b, 0);
                record_defeat(x, 2);
                x.set8(Boss::DEATH_CREDIT, 1);
                var::BOSS_DOWN.set(x.b, 0xff);
                crate::hw::sound::service(x.b, crate::hw::sound::Service::Ff05);
                lib::sound(x, crate::hw::sound::CMD_FF);
                lib::flash_and_shake(x);
            } else {
                lib::object_event(x);
            }
            lib::drop_rewards(x);
            x.set16(Enemy::AX, 0);
            x.set16(Enemy::VZ, 0x600);
            x.set16(Enemy::GRAVITY, (-0x38i16) as u16);
            // the original loads 0x180 into D0 then overwrites it with the camera position: the speed is that position
            // + 0xc0
            let center = var::SCROLL_X.get(x.b).wrapping_add(0xc0);
            let (vx, side) = if Fl::sub16(center, x.u16(Obj::X)).ge() { (center, 0u8) } else { (center.wrapping_neg(), 0x20) };
            x.set16(Enemy::VX, vx);
            x.set8(f::FLIP, side);
            x.move_xz();
            next6(x);
            let n = if second_form(x) { 0x17 } else { 0x16 };
            start(x, Set::A, n);
        }
        2 => {
            x.move_xz();
            if (x.u16(Obj::Z) as i16) > 0 {
                return;
            }
            next6(x);
            x.set16(Obj::Z, 0);
            let right = x.u8(f::FLIP) != 0;
            dust_trail(x, right);
            lib::sound(x, crate::hw::sound::CMD_35D);
            if !second_form(x) {
                effct_33(x);
            }
            start(x, Set::A, 0x18);
        }
        4 => {
            if x.anim_step() {
                next6(x);
            }
        }
        _ => {}
    }
}

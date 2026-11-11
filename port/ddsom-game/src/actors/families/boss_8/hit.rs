//! Boss 8, state 4 "hit" (0x89292). `+5` groups: 0 reaction (common enemy routine chosen by the hit code
//! `+0x49` in the table of longs 0x89330), 2 thrown, 4 fall, 6 death, 8 and 10 return to combat.

use crate::actors::families::cx::Bench;
use crate::world::data::RomAddr;
use super::aids;
use super::*;
use crate::world::objects::Current;
use crate::world::machine::Machine;

/// Table of reaction routines, indexed by the hit code `+0x49` (longs, all common).
const REACTIONS: RomAddr = RomAddr(0x89330);

pub(super) fn run<R: Bench>(x: &mut Cx<'_, R>) {
    x.set8(Warrior::HIT_ACK, 0);
    if tst8(x, f::FREEZE).gt() {
        sub8(x, f::FREEZE, 1);
    } else {
        match x.u8(f::SUB) {
            0 => reaction(x),
            2 => thrown(x),
            4 => fall(x),
            6 => death(x),
            _ => x.set32(f::STATE, 0x0202_0000),
        }
    }
    lib::keep_in_zone(x);
    lib::collide_terrain(x);
}


fn paused<R: Bench>(x: &mut Cx<'_, R>) -> bool {
    var::PAUSE.get(x.b) != 0
}

/// 0x892ce: the hit is handled by the common routine; with no HP the boss dies.
fn reaction<R: Bench>(x: &mut Cx<'_, R>) {
    if x.u8(f::SUB2) == 0 {
        x.set_bit(Enemy::DISPLAY, 1, false);
    }
    if x.u8(Enemy::REACTION) != 0xa8 {
        x.set8(Enemy::GRABBED, 0);
    }
    lib::set_68(x);
    x.set8(Warrior::PHASE_TIMER, 0);
    if (x.u16(f::HP) as i16) <= 0 {
        x.set8(f::SUB, 6);
        x.set8(Enemy::ACTIVE, 2);
        return death(x);
    }
    sub16(x, Warrior::IDLE_WAIT, 1);
    let routine = reaction_routine(x, REACTIONS.0);
    run_reaction(x, routine);
    if x.u8(f::FREEZE) != 0 && x.u8(Warrior::REPLAY) != 0 {
        addq8(x, f::FREEZE, 1);
        x.set8(Warrior::REPLAY, 0);
    }
}

/// 0x893dc: the boss is thrown backward, bounces, then gets up or counterattacks.
fn thrown<R: Bench>(x: &mut Cx<'_, R>) {
    if paused(x) {
        return;
    }
    match x.u8(f::SUB2) {
        0 => {
            x.add8(f::SUB2, 2);
            x.set8(Warrior::ANIM_KIND, 0x1e);
            x.set8(Warrior::ANIM_LAST, 0x1e);
            lib::set_68(x);
            x.set8(Enemy::ACTIVE, 2);
            x.set16(Enemy::VX, 0x500);
            x.set16(Enemy::AX, (-0x28i16) as u16);
            let side = x.u8(Enemy::ATTACKER_FLIP) ^ 0x20;
            x.set8(f::FLIP, side);
            if side != 0 {
                let (a, b) = (x.u16(Enemy::VX).wrapping_neg(), x.u16(Enemy::AX).wrapping_neg());
                x.set16(Enemy::VX, a);
                x.set16(Enemy::AX, b);
            }
            body(x, 9);
        }
        2 => {
            let v = var::FRAME_COUNTER.get(x.b) & 3;
            var::FRAME_COUNTER.set(x.b, v);
            if v == 0 {
                lib::shock(x, 1, 2, 0x20, 0xffff);
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
            let n = var::PLAYERS_ALIVE.get(x.b) as u32;
            // D5 = the index (leftover re-read by the following handlers, `dispatch::Leftovers`)
            let i = ((n as u16) << 2) as u32;
            *x.c.d5() = i;
            let mask = long(x, "boss_8/counter_masks", i);
            let rng = x.random();
            let i = rng & 0x1f;
            if mask >> i & 1 == 0 {
                x.set8(Enemy::ACTIVE, 1);
                x.set32(f::STATE, 0x0202_0000);
                return;
            }
            x.set8(Enemy::STRIKE_POWER, 5);
            x.set8(Enemy::STRIKE_KIND, 0x44);
            x.set8(Enemy::ACTIVE, 1);
            x.add8(f::SUB2, 2);
            strike(x, 6);
        }
        _ => {
            if x.anim_step() {
                x.set32(f::STATE, 0x0202_0000);
            }
        }
    }
}

/// 0x894f4: fall, wait on the ground, getting up.
fn fall<R: Bench>(x: &mut Cx<'_, R>) {
    match x.u8(f::SUB2) {
        0 => {
            x.add8(f::SUB2, 2);
            lib::behaviour_by_kind(x);
            x.set16(Obj::Z, 0);
            body(x, 8);
        }
        2 => {
            if paused(x) {
                return;
            }
            if sub16(x, Warrior::HITS, 1).gt() {
                return;
            }
            x.add8(f::SUB2, 2);
            x.set8(Enemy::ACTIVE, 2);
            x.set16(Warrior::HITS, 0);
            x.set8(Enemy::DOWNED, 0);
            x.set8(Enemy::REACTION, 0);
        }
        4 => {
            if paused(x) || !x.anim_step() {
                return;
            }
            x.add8(f::SUB2, 2);
            aids::dodge_nearby(x);
            x.set16(Warrior::TIMER, 8);
            x.set8(Warrior::PHASE_TIMER, 0x10);
            x.set8(Enemy::ACTIVE, 1);
            x.set_bit(Enemy::DISPLAY, 1, true);
            body(x, 5);
        }
        6 => {
            if paused(x) {
                return;
            }
            if sub16(x, Warrior::TIMER, 1).mi() {
                x.add8(f::SUB2, 2);
                x.set_bit(Enemy::DISPLAY, 1, false);
            }
        }
        _ => {
            if paused(x) || !x.anim_step() {
                return;
            }
            x.set8(Enemy::VULNERABLE, 1);
            x.set32(f::STATE, 0x0202_0000);
            x.set8(Enemy::ACTIVE, 1);
        }
    }
}

/// 0x895c2: death (fall, effects, wait, blinking, release).
fn death<R: Bench>(x: &mut Cx<'_, R>) {
    match x.u8(f::SUB2) {
        0 => {
            x.add8(f::SUB2, 2);
            lib::sound(x, crate::hw::sound::CMD_344_344);
            x.set8(Enemy::ACTIVE, 2);
            let stun = x.u8(Enemy::STUN);
            x.set8(f::FREEZE, stun);
            var::PLAYERS_FROZEN.set(x.b, 1);
            var::BOSS_FIGHT.set(x.b, 0);
            lib::flash_and_shake(x);
            lib::sound(x, crate::hw::sound::CMD_FF);
            crate::hw::sound::service(x.b, crate::hw::sound::Service::Ff05);
            lib::drop_rewards(x);
            x.set16(Enemy::AX, 0);
            x.set16(Enemy::VZ, 0x500);
            x.set16(Enemy::GRAVITY, (-0x38i16) as u16);
            let side = x.u8(Enemy::ATTACKER_FLIP) ^ 0x20;
            x.set8(f::FLIP, side);
            let mut v = (-0x300i16) as u16;
            if x.u8(f::FLIP) == 0 {
                v = v.wrapping_neg();
            }
            x.set16(Enemy::VX, v);
            x.move_xz();
            body(x, 0xa);
        }
        2 => {
            if x.u8(f::KEY_FRAME) != 0 {
                x.set8(f::KEY_FRAME, 0);
                crate::actors::families::effects_creators::linked(x.b, x.o, crate::actors::families::effects_creators::kind::BUBBLE, None);
            }
            x.move_xz();
            if (x.u16(Obj::Z) as i16) > 0 {
                x.anim_step();
                return;
            }
            x.add8(f::SUB2, 2);
            x.set16(Obj::Z, 0);
            if x.u8(f::FLIP) != 0 {
                aids::burst_back(x);
            } else {
                aids::burst_front(x);
            }
            lib::sound(x, crate::hw::sound::CMD_35D);
            body(x, 0xb);
        }
        4 => {
            if x.anim_step() {
                x.set16(Warrior::TIMER, 0x96);
                x.add8(f::SUB2, 2);
                record_defeat(x, 8);
            }
        }
        6 => {
            if sub16(x, Warrior::TIMER, 1).mi() {
                x.add8(f::SUB2, 2);
                x.set16(Warrior::TIMER, 0x3c);
            }
        }
        _ => {
            if sub16(x, Warrior::TIMER, 1).mi() {
                x.set8(f::STATE, 6);
                return;
            }
            x.set_bit(Enemy::DISPLAY, 1, false);
            let b = x.u16(Warrior::TIMER) & 1;
            if b != 0 {
                x.set_bit(Enemy::DISPLAY, 1, true);
            }
        }
    }
}

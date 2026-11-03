//! Boss 11/12, state 4 "hit" (0x8e39a). `+5` groups: 0 reaction (table of longs 0x8e3dc: six routines according to
//! the hit code `+0x49`), 4 stunned, 2, 6, 8, 10 death.

use crate::families::cx::Bench;
use super::*;
use crate::data::RomAddr;
use crate::objects::Current;
use crate::machine::Machine;

pub(super) fn run<R: Bench>(x: &mut Cx<'_, R>) {
    if tst8(x, f::FREEZE).gt() {
        sub8(x, f::FREEZE, 1);
        return;
    }
    x.set8(Boss11::ATTACKING, 0);
    match x.u8(f::SUB) {
        0 => reaction(x),
        4 => stunned(x),
        _ => death(x),
    }
}

/// 0x8e8ca: true if the game is paused (the snake then stays displayed: `+0` = 2).
/// Reaction table (one routine per hit code `+0x49`, `reactions/boss_11`).
const REACTIONS: RomAddr = RomAddr(0x8e3dc);

fn paused<R: Bench>(x: &mut Cx<'_, R>) -> bool {
    if var::PAUSE.get(x.b) != 0 {
        x.set8(Enemy::ACTIVE, 2);
        return true;
    }
    false
}

/// 0x8e8de: end of the reaction, return to the pursuit.
fn recover<R: Bench>(x: &mut Cx<'_, R>) {
    x.set8(Enemy::ACTIVE, 1);
    x.set8(Enemy::DOWNED, 0);
    x.set8(Enemy::GRABBED, 0);
    x.set8(Enemy::REACTION, 0);
    x.set32(Enemy::HIT_LOCK, 0);
    x.set32(Enemy::RESUME_STATE, 0);
    to_chase(x)
}

/// Out of HP: group 6 (death) takes over.
fn dying<R: Bench>(x: &mut Cx<'_, R>) {
    x.set8(f::SUB, 6);
    x.set16(f::SUB2, 0);
    death(x)
}

fn reaction<R: Bench>(x: &mut Cx<'_, R>) {
    let routine = reaction_routine(x, REACTIONS.0);
    match routine {
        0x8e488 => {
            if var::PAUSE.get(x.b) != 0 {
                return;
            }
            x.set16(f::HP, 0);
            x.set8(f::SUB, 6);
            x.set16(f::SUB2, 0);
            death(x)
        }
        0x8e4a2 => hit_a(x),
        0x8e574 => hit_b(x),
        0x8e5da => generic(x),
        0x8e69e => stun(x),
        _ => throw(x),
    }
}

/// 0x8e4a2 : recul (codes 0, 0x1c, 0x38, 0x3c, 0x6c, 0x98).
fn hit_a<R: Bench>(x: &mut Cx<'_, R>) {
    if x.u8(f::SUB2) == 0 {
        if (x.u16(f::HP) as i16) <= 0 {
            return dying(x);
        }
        if Fl::sub16(x.u16(Obj::Z), 0x10).gt() {
            x.set8(Enemy::REACTION, 4);
            return generic(x);
        }
        x.set16(Obj::Z, 0);
        x.add8(f::SUB2, 2);
        lib::sound(x, crate::sound::CMD_412);
        if x.u8(Enemy::REACTION) == 0x78 {
            lib::attack_effect(x, 1, 0);
        }
        paused(x);
        let stun = x.u8(Enemy::STUN);
        x.set8(f::FREEZE, stun);
        x.set8(Boss::SHAKE, 0x10);
        stop(x);
        let (mut d0, mut d1) = (0x400u16, (-0x48i16) as u16);
        if x.u8(f::FLIP) != 0 {
            d0 = d0.wrapping_neg();
            d1 = d1.wrapping_neg();
        }
        x.set16(Enemy::VX, d0);
        x.set16(Enemy::AX, d1);
        return hurt(x, 0);
    }
    if paused(x) {
        return;
    }
    x.set8(Enemy::ACTIVE, 1);
    x.move_x();
    let mut d0 = x.u16(Enemy::VX);
    if x.u8(f::FLIP) != 0 {
        d0 = d0.wrapping_neg();
    }
    if (d0 as i16) < 0 {
        x.set32(Enemy::VX_LONG, 0);
    }
    if !x.anim_step() {
        return sync(x);
    }
    if x.u8(Enemy::GRABBED) == 0 {
        return recover(x);
    }
    x.set8(Enemy::REACTION, 0x68);
    x.set8(f::SUB2, 0);
    hit_b(x)
}

/// 0x8e574: knockback repeated `+0x87` times (code 0x68).
fn hit_b<R: Bench>(x: &mut Cx<'_, R>) {
    if x.u8(f::SUB2) != 0 {
        if !sub8(x, Enemy::GRABBED, 1).eq() {
            x.anim_step();
            return sync(x);
        }
        return recover(x);
    }
    if (x.u16(f::HP) as i16) <= 0 {
        return dying(x);
    }
    if Fl::sub16(x.u16(Obj::Z), 0x10).gt() {
        x.set8(Enemy::REACTION, 4);
        return generic(x);
    }
    x.set16(Obj::Z, 0);
    x.add8(f::SUB2, 2);
    paused(x);
    let stun = x.u8(Enemy::STUN);
    x.set8(f::FREEZE, stun);
    stop(x);
    body(x, 2);
    crate::families::effects_creators::attack(x.b, x.o, 3, 0);
}

/// 0x8e5da: throw (codes 4 to 0x94 with no routine of their own).
fn generic<R: Bench>(x: &mut Cx<'_, R>) {
    match x.u8(f::SUB2) {
        0 => {
            if (x.u16(f::HP) as i16) <= 0 {
                return dying(x);
            }
            x.add8(f::SUB2, 2);
            lib::sound(x, crate::sound::CMD_412);
            x.set8(Enemy::GRABBED, 0);
            paused(x);
            let stun = x.u8(Enemy::STUN);
            x.set8(f::FREEZE, stun);
            x.set8(Boss::SHAKE, 0x10);
            stop(x);
            let mut d0 = 0x500u16;
            if x.u8(f::FLIP) != 0 {
                d0 = d0.wrapping_neg();
            }
            x.set16(Enemy::VX, d0);
            x.set16(Enemy::VZ, 0x600);
            x.set16(Enemy::GRAVITY, (-0x80i16) as u16);
            hurt(x, 1)
        }
        2 => {
            if paused(x) {
                return;
            }
            x.set8(Enemy::ACTIVE, 1);
            x.anim_step();
            sync(x);
            x.move_xz();
            if tst16(x, Obj::Z).gt() {
                return;
            }
            x.set16(Obj::Z, 0);
            x.add8(f::SUB2, 2);
            stop(x);
            lib::shake_58(x, 8);
            lib::sound(x, crate::sound::CMD_360_460);
            hurt(x, 2)
        }
        _ => {
            if x.anim_step() {
                return fallen(x);
            }
            sync(x);
        }
    }
}

/// 0x8e8fc: getting up after the fall (state 4, group 4).
fn fallen<R: Bench>(x: &mut Cx<'_, R>) {
    x.set8(Enemy::ACTIVE, 1);
    x.set32(f::STATE, 0x0404_0000);
    x.set8(Enemy::DOWNED, 1);
    x.set8(Enemy::GRABBED, 0);
    x.set8(Enemy::REACTION, 0);
    x.set32(Enemy::RESUME_STATE, 0);
    lib::draw_84(x)
}

/// 0x8e69e: stun (code 0x4c).
fn stun<R: Bench>(x: &mut Cx<'_, R>) {
    match x.u8(f::SUB2) {
        0 => {
            if (x.u16(f::HP) as i16) <= 0 {
                return dying(x);
            }
            if !sub8(x, Boss11::STUN_TIME, 1).gt() {
                x.set8(Enemy::ACTIVE, 1);
                x.set8(Enemy::DOWNED, 0);
                x.set8(Enemy::REACTION, 0);
                x.set32(Enemy::RESUME_STATE, 0);
                x.set8(Boss11::SLOW, 8);
                return to_attack_7(x);
            }
            x.add8(f::SUB2, 2);
            let v = x.u16(Boss::DOWN_TIME).wrapping_sub(8);
            x.set16(Boss::DOWN_TIME, v);
            lib::sound(x, crate::sound::CMD_412);
            paused(x);
            let stun = x.u8(Enemy::STUN);
            x.set8(f::FREEZE, stun);
            let px = x.u16(Obj::X);
            x.set16(Enemy::X_SAVED, px);
            x.set8(Boss::SHAKE, 0x10);
            stop(x);
            hurt(x, 2);
            x.set8(Enemy::ACTIVE, 2);
        }
        _ => {
            let d1 = rd16(x.b, x.o.0.wrapping_sub(0x7f92)) & 1;
            let mut d = 2u16;
            if d1 != 0 {
                d = d.wrapping_neg();
            }
            setw(x, 0, d);
            x.add16(Obj::X, d);
            if x.anim_step() {
                let px = x.u16(Enemy::X_SAVED);
                x.set16(Obj::X, px);
                x.set8(Enemy::ACTIVE, 1);
                let mut d0 = 0x0404_0000u32;
                d0 = d0 & !0xff | x.u8(Enemy::SUB_STEP) as u32;
                x.set32(f::STATE, d0);
                x.set8(Enemy::DOWNED, 1);
                x.set8(Enemy::REACTION, 0);
                x.set32(Enemy::HIT_LOCK, 0);
                x.set32(Enemy::RESUME_STATE, 0);
                return;
            }
            sync(x);
        }
    }
}

/// 0x8e740: arcing throw (code 0xa4).
fn throw<R: Bench>(x: &mut Cx<'_, R>) {
    match x.u8(f::SUB2) {
        0 => {
            x.set8(Enemy::VULNERABLE, 0);
            x.set8(Enemy::DOWNED, 0);
            x.add8(f::SUB2, 2);
            x.set32(Enemy::VX_LONG, 0);
            x.set32(Enemy::VZ_LONG, 0);
            x.set32(Enemy::VY_LONG, 0);
            x.set8(Enemy::GRABBED, 0);
            x.set8(Enemy::DOWNED, 0);
            x.set8(Boss::SHAKE, 2);
            if x.counter() & 1 != 0 {
                x.set8(Boss::SHAKE, 1);
            }
            let mut d2 = if x.u8(f::FLIP) != 0 { 0x18 } else { 8 };
            let rng = x.random();
            d2 &= 0xf;
            let d0 = rng.wrapping_add(d2 as u16) & 0x1f;
            x.set8(Enemy::HEADING, d0 as u8);
            hurt(x, 1)
        }
        2 => arc(x),
        4 => {
            x.anim_step();
            sync(x);
            x.move_xyz();
            if tst16(x, Obj::Z).gt() {
                return arc_end(x);
            }
            x.add8(f::SUB2, 2);
            x.set32(Obj::Z, 0);
            x.set32(Enemy::VZ_LONG, 0);
            lib::sound(x, crate::sound::CMD_360_460);
            if (x.u16(f::HP) as i16) > 0 {
                return arc_end(x);
            }
            x.set8(f::SUB, 6);
            x.set16(f::SUB2, 0);
            death(x)
        }
        _ => fallen(x),
    }
}

fn arc_end<R: Bench>(x: &mut Cx<'_, R>) {
    if x.u8(Enemy::BLOCKED) == 0 {
        return;
    }
    x.set16(Enemy::VX, 0);
    x.set16(Enemy::AX, 0);
}

/// 0x8e7b4: the head describes an arc (vectors from table 0x1c94b6).
fn arc<R: Bench>(x: &mut Cx<'_, R>) {
    let z = x.u32(Obj::Z).wrapping_add(0x20000);
    x.set32(Obj::Z, z);
    if !Fl::sub16(x.u16(Obj::Z), 0x60).lt() {
        x.add8(f::SUB2, 2);
        x.set16(Enemy::VZ, 0x200);
        x.set16(Enemy::GRAVITY, (-0x80i16) as u16);
        return;
    }
    if !sub8(x, Boss::SHAKE, 1).eq() {
    } else {
        x.set8(Boss::SHAKE, 1);
        let a = x.u8(Enemy::HEADING).wrapping_add(2) & 0x1f;
        x.set8(Enemy::HEADING, a);
        x.set8(f::FLIP, 0);
        let d0 = a.wrapping_sub(8);
        setb(x, 0, d0);
        if d0 >= 0x10 {
            x.set8(f::FLIP, 0x20);
        }
    }
    let i = (x.u8(Enemy::HEADING) as u16) << 2;
    let vx = tables::circle(x.b, i as u32).wrapping_add(x.u16(Enemy::AX));
    x.set16(Enemy::VX, vx);
    let step = (((vx as u32) << 16) as i32 >> 8) as u32;
    x.add32(Obj::X, step);
    let vy = tables::circle(x.b, 2 + i as u32).wrapping_add(x.u16(Enemy::AY));
    x.set16(Enemy::VY, vy);
    let step = (((vy as u32) << 16) as i32 >> 8) as u32;
    x.add32(Obj::Y, step);
}

/// 0x8e922 (`+5` = 4): on the ground, stunned.
fn stunned<R: Bench>(x: &mut Cx<'_, R>) {
    if paused(x) {
        return;
    }
    x.set8(Enemy::ACTIVE, 1);
    match x.u8(f::SUB2) {
        0 => {
            x.add8(f::SUB2, 2);
            x.set16(Obj::Z, 0);
            wait_init(x);
            hurt(x, 3);
            sync(x);
        }
        2 => {
            if var::PAUSE.get(x.b) != 0 {
                return;
            }
            if sub16(x, Boss::DOWN_TIME, 1).gt() {
                x.anim_step();
                return;
            }
            x.add8(f::SUB2, 2);
            x.set8(Enemy::DOWNED, 0);
            x.set8(Enemy::REACTION, 0);
            body(x, 4)
        }
        _ => {
            if !x.anim_step() {
                return;
            }
            x.set8(Enemy::ACTIVE, 1);
            x.set8(Enemy::DOWNED, 0);
            x.set8(Enemy::REACTION, 0);
            x.set32(Enemy::HIT_LOCK, 0);
            x.set32(Enemy::RESUME_STATE, 0);
            to_attack_3(x)
        }
    }
}

/// 0x8e95a: stun duration (`+0xb8`, depending on difficulty), set on the first frame.
fn wait_init<R: Bench>(x: &mut Cx<'_, R>) {
    if x.u8(f::SUB3) != 0 {
        return;
    }
    x.add8(f::SUB3, 2);
    let level = var::RANK_HIGH.get(x.b) >> 1;
    setb(x, 0, level);
    let v = byte(x, "boss_11/stun_times", level as u32);
    setb(x, 0, v);
    x.set8(Boss11::STUN_TIME, v);
}

/// 0x8e9d2: death (fall, effects, wait for the end of the animation).
fn death<R: Bench>(x: &mut Cx<'_, R>) {
    match x.u8(f::SUB2) {
        0 => {
            x.add8(f::SUB2, 2);
            x.set8(Enemy::ACTIVE, 2);
            var::PLAYERS_FROZEN.set(x.b, 1);
            var::BOSS_FIGHT.set(x.b, 0);
            let kind = x.u8(Enemy::KIND);
            record_defeat(x, kind);
            let mut d0 = 0xb00u16;
            if x.u8(Enemy::KIND) != 0xb {
                d0 = 0xc00;
            }
            setw(x, 0, d0);
            var::DEFEAT_NAME_WORD.set(x.b, d0);
            x.set16(Boss11::TIMER, 0x10);
            lib::drop_rewards(x);
            crate::sound::service(x.b, crate::sound::Service::Ff05);
            lib::sound(x, crate::sound::CMD_FF);
            lib::flash_and_shake(x);
            let mut d0 = 0x500u16;
            setw(x, 0, d0);
            if x.u8(f::FLIP) != 0 {
                d0 = d0.wrapping_neg();
                setw(x, 0, d0);
            }
            x.set16(Enemy::VX, d0);
            x.set16(Enemy::VZ, 0x600);
            x.set16(Enemy::GRAVITY, (-0x80i16) as u16);
            hurt(x, 1);
            lib::sound(x, crate::sound::CMD_413)
        }
        2 => {
            x.anim_step();
            sync(x);
            x.move_xz();
            if tst16(x, Obj::Z).gt() {
                return;
            }
            x.set16(Obj::Z, 0);
            x.add8(f::SUB2, 2);
            stop(x);
            hurt(x, 3);
            lib::shake_58(x, 8)
        }
        _ => {
            x.anim_step();
            sync(x);
        }
    }
}

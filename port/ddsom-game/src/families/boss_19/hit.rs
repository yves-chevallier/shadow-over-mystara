//! Boss 14 / 19, state 4 "hit" (0x908ea). `+5` groups: 0 and 4 reaction to the hit (table 0x909a0 according to the code
//! `+0x49`), 2 knockback (routine 0x90cb8), 6 death.
//!
//! A hit arriving during the breath (attack 4) has no effect on the attack: the dragon resumes it (`resume`).
//! The reaction routines specific to the dragon are 0x90b26 (ordinary hit: knockback, slide), 0x90a4c (code 0x68:
//! lying down), 0x90c34 (0x80) and 0x90a96 (0xa8: trembling then death).

use crate::families::cx::Bench;
use crate::data::RomAddr;
use super::*;
use crate::objects::Current;
use crate::objects::Obj;

/// Table of reaction routines, indexed by the hit code `+0x49` (longs).
const REACTIONS: RomAddr = RomAddr(0x909a0);

pub(super) fn run<R: Bench>(x: &mut Cx<'_, R>) {
    let alive = (x.u16(f::HP) as i16) > 0;
    if alive && x.u8(f::SUB) == 0 && x.u32(Boss19::SAVED_STATE) & 0xffff_ff00 == ATTACK_4 && var::PAUSE.get(x.b) == 0 {
        // hit during the breath: the dragon resumes it
        x.set8(Boss::SHAKE, 0x10);
        x.set8(Boss19::SHIELD, 8);
        let s = x.u32(Boss19::SAVED_STATE);
        x.set32(f::STATE, s);
        x.set8(Enemy::ACTIVE, 1);
        x.set8(Enemy::REACTION, 0);
        x.set8(Boss19::PUSHED, 0x30);
        return active(x);
    }
    if alive && x.u8(f::SUB) != 0 {
        return dispatch(x);
    }
    x.set32(Boss19::SAVED_STATE, 0);
    if tst8(x, f::FREEZE).gt() {
        x.set8(Boss19::SHIELD, 2);
        sub8(x, f::FREEZE, 1);
        return tail(x);
    }
    dispatch(x);
}

fn dispatch<R: Bench>(x: &mut Cx<'_, R>) {
    match x.u8(f::SUB) {
        0 | 4 => reaction(x),
        2 => pushed_back(x),
        6 => death(x),
        // table 0x90970 has six entries: 8 (0x8fae2, `rts`) and 0xa (0x9101a, back to the fight) are never chosen
        // (no writer sets `+5` = 8 or 0xa in state 4: docs/code-mort.md), kept like the original
        8 => {}
        0xa => chase(x),
        s => unreachable!("boss 19 hit: sub-state {s:#x} outside table 0x90970"),
    }
    if tst8(x, Boss19::SHIELD).gt() {
        sub8(x, Boss19::SHIELD, 1);
        x.set8(Enemy::FRAME_TAG, 0);
    }
    tail(x);
}

fn tail<R: Bench>(x: &mut Cx<'_, R>) {
    lib::collide_terrain(x);
    x.reinsert();
}

/// 0x9097c: reaction to the hit: the table's routine, according to the hit code.
fn reaction<R: Bench>(x: &mut Cx<'_, R>) {
    // The original indexes the table by D0.w of which only the low byte is set (the hit code). The high byte is zero:
    // the group dispatch (0x90948) does `moveq #0,D0 ; move.b (5,A0),D0b`, and 0x10112 (and 0xb648) leave it
    // zero.
    if x.u8(f::SUB2) == 0 {
        lib::set_68(x);
        if var::SCENE.get(x.b) == 0x20 {
            lib::drop_once(x);
        }
    }
    let routine = reaction_routine(x, REACTIONS.0);
    match routine {
        0x90b26 => knock(x),
        0x90a4c => lie_down(x),
        0x90c34 => stagger(x),
        0x90a96 => shake(x),
        t => unreachable!("boss 19: reaction {t:x}"),
    }
}

// --- 0x90b26: ordinary hit -------------------------------------------------------------------------

fn knock<R: Bench>(x: &mut Cx<'_, R>) {
    match x.u8(f::SUB2) {
        0 => {
            if (x.u16(f::HP) as i16) <= 0 {
                x.set8(f::SUB, 6);
                return death(x);
            }
            if retaliate(x) {
                return;
            }
            x.add8(f::SUB2, 2);
            let burst = match x.u8(Enemy::REACTION) {
                0x60 => Some(7),
                0x70 | 0x90 => Some(8),
                0x78 => Some(1),
                0x94 => Some(9),
                _ => None,
            };
            if let Some(n) = burst {
                lib::attack_effect(x, n, 0);
            }
            let stun = x.u8(Enemy::STUN);
            x.set8(f::FREEZE, stun);
            x.set8(Boss::SHAKE, 0x10);
            let (vx, vz) = if x.u8(f::FLIP) != 0 { (-0x800i16, 0x80i16) } else { (0x800, -0x80) };
            x.set16(Enemy::VX, vx as u16);
            x.set16(Enemy::AX, vz as u16);
            x.set32(Enemy::VZ_LONG, 0);
            x.set32(Enemy::VY_LONG, 0);
            rest(x);
            knock_slide(x);
        }
        2 => knock_slide(x),
        _ => {}
    }
}

/// 0x90c8c: the counterattack is due (the `+0xac` count is exhausted): the dragon attacks (attack 10) instead of
/// recoiling.
fn retaliate<R: Bench>(x: &mut Cx<'_, R>) -> bool {
    if sub8(x, Boss19::RETALIATE, 1).gt() {
        return false;
    }
    x.set8(Enemy::ACTIVE, 1);
    x.set32(f::STATE, ATTACK_10);
    x.set8(Boss19::SHIELD, 8);
    x.set8(Enemy::REACTION, 0);
    x.set32(Enemy::HIT_LOCK, 0);
    true
}

fn knock_slide<R: Bench>(x: &mut Cx<'_, R>) {
    if var::PAUSE.get(x.b) != 0 {
        x.set8(Enemy::ACTIVE, 2);
        return;
    }
    x.set8(Enemy::ACTIVE, 1);
    if x.u16(Enemy::VX) != 0 {
        x.move_xz();
        let mut v = x.u16(Enemy::VX) as i16;
        if x.u8(f::FLIP) == 0 {
            v = v.wrapping_neg();
        }
        if v < 0 {
            return;
        }
        x.set32(Enemy::VX_LONG, 0);
        return;
    }
    if tst8(x, Boss::SHAKE).gt() {
        sub8(x, Boss::SHAKE, 1);
        let mut d: i16 = 2;
        if x.u8(Boss::SHAKE) & 1 != 0 {
            d = -d;
        }
        x.add16(Obj::X, d as u16);
        return;
    }
    recover(x);
}

/// 0x90c6c: end of the reaction: standing.
fn recover<R: Bench>(x: &mut Cx<'_, R>) {
    x.set8(Enemy::ACTIVE, 1);
    x.set8(Enemy::GRABBED, 0);
    x.set8(Enemy::REACTION, 0);
    x.set32(Enemy::HIT_LOCK, 0);
    stand(x);
}

// --- 0x90a4c: lying down (code 0x68) ------------------------------------------------------------------------

fn lie_down<R: Bench>(x: &mut Cx<'_, R>) {
    x.set8(Boss19::HURT_BOX, 5);
    if x.u8(f::SUB2) != 0 {
        if sub8(x, Enemy::GRABBED, 1).eq() {
            return recover(x);
        }
        x.set8(Boss19::HOVER, 1);
        hover(x);
        x.anim_step();
        return;
    }
    if (x.u16(f::HP) as i16) <= 0 {
        x.set8(f::SUB, 6);
        return death(x);
    }
    x.add8(f::SUB2, 2);
    rest(x);
    lib::attack_effect(x, 3, 0);
}

// --- 0x90c34: staggering (code 0x80) -----------------------------------------------------------------------

fn stagger<R: Bench>(x: &mut Cx<'_, R>) {
    x.set8(Boss19::HURT_BOX, 5);
    match x.u8(f::SUB2) {
        0 => {
            x.add8(f::SUB2, 2);
            rest(x);
            stagger_wait(x);
        }
        2 => stagger_wait(x),
        _ => {}
    }
}

fn stagger_wait<R: Bench>(x: &mut Cx<'_, R>) {
    if var::PAUSE.get(x.b) != 0 {
        return;
    }
    x.set8(Enemy::REACTION, 0);
    x.set8(f::SUB2, 0);
    knock(x);
}

// --- 0x90a96: trembling then death (code 0xa8) ---------------------------------------------------------------

fn shake<R: Bench>(x: &mut Cx<'_, R>) {
    if x.u8(f::SUB2) == 0 {
        x.add8(f::SUB2, 2);
        let z = x.u16(Obj::Z);
        x.set16(Enemy::VZ, z);
        let px = x.u16(Obj::X);
        x.set16(Enemy::GRAVITY, px);
        x.set16(Boss19::TIMER, 0x100);
    }
    let t = x.u16(Boss19::TIMER).wrapping_sub(1);
    if t & 0x8000 != 0 {
        let (px, z) = (x.u16(Enemy::GRAVITY), x.u16(Enemy::VZ));
        x.set16(Obj::X, px);
        x.set16(Obj::Z, z);
        x.set16(f::HP, 0);
        x.set8(f::SUB, 6);
        x.set16(f::SUB2, 0);
        return death(x);
    }
    x.set16(Boss19::TIMER, t);
    let mut d0 = t as i16;
    // jump table 0x90ada: 0x90ae4, 0x90afa, 0x90ae2, 0x90af8
    match t & 3 {
        0 => shake_x(x, d0),
        1 => shake_z(x, d0),
        2 => {
            d0 = d0.wrapping_neg();
            shake_x(x, d0);
        }
        _ => {
            d0 = d0.wrapping_neg();
            shake_z(x, d0);
        }
    }
}

fn shake_x<R: Bench>(x: &mut Cx<'_, R>, d0: i16) {
    let px = x.u16(Enemy::GRAVITY).wrapping_add((d0 >> 2) as u16);
    x.set16(Obj::X, px);
    let z = x.u16(Enemy::VZ);
    x.set16(Obj::Z, z);
}

fn shake_z<R: Bench>(x: &mut Cx<'_, R>, d0: i16) {
    let z = x.u16(Enemy::VZ).wrapping_add((d0 >> 1) as u16);
    x.set16(Obj::Z, z);
}

// --- +5 = 2: knockback from a big hit (0x90cb8) --------------------------------------------------------------------

fn pushed_back<R: Bench>(x: &mut Cx<'_, R>) {
    if var::PAUSE.get(x.b) != 0 {
        return;
    }
    if x.u8(f::SUB2) == 0 {
        x.add8(f::SUB2, 2);
        let half = x.u8(Enemy::STUN) >> 1;
        x.set8(f::FREEZE, half);
        x.set16(Boss19::TIMER, 0xf);
        x.set16(Enemy::VX, 0x480);
        x.set16(Enemy::AX, (-0x80i16) as u16);
        let side = x.u8(Enemy::ATTACKER_FLIP) ^ 0x20;
        x.set8(f::FLIP, side);
        if side != 0 {
            let (a, b) = (x.u16(Enemy::VX).wrapping_neg(), x.u16(Enemy::AX).wrapping_neg());
            x.set16(Enemy::VX, a);
            x.set16(Enemy::AX, b);
        }
    }
    if x.u16(Boss19::TIMER) != 0 {
        let t = x.u16(Boss19::TIMER) - 1;
        x.set16(Boss19::TIMER, t);
        if t != 0 {
            x.anim_step();
            return;
        }
    }
    if x.u8(Enemy::VX) != 0 {
        x.move_x();
        return;
    }
    x.set32(Enemy::HIT_LOCK, 0);
    stand(x);
}

// --- death ----------------------------------------------------------------------------------------------------------

/// 0x90dca: death sequence, specific to each type and variant.
fn death<R: Bench>(x: &mut Cx<'_, R>) {
    x.set8(Boss19::HURT_BOX, 0);
    if x.u8(Enemy::KIND) == TYPE_GREEN {
        return death_green(x);
    }
    if x.u8(Obj::VARIANT) != 0 {
        return death_pet(x);
    }
    match x.u8(f::SUB2) {
        0 => {
            x.add8(f::SUB2, 2);
            x.set8(Enemy::ACTIVE, 2);
            var::PLAYERS_FROZEN.set(x.b, 1);
            var::BOSS_FIGHT.set(x.b, 0);
            var::DEFEAT_NAME_WORD.set(x.b, 0x1300);
            let bar = x.u8(Enemy::EVENT_KIND);
            var::DEFEAT_BONUS.set(x.b, bar);
            fall_start(x);
            lib::drop_rewards(x);
            lib::flash_and_shake(x);
            crate::sound::service(x.b, crate::sound::Service::Ff05);
            lib::sound(x, crate::sound::CMD_313_413);
        }
        2 => {
            x.move_xz();
            let cam_y = camera_y(x);
            let d = x.u16(Obj::Y).wrapping_sub(cam_y).wrapping_add(x.u16(Obj::Z));
            if (d as i16) <= 0x100 {
                return;
            }
            x.add8(f::SUB2, 2);
            x.set8(Boss19::TIMER, 0x20);
            x.set16(Obj::Y, cam_y);
            x.set16(Obj::Z, 0x100);
            x.set32(Enemy::VX_LONG, 0);
            x.set32(Enemy::VY_LONG, 0);
            x.set16(Enemy::VZ, (-0x400i16) as u16);
            x.set16(Enemy::GRAVITY, 0);
            set_anim(x, 4, 2);
            fall_end(x);
        }
        4 => fall_end(x),
        _ => {}
    }
}

fn camera_y<R: Bench>(x: &mut Cx<'_, R>) -> u16 {
    if var::CAMERA_SELECT.get(x.b) != 0 {
        var::CAMERA_ALT_Y.get(x.b)
    } else {
        var::SCROLL_Y.get(x.b)
    }
}


/// Start of the fall of the non-green dragons: invulnerability freeze, descent of 0x200.
fn fall_start<R: Bench>(x: &mut Cx<'_, R>) {
    let stun = x.u8(Enemy::STUN);
    x.set8(f::FREEZE, stun);
    x.set32(Enemy::VX_LONG, 0);
    x.set16(Enemy::VZ, 0x200);
    x.set16(Enemy::GRAVITY, 0);
    rest(x);
}

/// End of the fall: the camera is passed from below, the dragon is released.
fn fall_end<R: Bench>(x: &mut Cx<'_, R>) {
    x.move_xz();
    if (x.u16(Obj::Z) as i16) > -0x100 {
        return;
    }
    release(x);
}

fn release<R: Bench>(x: &mut Cx<'_, R>) {
    x.set32(f::STATE, FREE);
}

/// Green dragon: same fall with its own steps.
fn death_green<R: Bench>(x: &mut Cx<'_, R>) {
    match x.u8(f::SUB2) {
        0 => {
            x.add8(f::SUB2, 2);
            x.set8(Enemy::ACTIVE, 2);
            var::PLAYERS_FROZEN.set(x.b, 1);
            var::BOSS_FIGHT.set(x.b, 0);
            var::DEFEAT_NAME_WORD.set(x.b, 0x0e00);
            let bar = x.u8(Enemy::EVENT_KIND);
            var::DEFEAT_BONUS.set(x.b, bar);
            fall_start(x);
            lib::drop_rewards(x);
            lib::flash_and_shake(x);
            crate::sound::service(x.b, crate::sound::Service::Ff05);
            lib::sound(x, crate::sound::CMD_313_413_B);
            lib::sound(x, crate::sound::CMD_FF);
            green_fall(x);
        }
        2 => green_fall(x),
        4 => fall_end_green(x),
        _ => {}
    }
}

fn green_fall<R: Bench>(x: &mut Cx<'_, R>) {
    x.move_xz();
    let cam_y = camera_y(x);
    let d = x.u16(Obj::Y).wrapping_sub(cam_y).wrapping_add(x.u16(Obj::Z));
    if (d as i16) <= 0x100 {
        return;
    }
    x.add8(f::SUB2, 2);
    x.set8(Boss19::TIMER, 0x20);
    let y = x.u16(Obj::Y).wrapping_add(0x50);
    x.set16(Obj::Y, y);
    let z = x.u16(Obj::Z).wrapping_sub(0x50);
    x.set16(Obj::Z, z);
    x.set32(Enemy::VX_LONG, 0);
    x.set32(Enemy::VY_LONG, 0);
    x.set16(Enemy::VZ, (-0x400i16) as u16);
    x.set16(Enemy::GRAVITY, 0);
    x.anim_start(0x17dbd2, 1);
    fall_end_green(x);
}

fn fall_end_green<R: Bench>(x: &mut Cx<'_, R>) {
    x.move_xz();
    if (x.u16(Obj::Z) as i16) > -0x100 {
        return;
    }
    release(x);
}

/// Nagpa's dragon: disappears blinking (0x90fa8).
fn death_pet<R: Bench>(x: &mut Cx<'_, R>) {
    match x.u8(f::SUB2) {
        0 => {
            x.add8(f::SUB2, 2);
            x.set8(Enemy::ACTIVE, 2);
            x.set16(f::IMMUNITY, 0xffff);
            x.set16(Boss19::TIMER, 0x20);
            lib::object_event(x);
            rest(x);
            lib::drop_rewards(x);
            lib::sound(x, crate::sound::CMD_313_413);
        }
        2 => {
            x.set_bit(Enemy::DISPLAY, 1, true);
            if x.u16(Boss19::TIMER) & 1 == 0 {
                x.set_bit(Enemy::DISPLAY, 1, false);
            }
            if sub16(x, Boss19::TIMER, 1).gt() {
                return;
            }
            release(x);
        }
        _ => {}
    }
}

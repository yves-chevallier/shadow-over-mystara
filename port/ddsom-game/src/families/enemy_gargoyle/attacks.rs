//! Type 8, substates 4 (attacks), 6, 8, 10 (rest), 12 (grab) and 14 (end of move).
//! Substate 4, `+6`: 0 and 2 claw, 4 short strike, 6 lateral charge, 8 backward jump, 10 dive, 12 grab of a player.

use crate::families::cx::Bench;
use super::*;
use crate::objects::Current;
use crate::objects::Obj;
use crate::player::Player;

/// End of a move: if nobody is alive any more the creature withdraws, otherwise it sets off again at random.
fn finish<R: Bench>(x: &mut Cx<'_, R>) {
    if lib::players_alive(x) {
        go::wander(x);
    } else {
        go::retire(x);
    }
}

/// The attack hits when animation key `+0x33` goes by: it is reset to zero immediately.
fn strike_hit<R: Bench>(x: &mut Cx<'_, R>) {
    if x.u8(f::KEY_FRAME) != 0 {
        x.set8(f::KEY_FRAME, 0);
        lib::sound(x, crate::sound::CMD_361);
    }
}

/// Releases the grabbed victim (`+0x54`): its grab flags drop back.
fn release_victim<R: Bench>(x: &mut Cx<'_, R>) {
    if x.u16(Enemy::HOLDER) != 0 {
        let v = x.linked(Enemy::HOLDER);
        v.set_u8(x.b, Enemy::EVENT_KIND, 0);
        v.set_u8(x.b, Enemy::FREEZE, 0);
    }
}

pub(super) fn run<R: Bench>(x: &mut Cx<'_, R>) {
    match x.u8(f::SUB2) {
        0 => claw(x),
        2 => claw_b(x),
        4 => short_blow(x),
        6 => sidestep(x),
        8 => jump_back(x),
        10 => plunge_attack(x),
        12 => stoop(x),
        _ => {}
    }
}

// --- +6 = 0 : griffe ----------------------------------------------------------------------------

fn claw<R: Bench>(x: &mut Cx<'_, R>) {
    match x.u8(f::SUB3) {
        0 => {
            x.add8(f::SUB3, 2);
            look(x, 0x10);
            strike(x, 0);
        }
        2 => {
            if !lib::step(x) {
                return strike_hit(x);
            }
            let grab = lib::players_alive(x) && x.u8(Gargoyle::GRABBED_ONCE) != 0 && x.u8(Gargoyle::HIT_FLAGS) & 4 != 0;
            if grab {
                release_victim(x);
                return go::strike_2(x, true);
            }
            x.add8(f::SUB3, 2);
            strike(x, 1);
        }
        4 => {
            if lib::step(x) {
                finish(x);
            }
        }
        _ => {}
    }
}

// --- +6 = 2 : griffe lourde ---------------------------------------------------------------------

fn claw_b<R: Bench>(x: &mut Cx<'_, R>) {
    match x.u8(f::SUB3) {
        0 => {
            x.add8(f::SUB3, 2);
            look(x, 0x11);
            strike(x, 2);
        }
        2 => {
            if !lib::step(x) {
                return strike_hit(x);
            }
            let grab = lib::players_alive(x) && x.u8(Gargoyle::GRABBED_ONCE) != 0 && x.u8(Gargoyle::HIT_FLAGS) & 4 != 0;
            if grab {
                release_victim(x);
                return go::strike_4(x, true);
            }
            x.add8(f::SUB3, 2);
            strike(x, 3);
        }
        4 => {
            if lib::step(x) {
                finish(x);
            }
        }
        _ => {}
    }
}

// --- +6 = 4 : coup bref --------------------------------------------------------------------------

fn short_blow<R: Bench>(x: &mut Cx<'_, R>) {
    match x.u8(f::SUB3) {
        0 => {
            x.add8(f::SUB3, 2);
            look(x, 0x12);
            let n = if x.u8(Gargoyle::GRABBED_ONCE) != 0 { 5 } else { 4 };
            strike(x, n);
        }
        2 => {
            if !lib::step(x) {
                return strike_hit(x);
            }
            finish(x);
        }
        _ => {}
    }
}

// --- +6 = 6: lateral charge --------------------------------------------------------------------

fn sidestep<R: Bench>(x: &mut Cx<'_, R>) {
    match x.u8(f::SUB3) {
        0 => {
            x.add8(f::SUB3, 2);
            look(x, 0x13);
            strike(x, 6);
        }
        2 => {
            lib::step(x);
            if x.u8(Enemy::ANIM_END_BYTE) == 0 {
                return;
            }
            x.add8(f::SUB3, 2);
            x.set16(Gargoyle::TIMER, 0x1a);
            x.set16(Gargoyle::TIMER2, 0);
            let (angle, vy) = if x.u8(f::FLIP) != 0 { (8, -0x38i16) } else { (0x18, 0x38) };
            x.set8(Obj::SPEED_INDEX, angle);
            x.set16(Obj::AX, vy as u16);
            set_speed(x, 0x40);
            lib::sound(x, crate::sound::CMD_363_363);
        }
        4 => {
            if !count_down(x, Gargoyle::TIMER2) {
                x.set16(Gargoyle::TIMER2, 4);
                spark(x, 8);
            }
            x.move_x();
            if count_down(x, Gargoyle::TIMER) {
                return;
            }
            x.add8(f::SUB3, 2);
            x.set16(Gargoyle::TIMER, 0x14);
            x.set16(Obj::AX, 0);
            strike(x, 7);
        }
        6 => {
            if !count_down(x, Gargoyle::TIMER2) {
                x.set16(Gargoyle::TIMER2, 6);
                spark(x, 8);
            }
            if count_down(x, Gargoyle::TIMER) {
                x.move_x();
                lib::step(x);
            } else if lib::players_alive(x) {
                go::wander(x);
            } else {
                go::retire(x);
            }
        }
        _ => {}
    }
}

// --- +6 = 8: backward jump --------------------------------------------------------------------

fn jump_back<R: Bench>(x: &mut Cx<'_, R>) {
    match x.u8(f::SUB3) {
        0 => {
            x.add8(f::SUB3, 2);
            look(x, 0x14);
            strike(x, 8);
        }
        2 => {
            lib::step(x);
            if x.u8(f::KEY_FRAME) != 0 {
                return;
            }
            x.add8(f::SUB3, 2);
            let (vx, vy) = if x.u8(f::FLIP) != 0 { (-0x400i16, 0x10i16) } else { (0x400, -0x10) };
            x.set16(Obj::VX, vx as u16);
            x.set16(Obj::AX, vy as u16);
            x.set16(Obj::VZ, 0x5c0);
            x.set16(Obj::AZ, -0x30i16 as u16);
        }
        4 => {
            x.move_xz();
            if (x.u16(Obj::VZ) as i16) > 0x1c0 {
                return;
            }
            x.add8(f::SUB3, 2);
            strike(x, 9);
        }
        6 => {
            x.move_xz();
            if (x.u16(Obj::VZ) as i16) > -0x1e0 {
                lib::step(x);
                return;
            }
            x.add8(f::SUB3, 2);
            x.set8(Gargoyle::INVULNERABLE, 0);
            x.set16(Obj::AX, 0);
            x.set16(Obj::AZ, -0x20i16 as u16);
            strike(x, 0xa);
        }
        8 => {
            x.move_xz();
            if (x.u16(Obj::Z) as i16) > 0 {
                lib::step(x);
                return;
            }
            x.add8(f::SUB3, 2);
            x.set16(Obj::Z, 0);
            fly(x, 0xb);
        }
        10 => {
            if lib::step(x) {
                finish(x);
            }
        }
        _ => {}
    }
}

// --- +6 = 10: dive onto the target ---------------------------------------------------------------

fn plunge_attack<R: Bench>(x: &mut Cx<'_, R>) {
    match x.u8(f::SUB3) {
        0 => {
            x.add8(f::SUB3, 2);
            x.set16(Obj::AX, 0);
            let angle = if x.u8(f::FLIP) != 0 { 8 } else { 0x18 };
            x.set8(Obj::SPEED_INDEX, angle);
            set_speed(x, 0x17);
            // number of frames to cover the distance to the target: the vertical speed follows from it
            let target = load_target(x);
            let mut dist = x.u32(Obj::X).wrapping_sub(target.u32(x.b, Obj::X)) as i32;
            if dist < 0 {
                dist = dist.wrapping_neg();
            }
            let mut speed = ((x.u16(Obj::VX) as u32) << 16) as i32 >> 8;
            if speed < 0 {
                speed = speed.wrapping_neg();
            }
            let mut d1 = 0x100u16;
            let mut guard = 0;
            loop {
                dist = dist.wrapping_sub(speed);
                if dist <= 0 || guard > 0x10000 {
                    break;
                }
                d1 = d1.wrapping_add(0xe);
                guard += 1;
            }
            x.set16(Obj::VZ, d1);
            x.set16(Obj::AZ, -0x20i16 as u16);
            look(x, 0x15);
            strike(x, 0xb);
        }
        2 => {
            x.anim_step();
            if x.u8(f::KEY_FRAME) != 0 {
                return;
            }
            x.move_xz();
            if (x.u16(Obj::VZ) as i16) > 0xe0 {
                return;
            }
            x.add8(f::SUB3, 2);
            strike(x, 0xc);
        }
        4 => {
            x.move_xz();
            if (x.u16(Obj::VZ) as i16) > -0xa0 {
                lib::step(x);
                return;
            }
            x.add8(f::SUB3, 2);
            x.set16(Obj::AZ, -0x40i16 as u16);
            strike(x, 0xd);
        }
        6 => {
            strike_hit(x);
            x.move_xz();
            if (x.u16(Obj::Z) as i16) > 0 {
                lib::step(x);
                return;
            }
            x.add8(f::SUB3, 2);
            x.set16(Gargoyle::TIMER, 0x14);
            x.set16(Gargoyle::TIMER2, 0);
            x.set16(Obj::Z, 0);
            fly(x, 6);
        }
        8 => {
            if !count_down(x, Gargoyle::TIMER2) {
                x.set16(Gargoyle::TIMER2, 4);
                spark(x, 8);
            }
            if count_down(x, Gargoyle::TIMER) {
                x.move_x();
                lib::step(x);
            } else {
                finish(x);
            }
        }
        _ => {}
    }
}

// --- +6 = 12: the gargoyle swoops onto the target --------------------------------------------------------

fn stoop<R: Bench>(x: &mut Cx<'_, R>) {
    match x.u8(f::SUB3) {
        0 => {
            x.add8(f::SUB3, 2);
            look(x, 4);
            fly(x, 1);
        }
        2 => {
            if (x.u16(Obj::Z) as i16) >= 0x34 && x.u8(f::KEY_FRAME) == 4 {
                x.add8(f::SUB3, 2);
                x.set16(Gargoyle::TIMER, 2);
            } else {
                rise(x);
            }
            lib::step(x);
        }
        4 => {
            bob(x);
            face_target(x);
            if count_down(x, Gargoyle::TIMER) {
                lib::step(x);
            } else {
                x.add8(f::SUB3, 2);
                aim_dive(x);
            }
        }
        6 => {
            x.move_xyz();
            if (x.u16(Obj::Z) as i16) <= 0 {
                return land_from_dive(x);
            }
            if (x.u16(Gargoyle::TIMER) as i16) > 0 {
                let n = x.u16(Gargoyle::TIMER).wrapping_sub(1);
                x.set16(Gargoyle::TIMER, n);
                if (n as i16) <= 0 {
                    x.set16(Gargoyle::TIMER, 0);
                    x.set16(Obj::VZ, 0);
                    x.set16(Obj::AZ, -0x30i16 as u16);
                }
            }
            let Some(player) = lib::attack_hits_a_player(x, 9) else { return };
            lib::seize_player(x, player, 0);
        }
        8 => {
            if !count_down(x, Gargoyle::TIMER2) {
                x.set16(Gargoyle::TIMER2, 5);
                spark(x, 8);
            }
            lib::step(x);
            if count_down(x, Gargoyle::TIMER) {
                x.move_xy();
            } else if lib::players_alive(x) {
                go::wander(x);
            } else {
                go::retire(x);
            }
        }
        _ => {}
    }
}

/// 0x64e00: aims at the target: angle, vertical speed to get there in as many frames as the horizontal run.
fn aim_dive<R: Bench>(x: &mut Cx<'_, R>) {
    let target = load_target(x);
    let (tx, tz) = (target.u16(x.b, Obj::X), target.u16(x.b, Obj::Y));
    x.set16(Gargoyle::GOAL_X, tx);
    x.set16(Gargoyle::GOAL_DEPTH, tz);
    let angle = lib::aim_from(x, (tx, tz));
    x.set8(Obj::SPEED_INDEX, angle);
    set_speed(x, 0x35);
    let abs = |v: u16| (v as i16).unsigned_abs();
    let mut dist = abs(x.u16(Gargoyle::GOAL_X).wrapping_sub(x.u16(Obj::X))) as i32;
    let dz = abs(x.u16(Gargoyle::GOAL_DEPTH).wrapping_sub(x.u16(Obj::Y))) as i32;
    let mut speed = abs(x.u16(Obj::VX)) as i32;
    let speed2 = abs(x.u16(Obj::VY)) as i32;
    if (dist as i16) < (dz as i16) {
        dist = dz;
        speed = speed2;
    }
    let mut remaining = dist << 16;
    let step = (speed << 16) >> 8;
    let mut frames = 0u16;
    let mut guard = 0;
    loop {
        remaining = remaining.wrapping_sub(step);
        if remaining <= 0 || guard > 0x10000 {
            break;
        }
        frames = frames.wrapping_add(1);
        guard += 1;
    }
    // 0x17ea: height / number of frames. Original glitch: the division leaves its remainder in D2 and its counter in D3
    // (leftovers).
    let (q, r) = crate::aids::divide(x.u16(Obj::Z) as u32, frames as u32);
    *x.c.d2() = r;
    *x.c.d3() = 0xffff;
    let mut d0 = (q.rotate_left(16) as i32 >> 8) as i16;
    if d0 > 0x400 {
        d0 = 0x400;
    }
    x.set16(Obj::VZ, d0.wrapping_neg() as u16);
    x.set16(Obj::AX, 0);
    x.set16(Obj::AZ, 0);
    x.set16(Obj::AY, 0);
    lib::sound(x, crate::sound::CMD_373_473);
    x.set16(Gargoyle::TIMER, 0x5a);
    strike(x, 0xe);
}

/// 0x64eee: landing of the dive: the creature settles, sliding in the other direction.
fn land_from_dive<R: Bench>(x: &mut Cx<'_, R>) {
    x.add8(f::SUB3, 2);
    x.set16(Gargoyle::TIMER, 0x18);
    x.set16(Gargoyle::TIMER2, 0);
    x.set16(Obj::Z, 0);
    let vx = (x.u16(Obj::VX) as i16).wrapping_neg() >> 5;
    let vy = (x.u16(Obj::VY) as i16).wrapping_neg() >> 5;
    x.set16(Obj::AX, vx as u16);
    x.set16(Obj::AY, vy as u16);
    fly(x, 6);
}

// --- substates 6, 8, 10: rest -----------------------------------------------------------------------

pub(super) fn rest<R: Bench>(x: &mut Cx<'_, R>) {
    if var::GAME_ON.get(x.b) != 0 && (var::PLAYERS_ALIVE.get(x.b) as i8) > 0 {
        return go::wander(x);
    }
    match x.u8(f::SUB2) {
        0 => {
            x.add8(f::SUB2, 2);
            x.set8(Gargoyle::LOOK_PREV, 0xff);
            fly(x, 0);
        }
        2 => {
            lib::step(x);
        }
        _ => {}
    }
}

// --- substate 12: grab ------------------------------------------------------------------------------

pub(super) fn grab<R: Bench>(x: &mut Cx<'_, R>) {
    let victim = x.linked(f::TARGET);
    if victim.u8(x.b, Enemy::ACTIVE) == 0 {
        return go::strike_10(x);
    }
    if victim.u8(x.b, Player::HELD) == 0 {
        return go::circle(x);
    }
    if x.u8(f::SUB2) != 0 {
        return;
    }
    match x.u8(f::SUB3) {
        0 => {
            x.add8(f::SUB3, 2);
            let (px, pz) = (victim.u16(x.b, Obj::X), victim.u32(x.b, Obj::Y));
            x.set16(Obj::X, px);
            x.set32(Obj::Y, pz.wrapping_sub(1));
            let shift = if x.u8(f::FLIP) != 0 { -0x1bi16 } else { 0x1b };
            x.add16(Obj::X, shift as u16);
            x.set16(Gargoyle::TIMER, 0xe);
            strike(x, 0xf);
        }
        2 => {
            if count_down(x, Gargoyle::TIMER) {
                lib::step(x);
                return;
            }
            x.add8(f::SUB3, 2);
            let n = if (x.u8(Gargoyle::PHASE) as i8) < 2 { 0x10 } else { 0x11 };
            strike(x, n);
            x.set16(Gargoyle::TIMER, 6);
            x.set16(Gargoyle::TIMER2, 0);
        }
        4 => {
            if count_down(x, Gargoyle::TIMER2) {
                lib::step(x);
                return;
            }
            let shift = if x.u8(f::FLIP) != 0 { 0x23i16 } else { -0x23 };
            let (d2, d3, d4) = (x.u16(Obj::X).wrapping_add(shift as u16), x.u16(Obj::Z).wrapping_add(0x11), x.u16(Obj::Y).wrapping_sub(1));
            *x.c.d2() = d2 as u32;
            *x.c.d3() = d3 as u32;
            *x.c.d4() = d4 as u32;
            crate::families::effects_creators::spark_at(x.b, x.o, victim.0, 2, (d2, d3, d4));
            lib::sound(x, crate::sound::CMD_341_441);
            let victim = x.linked(f::TARGET);
            let hp = victim.u16(x.b, f::HP).wrapping_sub(4);
            victim.set_u16(x.b, f::HP, hp);
            if (hp as i16) <= 0 {
                victim.set_u16(x.b, f::HP, 1);
                return dismount(x);
            }
            if !count_down(x, Gargoyle::TIMER) {
                return dismount(x);
            }
            let rank = x.u8(Gargoyle::PHASE) as i8 as i16 as u16;
            let t = tables::table("gargoyle/ride_times").at16(x.b, 2 * rank as u32);
            x.set16(Gargoyle::TIMER2, t);
            lib::step(x);
        }
        _ => {}
    }
}

/// 0x650a6: the victim is released (`+0x6f` raised) and the creature goes into circular flight.
fn dismount<R: Bench>(x: &mut Cx<'_, R>) {
    let victim = x.linked(f::TARGET);
    victim.set_u8(x.b, crate::player::Player::TIMER_6F, 1);
    lib::release_player(x, victim);
    go::circle(x);
}

/// Substate 14: end of move, replacement target.
pub(super) fn done<R: Bench>(x: &mut Cx<'_, R>) {
    if x.u8(f::SUB2) == 0 {
        regroup(x);
    }
}

/// 0x653b2: the fight resumes: target chosen, HP threshold crossed, new move.
pub(crate) fn regroup<R: Bench>(x: &mut Cx<'_, R>) {
    x.set8(Gargoyle::HIT_FLAGS, 0);
    x.set8(Enemy::ACTIVE, 1);
    x.set8(Gargoyle::LOOK_PREV, 0xff);
    if !pick_target(x) {
        return go::retire(x);
    }
    lib::next_threshold(x, 0x105e2);
    if lib::target_in_box(x, 0, 0x28, 0xc) {
        go::strike_0(x, false);
    } else {
        go::wander(x);
    }
}

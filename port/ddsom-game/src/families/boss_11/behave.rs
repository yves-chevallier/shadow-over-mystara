//! Boss 11/12, active states other than attacks: arrival (group 0), wait (6), pursuit (2, 8, 10, 12) and the tests
//! that trigger an attack or a maneuver.

use crate::families::cx::Bench;
use super::*;
use crate::objects::Current;
use crate::machine::Machine;

// --- group 0: arrival --------------------------------------------------------------------------------------

/// 0x8d7b8: the snake bursts in on the right of the screen, drops, bounces, starts walking.
pub(super) fn intro<R: Bench>(x: &mut Cx<'_, R>) {
    match x.u8(f::SUB2) {
        0 => {
            x.add8(f::SUB2, 2);
            x.set8(Boss11::NO_ZONE, 0xff);
            let px = var::SCROLL_X.get(x.b).wrapping_add(0x190);
            x.set16(Obj::X, px);
            let py = var::SCROLL_Y.get(x.b).wrapping_add(0x40);
            x.set16(Obj::Y, py);
            x.set16(Obj::Z, 0);
            x.set16(Enemy::VX, (-0x200i16) as u16);
            x.set16(Enemy::AX, 0);
            x.set16(Enemy::VZ, 0x400);
            x.set16(Enemy::GRAVITY, (-0x20i16) as u16);
            x.set32(Enemy::VY_LONG, 0);
            body(x, 6);
            sync(x);
            fall(x)
        }
        2 => fall(x),
        4 => {
            x.set8(Boss11::NO_ZONE, 0xff);
            x.move_x();
            if x.anim_step() {
                x.add8(f::SUB2, 2);
                body(x, 0);
            }
            sync(x);
        }
        _ => {
            if Fl::sub8(var::PHASE.get(x.b), 0xc).lt() {
                x.anim_step();
                return sync(x);
            }
            x.set8(Enemy::ACTIVE, 1);
            x.add8(f::SUB, 2);
            x.set16(f::SUB2, 0);
            lib::set_immunities(x);
            x.set16(Boss11::ANIM_KIND, 0xffff);
            body(x, 0)
        }
    }
}

/// 0x8d818: fall to the ground.
fn fall<R: Bench>(x: &mut Cx<'_, R>) {
    x.set8(Boss11::NO_ZONE, 0xff);
    x.move_xz();
    if !tst16(x, Obj::Z).mi() {
        return;
    }
    x.set16(Obj::Z, 0);
    x.add8(f::SUB2, 2);
    x.set32(Enemy::VZ_LONG, 0);
    x.set32(Enemy::VY_LONG, 0);
    let v = asr_w(x.u16(Enemy::VX), 2);
    x.set16(Enemy::VX, v);
    body(x, 7);
    lib::shake_58(x, 8);
    lib::sound(x, crate::sound::CMD_328_428_B);
}

// --- group 6: no player ----------------------------------------------------------------------------------

/// 0x8d8bc: waits; a player comes back (life > 0) and the `+0xb6` counter restarts.
pub(super) fn lone<R: Bench>(x: &mut Cx<'_, R>) {
    if var::PLAYERS_ALIVE.get(x.b) != 0 && lib::target_by_criterion(x, lib::LOWEST_HP).is_some() {
            if tst8(x, Boss11::REST).gt() {
                sub8(x, Boss11::REST, 1);
            } else {
                x.set8(Boss11::REST, 0);
                return to_chase(x);
            }
        }
    if x.u8(f::SUB3) == 0 {
        x.add8(f::SUB3, 2);
        return body(x, 0);
    }
    x.anim_step();
    sync(x);
}

// --- groups 2, 8, 10, 12: pursuit ---------------------------------------------------------------------------

/// 0x8eb52: walk delay depending on difficulty.
fn walk_delay<R: Bench>(x: &mut Cx<'_, R>) {
    let v = per_rank(x, "boss_11/walk_delays");
    x.set8(Boss::WALK_DELAY, v);
}

/// 0x8eb7e: number of steps drawn.
fn walk_steps<R: Bench>(x: &mut Cx<'_, R>) {
    let rng = x.random();
    let i = rng & 0x1f;
    let v = byte(x, "boss_11/walk_steps", i as u32);
    x.set8(Boss::WALK_STEPS, v);
}

/// 0x8d902.
pub(super) fn chase<R: Bench>(x: &mut Cx<'_, R>) {
    if var::PLAYERS_ALIVE.get(x.b) == 0 {
        return to_wait(x);
    }
    if lib::target_by_criterion(x, lib::LOWEST_HP).is_none() {
        return to_wait(x);
    }
    if tst16(x, Obj::Z).gt() {
        return to_attack_2(x);
    }
    let mut aim = true;
    if x.u8(f::SUB3) == 0 {
        x.add8(f::SUB3, 2);
        walk_steps(x);
        walk_delay(x);
    } else if sub8(x, Boss::WALK_DELAY, 1).gt() {
        aim = false;
    } else {
        walk_delay(x);
        if sub8(x, Boss::WALK_STEPS, 1).gt() {
        } else {
            walk_steps(x);
        }
    }
    if aim {
        lib::approach_target(x);
        speed(x, 0x17);
    }
    x.move_by_table_xy();
    pose(x);
    if turn_needed(x) {
        return to_turn(x);
    }
    if hit_near(x) {
        return pick(x, "boss_11/close_attacks");
    }
    if far_roll(x) {
        return pick(x, "boss_11/far_attacks");
    }
    if near_roll(x) {
        return pick(x, "boss_11/near_attacks");
    }
    if off_screen(x) {
        to_attack_2(x)
    }
}

/// 0x8d998: walk animation by orientation.
fn pose<R: Bench>(x: &mut Cx<'_, R>) {
    let w = x.u16(Boss11::ANIM_KIND);
    if w == 1 {
        if x.anim_step() {
            body(x, 0);
        }
        return sync(x);
    }
    if w == 0 || w == 8 || w == 9 {
        let mut d1 = x.u8(Enemy::HEADING) as u16;
        if x.u8(f::FLIP) != 0 {
            d1 = d1.wrapping_add(0x10);
        }
        d1 &= 0x1f;
        let n = if Fl::sub16(d1, 0x10).gt() { 8 } else { 9 };
        if n as u8 != x.u8(Boss11::ANIM_N) {
            return body(x, n);
        }
        x.anim_step();
        return sync(x);
    }
    body(x, 0);
    sync(x)
}

/// 0x8da02: the snake is off screen.
fn off_screen<R: Bench>(x: &mut Cx<'_, R>) -> bool {
    let mut d0 = var::SCROLL_X.get(x.b);
    if var::CAMERA_SELECT.get(x.b) != 0 {
        d0 = var::CAMERA_ALT_X.get(x.b);
    }
    let here = x.u16(Obj::X);
    if Fl::sub16(d0, here).ge() {
        return true;
    }
    d0 = d0.wrapping_add(0x1c0);
    if Fl::sub16(d0, here).ge() {
        return false;
    }
    true
}

/// 0x8da28: the target is behind the snake.
fn turn_needed<R: Bench>(x: &mut Cx<'_, R>) -> bool {
    let w = x.u16(f::TARGET);
    if (w as i16) >= 0 {
        return false;
    }
    let target = w as i16 as i32 as u32 & 0xff_ffff;
    if rd8(x.b, target) == 0 {
        return false;
    }
    let mut d = rd16(x.b, target + 8).wrapping_sub(x.u16(Obj::X));
    if x.u8(f::FLIP) == 0 {
        d = d.wrapping_neg();
    }
    if Fl::sub16(d, 0xffd0).gt() {
        return false;
    }
    true
}

/// 0x8dafe: a player very close ahead.
fn hit_near<R: Bench>(x: &mut Cx<'_, R>) -> bool {
    // re-read: D2 of the box (0xb5e0), whose high byte `roll` (0x8db98) reads (original glitch)
    lib::target_in_box(x, -0x10, 0x20, 0x10)
}

/// 0x8dae2: life rank (about 0 to 3): number of times the maximum HP fits in four times the HP.
/// Returns the rank (D0.w). Original glitch: the sum and the quadruple of the HP stay in D2 and D3
/// (`dispatch::Leftovers`).
pub(super) fn life_rank<R: Bench>(x: &mut Cx<'_, R>) -> u16 {
    let max = x.u16(Enemy::HP_MAX) as u32;
    let goal = (x.u16(f::HP) as u32) << 2;
    *x.c.d3() = goal;
    let (mut sum, mut rank) = (0u32, 0xffffu16);
    for _ in 0..0x20000 {
        sum = sum.wrapping_add(max);
        rank = rank.wrapping_add(1);
        if !Fl::sub32(sum, goal).lt() {
            break;
        }
    }
    *x.c.d2() = sum;
    rank
}

/// 0x8da56: draw according to the life rank (table 0x8da88).
fn near_roll<R: Bench>(x: &mut Cx<'_, R>) -> bool {
    let i = (life_rank(x) & 3) * 4;
    let mask = long(x, "boss_11/near_masks", i as u32);
    let rng = x.random();
    let r = rng & 1;
    setw(x, 0, r);
    if r == 0 {
        return false;
    }
    let rng = x.random();
    let b = rng & 0x1f;
    setw(x, 0, b);
    if mask >> b & 1 != 0 {
        return true;
    }
    false
}

/// 0x8da98: draw according to the life rank (table 0x8dad2), then a player in the box.
fn far_roll<R: Bench>(x: &mut Cx<'_, R>) -> bool {
    let i = (life_rank(x) & 3) * 4;
    let mask = long(x, "boss_11/far_masks", i as u32);
    let rng = x.random();
    let b = rng & 0x1f;
    setw(x, 0, b);
    if mask >> b & 1 != 0 {
        // re-read: D2 of the box (0xb5e0), whose high byte `roll` (0x8db98) reads (original glitch)
        if lib::target_in_box(x, -0x20, 0xa0, 0x80) {
            return true;
        }
    }
    false
}

/// 0x8db2a: draws an attack from table `table` (16 bytes), or gives up.
fn pick<R: Bench>(x: &mut Cx<'_, R>, table: &str) {
    if !roll(x) {
        let rng = x.random();
        let i = rng & 0xf;
        setw(x, 0, i);
        let v = byte(x, table, i as u32);
        setb(x, 1, v);
        if (v as i8) >= 0 {
            x.set32(f::STATE, 0x0204_0000);
            x.set8(f::SUB2, v);
            return;
        }
    }
    let i = life_rank(x) & 3;
    let v = byte(x, "boss_11/rests", i as u32);
    setb(x, 0, v);
    if v == 0 {
        return;
    }
    x.set8(Boss11::REST, v);
    to_wait(x)
}

/// 0x8db98: "give up" draw according to difficulty and number of players; true if the bit falls.
fn roll<R: Bench>(x: &mut Cx<'_, R>) -> bool {
    let rng = x.random();
    let r = rng & 7;
    setw(x, 0, r);
    let n = var::PLAYERS_ALIVE.get(x.b).wrapping_sub(1);
    let d1 = (n as u16 & 2).wrapping_mul(4);
    let level = var::RANK_HIGH.get(x.b);
    setb(x, 2, level);
    // Original glitch: only the low byte of D2 is set; the high byte is the one left by the preceding box test
    // (0xb5e0), or by the handler (`dispatch::Leftovers`).
    let d2 = (*x.c.d2() as u16) >> 2;
    let d1 = d1.wrapping_add(d2);
    let v = byte(x, "boss_11/give_up_masks", d1 as u32);
    setb(x, 1, v);
    v >> (r & 7) & 1 != 0
}

//! Boss 9, active state: stage entrance (`+5` = 0), departure (2), passes and stops (4), wait (6).

use crate::families::cx::Bench;
use super::*;
use crate::objects::pools;
use crate::objects::Current;

// --- group 0: stage entrance --------------------------------------------------------------------

/// 0x8a96a: the crew climbs aboard, dialogue, the vehicle crosses the screen then comes back in reverse.
pub(super) fn intro<R: Bench>(x: &mut Cx<'_, R>) {
    match x.u8(f::SUB2) {
        2 => board(x),
        4 => {
            if x.u8(CREW_LOST) != 0 {
                return;
            }
            x.set16(TIMER, 0xa);
            x.set16(0x14, 0);
            x.set16(0x16, 0);
            let Some(bubble) = crate::families::creators::scene_set(x.b, 5) else { return };
            x.set16(MESSAGE, bubble.word());
            next6(x);
        }
        6 => {
            if x.u16(MESSAGE) != 0 {
                let m = x.linked(MESSAGE);
                if m.u8(x.b, 0x22) == 0 {
                    return;
                }
            }
            next6(x);
            x.set16(0x14, 0);
            x.set16(0x16, 0x28);
            x.set8(GAIT, 0);
            lib::sound(x, 0xea91a);
            let n = x.a5_u8(0x4ff).wrapping_add(4);
            x.set_a5_u8(0x4ff, n);
        }
        8 => {
            jolt(x);
            smoke_every(x, 7);
            if x.u16(0x14) as i16 >= 0x300 {
                next6(x);
                x.set16(0x16, 4);
            }
            roll(x);
        }
        10 => {
            jolt(x);
            smoke_every(x, 3);
            let cam = x_cam(x);
            if left_of(x, 0x80, cam) {
                return next6(x);
            }
            roll(x);
        }
        14 => {
            smoke_every(x, 3);
            next6(x);
            x.set16(0x14, 0x800);
            x.set8(GAIT, 1);
            lib::sound(x, 0xe45ca);
        }
        16 => {
            jolt(x);
            smoke_every(x, 3);
            let edge = x_cam(x).wrapping_add(0x180);
            if !right_of(x, 0x80, edge) {
                return roll(x);
            }
            next6(x);
            x.set16(TIMER, 0x3c);
            x.set8(0x38, 1);
            x.set8(0, 1);
            x.set8(f::FLIP, 0);
            x.set8(GAIT, 2);
        }
        18 => {
            if sub16(x, TIMER, 1).pl() {
                return;
            }
            next6(x);
            x.set16(TIMER, 0x70);
            x.set8(GAIT, 0);
            x.set8(0x2d, 0x18);
            x.set16(0x14, 0);
            x.set16(0x16, (-8i16) as u16);
            lib::sound(x, 0xe4626);
        }
        20 => {
            jolt(x);
            smoke_every(x, 7);
            if sub16(x, TIMER, 1).mi() {
                next6(x);
                x.set8(GAIT, 2);
                lib::sound(x, 0xea984);
                lib::create_set_14(x);
            }
            roll(x);
        }
        22 => {
            x.set32(f::IMMUNITY, 0xffff_ffff);
            x.set8(0, 2);
            anim(x);
        }
        _ => {}
    }
}

/// X position of the vehicle's plane camera.
fn x_cam<R: Bench>(x: &mut Cx<'_, R>) -> u16 {
    x.a5_u16(CAM_X)
}

/// The vehicle rolls (0x1d78) and its wheels turn (0x1a56).
pub(super) fn roll<R: Bench>(x: &mut Cx<'_, R>) {
    x.move_x();
    anim(x);
}

/// 0x8a992: the crew climbs aboard (four ENEMY 0x13 gunners, variant 2: `+0xbc` = 5 to 2), if there is room.
fn board<R: Bench>(x: &mut Cx<'_, R>) {
    if (x.a5_u16(ENEMY_FREE) as i16) < 4 {
        return;
    }
    x.set8(CREW_LOST, 0);
    for k in 0..4u32 {
        let slot = 5 - k as u8;
        *x.c.d5() = slot as u32;
        let Some(m) = x.alloc(&pools::ENEMY) else { return };
        let active = x.u8(0);
        m.set_u8(x.b, 0, active);
        m.set_u8(x.b, 0xbc, slot);
        m.set_u8(x.b, 3, 2);
        m.set_u8(x.b, 2, 0x13);
        m.set_u16(x.b, 0x42, x.o.0 as u16);
        x.set16(CREW + 2 * k, m.0 as u16);
        if slot == 2 {
            break;
        }
        x.add8(CREW_LOST, 1);
    }
    next6(x);
}

// --- group 2: departure ---------------------------------------------------------------------------

/// 0x8abc2: smoke plume (SET 2, variant 0xe), then departure of the first pass.
pub(super) fn start<R: Bench>(x: &mut Cx<'_, R>) {
    if let Some(s) = x.alloc(&pools::SET) {
        s.set_u8(x.b, 0, 1);
        s.set_u8(x.b, 3, 0xe);
        s.set_u8(x.b, 2, 2);
    }
    restart(x)
}

/// 0x8abdc: (re)departure: first pass (`+5` = 4).
pub(super) fn restart<R: Bench>(x: &mut Cx<'_, R>) {
    lib::set_immunities(x);
    x.set8(0, 1);
    x.add8(f::SUB, 2);
    steps(x, 0, 2);
    x.set8(WAIT, 1);
    x.set16(0x16, 0);
    reset_lives(x);
    count_crew(x);
    set_speed(x);
    reset_dashes(x);
    draw_passes(x);
}

// --- group 4: passes and stops ------------------------------------------------------------

/// 0x8ac14: damage smoke (once); with no gunners, the vehicle waits for their replacement; otherwise step `+6`.
pub(super) fn fight<R: Bench>(x: &mut Cx<'_, R>) {
    if x.u8(SMOKE_DONE) == 0 && sub16(x, SMOKE_DELAY, 1).mi() {
        x.set8(SMOKE_DONE, 1);
        if let Some(s) = x.alloc(&pools::SET) {
            s.set_u8(x.b, 0, 1);
            s.set_u8(x.b, 3, 0x1c);
            s.set_u8(x.b, 2, 2);
        }
    }
    if x.u8(CREW_LOST) != 0 {
        x.set8(GAIT, 2);
        count_crew(x);
        if x.u8(CREW_COUNT) == 0 {
            refill_crew(x, false);
        }
        return anim(x);
    }
    match x.u8(f::SUB2) {
        0 => pass(x),
        2 => approach(x),
        4 | 16 => fire(x),
        6 => bob(x),
        8 | 20 => bomb(x),
        10 | 22 => bomb_then_fire(x),
        12 | 24 => halt(x, 0x14),
        14 | 26 => halt(x, 0x30),
        _ => volley(x),
    }
    if var::PLAYERS_ALIVE.get(x.b) == 0 {
        x.set32(f::STATE, 0x0206_0000);
    }
}

/// 0x8acaa: a pass from one edge to the other (after `+0x6e` frames of waiting).
fn pass<R: Bench>(x: &mut Cx<'_, R>) {
    if x.u8(0x7) == 0 {
        if sub8(x, WAIT, 1).pl() {
            return;
        }
        next7(x);
        lib::sound(x, 0xea8be);
        x.set8(0x45, 0xa);
        x.set8(0x44, 0x2d);
        count_crew(x);
        if !try_dash(x) {
            set_speed(x);
        }
        retarget(x);
        return pick_lane(x);
    }
    if crew_gone(x) {
        return;
    }
    count_crew(x);
    set_speed(x);
    if x.u8(CREW_COUNT) != 0 {
        let v = x.u16(0x14);
        if v == 0xf800 || v == 0x800 {
            if every(x, 3) {
                smoke_pair(x, 1, 0);
            }
        } else {
            crew_smoke(x);
        }
    }
    if !past_edge(x) {
        x.move_x();
        return;
    }
    turn_back(x);
    if sub8(x, PASSES, 1).mi() {
        next6(x);
        x.set8(0x7, 0);
        reset_dashes(x);
        draw_passes(x);
        draw_stops(x);
        x.move_x();
        return;
    }
    x.set8(0x7, 0);
    reset_wait(x)
}

/// At the edge: about-face, and the lost crew is replaced.
fn turn_back<R: Bench>(x: &mut Cx<'_, R>) {
    let side = x.u8(f::FLIP) ^ 0x20;
    x.set8(f::FLIP, side);
    refill_crew(x, false);
}

/// 0x8adfc: rolls to the stop X position, waits `+0xbe` frames, then stops to fire according to the target.
fn approach<R: Bench>(x: &mut Cx<'_, R>) {
    match x.u8(0x7) {
        0 => {
            next7(x);
            reset_lives(x);
            reset_count(x);
            retarget(x);
            pick_lane(x);
            draw_stop_x(x);
            count_crew(x);
            try_dash(x);
            set_speed(x);
        }
        2 => {
            if past_edge(x) {
                turn_back(x);
                x.set8(0x7, 0);
                return reset_wait(x);
            }
            count_crew(x);
            if x.u8(CREW_COUNT) != 0 {
                crew_smoke(x);
            }
            let d = x.u16(STOP_X).wrapping_sub(x.u16(0x8)).wrapping_add(8);
            if d > 0x20 {
                x.move_x();
                return;
            }
            x.set8(GAIT, 2);
            if sub8(x, COUNT, 1).pl() {
                return;
            }
            lib::sound(x, 0xea984);
            reset_lives(x);
            reset_count(x);
            retarget(x);
            choose_halt(x);
        }
        _ => {
            if crew_gone(x) {
                return;
            }
            if !sub8(x, STOPS, 1).ne() {
                return steps(x, 0, 2);
            }
            if sub8(x, COUNT, 1).pl() {
                return;
            }
            reset_count(x);
            retarget(x);
            choose_halt(x);
        }
    }
}

/// 0x8aed8 / 0x8af4c: firing stop according to the target's range; out of range, a new pass.
fn choose_halt<R: Bench>(x: &mut Cx<'_, R>) {
    match target_range(x) {
        0 => draw_halt(x, 0xe),
        2 => steps(x, 0, 2),
        _ => draw_halt(x, 2),
    }
}

/// 0x8c952: firing damage (bit 0 of `+0xa8`): the stop is skipped; true.
fn cannon_broken<R: Bench>(x: &mut Cx<'_, R>) -> bool {
    if x.bit(DAMAGE, 0) {
        steps(x, 2, 4);
        return true;
    }
    false
}

/// 0x8c960: bombardment damage (bits 3 or 4); true.
fn bombard_broken<R: Bench>(x: &mut Cx<'_, R>) -> bool {
    if x.bit(DAMAGE, 3) || x.bit(DAMAGE, 4) {
        steps(x, 2, 4);
        return true;
    }
    false
}

/// 0x8af74: stop, firing order to the parts (bit 0 of `+0xa9`).
fn fire_order<R: Bench>(x: &mut Cx<'_, R>) {
    if cannon_broken(x) {
        return;
    }
    next7(x);
    x.set8(GAIT, 2);
    x.set16(0x14, 0);
    x.set_bit(ORDERS, 0, true);
}

/// 0x8af94: waits for the end of the fire, then sets off again.
fn fire_wait<R: Bench>(x: &mut Cx<'_, R>) {
    if !x.bit(ORDERS, 0) {
        if crew_gone(x) {
            return;
        }
        steps(x, 2, 4);
    }
    anim(x)
}

/// 0x8af62 / 0x8b3bc : tir.
fn fire<R: Bench>(x: &mut Cx<'_, R>) {
    if x.u8(0x7) == 0 {
        fire_order(x)
    } else {
        fire_wait(x)
    }
}

/// Rises and descends in depth (`+0x1c`) between 0x40 and 0x70 while the crew holds.
fn sway<R: Bench>(x: &mut Cx<'_, R>) {
    count_crew(x);
    if x.u8(CREW_COUNT) != 0 {
        let z = x.u16(0x10) as i16;
        if x.u8(COUNT) != 0 && z > 0x70 {
            negate(x, 0x1c);
            x.set8(COUNT, 0);
        } else if z <= 0x40 {
            negate(x, 0x1c);
            x.set8(COUNT, 1);
        }
        x.set16(0x14, 0);
        x.move_xy();
    }
    anim(x)
}

/// 0x8afd6: fire while moving in depth.
fn bob<R: Bench>(x: &mut Cx<'_, R>) {
    match x.u8(0x7) {
        0 => {
            if cannon_broken(x) {
                return;
            }
            next7(x);
            x.set8(GAIT, 2);
            x.set16(0x14, 0);
            x.set16(0x16, 0);
            x.set8(COUNT, 1);
            x.set16(0x1c, 0x120);
            x.set_bit(ORDERS, 0, true);
            x.set8(READY, 1);
        }
        2 => {
            if x.u8(READY) == 0 {
                next7(x);
                x.set8(READY, 1);
            }
            anim(x)
        }
        4 => {
            x.set8(GAIT, 0);
            if x.u8(READY) == 0 {
                return next7(x);
            }
            sway(x)
        }
        _ => {
            x.set8(GAIT, 2);
            if x.bit(ORDERS, 0) {
                return anim(x);
            }
            stop_over(x)
        }
    }
}

/// 0x8b100: toward the target's depth (`+0x1c`), and number of bombardments.
fn aim_lane<R: Bench>(x: &mut Cx<'_, R>) {
    x.set16(0x14, 0);
    x.set16(0x16, 0);
    x.set8(COUNT, 0);
    x.set16(0x1c, (-0x120i16) as u16);
    let t = load_target(x);
    let tz = t.u16(x.b, 0x10);
    if !Fl::sub16(x.u16(0x10), tz).gt() {
        x.set8(COUNT, 1);
        x.set16(0x1c, 0x120);
    }
    next7(x);
    draw_bombs(x)
}

/// 0x8b13e: bombardment order (bits 3 and 4 of `+0xa9`).
fn bomb_order<R: Bench>(x: &mut Cx<'_, R>) {
    if bombard_broken(x) {
        return;
    }
    next7(x);
    x.set8(GAIT, 2);
    x.set16(0x14, 0);
    x.set_bit(ORDERS, 3, true);
    x.set_bit(ORDERS, 4, true);
    x.set8(READY, 1);
}

/// 0x8b0ea: repeated bombardments while moving in depth.
fn bomb<R: Bench>(x: &mut Cx<'_, R>) {
    match x.u8(0x7) {
        0 => aim_lane(x),
        2 => bomb_order(x),
        4 => {
            if x.u8(READY) == 0 {
                next7(x);
                x.set8(READY, 1);
                x.set8(GAIT, 0);
            }
        }
        _ => {
            if x.bit(ORDERS, 4) {
                return sway(x);
            }
            if sub8(x, BOMBS, 1).pl() {
                return x.add8(0x7, 0xfc);
            }
            stop_over(x)
        }
    }
}

/// 0x8b226 / 0x8b454: bombardment then fire.
fn bomb_then_fire<R: Bench>(x: &mut Cx<'_, R>) {
    match x.u8(0x7) {
        0 => bomb_order(x),
        2 => {
            if x.bit(ORDERS, 4) {
                return anim(x);
            }
            next7(x);
        }
        4 => fire_order(x),
        _ => fire_wait(x),
    }
}

/// 0x8b328 / 0x8b33a: stop of `ticks` frames.
fn halt<R: Bench>(x: &mut Cx<'_, R>, ticks: u16) {
    if x.u8(0x7) == 0 {
        next7(x);
        x.set16(TIMER, ticks);
        x.set8(GAIT, 2);
        x.set16(0x14, 0);
        x.set16(0x16, 0);
        return;
    }
    // the original counts down the high byte of the word: the stop ends immediately
    if sub8(x, TIMER, 1).pl() {
        return;
    }
    stop_over(x)
}

/// 0x8b3ce: repeated fire (`+0xb9` salvos).
fn volley<R: Bench>(x: &mut Cx<'_, R>) {
    match x.u8(0x7) {
        0 => {
            next7(x);
            draw_bombs(x)
        }
        2 => fire_order(x),
        _ => {
            if x.bit(ORDERS, 0) {
                return anim(x);
            }
            if sub8(x, BOMBS, 1).pl() {
                return x.add8(0x7, 0xfe);
            }
            stop_over(x)
        }
    }
}

// --- group 6: wait ------------------------------------------------------------------------------

/// 0x8b46a: a player comes back: departure.
pub(super) fn wait_for_players<R: Bench>(x: &mut Cx<'_, R>) {
    x.set8(GAIT, 2);
    if var::PLAYERS_ALIVE.get(x.b) == 0 {
        return anim(x);
    }
    x.set32(f::STATE, 0x0202_0000);
    restart(x)
}

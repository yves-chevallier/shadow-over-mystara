//! "GiantBeetle" enemy (ENEMY type 9, 0x65ae8). See `docs/families/enemy.md`.
//!
//! Insect that scrolls along the side of the screen (variants `+3` = 0 to 4). Variant 4 is the nest: it
//! spawns pieces (ESHL 0x38) from a ROM list then waits to be visible. The trajectory group
//! `+0x2f` (0 to 7) picks the behavior of the active state, substate 4; `+7` is its step.
//!
//! | `+5` | Role (h) |
//! |---|---|
//! | 0, 2 | walks with U-turns (`steer`), watches for a player to bite (`bite_check`), leaves the screen |
//! | 4 | trajectories by group `+0x2f`: 0-1 crossing, 2-3 drift, 4-5 and 6-7 flight stopped in front of a player |
//! | 6 | leap: wind-up, flight, landing |
//! | 8 | dormant while the insect is off screen |
//!
//! State 4 (hit): recoil, fall then flashing death.
//! Animations 0x163a86 (movements) and 0x163a9e (hit).

use crate::families::cx::Bench;
use super::cx::{handlers, Cx};
use super::enemy_common::*;
use crate::objects::Obj;

const ANIMS: u32 = 0x163a86;
const ANIMS_HIT: u32 = 0x163a9e;

/// Fields specific to this type.
mod g {
    pub const GROUP: u32 = 0x2f;
    /// Current speed table (long).
    pub const SPEEDS: u32 = 0x4c;
    pub const HIT_CODE: u32 = 0x49;
    pub const FREEZE_AT_HIT: u32 = 0x56;
    /// Saved position (X and depth) and turn rate.
    pub const SAVED_X: u32 = 0xa8;
    pub const SAVED_DEPTH: u32 = 0xac;
    pub const TURN_PERIOD: u32 = 0xa2;
    pub const TURN_TIMER: u32 = 0xa3;
    pub const SIDE: u32 = 0xa4;
    pub const TURN_STEP: u32 = 0xa6;
    pub const STOP_STEP: u32 = 0xa7;
    pub const BITING: u32 = 0xb8;
    pub const DRIFT: u32 = 0xb4;
    pub const STOP_X: u32 = 0xb6;
    pub const SAVED_STATE: u32 = 0x8a;
}
use g::*;
use crate::objects::pools;
use crate::objects::Current;

/// ROM tables: speeds per difficulty (pointers), timings, turn directions.
const SPEEDS_WALK: u32 = 0x66700;
const SPEEDS_FLY: u32 = 0x66720;
const SPEEDS_DRIFT: u32 = 0x1c9036;
const TIMER_A: u32 = 0x666b4;
const TIMER_B: u32 = 0x666b8;
const TURN_DELTA: u32 = 0x66750;
const TURN_TARGETS: u32 = 0x667d4;
const STOP_XS: u32 = 0x660d6;
const SPEED_Y_ROW: u32 = 0x66666;
const BITE_ODDS: u32 = 0x6623a;
const SHAKE_TIMES: u32 = 0x664c6;
/// Nest piece lists (pointers by `+0x42`) and table of allocation routines.
const NEST_LISTS: u32 = 0x65c78;
const NEST_ALLOCS: u32 = 0x65c88;

const STANCE: u32 = 0x0200_0000;
const HELD: u32 = 0x0404_0000;
const POUNCE: u32 = 0x0206_0000;
const AFTER_POUNCE: u32 = 0x0204_0200;
const DEAD: u32 = 0x0600_0000;

pub fn run<R: Bench>(x: &mut Cx<'_, R>) {
    if suspended(x) {
        return;
    }
    match x.u8(f::STATE) {
        st::INIT => init(x),
        st::ACTIVE => active(x),
        st::HIT => hit(x),
        st::DEAD => free(x),
        _ => {}
    }
}

// --- aides --------------------------------------------------------------------------------------

fn anim_play<R: Bench>(x: &mut Cx<'_, R>, n: u32) {
    x.anim_start(ANIMS, n as u16);
}

/// 0x6662e: camera position, X in the low word of D6, Y in the high word.
fn camera<R: Bench>(x: &mut Cx<'_, R>) -> (u16, u16) {
    let (cx, cy) = if var::CAMERA_SELECT.get(x.b) != 0 { (var::CAMERA_ALT_X.get(x.b), var::CAMERA_ALT_Y.get(x.b)) } else { (var::SCROLL_X.get(x.b), var::SCROLL_Y.get(x.b)) };
    (cx, cy)
}

/// 0x666de: adopts a speed table (pairs of words per angle `+0x2d`).
fn speeds<R: Bench>(x: &mut Cx<'_, R>, table: u32) {
    x.set32(SPEEDS, table);
    let off = (x.u8(Obj::SPEED_INDEX) as u32) << 2;
    let (vx, vy) = (x.data16(table + off), x.data16(table + off + 2));
    x.set16(Obj::VX, vx);
    x.set16(Obj::VY, vy);
    x.set16(Obj::AX, 0);
    x.set16(Obj::AY, 0);
}

/// 0x666d2: speed table chosen by the difficulty (`A5+0x7f92` & 0x1c).
fn speeds_by_level<R: Bench>(x: &mut Cx<'_, R>, list: u32) {
    let level = (var::RANK.get(x.b) & 0x1c) as u32;
    let table = x.data32(list + level);
    speeds(x, table);
}

/// 0x666a2: turn rate (bytes `+0xa2/+0xa3`) according to the difficulty.
fn turn_timer<R: Bench>(x: &mut Cx<'_, R>, table: u32) {
    let off = ((var::RANK.get(x.b) & 0x10) >> 3) as u32;
    let v = x.data16(table + off);
    x.set16(TURN_PERIOD, v);
}

/// 0x6664c / 0x6664e: speed `+0x1e` read from the table according to the difficulty, `base` = row offset.
fn set_speed_y<R: Bench>(x: &mut Cx<'_, R>, base: u16) {
    let off = ((var::RANK.get(x.b) & 0x18) >> 2).wrapping_add(base);
    let v = x.data16(SPEED_Y_ROW + off as u32);
    x.set16(Obj::AY, v);
}

// --- state 0: creation --------------------------------------------------------------------------

fn init<R: Bench>(x: &mut Cx<'_, R>) {
    x.add8(f::STATE, 2);
    x.set16(f::SUB2, 0);
    x.set8(f::BOX_SET, 0x15);
    lib::initial_hp(x);
    lib::set_immunities(x);
    for (off, v) in [(0x7e, 4), (0x7f, 0), (0x6c, 3), (0x74, 3), (0x76, 3)] {
        x.set8(off, v);
    }
    x.set16(Obj::Z, 0x36);
    x.set8(0xa5, 0);
    x.set8(BITING, 0);
    match x.u8(Obj::VARIANT) {
        0 | 1 | 3 => start_walk(x),
        2 => enter_from_edge(x),
        4 => nest(x),
        _ => {}
    }
}

/// 0x65b6c: the insect leaves in the direction dictated by the parity of `+0x2f`.
fn start_walk<R: Bench>(x: &mut Cx<'_, R>) {
    let (px, pz) = (x.u16(Obj::X), x.u16(Obj::Y));
    x.set16(SAVED_X, px);
    x.set16(SAVED_DEPTH, pz);
    x.set8(SIDE, 0);
    x.set8(Obj::SPEED_INDEX, 8);
    x.set8(f::FLIP, 0x20);
    x.set8(TURN_STEP, 8);
    if x.u8(GROUP) & 1 == 0 {
        x.set8(Obj::SPEED_INDEX, 0x18);
        x.set8(f::FLIP, 0);
        x.set8(TURN_STEP, 0);
    }
    anim_play(x, 0);
}

/// 0x65bb4: resumes walking after a leap, according to the orientation and the parity of `+0x2f`.
fn resume_walk<R: Bench>(x: &mut Cx<'_, R>) {
    let (px, pz) = (x.u16(Obj::X), x.u16(Obj::Y));
    x.set16(SAVED_X, px);
    x.set16(SAVED_DEPTH, pz);
    let flipped = x.u8(f::FLIP) != 0;
    x.set8(Obj::SPEED_INDEX, if flipped { 8 } else { 0x18 });
    x.set8(TURN_STEP, if flipped { 8 } else { 0 });
    let odd_group = x.u8(GROUP) != 0;
    x.set8(SIDE, if flipped != odd_group { 0x10 } else { 0 });
    anim_play(x, 0);
}

/// Variant 2: enters from the right edge (or left for odd groups) of the screen.
fn enter_from_edge<R: Bench>(x: &mut Cx<'_, R>) {
    x.set8(f::SUB, 4);
    let (cx, _) = camera(x);
    let mut offset = 0x190u16;
    x.set8(SIDE, 0);
    x.set8(f::FLIP, 0);
    x.set8(TURN_STEP, 0);
    x.set8(Obj::SPEED_INDEX, 0x18);
    if x.u8(GROUP) & 1 != 0 {
        offset = -0x10i16 as u16;
        x.set8(Obj::SPEED_INDEX, 8);
        x.set8(f::FLIP, 0x20);
        x.set8(TURN_STEP, 8);
    }
    let px = cx.wrapping_add(offset);
    x.set16(Obj::X, px);
    anim_play(x, 0);
}

/// Variant 4: the nest spawns its pieces (list `+0x42`) then sleeps until visible.
fn nest<R: Bench>(x: &mut Cx<'_, R>) {
    start_walk(x);
    x.set8(f::SUB, 8);
    let list = x.u8(0x42) as u32;
    if list == 3 {
        x.set16(Obj::Z, 0x60);
    }
    spawn_list(x, NEST_LISTS, NEST_ALLOCS, list);
}

// --- state 2: active -----------------------------------------------------------------------------

fn active<R: Bench>(x: &mut Cx<'_, R>) {
    if !paused(x) {
        let freeze = x.u8(f::FREEZE) as i8;
        if freeze > 0 {
            x.set8(f::FREEZE, (freeze - 1) as u8);
        } else {
            match x.u8(f::SUB) {
                0 | 2 => march(x),
                4 => routes(x),
                6 => pounce(x),
                8 => dormant(x),
                _ => {}
            }
        }
    }
    x.reinsert();
}

/// Substates 0 and 2: walks with U-turns; leaves the screen when it is too far from it.
fn march<R: Bench>(x: &mut Cx<'_, R>) {
    if grabbed(x) {
        return set_state_long(x, HELD);
    }
    if x.u8(f::SUB3) == 0 {
        x.add8(f::SUB3, 2);
        speeds_by_level(x, SPEEDS_WALK);
        turn_timer(x, TIMER_B);
    }
    steer(x);
    bite_check(x);
    let (cx, cy) = camera(x);
    let d5 = x.u16(SAVED_X).wrapping_sub(cx).wrapping_add(0x80);
    let inside = d5 < 0x280 && x.u16(SAVED_DEPTH).wrapping_add(x.u16(Obj::Z)).wrapping_sub(cy).wrapping_add(0x10) <= 0x120;
    if !inside {
        set_state_long(x, DEAD);
        detach_from_host(x);
    }
    x.anim_step();
}

/// 0x66740: advances in the speed table; every `+0xa2` frames it turns one notch toward the wanted angle
/// and chains the turn animation.
fn steer<R: Bench>(x: &mut Cx<'_, R>) {
    x.move_by_table_xy();
    let n = x.u8(TURN_TIMER).wrapping_sub(1);
    x.set8(TURN_TIMER, n);
    if n != 0 {
        return;
    }
    let period = x.u8(TURN_PERIOD);
    x.set8(TURN_TIMER, period);
    let idx = ((x.u8(TURN_STEP) & 8) >> 3).wrapping_add(x.u8(SIDE) >> 3) as u32;
    let delta = x.rom8(TURN_DELTA + idx);
    let angle = x.u8(Obj::SPEED_INDEX).wrapping_add(delta) & 0x1f;
    x.set8(Obj::SPEED_INDEX, angle);
    let key = x.u8(TURN_STEP).wrapping_add(x.u8(SIDE)) as u32;
    let entry = TURN_TARGETS + key;
    if angle & 0xf != x.data8(entry) {
        return;
    }
    if x.u8(TURN_STEP) & 6 == 6 {
        let flip = x.u8(f::FLIP) ^ 0x20;
        x.set8(f::FLIP, flip);
        let side = x.u8(SIDE) ^ 0x10;
        x.set8(SIDE, side);
    }
    let step = x.u8(TURN_STEP).wrapping_add(2) & 0xf;
    x.set8(TURN_STEP, step);
    let n = x.data8(entry + 1);
    anim_play(x, n as u32);
}

/// 0x661ce: a player in front of the insect, on floor 0xc of state 2: a draw that triggers it into a leap.
fn bite_check<R: Bench>(x: &mut Cx<'_, R>) {
    if !find_player(x, -0x20, 0x20, 8) {
        return x.set8(BITING, 0);
    }
    if x.u8(BITING) == 0 {
        let player = x.linked(f::TARGET);
        if player.u16(x.b, 4) == 0x200 && player.u8(x.b, 6) == 0xc {
            let roll = random(x) & 0x1f;
            let row = ((var::RANK.get(x.b) >> 1) as u32) << 2;
            let odds = rom32(x, BITE_ODDS + row);
            if odds >> roll & 1 != 0 {
                let state = x.u32(f::STATE);
                x.set32(SAVED_STATE, state);
                set_state_long(x, POUNCE);
            }
        }
    }
    x.set8(BITING, 1);
}

// --- substate 4: trajectories -----------------------------------------------------------------

fn routes<R: Bench>(x: &mut Cx<'_, R>) {
    if x.u8(f::SUB2) != 0 {
        return turn_back(x);
    }
    match x.u8(GROUP) {
        0 | 1 => cross(x),
        2 | 3 => drift(x),
        4 | 5 => halt_at(x),
        6 | 7 => halt_and_bite(x),
        _ => {}
    }
}

/// 0x65ea8: start of a crossing.
fn cross_start<R: Bench>(x: &mut Cx<'_, R>) {
    x.add8(f::SUB3, 2);
    speeds_by_level(x, SPEEDS_FLY);
    turn_timer(x, TIMER_A);
    anim_play(x, 0);
}

/// Groups 0 and 1: crossing, stop walking then turn, exit.
fn cross<R: Bench>(x: &mut Cx<'_, R>) {
    match x.u8(f::SUB3) {
        0 => cross_start(x),
        2 => {
            x.move_xy();
            let (cx, _) = camera(x);
            let d5 = x.u16(Obj::X).wrapping_sub(cx);
            let arrived = if x.u8(GROUP) & 1 != 0 { (d5 as i16) >= 0x140 } else { (d5 as i16) <= 0x40 };
            if arrived {
                x.add8(f::SUB3, 2);
                set_speed_y(x, 0x28);
            }
            x.anim_step();
        }
        4 => cross_turn(x),
        6 => leave(x),
        _ => {}
    }
}

/// 0x65f18: turn until the exit angle. The original tests the parity of a side passed in D0 but does the same on both
/// branches.
fn cross_turn<R: Bench>(x: &mut Cx<'_, R>) {
    steer(x);
    let wanted = if x.u8(GROUP) & 1 != 0 { 0x18 } else { 8 };
    if x.u8(Obj::SPEED_INDEX) == wanted {
        x.add8(f::SUB3, 2);
        x.set8(0, 1);
        speeds_by_level(x, SPEEDS_WALK);
    }
}

/// 0x65f5c: exit through the edge; the insect is removed once it has passed it.
fn leave<R: Bench>(x: &mut Cx<'_, R>) {
    x.move_xy();
    let (cx, _) = camera(x);
    let d5 = x.u16(Obj::X).wrapping_sub(cx);
    let out = if x.u8(GROUP) & 1 != 0 { (d5 as i16) < -0x10 } else { (d5 as i16) >= 0x190 };
    if out {
        x.set8(f::STATE, 6);
        detach_from_host(x);
    }
    x.anim_step();
}

/// Groups 2 and 3: constant lateral drift (`+0xb4`) until it leaves the screen.
fn drift<R: Bench>(x: &mut Cx<'_, R>) {
    match x.u8(f::SUB3) {
        0 => {
            x.add8(f::SUB3, 2);
            x.set8(Obj::SPEED_INDEX, 0x18);
            let mut d6 = 0x11;
            if x.u8(GROUP) & 1 != 0 {
                x.set8(Obj::SPEED_INDEX, 8);
                d6 = 1;
            }
            speeds(x, SPEEDS_DRIFT);
            x.set16(TURN_PERIOD, 0x101);
            x.set16(Obj::VX, 0x300);
            if x.u8(Obj::SPEED_INDEX) != 8 {
                negate(x, Obj::VX);
            }
            let vx = x.u16(Obj::VX);
            x.set16(DRIFT, vx);
            x.set8(Obj::SPEED_INDEX, d6 as u8);
            anim_play(x, 0);
        }
        2 => {
            let pos = x.u32(Obj::X);
            x.set32(SAVED_X, pos);
            x.move_by_table_xz();
            let pos = x.u32(SAVED_X);
            x.set32(Obj::X, pos);
            let shift = ((x.u16(DRIFT) as i16 as i32) << 8) as u32;
            x.add32(Obj::X, shift);
            let n = x.u8(TURN_TIMER).wrapping_sub(1);
            x.set8(TURN_TIMER, n);
            if n == 0 {
                let period = x.u8(TURN_PERIOD);
                x.set8(TURN_TIMER, period);
                x.add8(Obj::SPEED_INDEX, 1);
                let a = x.u8(Obj::SPEED_INDEX) & 0x1f;
                x.set8(Obj::SPEED_INDEX, a);
                let (cx, _) = camera(x);
                let d5 = x.u16(Obj::X).wrapping_sub(cx);
                let out = if x.u8(GROUP) & 1 != 0 { (d5 as i16) >= 0x190 } else { (d5 as i16) <= -0x10 };
                if out {
                    set_state_long(x, DEAD);
                    detach_from_host(x);
                }
            }
            x.anim_step();
        }
        _ => {}
    }
}

/// 0x660bc: stop abscissa (`+0xb6`) read from the table according to the stop rank.
fn set_stop<R: Bench>(x: &mut Cx<'_, R>, rank: u8) {
    let base = STOP_XS + if x.u8(GROUP) & 1 != 0 { 6 } else { 0 };
    let v = x.data16(base + rank as u32);
    x.set16(STOP_X, v);
}

/// 0x660aa: start of a stopped flight.
fn halt_start<R: Bench>(x: &mut Cx<'_, R>) {
    cross_start(x);
    x.set8(STOP_STEP, 0);
    speeds_by_level(x, SPEEDS_FLY);
    turn_timer(x, TIMER_A);
    set_stop(x, 0);
}

/// 0x660e2: flight to the stop abscissa; in front of a player, stops rank by rank then charges.
fn halt_run<R: Bench>(x: &mut Cx<'_, R>) {
    x.move_xy();
    let (cx, _) = camera(x);
    let d5 = x.u16(Obj::X).wrapping_sub(cx);
    let stop = x.u16(STOP_X);
    *x.c.d5() = *x.c.d5() & 0xffff_0000 | d5 as u32;
    let before = if x.u8(GROUP) & 1 != 0 { (d5 as i16) < stop as i16 } else { (d5 as i16) > stop as i16 };
    if before {
        x.anim_step();
        return;
    }
    if x.u8(STOP_STEP) != 4 {
        let d1 = if x.u8(STOP_STEP) != 0 { 0x60 } else { 0x80 };
        if find_player(x, 0, d1, 0x30) {
            x.add8(STOP_STEP, 2);
            let rank = x.u8(STOP_STEP);
            set_stop(x, rank);
            x.anim_step();
            return;
        }
    }
    x.set8(f::SUB3, 4);
    if !find_player(x, 0, 0x180, 0x30) {
        set_speed_y(x, 0x28);
    } else {
        let player = load_target(x);
        x.set8(SIDE, 0);
        let mut gap = x.u16(Obj::Y).wrapping_sub(player.u16(x.b, Obj::Y)).wrapping_sub(4);
        if (gap as i16) < 0 {
            gap = gap.wrapping_neg();
            x.set8(SIDE, 0x10);
        }
        if gap > 0x30 {
            set_speed_y(x, 0x28);
        } else {
            set_speed_y(x, gap & 0x38);
        }
    }
    if x.u8(SIDE) != 0 {
        negate(x, Obj::AY);
    }
    x.anim_step();
}

/// Groups 4 and 5.
fn halt_at<R: Bench>(x: &mut Cx<'_, R>) {
    match x.u8(f::SUB3) {
        0 => halt_start(x),
        2 => halt_run(x),
        4 => side_turn(x),
        6 => leave(x),
        _ => {}
    }
}

/// Groups 6 and 7: like 4 and 5, with the bite checked at each step.
fn halt_and_bite<R: Bench>(x: &mut Cx<'_, R>) {
    match x.u8(f::SUB3) {
        0 => {
            halt_start(x);
            x.set8(BITING, 0);
        }
        2 => {
            halt_run(x);
            bite_check(x);
        }
        4 => {
            side_turn(x);
            bite_check(x);
        }
        6 => {
            leave(x);
            bite_check(x);
        }
        _ => {}
    }
}

/// 0x65efe: sideways turn. The original computes a side from `+0xa4` and the parity of `+0x2f` for 0x65f18, which does
/// not use it.
fn side_turn<R: Bench>(x: &mut Cx<'_, R>) {
    cross_turn(x);
}

/// Substate 4 with `+6` non-zero (0x6628a): turns toward a player then climbs back up.
fn turn_back<R: Bench>(x: &mut Cx<'_, R>) {
    match x.u8(f::SUB3) {
        0 => {
            x.set8(f::SUB3, 4);
            let player = load_target(x);
            let (px, pz) = (player.u16(x.b, Obj::X), player.u16(x.b, Obj::Y));
            x.set16(0xb0, px);
            x.set16(0xb2, pz);
            let angle = lib::aim_to(x, player);
            x.set8(Obj::SPEED_INDEX, angle);
            if angle & 0xf != 0 {
                let side = (angle.wrapping_add(0x10) & 0x10) << 1;
                if side ^ x.u8(f::FLIP) != 0 {
                    x.set8(f::SUB3, 2);
                }
            }
            x.anim_step();
        }
        2 | 4 => {
            if x.u8(f::SUB3) == 2 {
                x.add8(f::SUB3, 2);
                let flip = x.u8(f::FLIP) ^ 0x20;
                x.set8(f::FLIP, flip);
            }
            x.add8(f::SUB3, 2);
            x.set8(TURN_PERIOD, 0x3c);
            speeds_by_level(x, SPEEDS_FLY);
            x.set16(Obj::VZ, -0x400i16 as u16);
            x.set16(Obj::AZ, 0);
            if x.u8(Obj::VARIANT) == 0 {
                x.set16(Obj::VZ, -0x200i16 as u16);
            }
            x.anim_step();
        }
        6 => {
            x.move_by_table_xy();
            x.move_z();
            let floor = if x.u8(Obj::VARIANT) == 0 { 0x30 } else { 0x10 };
            if floor >= x.u16(Obj::Z) {
                x.set16(Obj::Z, floor);
                x.set16(Obj::VZ, 0);
                x.add8(f::SUB3, 2);
            }
            x.anim_step();
        }
        8 => {
            x.move_xy();
            let (cx, cy) = camera(x);
            if x.u8(Obj::VARIANT) == 0 {
                if find_player(x, -0x80, 0x40, 0x20) {
                    x.anim_step();
                } else {
                    resume_walk(x);
                    set_state_long(x, STANCE);
                }
                return;
            }
            let d5 = x.u16(Obj::X).wrapping_sub(cx).wrapping_add(0x40);
            let ok = d5 < 0x200 && x.u16(Obj::Y).wrapping_add(x.u16(Obj::Z)).wrapping_sub(cy).wrapping_add(0x10) <= 0x120;
            if !ok {
                x.set8(f::STATE, 6);
                detach_from_host(x);
            }
            x.anim_step();
        }
        _ => {}
    }
}

// --- substate 6: leap -------------------------------------------------------------------------

fn pounce<R: Bench>(x: &mut Cx<'_, R>) {
    match x.u8(f::SUB2) {
        0 => {
            x.add8(f::SUB2, 2);
            x.set16(Obj::VX, 0x400);
            x.set16(Obj::VZ, 0x400);
            x.set16(Obj::AX, 0);
            x.set16(Obj::AZ, 0);
            x.set8(0x6e, 0xc);
            if x.u8(f::FLIP) != 0 {
                negate(x, Obj::VX);
            }
            anim_play(x, 8);
        }
        2 => {
            x.move_xz();
            let n = x.u8(0x6e).wrapping_sub(1);
            x.set8(0x6e, n);
            if n == 0 {
                x.add8(f::SUB2, 2);
                x.set8(0x6e, 0x40);
            }
            x.anim_step();
        }
        4 => {
            let n = x.u8(0x6e).wrapping_sub(1);
            x.set8(0x6e, n);
            if n != 0 {
                x.anim_step();
                return;
            }
            x.set8(0xa5, 0);
            set_state_long(x, AFTER_POUNCE);
        }
        _ => {}
    }
}

/// Substate 8: dormant; woken up (substate 0) when it is less than 0x1c0 pixels from the camera.
fn dormant<R: Bench>(x: &mut Cx<'_, R>) {
    let cam = if var::CAMERA_SELECT.get(x.b) != 0 { var::CAMERA_ALT_X.get(x.b) } else { var::SCROLL_X.get(x.b) };
    let d0 = x.u16(Obj::X).wrapping_sub(cam).wrapping_add(0x20);
    if d0 <= 0x1c0 {
        x.set8(f::SUB, 0);
    }
}

// --- state 4: hit ----------------------------------------------------------------------------

fn hit<R: Bench>(x: &mut Cx<'_, R>) {
    let freeze = x.u8(f::FREEZE) as i8;
    if freeze > 0 {
        x.set8(f::FREEZE, (freeze - 1) as u8);
    } else {
        match x.u8(f::SUB) {
            0 => react_first(x),
            2 => recoil(x),
            4 => fall(x),
            _ => {}
        }
    }
    x.reinsert();
}

/// Substate 0: a hit with code 0x5c shakes the insect for a few frames (`+6` = 0, 2) before the recoil.
fn react_first<R: Bench>(x: &mut Cx<'_, R>) {
    if x.u8(HIT_CODE) != 0x5c {
        return knock_back(x);
    }
    match x.u8(f::SUB2) {
        0 => {
            x.add8(f::SUB2, 2);
            shake_timer(x);
        }
        2 => {
            if var::PAUSE.get(x.b) == 0 {
                x.add8(f::SUB2, 2);
            }
            let n = x.u8(0xa2).wrapping_sub(1);
            x.set8(0xa2, n);
            if n != 0 {
                return;
            }
            if lib::attack_effect(x, 0, 0) {
                shake_timer(x);
            } else {
                x.set8(0xa2, 1);
            }
        }
        4 => knock_back(x),
        _ => {}
    }
}

/// 0x664b4: randomly drawn duration of the shake.
fn shake_timer<R: Bench>(x: &mut Cx<'_, R>) {
    let r = random(x) & 3;
    let t = x.rom8(SHAKE_TIMES + r);
    x.set8(0xa2, t);
}

/// 0x664f8: recoil, facing away from the hit.
fn knock_back<R: Bench>(x: &mut Cx<'_, R>) {
    x.add8(f::SUB, 2);
    x.set8(f::SUB2, 0);
    let freeze = x.u8(FREEZE_AT_HIT);
    x.set8(f::FREEZE, freeze);
    x.set16(Obj::VZ, 0x400);
    x.set16(Obj::AZ, -0x40i16 as u16);
    x.set16(Obj::VX, 0x200);
    x.set16(Obj::AX, 0);
    let flip = x.u8(0x4a) ^ 0x20;
    x.set8(f::FLIP, flip);
    if flip != 0 {
        negate(x, Obj::VX);
        negate(x, Obj::AX);
    }
    x.set8(0, 2);
    x.anim_start(ANIMS_HIT, 0);
}

/// Substate 2: landing then blinking until death.
fn recoil<R: Bench>(x: &mut Cx<'_, R>) {
    match x.u8(f::SUB2) {
        0 => {
            x.move_xz();
            if x.u8(0x29) != 0 {
                x.set16(Obj::VX, 0);
                x.set16(Obj::AX, 0);
            }
            if (x.u16(Obj::Z) as i16) <= 0 {
                x.add8(f::SUB2, 2);
                x.set32(Obj::Z, 0);
                x.set8(0xa2, 0x28);
                x.set16(0x30, 1);
                detach_from_host(x);
            }
            x.anim_step();
        }
        2 => {
            let n = x.u8(0xa2).wrapping_sub(1);
            x.set8(0xa2, n);
            if n == 0 {
                set_state_long(x, DEAD);
                return lib::object_event(x);
            }
            x.set_bit(1, 1, n & 3 != 0);
            x.anim_step();
        }
        _ => {}
    }
}

/// Substate 4: grabbed by a player, the insect struggles then leaves the screen.
fn fall<R: Bench>(x: &mut Cx<'_, R>) {
    if paused(x) {
        return;
    }
    if x.u8(f::SUB2) == 0 {
        x.add8(f::SUB2, 2);
        x.set16(Obj::AX, 0);
        x.set16(Obj::AY, 0);
        let angle = if x.u8(f::FLIP) != 0 { 8 } else { 0x18 };
        x.set8(Obj::SPEED_INDEX, angle);
        return anim_play(x, 4);
    }
    x.move_by_table_xy();
    if !x.in_arena_default() {
        detach_from_host(x);
        set_state_long(x, DEAD);
    }
    x.anim_step();
}

fn free<R: Bench>(x: &mut Cx<'_, R>) {
    x.free(&pools::ENEMY);
}

handlers! {
    0x65ae8 "enemy_9" run,
}

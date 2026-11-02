//! "Harpy" enemy (ENEMY type 21, 0x756c4): flying creature that crosses the screen. See
//! `docs/families/enemy.md`.
//!
//! Ten variants (`+3`): 0 and 1 fly from one edge to the other, 2 to 5 skim the terrain, 6 and 7 turn
//! around, 8 goes to the foreground, 9 is the model with no movement of its own. Each creature creates at
//! birth a companion (shadow, ESHL type 0x10) whose link `+0xaa` keeps. Substates in the active state:
//! 0 flight, 2 attack (`+0x2f` picks the series: 0 projectiles, 1 hits, 2 shot), 4 pause, 6 exit
//! from the screen, 8 exit through the top.

use crate::families::cx::Bench;
use super::cx::{handlers, Cx};
use super::enemy_common::*;
use crate::objects::Obj;
use super::effects_creators::leftovers::Pure;

/// Animation sets: flight and attacks.
const SET_FLY: u32 = 0x16a10c;
const SET_ATTACK: u32 = 0x16a116;
const SET_HIT: u32 = 0x16a132;
/// Rows of eight bytes per variant (orientation, durations, X offset) and Y offsets according to `+0x42`.
const VARIANT_ROWS: u32 = 0x756e6;
const OFFSETS_Y: u32 = 0x757d2;
/// Hit reaction animation, according to the attack series `+0x2f`.
const HIT_ANIM: u32 = 0x75e84;

mod g {
    pub const SERIES: u32 = 0x2f;
    pub const PARAM: u32 = 0x42;
    pub const KIND: u32 = 0x43;
    pub const COMPANION: u32 = 0xaa;
    pub const TIMER_A: u32 = 0xa0;
    pub const TIMER_B: u32 = 0xa1;
    pub const STEP: u32 = 0xa2;
    pub const PASSES: u32 = 0xa3;
    pub const SHOT_DONE: u32 = 0xa5;
    pub const SHOT: u32 = 0xa6;
    pub const EXIT_LIMIT: u32 = 0xae;
    pub const TURN_Z: u32 = 0xb0;
    pub const RESUME: u32 = 0xb1;
}
use g::*;
use crate::objects::Current;

pub fn run<R: Bench>(x: &mut Cx<'_, R>) {
    if suspended(x) {
        return;
    }
    match x.u8(f::STATE) {
        st::INIT => init(x),
        st::ACTIVE => active(x),
        st::HIT => hit(x),
        st::DEAD => dead(x),
        _ => {}
    }
}

// --- aides ------------------------------------------------------------------------------------

fn step<R: Bench>(x: &mut Cx<'_, R>) {
    x.anim_step();
}

fn set_state_long<R: Bench>(x: &mut Cx<'_, R>, v: u32) {
    x.set32(f::STATE, v);
}

/// Re-listing: variant 8 lives in the foreground list.
fn reinsert<R: Bench>(x: &mut Cx<'_, R>) {
    if x.u8(Obj::VARIANT) == 8 {
        x.reinsert_foreground();
    } else {
        x.reinsert();
    }
}

/// The companion (`+0xaa`) goes to state 6 (release).
fn release_companion<R: Bench>(x: &mut Cx<'_, R>) {
    let c = x.linked(COMPANION);
    c.set_u8(x.b, f::STATE, 6);
}

/// End of the flight: the creature and its companion go to the next state.
fn vanish<R: Bench>(x: &mut Cx<'_, R>) {
    x.add8(f::STATE, 4);
    release_companion(x);
}

/// Screen exit: like `vanish`, first removing the creature from the scene (0x27ee).
fn leave<R: Bench>(x: &mut Cx<'_, R>) {
    x.add8(f::STATE, 4);
    detach_from_host(x);
    release_companion(x);
}

fn speed<R: Bench>(x: &mut Cx<'_, R>, n: u32) {
    x.select_speed(&crate::objects::motion::SPEEDS_B, n as u16);
}

fn move_xz_table<R: Bench>(x: &mut Cx<'_, R>) {
    x.move_by_table_xz();
}

/// One turn of the flight angle: every `reload` frames, `+0xa2` is added to the direction. In free flight
/// (`clamp`, 0x758aa) an angle that became negative is reset to zero.
fn bank<R: Bench>(x: &mut Cx<'_, R>, reload: u8, clamp: bool) {
    let t = x.u8(TIMER_A).wrapping_sub(1);
    x.set8(TIMER_A, t);
    if (t as i8) < 0 {
        x.set8(TIMER_A, reload);
        let mut angle = x.u8(Obj::SPEED_INDEX).wrapping_add(x.u8(STEP));
        if clamp && (angle as i8) < 0 {
            angle = 0;
        }
        x.set8(Obj::SPEED_INDEX, angle);
    }
}

// --- state 0: creation --------------------------------------------------------------------------

fn init<R: Bench>(x: &mut Cx<'_, R>) {
    x.add8(f::STATE, 2);
    x.set8(0, 1);
    x.set8(f::BOX_SET, 0xe);
    lib::initial_hp(x);
    let index = lib::set_immunities(x);
    x.set8(0x68, 4);
    x.set8(0x69, 4);
    x.set8(0x76, 4);
    x.set8(0x67, 0);
    // Original glitch: `+0x8e` receives the low byte of D0, the index into the immunity table that 0xfe5a leaves.
    x.set8(0x8e, index as u8);
    x.set8(0x7f, 0);
    x.set8(0x7e, 0xc);
    x.set32(0x8a, 0);
    x.set8(PASSES, 2);
    // variant row: orientation, durations `+0xa0..+0xa3`, X offset
    let row = VARIANT_ROWS + x.u8(Obj::VARIANT).wrapping_shl(3) as u32;
    let flip = x.data16(row);
    x.set16(f::FLIP, flip);
    let timers = x.data32(row + 2);
    x.set32(TIMER_A, timers);
    let dx = x.data16(row + 6);
    x.add16(0x8, dx);
    let mut d4 = x.u16(0xc) as u32;
    if x.u8(KIND) != 0 {
        d4 = d4 & !0xff | x.u8(PARAM) as u32;
        let a2 = OFFSETS_Y.wrapping_add(d4 as u16 as i16 as i32 as u32);
        let dy = x.data16(a2);
        x.add16(0x10, dy);
        d4 = d4 & !0xffff | x.data16(a2 + 2) as u32;
    }
    let d4 = d4 as u16;
    let variant = x.u8(Obj::VARIANT);
    if variant == 8 || (variant as i8) < 2 {
        x.set16(EXIT_LIMIT, 0x100u16.wrapping_add(d4));
        x.set16(0xc, d4);
    } else {
        let limit = if variant & 1 != 0 { -0x100i16 as u16 } else { 0x280 };
        x.set16(EXIT_LIMIT, limit);
    }
    let anim = (x.u8(KIND).wrapping_add(x.u8(SERIES)) as i8) as i16 as u16;
    x.anim_start(SET_FLY, (anim as u32) as u16);
    speed(x, 0x18);
    if variant >> 1 == 0 {
        let vz = x.u16(0x1c);
        x.set16(0x18, vz);
    }
    // 0xa036e: companion; on failure, the original still stores the A1 left by the allocation.
    let companion = match super::small::harpy_companion_via(&mut Pure, x.b, x.o) {
        Ok((o, hp)) => {
            // Original glitch: HP/4 stays in D2.
            *x.c.d2() = *x.c.d2() & 0xffff_0000 | hp as u32;
            o
        }
        Err(a1) => {
            x.add8(f::STATE, 4);
            a1
        }
    };
    x.set16(COMPANION, companion.word());
    reinsert(x);
}

// --- state 2: active -----------------------------------------------------------------------------

fn active<R: Bench>(x: &mut Cx<'_, R>) {
    let freeze = x.u8(f::FREEZE) as i8;
    if freeze > 0 {
        x.set8(f::FREEZE, (freeze - 1) as u8);
    }
    match x.u8(f::SUB) {
        0 => fly(x),
        2 => attack(x),
        4 => pause(x),
        6 => exit_flight(x),
        8 => attack_end(x),
        _ => {}
    }
    reinsert(x);
}

/// +5 = 0 (0x7594a): flight of the variant.
fn fly<R: Bench>(x: &mut Cx<'_, R>) {
    if grabbed(x) {
        return set_state_long(x, 0x0206_0000);
    }
    match x.u8(Obj::VARIANT) {
        0 | 1 => fly_loop(x, 0xc),
        2..=5 => fly_loop_b(x),
        6 | 7 => turn_back(x),
        8 => fly_front(x),
        _ => {}
    }
}

/// End of a pass: `+0xa1` elapsed, the creature leaves again or attacks (variants 0 and 1).
fn fly_loop<R: Bench>(x: &mut Cx<'_, R>, reload: u8) {
    move_xz_table(x);
    step(x);
    bank(x, reload, false);
    let t = x.u8(TIMER_B).wrapping_sub(1);
    x.set8(TIMER_B, t);
    if (t as i8) < 0 {
        let passes = x.u8(PASSES).wrapping_sub(1);
        x.set8(PASSES, passes);
        if (passes as i8) < 0 {
            return x.add8(f::SUB, 6);
        }
        x.set8(TIMER_B, 0x14);
        x.add8(f::SUB, 2);
        return x.set8(RESUME, 0);
    }
    if (x.u16(0xc) as i16) > (x.u16(EXIT_LIMIT) as i16) {
        vanish(x);
    }
}

/// Variants 2 to 5 (0x75aec): same flight, slower pass and a pause of 0x30 frames.
fn fly_loop_b<R: Bench>(x: &mut Cx<'_, R>) {
    move_xz_table(x);
    step(x);
    bank(x, 0x10, false);
    let t = x.u8(TIMER_B).wrapping_sub(1);
    x.set8(TIMER_B, t);
    if (t as i8) < 0 {
        let passes = x.u8(PASSES).wrapping_sub(1);
        x.set8(PASSES, passes);
        if (passes as i8) < 0 {
            return x.add8(f::SUB, 6);
        }
        x.set8(RESUME, 0);
        x.set8(TIMER_B, 0x30);
        x.add8(f::SUB, 2);
    }
}

/// Variant 8 (0x75aa2): flight in the foreground.
fn fly_front<R: Bench>(x: &mut Cx<'_, R>) {
    move_xz_table(x);
    step(x);
    bank(x, 0xc, false);
    let t = x.u8(TIMER_B).wrapping_sub(1);
    x.set8(TIMER_B, t);
    if (t as i8) < 0 {
        return set_state_long(x, 0x0208_0000);
    }
    if (x.u16(0xc) as i16) > (x.u16(EXIT_LIMIT) as i16) {
        vanish(x);
    }
}

/// Variants 6 and 7 (0x75980): crosses the screen then turns around.
fn turn_back<R: Bench>(x: &mut Cx<'_, R>) {
    match x.u8(f::SUB2) {
        0 => {
            x.move_x();
            let t = x.u8(TIMER_B).wrapping_sub(1);
            x.set8(TIMER_B, t);
            if (t as i8) < 0 {
                let z = x.u16(0xc);
                x.set16(TURN_Z, z);
                x.set8(Obj::SPEED_INDEX, 0);
                x.set8(RESUME, 2);
                set_state_long(x, 0x0202_0000);
            }
        }
        2 => {
            move_xz_table(x);
            step(x);
            if (x.u16(0xc) as i16) >= 0xf0 {
                let mut d = -0x30i16;
                if x.u8(f::FLIP) != 0 {
                    d = -d;
                }
                x.add16(0x8, d as u16);
                x.set8(Obj::SPEED_INDEX, 0x10);
                x.add8(f::SUB2, 2);
                let c = x.linked(COMPANION);
                let flip = c.u8(x.b, f::FLIP) ^ 0x20;
                c.set_u8(x.b, f::FLIP, flip);
                let own = x.u8(f::FLIP) ^ 0x20;
                x.set8(f::FLIP, own);
            }
        }
        4 => {
            move_xz_table(x);
            step(x);
            if (x.u16(TURN_Z) as i16) > (x.u16(0xc) as i16) {
                let angle = if x.u8(f::FLIP) == 0 { 8 } else { 0x18 };
                x.set8(Obj::SPEED_INDEX, angle);
                let z = x.u16(TURN_Z);
                x.set16(0xc, z);
                set_state_long(x, 0x0202_0000);
                x.set8(RESUME, 6);
            }
        }
        6 => {
            set_state_long(x, 0x0206_0000);
            exit_flight(x);
        }
        _ => {}
    }
}

/// +5 = 4 (0x75b2e): wait then back to flight.
fn pause<R: Bench>(x: &mut Cx<'_, R>) {
    step(x);
    let n = x.u8(0x77).wrapping_sub(1);
    x.set8(0x77, n);
    if (n as i8) < 0 {
        x.set8(f::SUB, 0);
    }
}

/// +5 = 6 (0x75884): screen exit depending on the variant.
fn exit_flight<R: Bench>(x: &mut Cx<'_, R>) {
    move_xz_table(x);
    step(x);
    match x.u8(Obj::VARIANT) >> 1 {
        0 => {
            bank(x, 0x14, true);
            if x.u16(0xc) > x.u16(EXIT_LIMIT) {
                leave(x);
            }
        }
        1..=3 => {
            let mut left = var::SCROLL_X.get(x.b);
            if x.u8(PARAM) == 4 {
                left = var::CAMERA_ALT_X.get(x.b);
            }
            let limit = x.u16(EXIT_LIMIT).wrapping_add(left) as i16;
            let here = x.u16(0x8).wrapping_add(left) as i16;
            let gone = if x.u8(Obj::VARIANT) & 1 == 0 { limit <= here } else { limit >= here };
            if gone {
                leave(x);
            }
        }
        _ => {
            let top = var::SCROLL_Y.get(x.b).wrapping_add(0x100);
            if top <= x.u16(0xc) {
                leave(x);
            }
        }
    }
}

/// +5 = 8 (0x75b40): end of the flight of variant 8.
fn attack_end<R: Bench>(x: &mut Cx<'_, R>) {
    match x.u8(f::SUB2) {
        0 => attack_start(x),
        2 => attack_aim(x),
        4 => attack_wait(x),
        _ => {}
    }
}

// --- substate 2: attack ----------------------------------------------------------------------

fn attack<R: Bench>(x: &mut Cx<'_, R>) {
    match x.u8(SERIES) {
        0 => attack_shots(x),
        2 => attack_claws(x),
        4 => attack_fire(x),
        _ => {}
    }
}

fn attack_shots<R: Bench>(x: &mut Cx<'_, R>) {
    match x.u8(f::SUB2) {
        0 => attack_start(x),
        2 => attack_aim(x),
        4 => attack_wait(x),
        6 => attack_return(x),
        _ => {}
    }
}

/// 0x75b7c: the creature aims at a player and throws a projectile.
fn attack_start<R: Bench>(x: &mut Cx<'_, R>) {
    x.add8(f::SUB2, 2);
    if let Some(target) = lib::pick_target(x) {
        // 0xa0566 with D5 = 0 (left as a leftover) and A2 = the target
        *x.c.d5() = 0;
        if let Some(shot) = super::small::harpy_shot_via(&mut Pure, x.b, x.o, 0, target.0) {
            x.set32(SHOT, shot.0);
            let n = x.u8(KIND) as u32;
            x.anim_start(SET_ATTACK, n as u16);
            return lib::sound(x, crate::sound::CMD_36C);
        }
    }
    set_state_long(x, 0x0204_0000);
    x.set8(0x77, 0x1e);
    let n = x.u8(KIND) as u32;
    x.anim_start(SET_FLY, n as u16);
}

/// 0x75bd0: waits for the end of the firing gesture.
fn attack_aim<R: Bench>(x: &mut Cx<'_, R>) {
    step(x);
    if x.u8(SHOT_DONE) != 0 {
        x.add8(f::SUB2, 2);
        let n = 2 + x.u8(KIND) as u32;
        x.anim_start(SET_ATTACK, n as u16);
    }
}

/// 0x75bf4: releases the projectile at the key frame.
fn attack_wait<R: Bench>(x: &mut Cx<'_, R>) {
    if x.anim_step() {
        return x.add8(f::SUB2, 2);
    }
    if x.u8(f::KEY_FRAME) != 0 {
        x.set8(f::KEY_FRAME, 0);
        let shot = Obj(x.u32(SHOT) & 0xff_ffff);
        shot.set_u8(x.b, 0x55, 1);
        x.set8(TIMER_A, 0x1e);
        lib::sound(x, crate::sound::CMD_36D);
    }
}

/// 0x75c36: back to flight after the shot.
fn attack_return<R: Bench>(x: &mut Cx<'_, R>) {
    let t = x.u8(TIMER_A).wrapping_sub(1);
    x.set8(TIMER_A, t);
    if (t as i8) >= 0 {
        return;
    }
    set_state_long(x, 0x0202_0000);
    if (x.random() as u32) & 1 != 0 {
        let n = x.u8(0xa4).wrapping_sub(1);
        x.set8(0xa4, n);
        if (n as i8) >= 0 {
            return;
        }
        x.set8(0xa4, 2);
    }
    set_state_long(x, 0x0200_0000);
    let resume = x.u8(RESUME);
    x.set8(f::SUB2, resume);
}

fn attack_claws<R: Bench>(x: &mut Cx<'_, R>) {
    match x.u8(f::SUB2) {
        0 => {
            x.add8(f::SUB2, 2);
            let n = 4 + x.u8(KIND) as u32;
            x.anim_start(SET_ATTACK, n as u16);
        }
        2 => {
            if x.anim_step() {
                x.add8(f::SUB2, 2);
                let n = 6 + x.u8(KIND) as u32;
                x.anim_start(SET_ATTACK, n as u16);
            }
        }
        4 => {
            if x.anim_step() {
                x.set8(TIMER_A, 0x1e);
                x.add8(f::SUB2, 2);
                let n = 2 + x.u8(KIND) as u32;
                x.anim_start(SET_FLY, n as u16);
            } else if x.u8(f::KEY_FRAME) != 0 {
                x.set8(f::KEY_FRAME, 0);
                if pick_target(x) {
                    if let (Some(shot), false) = lib::eshl_12(x) {
                        x.set32(SHOT, shot.0);
                    }
                }
            }
        }
        _ => attack_return(x),
    }
}

fn attack_fire<R: Bench>(x: &mut Cx<'_, R>) {
    match x.u8(f::SUB2) {
        0 => {
            x.add8(f::SUB2, 2);
            x.anim_start(SET_ATTACK, 9);
        }
        2 => {
            if x.anim_step() {
                x.add8(f::SUB2, 2);
                x.set8(TIMER_A, 0x30);
                x.anim_start(SET_FLY, 4);
            } else if x.u8(f::KEY_FRAME) != 0 {
                x.set8(f::KEY_FRAME, 0);
                if let Some(target) = lib::pick_target(x) {
                    if let Some(shot) = lib::eshl_13(x, target.0) {
                        x.set32(SHOT, shot.0);
                    }
                }
            }
        }
        _ => attack_return(x),
    }
}

// --- state 4: hit ----------------------------------------------------------------------------

fn hit<R: Bench>(x: &mut Cx<'_, R>) {
    let freeze = x.u8(f::FREEZE) as i8;
    if freeze > 0 {
        x.set8(f::FREEZE, (freeze - 1) as u8);
    } else {
        match x.u8(f::SUB) {
            0 => hit_recoil(x),
            2 => hit_fall(x),
            4 => hit_dying(x),
            _ => {}
        }
    }
    x.reinsert_foreground();
}

/// Fall animation of the attack series (`+0x2f`).
fn hit_anim<R: Bench>(x: &mut Cx<'_, R>) -> u8 {
    let series = x.u8(SERIES) as u32;
    x.rom8(HIT_ANIM + series)
}

fn hit_recoil<R: Bench>(x: &mut Cx<'_, R>) {
    match x.u8(f::SUB2) {
        0 => {
            x.add8(f::SUB2, 2);
            let c = x.linked(COMPANION);
            c.set_u8(x.b, 0x5a, 1);
            x.set8(f::FREEZE, 1);
            x.anim_start(SET_HIT, 0xf);
        }
        2 => {
            if x.anim_step() {
                x.add8(f::SUB2, 2);
                x.set16(0x18, 0x400);
                x.set16(0x1a, (-0x40i16) as u16);
                let vx = if x.u8(f::FLIP) != 0 { -0x200i16 } else { 0x200 };
                x.set16(0x14, vx as u16);
                x.set16(0x16, 0);
                x.anim_start(SET_HIT, 0x10);
            }
        }
        4 => {
            step(x);
            x.move_xz();
            let z = x.u16(0xc).wrapping_add(x.u16(0x10));
            let mut limit = var::SCROLL_Y.get(x.b);
            if var::CAMERA_SELECT.get(x.b) != 0 {
                limit = var::CAMERA_ALT_Y.get(x.b);
            }
            if (z as i16) <= (limit as i16) {
                x.add8(f::STATE, 2);
            }
        }
        _ => {}
    }
}

fn hit_fall<R: Bench>(x: &mut Cx<'_, R>) {
    let c = x.linked(COMPANION);
    c.set_u8(x.b, 0x5a, 1);
    let kind = x.u8(KIND) != 0;
    if kind && x.u8(f::SUB2) != 0 {
        if x.u8(1) & 2 == 0 {
            x.add8(f::STATE, 2);
        }
        x.move_z();
        return step(x);
    }
    match x.u8(f::SUB2) {
        0 => {
            x.add8(f::SUB2, 2);
            x.set32(0x58, 0);
            let base = hit_anim(x) as u32;
            x.set8(TIMER_A, 0xf);
            x.set8(RESUME, base as u8 + 2);
            x.set16(0x18, (-0x300i16) as u16);
            let n = base.wrapping_add(x.u8(KIND) as u32) & 0xff;
            x.anim_start(SET_HIT, n as u16);
        }
        2 => {
            if (x.u16(0xc) as i16) >= 0 {
                x.move_z();
                return step(x);
            }
            x.add8(f::SUB2, 2);
            x.set32(0xc, 0);
            x.set8(TIMER_A, 0x10);
            let n = x.u8(RESUME) as i8 as i16 as u16;
            x.anim_start(SET_HIT, (n as u32) as u16);
        }
        _ => {
            let t = x.u8(TIMER_A).wrapping_sub(1);
            x.set8(TIMER_A, t);
            if (t as i8) < 0 {
                x.add8(f::STATE, 2);
            }
        }
    }
}

fn hit_dying<R: Bench>(x: &mut Cx<'_, R>) {
    if x.u8(f::SUB2) != 0 {
        if x.anim_step() {
            set_state_long(x, 0x0200_0000);
        }
        return;
    }
    x.add8(f::SUB2, 2);
    if (x.u8(f::HP) as i8) < 0 {
        set_state_long(x, 0x0400_0000);
    }
    x.set32(0x58, 0);
    let base = hit_anim(x).wrapping_add(x.u8(KIND)).wrapping_sub(2);
    x.anim_start(SET_HIT, (base as u32) as u16);
}

// --- state 6: release ---------------------------------------------------------------------------

fn dead<R: Bench>(x: &mut Cx<'_, R>) {
    detach_from_host(x);
    free(x);
}

handlers! {
    0x756c4 "enemy_21" run,
}

//! Boss 8, group 4: the attacks (`+6` = 0 to 18), each in steps `+7` = 0, 2, 4...
//! At the end of an attack, `aids::after_attack` chooses what follows.

use crate::families::cx::Bench;
use super::aids;
use super::*;
use crate::objects::Current;

/// Animation key `kind` (`+0xa2`, copied to `+0xa3`).
fn key<R: Bench>(x: &mut Cx<'_, R>, kind: u8) {
    x.set8(ANIM_KIND, kind);
    x.set8(ANIM_LAST, kind);
}

/// End of a countdown step: has the countdown of `+0xa0` elapsed?
fn timer_done<R: Bench>(x: &mut Cx<'_, R>) -> bool {
    sub16(x, TIMER, 1).mi()
}

/// Common final step of the attacks (0x8880a): plays the animation to the end, then chooses what follows.
fn finish<R: Bench>(x: &mut Cx<'_, R>) {
    if x.anim_step() {
        aids::after_attack(x);
    }
}

pub(super) fn run<R: Bench>(x: &mut Cx<'_, R>) {
    match dispatch(x, 0x887ba, f::SUB2) {
        0x887ce => slash(x),
        0x88e0e => lunge(x, true),
        0x888ac => throw_knives(x),
        0x8881a => combo(x),
        0x88a64 => charge(x),
        0x88b92 => leap(x),
        0x88cae => rain(x),
        0x88d86 => vanish(x),
        _ => lunge(x, false),
    }
}

fn step<R: Bench>(x: &mut Cx<'_, R>, table: u32) -> u32 {
    dispatch(x, table, f::SUB3)
}

/// 0x887ce (`+6` = 0): a single hit.
fn slash<R: Bench>(x: &mut Cx<'_, R>) {
    if step(x, 0x887dc) == 0x887e0 {
        addq8(x, f::SUB3, 2);
        x.set8(0x45, 5);
        x.set8(0x44, 0x44);
        key(x, 0xa);
        strike(x, 0);
    } else {
        finish(x)
    }
}

/// 0x8881a (`+6` = 8): three-beat hit (the boss only strikes at the key frames `+0x33`).
fn combo<R: Bench>(x: &mut Cx<'_, R>) {
    match step(x, 0x88828) {
        0x88830 => {
            addq8(x, f::SUB3, 2);
            key(x, 0xb);
            x.set8(PHASE_TIMER, 0x1e);
            strike(x, 1);
        }
        0x88854 => {
            if x.u8(f::KEY_FRAME) != 0 {
                addq8(x, f::SUB3, 2);
                x.set8(f::KEY_FRAME, 0);
                lib::sound(x, 0xe42c6);
            }
            x.anim_step();
        }
        0x88870 => {
            if x.u8(f::KEY_FRAME) != 0 {
                x.set8(VULNERABLE, 1);
                addq8(x, f::SUB3, 2);
                x.set8(f::KEY_FRAME, 0);
                crate::families::eshl_creators::series_3b(&mut crate::families::effects_creators::leftovers::Pure, x.b, x.o);
            }
            x.anim_step();
        }
        _ => {
            if x.u8(HIT_ACK) != 0 {
                x.set8(VULNERABLE, 0);
                aids::after_attack(x);
                x.set8(HIT_ACK, 0);
            }
            x.anim_step();
        }
    }
}

/// Launches a projectile (0xb4f44, setting 5) toward a player chosen by `pick`.
fn shoot<'a, R: Bench>(x: &mut Cx<'a, R>, pick: fn(&mut Cx<'a, R>)) {
    pick(x);
    crate::families::effects_creators::spell(x.b, x.o, 5);
}

/// 0x888ac (`+6` = 4, 6) and 0x88cae (`+6` = 14): volley of projectiles, each toward a player.
fn volley_start<R: Bench>(x: &mut Cx<'_, R>, kind: u8, extra: bool) {
    addq8(x, f::SUB3, 2);
    if !extra {
        x.set8(PHASE_TIMER, 0xa);
        x.set8(0x45, 5);
        x.set8(0x44, 0x45);
    } else {
        x.set8(0x44, 0x45);
    }
    key(x, kind);
    x.set8(PICKED, 0);
    if extra {
        x.set8(GRABBED, 2);
    }
    strike(x, 3);
}

/// First shot of a volley: sounds, effect, then one shot per living player.
fn volley_fire<'a, R: Bench>(x: &mut Cx<'a, R>, pick: fn(&mut Cx<'a, R>)) {
    lib::shake_58(x, 0x30);
    lib::sound(x, 0xedd2c);
    addq8(x, f::SUB3, 2);
    x.set16(TIMER, 0x32);
    let n = var::PLAYERS_ALIVE.get(x.b);
    x.set8(VOLLEY, n);
    sub8(x, VOLLEY, 1);
    shoot(x, pick);
    strike(x, 4);
}

/// One more shot as long as there are players left to aim at.
fn volley_next<'a, R: Bench>(x: &mut Cx<'a, R>, pick: fn(&mut Cx<'a, R>)) {
    let f = sub8(x, VOLLEY, 1);
    if !f.eq() && !f.mi() {
        shoot(x, pick);
    }
}

fn throw_knives<R: Bench>(x: &mut Cx<'_, R>) {
    match step(x, 0x888ba) {
        0x888c2 => volley_start(x, 0xc, false),
        0x888f6 => {
            if x.anim_step() {
                volley_fire(x, aids::pick_new);
            }
        }
        0x8893e => {
            volley_next(x, aids::pick_new);
            if timer_done(x) {
                addq8(x, f::SUB3, 2);
                strike(x, 5);
            }
        }
        _ => finish(x),
    }
}

/// 0x88cae (`+6` = 14): rain of projectiles, repeated until `+0xaa` is exhausted.
fn rain<R: Bench>(x: &mut Cx<'_, R>) {
    match step(x, 0x88cbc) {
        0x88cc4 => volley_start(x, 0x11, true),
        0x88cf2 => {
            if x.anim_step() {
                volley_fire(x, aids::pick_any);
            }
        }
        0x88d3a => {
            volley_next(x, aids::pick_any);
            if !timer_done(x) {
                return;
            }
            if (sub8(x, GRABBED, 1)).mi() {
                addq8(x, f::SUB3, 2);
                strike(x, 5);
            } else {
                sub8(x, f::SUB3, 2);
                strike(x, 0xb);
            }
        }
        _ => finish(x),
    }
}

/// 0x88a64 (`+6` = 10): lateral charge with puffs of dust, then stop.
fn charge<R: Bench>(x: &mut Cx<'_, R>) {
    match step(x, 0x88a72) {
        0x88a7c => {
            addq8(x, f::SUB3, 2);
            x.set8(PHASE_TIMER, 0xa);
            x.set8(0x45, 5);
            x.set8(0x44, 0x46);
            key(x, 0xf);
            strike(x, 7);
        }
        0x88aac => {
            if x.u8(f::KEY_FRAME) == 0 {
                x.anim_step();
                return;
            }
            addq8(x, f::SUB3, 2);
            x.set8(f::KEY_FRAME, 0);
            lib::sound(x, 0xecdba);
            let (mut d0, mut d1) = (-0x800i16, 0x20i16);
            if x.u8(f::FLIP) != 0 {
                d0 = d0.wrapping_neg();
                d1 = d1.wrapping_neg();
            }
            x.set16(0x14, d0 as u16);
            x.set16(0x16, d1 as u16);
        }
        0x88ae6 => {
            dust(x);
            x.move_x();
            if x.anim_step() {
                addq8(x, f::SUB3, 2);
                x.set16(TIMER, 0x14);
                strike(x, 8);
            }
        }
        0x88b2c => {
            dust(x);
            if sub16(x, TIMER, 1).mi() {
                addq8(x, f::SUB3, 2);
                x.set16(TIMER, 1);
                let v = x.u16(0x14).wrapping_neg();
                x.set16(0x14, v);
            }
            x.move_x();
            x.anim_step();
        }
        _ => {
            if sub16(x, TIMER, 1).mi() {
                x.set16(TIMER, 0);
                x.set16(0x14, 0);
                x.set16(0x16, 0);
                x.set8(0, 1);
                return aids::after_attack(x);
            }
            x.move_x();
            x.anim_step();
        }
    }
}

/// Every 4 frames, a puff of dust under the feet (`andi.w #3,(0x6e,A5) ; beq`).
fn dust<R: Bench>(x: &mut Cx<'_, R>) {
    let v = x.a5_u16(0x6e) & 3;
    x.set_a5_u16(0x6e, v);
    if v == 0 {
        lib::shock(x, 1, 1, 0xffe0, 0xffff);
    }
}

/// 0x88b92 (`+6` = 12): leap with fall, landing and burst of effects.
fn leap<R: Bench>(x: &mut Cx<'_, R>) {
    match dispatch(x, 0x88ba0, f::SUB3) {
        0x88bac => leap_start(x, -0x400, 0x10, 9),
        t => leap_rest(x, t),
    }
}

/// Start of the leap: horizontal momentum `vx` (reversed if the boss faces left), animation key `kind`.
pub(super) fn leap_start<R: Bench>(x: &mut Cx<'_, R>, vx: i16, kind: u8, anim: u32) {
    addq8(x, f::SUB3, 2);
    x.set8(VULNERABLE, 1);
    key(x, kind);
    x.set8(0x44, 0x45);
    x.set16(0x14, vx as u16);
    if x.u8(f::FLIP) != 0 {
        let v = x.u16(0x14).wrapping_neg();
        x.set16(0x14, v);
    }
    x.set16(0x16, 0);
    x.set16(0x18, 0x900);
    x.set16(0x1a, (-0x88i16) as u16);
    strike(x, anim);
}

/// Common steps of the leap (0x88bf8, 0x88c0e, 0x88c34, 0x88c7e, 0x88c9a).
pub(super) fn leap_rest<R: Bench>(x: &mut Cx<'_, R>, t: u32) {
    match t {
        0x88bf8 => {
            if x.u8(f::KEY_FRAME) != 0 {
                addq8(x, f::SUB3, 2);
                x.set8(VULNERABLE, 0);
            }
            x.anim_step();
        }
        0x88c0e => {
            x.move_xz();
            if !Fl::sub16(x.u16(0x18), 0xfd00).gt() || true {
                // cmpi.w #-0x300,(0x18,A0) ; bgt: the vertical momentum shrinks
            }
            if (x.u16(0x18) as i16) <= -0x300 {
                x.set16(0x1a, (-0x98i16) as u16);
                addq8(x, f::SUB3, 2);
                x.set16(0x30, 1);
            }
            x.anim_step();
        }
        0x88c34 => {
            if x.move_xz().gt() {
                x.anim_step();
                return;
            }
            lib::shake_58(x, 0x14);
            lib::sound(x, 0xecb98);
            aids::burst_small(x);
            x.set32(0xc, 0);
            addq8(x, f::SUB3, 2);
            x.set8(VULNERABLE, 1);
            x.set16(0x30, 1);
            x.set16(TIMER, 0x14);
            strike(x, 0xa);
        }
        0x88c7e => {
            if timer_done(x) {
                addq8(x, f::SUB3, 2);
                strike(x, 5);
            }
        }
        _ => {
            if x.anim_step() {
                x.set16(TIMER, 0);
                aids::after_attack(x);
            }
        }
    }
}

/// 0x88d86 (`+6` = 16): disappearance, reappearance beside.
fn vanish<R: Bench>(x: &mut Cx<'_, R>) {
    match step(x, 0x88d94) {
        0x88d9c => {
            addq8(x, f::SUB3, 2);
            x.set8(PHASE_TIMER, 0x10);
            key(x, 0x12);
            lib::sound(x, 0xecf7a);
            body(x, 4);
        }
        0x88dc6 => {
            if x.anim_step() {
                addq8(x, f::SUB3, 2);
                aids::random_sidestep(x);
                x.set_bit(0x1, 1, true);
                x.set16(TIMER, 8);
                body(x, 5);
            }
        }
        0x88df4 => {
            if timer_done(x) {
                addq8(x, f::SUB3, 2);
                x.set_bit(0x1, 1, false);
                lib::sound(x, 0xecf7a);
            }
        }
        _ => finish(x),
    }
}

/// 0x88e0e (`+6` = 2, `first`) and 0x88e50 (`+6` = 18): forward leap then hit (0xa4e84 receives D4).
fn lunge<R: Bench>(x: &mut Cx<'_, R>, first: bool) {
    let t = step(x, if first { 0x88e1c } else { 0x88e5e });
    match t {
        0x88e66 => {
            addq8(x, f::SUB3, 2);
            x.set8(PHASE_TIMER, 0xa);
            key(x, 0x13);
            strike(x, 0xc);
        }
        0x88e24 | 0x88e8a => {
            if x.anim_step() {
                addq8(x, f::SUB3, 2);
                x.set16(TIMER, 0x20);
                // D4 = `first` (leftover re-read by the following handlers, `dispatch::Leftovers`)
                *x.c.d4() = first as u32;
                crate::families::eshl_creators::series_36(&mut crate::families::effects_creators::leftovers::Pure, x.b, x.o, first as u8);
                strike(x, 0xd);
            }
        }
        0x88eb6 => {
            if timer_done(x) {
                addq8(x, f::SUB3, 2);
                strike(x, 0xe);
            } else {
                x.anim_step();
            }
        }
        _ => finish(x),
    }
}

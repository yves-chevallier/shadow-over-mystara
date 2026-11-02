//! Boss 21, group `+5` = 4: the attacks (`+6` = 0 to 16), each in steps `+7`.
//!
//! | `+6` | Attack |
//! |---|---|
//! | 0 | simple hit (animation 0 of the hits set) |
//! | 2 | hit with effect 0x32 at the key frame |
//! | 4 | forward leap, effect 0x32 (variant 3), push of 0x900 |
//! | 6 | jump onto the target with landing, effect 0x32 (variant 2) |
//! | 8, 10 | ESHL 0x20 shot (beam), `+0xb2` counts the remaining ones |
//! | 12 | ESHL 0x21 shot in cutscene (`A5+0x51e` = 4), `+0xb3` counts the remaining ones |
//! | 14 | dropping objects around the camera (`+0xb4`) |
//! | 16 | whirlwind: entry into slow motion |

use crate::families::cx::Bench;
use super::decide;
use super::*;
use crate::objects::pools;
use crate::objects::Current;

pub(super) fn run<R: Bench>(x: &mut Cx<'_, R>) {
    match x.u8(f::SUB2) {
        0 => strike_0(x),
        2 => strike_2(x),
        4 => strike_4(x),
        6 => leap(x),
        8 | 10 => beam(x),
        12 => cinematic_shot(x),
        14 => summon(x),
        16 => whirl(x),
        _ => {}
    }
}

/// Animates attack `n` of the hits set and sets the current set (`+0xa0` and `+0xa1`).
fn strike_anim<R: Bench>(x: &mut Cx<'_, R>, kind: u8, n: u32) {
    x.set8(ANIM_KIND, kind);
    x.set8(ANIM_LAST, kind);
    strike(x, n);
}

/// End of an attack: return to combat (the dead host and the absence of players take precedence).
fn finish<R: Bench>(x: &mut Cx<'_, R>, to: fn(&mut Cx<'_, R>)) {
    decide::after_attack(x, to);
}

/// Consumes the key frame `+0x33`; true if it was raised.
fn take_key<R: Bench>(x: &mut Cx<'_, R>) -> bool {
    if x.u8(f::KEY_FRAME) == 0 {
        return false;
    }
    x.set8(f::KEY_FRAME, 0);
    true
}

/// An effect 0x32 linked to the boss with variant `v` (dust, sparks...), without insisting.
fn puff<R: Bench>(x: &mut Cx<'_, R>, v: u8) {
    effect(x, false, 0x32, Some(v), true);
}

// --- +6 = 0 : coup simple -----------------------------------------------------------------------

fn strike_0<R: Bench>(x: &mut Cx<'_, R>) {
    match x.u8(0x7) {
        0 => {
            x.add8(0x7, 2);
            strike_anim(x, 0x10, 0);
        }
        2 => {
            if lib::step(x) {
                return finish(x, decide::chase);
            }
            if take_key(x) {
                lib::sound(x, 0xe580a);
            }
        }
        _ => {}
    }
}

// --- +6 = 2: hit with effect ------------------------------------------------------------------

fn strike_2<R: Bench>(x: &mut Cx<'_, R>) {
    match x.u8(0x7) {
        0 => {
            x.add8(0x7, 2);
            strike_anim(x, 0x11, 1);
        }
        2 => {
            if lib::step(x) {
                return finish(x, decide::chase);
            }
            if take_key(x) {
                puff(x, 0);
                lib::sound(x, 0xe580a);
            }
        }
        _ => {}
    }
}

// --- +6 = 4: forward leap --------------------------------------------------------------------

fn strike_4<R: Bench>(x: &mut Cx<'_, R>) {
    match x.u8(0x7) {
        0 => {
            x.add8(0x7, 2);
            strike_anim(x, 0x12, 2);
        }
        2 => {
            lib::step(x);
            if (x.u8(f::KEY_FRAME) as i8) >= 0 {
                return;
            }
            x.add8(0x7, 2);
            x.set8(f::KEY_FRAME, 0);
            x.set16(TIMER, 0x1e);
            puff(x, 3);
            let (mut vx, mut vz) = (0x900u16 as i16, -0x4ci16);
            if x.u8(f::FLIP) == 0 {
                vx = -vx;
                vz = -vz;
            }
            x.set16(0x14, vx as u16);
            x.set16(0x16, vz as u16);
            lib::sound(x, 0xecdba);
            x.move_x();
        }
        4 => {
            if !sub16(x, TIMER, 1).gt() {
                x.add8(0x7, 2);
                x.set16(TIMER, 8);
                x.set8(INVULNERABLE, 0);
                return;
            }
            x.move_x();
            lib::step(x);
            if take_key(x) {
                puff(x, 1);
                lib::sound(x, 0xe587e);
            }
        }
        6 => {
            if sub16(x, TIMER, 1).gt() {
                return;
            }
            x.add8(0x7, 2);
            strike(x, 3);
        }
        8 => {
            if lib::step(x) {
                finish(x, decide::chase);
            }
        }
        _ => {}
    }
}

// --- +6 = 6: jump onto the target ------------------------------------------------------------------

/// Distance covered in X to reach the targeted player (`+0x50`) at the current speed, converted into the vertical
/// jump speed: 0x100 + 0xe per step of required speed.
fn leap_speed<R: Bench>(x: &mut Cx<'_, R>) -> u16 {
    let target = x.linked(f::TARGET);
    let mut gap = x.u32(0x8).wrapping_sub(target.u32(x.b, 0x8)) as i32;
    if gap < 0 {
        gap = gap.wrapping_neg();
    }
    let mut per_step = ((x.u16(0x14) as u32) << 16) as i32 >> 8;
    if per_step < 0 {
        per_step = per_step.wrapping_neg();
    }
    let mut v = 0x100u16;
    // the original loops while the remainder is positive; a zero speed would never finish
    for _ in 0..0x10000 {
        gap = gap.wrapping_sub(per_step);
        if gap <= 0 {
            break;
        }
        v = v.wrapping_add(0xe);
    }
    v
}

fn leap<R: Bench>(x: &mut Cx<'_, R>) {
    match x.u8(0x7) {
        0 => {
            x.add8(0x7, 2);
            x.set16(0x16, 0);
            let angle = if x.u8(f::FLIP) != 0 { 8 } else { 0x18 };
            x.set8(Obj::SPEED_INDEX, angle);
            speed(x, 0x17);
            let v = leap_speed(x);
            x.set16(0x18, v);
            x.set16(0x1a, (-0x20i16) as u16);
            x.set8(ANIM_KIND, 0x13);
            x.set8(ANIM_LAST, 0x13);
            body(x, 5);
        }
        2 => {
            // 0x99e52: `jsr 0x1a56`, one animation step without copying the hit kind (`+0x44`)
            x.anim_step();
            if x.u8(f::KEY_FRAME) != 0 {
                return;
            }
            x.move_xz();
            if x.u16(0x18) as i16 <= 0x120 {
                x.add8(0x7, 2);
                strike(x, 4);
            }
        }
        4 => {
            x.move_xz();
            if cmp16(x, 0x18, 0xff60).le() {
                x.add8(0x7, 2);
                x.set16(0x1a, (-0x30i16) as u16);
                strike(x, 5);
            } else {
                lib::step(x);
            }
        }
        6 => {
            x.move_xz();
            if tst16(x, 0xc).le() {
                x.add8(0x7, 2);
                x.set16(0xc, 0);
                body(x, 0xa);
                return;
            }
            lib::step(x);
            if take_key(x) {
                puff(x, 2);
                lib::sound(x, 0xe587e);
            }
        }
        8 => {
            if lib::step(x) {
                finish(x, decide::chase);
            }
        }
        _ => {}
    }
}

// --- +6 = 8 and 10: beam ------------------------------------------------------------------------

fn beam<R: Bench>(x: &mut Cx<'_, R>) {
    match x.u8(0x7) {
        0 => {
            x.add8(0x7, 2);
            set_abs8(x, 0xff8043, 0x7f);
            x.set8(INVULNERABLE, 1);
            sub8(x, BUDGET_A, 1);
            let target = x.linked(f::TARGET);
            let (tx, ty) = (target.u16(x.b, 0x8), target.u16(x.b, 0x10));
            x.set16(MARK_X, tx);
            x.set16(MARK_Y, ty);
            strike_anim(x, 0x15, 7);
        }
        2 => {
            lib::step(x);
            if x.u8(f::KEY_FRAME) == 0 {
                return;
            }
            x.add8(0x7, 2);
            x.set16(TIMER, 0x14);
            // no room: jump to the last step
            let Some(shot) = x.alloc(&pools::ESHL) else { return beam_end_anim(x) };
            x.set16(CHILD, shot.0 as u16);
            shot.set_u8(x.b, 0, 1);
            shot.set_u8(x.b, 2, 0x20);
            shot.set_u8(x.b, 3, 1);
            shot.set_u16(x.b, 0x42, x.o.0 as u16);
            let side = x.u8(f::FLIP);
            shot.set_u8(x.b, f::FLIP, side);
            for off in [0x8u32, 0xc, 0x10] {
                let v = x.u16(off);
                shot.set_u16(x.b, off, v);
            }
            let dx: i16 = if side == 0 { -0x44 } else { 0x44 };
            let px = shot.u16(x.b, 0x8).wrapping_add(dx as u16);
            shot.set_u16(x.b, 0x8, px);
            shot.set_u16(x.b, 0xc, 0x30);
            lib::step(x);
        }
        4 => {
            if !sub16(x, TIMER, 1).gt() {
                x.add8(0x7, 2);
                x.set8(INVULNERABLE, 0);
                x.set16(TIMER, 0xa);
                let shot = x.linked(CHILD);
                shot.set_u8(x.b, 0x58, 1);
                lib::sound(x, 0xe59da);
            }
            lib::step(x);
        }
        6 => {
            let shot = x.linked(CHILD);
            if shot.u8(x.b, 0x5a) != 0 {
                return beam_end_anim(x);
            }
            lib::step(x);
        }
        8 => {
            if lib::step(x) {
                set_abs8(x, 0xff8043, 0xff);
                finish(x, decide::chase);
            }
        }
        _ => {}
    }
}

fn beam_end_anim<R: Bench>(x: &mut Cx<'_, R>) {
    x.add8(0x7, 2);
    strike(x, 8);
}

// --- +6 = 12: shot in cutscene ---------------------------------------------------------------

fn cinematic_shot<R: Bench>(x: &mut Cx<'_, R>) {
    match x.u8(0x7) {
        0 => {
            x.add8(0x7, 2);
            x.set_a5_u16(0x51e, 4);
            set_abs8(x, 0xff8043, 0x7f);
            x.set8(INVULNERABLE, 1);
            sub8(x, BUDGET_B, 1);
            strike_anim(x, 0x16, 9);
        }
        2 => {
            x.set_a5_u16(0x51e, 4);
            lib::step(x);
            if !take_key_keep(x) {
                return;
            }
            x.add8(0x7, 2);
            x.set8(INVULNERABLE, 0);
            x.set16(TIMER, 0xa);
            x.set8(f::KEY_FRAME, 0);
            let Some(shot) = x.alloc(&pools::ESHL) else { return cinematic_end(x) };
            x.set16(CHILD, shot.0 as u16);
            shot.set_u8(x.b, 0, 1);
            shot.set_u8(x.b, 2, 0x21);
            shot.set_u16(x.b, 0x42, x.o.0 as u16);
            let side = x.u8(f::FLIP);
            shot.set_u8(x.b, f::FLIP, side ^ 0x20);
            for off in [0x8u32, 0xc, 0x10] {
                let v = x.u16(off);
                shot.set_u16(x.b, off, v);
            }
            let dx: i16 = if side == 0 { -0x3b } else { 0x3b };
            let px = shot.u16(x.b, 0x8).wrapping_add(dx as u16);
            shot.set_u16(x.b, 0x8, px);
            let pz = shot.u16(x.b, 0xc).wrapping_add(0x30);
            shot.set_u16(x.b, 0xc, pz);
        }
        4 => {
            x.set_a5_u16(0x51e, 4);
            if sub16(x, TIMER, 1).gt() {
                return step_only(x);
            }
            x.add8(0x7, 2);
            x.set16(TIMER, 0x5a);
            let shot = x.linked(CHILD);
            shot.set_u8(x.b, 0x58, 1);
            lib::sound(x, 0xe5a4e);
            step_only(x);
        }
        6 => {
            x.set_a5_u16(0x51e, 4);
            if sub16(x, TIMER, 1).gt() {
                return step_only(x);
            }
            x.add8(0x7, 2);
            let shot = x.linked(CHILD);
            shot.set_u8(x.b, 0x59, 1);
            step_only(x);
        }
        8 => {
            let shot = x.linked(CHILD);
            if shot.u8(x.b, 0x5a) != 0 {
                return cinematic_end(x);
            }
            x.set_a5_u16(0x51e, 4);
            step_only(x);
        }
        10 => {
            if lib::step(x) {
                set_abs8(x, 0xff8043, 0xff);
                finish(x, decide::stand);
            }
        }
        _ => {}
    }
}

fn step_only<R: Bench>(x: &mut Cx<'_, R>) {
    lib::step(x);
}

/// Is the key raised? (without consuming it here: the original consumes it further on).
fn take_key_keep<R: Bench>(x: &mut Cx<'_, R>) -> bool {
    x.u8(f::KEY_FRAME) != 0
}

fn cinematic_end<R: Bench>(x: &mut Cx<'_, R>) {
    x.add8(0x7, 2);
    x.set_a5_u16(0x51e, 0);
    strike(x, 0xa);
}

// --- +6 = 14: dropping objects --------------------------------------------------------------------

fn summon<R: Bench>(x: &mut Cx<'_, R>) {
    match x.u8(0x7) {
        0 => {
            x.add8(0x7, 2);
            x.set8(INVULNERABLE, 1);
            sub8(x, BUDGET_C, 1);
            strike_anim(x, 0x17, 0xb);
        }
        2 => {
            lib::step(x);
            if x.u8(f::KEY_FRAME) != 0 {
                x.add8(0x7, 2);
                x.set16(TIMER, 0x1e);
                x.set8(f::KEY_FRAME, 0);
            }
        }
        4 => {
            if sub16(x, TIMER, 1).gt() {
                return step_only(x);
            }
            x.add8(0x7, 2);
            x.set8(INVULNERABLE, 0);
            decide::spawn_by_level(x);
            lib::sound(x, 0xe5ac2);
            x.set16(TIMER, 0x3c);
            step_only(x);
        }
        6 => {
            if sub16(x, TIMER, 1).gt() {
                return step_only(x);
            }
            x.add8(0x7, 2);
            strike(x, 0xc);
        }
        8 => {
            if lib::step(x) {
                finish(x, decide::stand);
            }
        }
        _ => {}
    }
}

// --- +6 = 16: whirlwind (entry into slow motion) ------------------------------------------------

fn whirl<R: Bench>(x: &mut Cx<'_, R>) {
    match x.u8(0x7) {
        0 => {
            x.add8(0x7, 2);
            x.set8(0, 2);
            strike_anim(x, 0x18, 0xd);
        }
        2 => {
            lib::step(x);
            if x.u8(f::KEY_FRAME) != 0 {
                x.add8(0x7, 2);
                x.set16(TIMER, 0xa);
                x.set8(f::KEY_FRAME, 0);
            }
        }
        4 => {
            if !sub16(x, TIMER, 1).gt() {
                x.add8(0x7, 2);
                x.set16(TIMER, 0x3c);
                if let Some(e) = x.alloc(&pools::EFFCT) {
                    e.set_u8(x.b, 0, 1);
                    e.set_u8(x.b, 2, 0x25);
                    e.set_u16(x.b, 0x42, x.o.0 as u16);
                    let (px, py) = (x.u16(0x8), x.u16(0x10));
                    e.set_u16(x.b, 0x8, px);
                    e.set_u16(x.b, 0x10, py);
                    e.set_u16(x.b, 0xc, 0xa0);
                    let mut d: i16 = 5;
                    if x.u8(f::FLIP) != 0 {
                        d = -d;
                    }
                    let nx = e.u16(x.b, 0x8).wrapping_add(d as u16);
                    e.set_u16(x.b, 0x8, nx);
                }
            }
            lib::step(x);
        }
        6 => {
            if sub16(x, TIMER, 1).gt() {
                return step_only(x);
            }
            x.add8(0x7, 2);
            strike(x, 0xe);
        }
        8 => {
            if lib::step(x) {
                x.set8(0, 1);
                x.set8(SLOW_TICK, 0);
                decide::pick_slow_time(x);
                decide::pick_slow_mode(x);
                let mode = x.u8(SLOW);
                x.set8(SLOW_COUNT, mode);
                finish(x, decide::stand);
            }
        }
        _ => {}
    }
}

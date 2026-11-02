//! Boss 4, group 4: the attacks (`+6` = 0 to 12), each in steps `+7` = 0, 2, 4...
//!
//! | `+6` | Role |
//! |---|---|
//! | 0 | cry (flight animation 0) |
//! | 2 | takeoff off screen, then dive on the target |
//! | 4 | takeoff, then crossing the screen at ground level |
//! | 6 | leap over the target |
//! | 8 | takeoff, breath in flight (ESHL 0x32 jets in a fan), descent |
//! | 10 | gesture of the first double (two signals) |
//! | 12 | breath on the ground (ESHL 0x32 variant 1 jets) |

use crate::families::cx::Bench;
use super::decide;
use super::*;
use crate::objects::pools;
use crate::objects::Current;

pub(super) fn run<R: Bench>(x: &mut Cx<'_, R>) {
    match x.u8(f::SUB2) {
        0 => roar(x),
        2 => dive(x),
        4 => swoop(x),
        6 => leap(x),
        8 => breath_in_flight(x),
        10 => double_gesture(x),
        _ => breath(x),
    }
}

/// Common end: if a player remains, the boss resumes walking.
fn back_to_walk<R: Bench>(x: &mut Cx<'_, R>) {
    if alive(x) {
        decide::walk(x)
    } else {
        decide::wait(x)
    }
}

/// 0x8232e (`+6` = 0) : cri.
fn roar<R: Bench>(x: &mut Cx<'_, R>) {
    if x.u8(0x7) == 0 {
        x.add8(0x7, 2);
        lib::sound(x, crate::sound::CMD_30C);
        kind(x, 0x10);
        return air(x, 0);
    }
    if lib::step(x) {
        back_to_walk(x);
    }
}

/// 0x823a6: start of the takeoff (flight animation 1, vertical speed 0x400).
fn take_off<R: Bench>(x: &mut Cx<'_, R>) {
    x.add8(0x7, 2);
    x.set16(0x18, 0x400);
    x.set16(0x1a, 0);
    air(x, 1);
}

/// 0x823c2: rise to above the screen, in time with the wing beats; then the boss disappears
/// (invulnerable) for 0x28 frames.
fn climb<R: Bench>(x: &mut Cx<'_, R>) {
    let top = var::CAMERA1_Y.get(x.b).wrapping_add(0xf0);
    let h = x.u16(0x10).wrapping_add(x.u16(0xc));
    setw(x, 0, top);
    if !Fl::sub16(h, top).lt() {
        x.add8(0x7, 2);
        x.set16(TIMER, 0x28);
        x.set8(INVULNERABLE, 1);
        x.set_bit(0x1, 1, true);
        return;
    }
    lib::push_players(x);
    lib::step(x);
    let k = x.u8(f::KEY_FRAME) & 0x7f;
    if (k as i8) < 3 {
        flap(x);
        x.add16(0x18, (-0x28i16) as u16);
        let v = speed_long(x, 0x18);
        x.add32(0xc, v);
        let s = x.u16(0x18);
        x.set16(0x1a, s);
    } else {
        flap(x);
        x.add16(0x1a, (-0x30i16) as u16);
        let v = speed_long(x, 0x1a);
        x.add32(0xc, v);
        x.set16(0x18, 0x400);
    }
}

/// 0x8237a (`+6` = 2): takeoff, then dive vertically onto the target.
fn dive<R: Bench>(x: &mut Cx<'_, R>) {
    match x.u8(0x7) {
        0 => {
            if !pick_target(x) {
                return decide::wait(x);
            }
            kind(x, 0x11);
            take_off(x);
        }
        2 => climb(x),
        4 => {
            if expired(x, TIMER) {
                x.add8(0x7, 2);
                x.set8(INVULNERABLE, 0);
                return air(x, 2);
            }
            // ten frames before the end, the boss reappears above the target
            if x.u16(TIMER) == 0xa {
                x.set_bit(0x1, 1, false);
                let t = load_target(x);
                let (tx, ty) = (t.u16(x.b, 0x8), t.u16(x.b, 0x10));
                x.set16(0x8, tx);
                x.set16(0x10, ty);
                let h = var::CAMERA1_Y.get(x.b).wrapping_add(0xf0).wrapping_sub(ty);
                setw(x, 0, h);
                x.set16(0xc, h);
                x.set16(0x18, 0xfb00);
                x.set16(0x1a, (-0x10i16) as u16);
            }
        }
        6 => {
            let a = x.u16(0x1a);
            setw(x, 0, a);
            x.add16(0x18, a);
            let v = speed_long(x, 0x18);
            x.add32(0xc, v);
            if (x.u16(0xc) as i16) > 0 {
                return;
            }
            x.add8(0x7, 2);
            x.set16(TIMER, 0x1e);
            x.set16(0xc, 0);
            lib::shake_58(x, 0x1e);
            lib::debris(x, 5);
            lib::sound(x, crate::sound::CMD_328_428_B);
            air(x, 3);
        }
        _ => {
            if (x.u16(TIMER) as i16) > 0 {
                sub16(x, TIMER, 1);
                return;
            }
            if !x.anim_step() {
                return;
            }
            if alive(x) {
                decide::idle(x)
            } else {
                decide::wait(x)
            }
        }
    }
}

/// 0x82546 (`+6` = 4): takeoff, then crossing the screen at ground level at the height of the target.
fn swoop<R: Bench>(x: &mut Cx<'_, R>) {
    match x.u8(0x7) {
        0 => {
            if !pick_target(x) {
                return decide::wait(x);
            }
            lib::sound(x, crate::sound::CMD_30C);
            kind(x, 0x12);
            take_off(x);
        }
        2 => climb(x),
        4 => {
            if !expired(x, TIMER) {
                return;
            }
            x.add8(0x7, 2);
            x.set8(INVULNERABLE, 0);
            x.set_bit(0x1, 1, false);
            let t = load_target(x);
            let ty = t.u16(x.b, 0x10);
            x.set16(0x10, ty);
            x.set16(0xc, 0x80);
            // entry from the edge opposite the one the boss faces
            let (dx, vx) = if x.u8(f::FLIP) != 0 { (0u16, 0x900u16) } else { (0x180, (-0x900i16) as u16) };
            let px = dx.wrapping_add(var::CAMERA1_X.get(x.b));
            x.set16(0x8, px);
            x.set16(0x14, vx);
            x.set16(0x18, 0xf700);
            x.set16(0x16, 0);
            x.set16(0x1a, 0);
            lib::sound(x, crate::sound::CMD_30C);
            air(x, 5);
        }
        6 => {
            x.move_xz();
            if (x.u16(0xc) as i16) > 0 {
                return;
            }
            x.add8(0x7, 2);
            x.set16(TIMER, 0x28);
            x.set16(DUST, 0);
            x.set16(0xc, 0);
            let mut d = (-0x38i16) as u16;
            if x.u8(f::FLIP) == 0 {
                d = d.wrapping_neg();
            }
            x.set16(0x16, d);
            landing(x, 0x1e, 6);
        }
        _ => {
            dust_tick(x);
            if !expired(x, TIMER) {
                x.move_x();
                lib::step(x);
                return;
            }
            decide::idle(x);
        }
    }
}

/// Landing: shake for `shake` frames, dust, flight animation `n`.
fn landing<R: Bench>(x: &mut Cx<'_, R>, shake: u16, n: u32) {
    lib::shake_54(x, shake);
    lib::sound(x, crate::sound::CMD_328_428);
    lib::sound(x, crate::sound::CMD_329_429);
    air(x, n);
}

/// 0x82660 (`+6` = 6): leap over the target; the vertical momentum depends on the distance to cover.
fn leap<R: Bench>(x: &mut Cx<'_, R>) {
    match x.u8(0x7) {
        0 => {
            x.add8(0x7, 2);
            x.set16(0x16, 0);
            let angle = if x.u8(f::FLIP) != 0 { 8 } else { 0x18 };
            x.set8(Obj::SPEED_INDEX, angle as u8);
            speed(x, 0x40);
            let t = load_target(x);
            let mut dist = x.u32(0x8).wrapping_sub(t.u32(x.b, 0x8));
            if (dist as i32) < 0 {
                dist = dist.wrapping_neg();
            }
            let mut v = speed_long(x, 0x14);
            if (v as i32) < 0 {
                v = v.wrapping_neg();
            }
            // 0x200, plus 0x10 per step of `v` until the distance is covered
            let mut vz = 0x200u16;
            loop {
                let f = Fl::sub32(dist, v);
                dist = dist.wrapping_sub(v);
                if f.le() {
                    break;
                }
                vz = vz.wrapping_add(0x10);
            }
            x.set16(0x18, vz);
            x.set16(0x1a, (-0x38i16) as u16);
            lib::sound(x, crate::sound::CMD_30C);
            kind(x, 0x13);
            air(x, 7);
        }
        2 => {
            lib::step(x);
            if x.u8(f::KEY_FRAME) == 0 {
                return;
            }
            x.move_xz();
            if !Fl::sub16(x.u16(0x18), 0x100).le() {
                return;
            }
            x.add8(0x7, 2);
            air(x, 8);
        }
        4 => {
            x.move_xz();
            if !Fl::sub16(x.u16(0x18), (-0x100i16) as u16).le() {
                return;
            }
            x.add8(0x7, 2);
            air(x, 9);
        }
        6 => {
            x.move_xz();
            if (x.u16(0xc) as i16) > 0 {
                return;
            }
            x.add8(0x7, 2);
            x.set16(TIMER, 0x14);
            x.set16(DUST, 0);
            x.set16(0xc, 0);
            x.set8(INVULNERABLE, 0);
            let mut d = 0x5cu16;
            if x.u8(f::FLIP) != 0 {
                d = d.wrapping_neg();
            }
            x.set16(0x16, d);
            landing(x, 0x14, 0xa);
        }
        _ => {
            dust_tick(x);
            if !expired(x, TIMER) {
                x.move_x();
                lib::step(x);
                return;
            }
            back_to_walk(x);
        }
    }
}

/// The steps in flight: one animation step, the oscillation (0x833b6), the wing wind, a beat.
fn hover<R: Bench>(x: &mut Cx<'_, R>) {
    lib::step(x);
    bob(x);
    lib::push_players(x);
    flap(x);
}

/// 0x827c6 (`+6` = 8): low-altitude takeoff, fan breath (ESHL 0x32 jets as long as the first double
/// keeps its mouth open), then descent.
fn breath_in_flight<R: Bench>(x: &mut Cx<'_, R>) {
    match x.u8(0x7) {
        0 => {
            kind(x, 0x14);
            take_off(x);
        }
        2 => {
            lib::push_players(x);
            flap(x);
            if !(Fl::sub16(x.u16(0xc), 0x28).lt() || x.u8(f::KEY_FRAME) != 4) {
                x.add8(0x7, 2);
                x.set8(INVULNERABLE, 0);
                x.set16(TIMER, 8);
                lib::step(x);
                return;
            }
            let i = (x.u8(f::KEY_FRAME) & 0x7f) as u32 * 4;
            let v = rom32(x, 0x82852 + i);
            x.add32(0xc, v);
            lib::step(x);
        }
        4 => {
            hover(x);
            if !expired(x, TIMER) {
                return;
            }
            x.add8(0x7, 2);
            x.set16(TIMER, 0xb4);
            x.set8(BREATH, 2);
            x.set8(BREATH_STEP, 0);
            cue_double(x);
        }
        6 => {
            hover(x);
            let d = double(x, DOUBLE_A);
            if d.u8(x.b, f::KEY_FRAME) != 0 {
                return;
            }
            if expired(x, TIMER) {
                x.add8(0x7, 2);
                x.set16(TIMER, 0xa);
                return cue_double(x);
            }
            if sub8(x, BREATH, 1).gt() {
                return;
            }
            x.set8(BREATH, 6);
            breath_shot(x, 0x8295c, None);
        }
        8 => {
            flap(x);
            lib::step(x);
            bob(x);
            lib::push_players(x);
            if expired(x, TIMER) {
                x.add8(0x7, 2);
            }
        }
        10 => {
            flap(x);
            let z = x.u32(0xc);
            x.set32(0xc, z.wrapping_sub(0x18000));
            if Fl::sub32(z, 0x18000).gt() {
                lib::push_players(x);
                lib::step(x);
                return;
            }
            x.add8(0x7, 2);
            x.set16(0xc, 0);
            x.set16(TIMER, 0x28);
            body(x, 0);
        }
        _ => {
            if !expired(x, TIMER) {
                lib::step(x);
                return;
            }
            back_to_walk(x);
        }
    }
}

/// One jet of the breath (ESHL 0x32, variant `variant`): its angle follows table `angles` (16 notches, flipped
/// when the boss faces left). False if the pool is full.
fn breath_shot<R: Bench>(x: &mut Cx<'_, R>, angles: u32, variant: Option<u8>) -> bool {
    let Some(s) = x.alloc(&pools::ESHL) else { return false };
    s.set_u8(x.b, 0, 1);
    s.set_u8(x.b, 2, 0x32);
    if let Some(v) = variant {
        s.set_u8(x.b, 3, v);
    }
    s.set_u16(x.b, f::CREATOR, x.o.0 as u16);
    let i = x.u8(BREATH_STEP) as u32;
    let mut a = x.rom8(angles + i);
    let side = x.u8(f::FLIP);
    s.set_u8(x.b, f::FLIP, side);
    if side != 0 {
        a = a.wrapping_add(0x10) & 0x1f;
    }
    s.set_u8(x.b, Obj::SPEED_INDEX, a);
    let n = x.u8(BREATH_STEP).wrapping_add(1) & 0xf;
    x.set8(BREATH_STEP, n);
    true
}

/// Wait of `+0xa6` frames, then next step with the timer `next` (and a signal to the first double).
fn timed<R: Bench>(x: &mut Cx<'_, R>, next: Option<u16>, cue: bool) {
    if expired(x, TIMER) {
        x.add8(0x7, 2);
        if let Some(t) = next {
            x.set16(TIMER, t);
        }
        if cue {
            cue_double(x);
        }
    }
    lib::step(x);
}

/// Last common step: wait, then the walk.
fn last<R: Bench>(x: &mut Cx<'_, R>) {
    if !expired(x, TIMER) {
        lib::step(x);
        return;
    }
    back_to_walk(x);
}

/// 0x82a0c (`+6` = 10): two signals to the first double, then wait.
fn double_gesture<R: Bench>(x: &mut Cx<'_, R>) {
    match x.u8(0x7) {
        0 => {
            x.add8(0x7, 2);
            x.set16(TIMER, 2);
            kind(x, 0x15);
            body(x, 0);
        }
        2 => timed(x, Some(0x5a), true),
        4 => timed(x, Some(0xa), true),
        6 => timed(x, Some(4), false),
        _ => last(x),
    }
}

/// 0x82ab6 (`+6` = 12): breath on the ground, ESHL 0x32 variant 1 jets every 3 frames for 0x64 frames.
fn breath<R: Bench>(x: &mut Cx<'_, R>) {
    match x.u8(0x7) {
        0 => {
            x.add8(0x7, 2);
            x.set16(TIMER, 2);
            kind(x, 0x16);
            body(x, 0);
        }
        2 => timed(x, None, true),
        4 => {
            let d = double(x, DOUBLE_A);
            if d.u8(x.b, f::KEY_FRAME) != 0 {
                x.add8(0x7, 2);
                x.set16(TIMER, 0x64);
                x.set8(BREATH, 1);
                x.set8(BREATH_STEP, 0);
            }
            lib::step(x);
        }
        6 => {
            if expired(x, TIMER) {
                x.add8(0x7, 2);
                x.set16(TIMER, 0xc);
                cue_double(x);
                lib::step(x);
                return;
            }
            if sub8(x, BREATH, 1).gt() {
                lib::step(x);
                return;
            }
            x.set8(BREATH, 3);
            if !breath_shot(x, 0x82ba2, Some(1)) {
                lib::step(x);
            }
        }
        8 => timed(x, Some(4), false),
        _ => last(x),
    }
}

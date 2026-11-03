//! Boss 7, group 4: the attacks (`+6` = 0 to 16), each in steps `+7` = 0, 2, 4...
//! At the end of an attack, `aids::after_attack` chooses what follows.

use super::aids;
use crate::families::cx::Bench;
use super::*;
use crate::objects::Current;

/// Common final step of the attacks (0x86286): plays the animation to the end, then chooses what follows.
fn finish<R: Bench>(x: &mut Cx<'_, R>) {
    if x.anim_step() {
        aids::after_attack(x);
    }
}


pub(super) fn run<R: Bench>(x: &mut Cx<'_, R>) {
    match x.u8(f::SUB2) {
        0 => slash(x),
        2 => combo(x),
        4 | 8 => throw_knives(x),
        6 => plunge(x),
        0xa => charge(x),
        0xc => leap(x),
        0xe => rain(x),
        _ => vanish(x),
    }
}

/// 0x8624a (`+6` = 0): a single hit.
fn slash<R: Bench>(x: &mut Cx<'_, R>) {
    if x.u8(f::SUB3) == 0 {
        addq8(x, f::SUB3, 2);
        x.set8(0x45, 5);
        x.set8(0x44, 0x44);
        key(x, 0xa);
        strike(x, 0);
    } else {
        finish(x)
    }
}

/// 0x86296 (`+6` = 2): three-beat hit (the boss only strikes at the key frames `+0x33`).
fn combo<R: Bench>(x: &mut Cx<'_, R>) {
    match x.u8(f::SUB3) {
        0 => {
            addq8(x, f::SUB3, 2);
            key(x, 0xb);
            x.set8(PHASE_TIMER, 0x1e);
            strike(x, 1);
        }
        2 => {
            if x.u8(f::KEY_FRAME) != 0 {
                addq8(x, f::SUB3, 2);
                x.set8(f::KEY_FRAME, 0);
                crate::families::effects_creators::spell_ring(x.b, x.o, 0);
                lib::sound(x, crate::sound::CMD_348);
            }
            x.anim_step();
        }
        4 => {
            if x.u8(f::KEY_FRAME) != 0 {
                x.set8(VULNERABLE, 0);
                addq8(x, f::SUB3, 2);
                x.set8(f::KEY_FRAME, 0);
                eshl_burst_27(x);
                lib::sound(x, crate::sound::CMD_347);
            }
            x.anim_step();
        }
        _ => {
            if x.u8(HIT_ACK) != 0 {
                x.set8(VULNERABLE, 1);
                aids::after_attack(x);
                x.set8(HIT_ACK, 0);
            }
            x.anim_step();
        }
    }
}

/// Launches a projectile (0xb4f44, setting 1) toward a player chosen by `pick`.
fn shoot<'a, R: Bench>(x: &mut Cx<'a, R>, pick: fn(&mut Cx<'a, R>)) {
    pick(x);
    crate::families::effects_creators::spell(x.b, x.o, 1);
}

/// Start of a projectile volley (key `kind`).
fn volley_start<R: Bench>(x: &mut Cx<'_, R>, kind: u8, rain: bool) {
    addq8(x, f::SUB3, 2);
    if !rain {
        x.set8(PHASE_TIMER, 4);
        x.set8(0x45, 5);
    }
    x.set8(0x44, 0x45);
    key(x, kind);
    x.set8(PICKED, 0);
    if rain {
        x.set8(BURSTS, 2);
    }
    strike(x, 3);
}

/// First shot of a volley: sounds, effect, then one shot per living player.
fn volley_fire<'a, R: Bench>(x: &mut Cx<'a, R>, pick: fn(&mut Cx<'a, R>)) {
    if !x.anim_step() {
        return;
    }
    lib::shake_58(x, 0x30);
    lib::sound(x, crate::sound::CMD_345);
    addq8(x, f::SUB3, 2);
    x.set16(TIMER, 0x32);
    crate::families::effects_creators::spell_ring(x.b, x.o, 2);
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

/// 0x86334 (`+6` = 4, 8): volley of knives, one toward each player.
fn throw_knives<R: Bench>(x: &mut Cx<'_, R>) {
    match x.u8(f::SUB3) {
        0 => volley_start(x, 0xc, false),
        2 => volley_fire(x, aids::pick_new),
        4 => {
            volley_next(x, aids::pick_new);
            if timer_done(x) {
                addq8(x, f::SUB3, 2);
                strike(x, 5);
            }
        }
        _ => finish(x),
    }
}

/// 0x8673c (`+6` = 14): rain of projectiles, repeated as long as `+0xaa` allows.
fn rain<R: Bench>(x: &mut Cx<'_, R>) {
    match x.u8(f::SUB3) {
        0 => volley_start(x, 0x11, true),
        2 => volley_fire(x, aids::pick_any),
        4 => {
            volley_next(x, aids::pick_any);
            if !timer_done(x) {
                return;
            }
            if sub8(x, BURSTS, 1).mi() {
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

/// 0x86400 (`+6` = 6): plunging hit on the target's retained position.
fn plunge<R: Bench>(x: &mut Cx<'_, R>) {
    match x.u8(f::SUB3) {
        0 => {
            if x.u16(f::TARGET) == 0 {
                return;
            }
            addq8(x, f::SUB3, 2);
            let t = load_target(x);
            let (tx, ty) = (t.u16(x.b, 0x8), t.u16(x.b, 0x10));
            x.set16(TARGET_X, tx);
            x.set16(TARGET_Y, ty);
            key(x, 0xd);
            strike(x, 2);
        }
        2 => {
            if x.u8(f::KEY_FRAME) != 0 {
                addq8(x, f::SUB3, 2);
                x.set8(f::KEY_FRAME, 0);
                crate::families::effects_creators::spell(x.b, x.o, 3);
            }
            x.anim_step();
        }
        _ => finish(x),
    }
}

/// 0x864f2 (`+6` = 10): lateral charge with puffs of dust, then stop.
fn charge<R: Bench>(x: &mut Cx<'_, R>) {
    match x.u8(f::SUB3) {
        0 => {
            addq8(x, f::SUB3, 2);
            x.set8(PHASE_TIMER, 4);
            x.set8(0x45, 5);
            x.set8(0x44, 0x46);
            key(x, 0xf);
            strike(x, 7);
        }
        2 => {
            if x.u8(f::KEY_FRAME) == 0 {
                x.anim_step();
                return;
            }
            addq8(x, f::SUB3, 2);
            x.set8(f::KEY_FRAME, 0);
            lib::sound(x, crate::sound::CMD_329_429);
            let (mut d0, mut d1) = (-0x800i16, 0x20i16);
            if x.u8(f::FLIP) != 0 {
                d0 = d0.wrapping_neg();
                d1 = d1.wrapping_neg();
            }
            x.set16(0x14, d0 as u16);
            x.set16(0x16, d1 as u16);
        }
        4 => {
            dust(x, 0);
            x.move_x();
            if x.anim_step() {
                addq8(x, f::SUB3, 2);
                x.set16(TIMER, 0x14);
                strike(x, 8);
            }
        }
        6 => {
            dust(x, 1);
            if timer_done(x) {
                addq8(x, f::SUB3, 2);
                x.set16(TIMER, 1);
                let v = x.u16(0x14).wrapping_neg();
                x.set16(0x14, v);
            }
            x.move_x();
            x.anim_step();
        }
        _ => {
            if timer_done(x) {
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

/// Every 4 frames, a puff of dust under the feet (`andi.w #3,(0x6e,A5) ; beq`), of kind `kind`.
fn dust<R: Bench>(x: &mut Cx<'_, R>, kind: u32) {
    let v = x.a5_u16(0x6e) & 3;
    x.set_a5_u16(0x6e, v);
    if v == 0 {
        lib::shock(x, kind, 1, 0xffe0, 0xffff);
    }
}

/// 0x86620 (`+6` = 12): forward leap with fall, landing and burst of effects.
fn leap<R: Bench>(x: &mut Cx<'_, R>) {
    match x.u8(f::SUB3) {
        0 => leap_start(x, -0x400),
        t => leap_rest(x, t),
    }
}

/// Start of the leap: horizontal momentum `vx` (reversed if the boss faces left).
pub(super) fn leap_start<R: Bench>(x: &mut Cx<'_, R>, vx: i16) {
    addq8(x, f::SUB3, 2);
    x.set8(VULNERABLE, 1);
    key(x, 0x10);
    x.set8(0x44, 0x45);
    x.set16(0x14, vx as u16);
    if x.u8(f::FLIP) != 0 {
        let v = x.u16(0x14).wrapping_neg();
        x.set16(0x14, v);
    }
    x.set16(0x16, 0);
    x.set16(0x18, 0x900);
    x.set16(0x1a, (-0x88i16) as u16);
    strike(x, 9);
}

/// Common steps of the leap (0x86686, 0x8669c, 0x866c2, 0x8670c, 0x86728).
pub(super) fn leap_rest<R: Bench>(x: &mut Cx<'_, R>, step: u8) {
    match step {
        2 => {
            if x.u8(f::KEY_FRAME) != 0 {
                addq8(x, f::SUB3, 2);
                x.set8(VULNERABLE, 0);
            }
            x.anim_step();
        }
        4 => {
            x.move_xz();
            if (x.u16(0x18) as i16) <= -0x300 {
                x.set16(0x1a, (-0x98i16) as u16);
                addq8(x, f::SUB3, 2);
                x.set16(0x30, 1);
            }
            x.anim_step();
        }
        6 => {
            if x.move_xz().gt() {
                x.anim_step();
                return;
            }
            lib::shake_58(x, 0x14);
            lib::sound(x, crate::sound::CMD_327_427_C);
            aids::burst_small(x);
            x.set32(0xc, 0);
            addq8(x, f::SUB3, 2);
            x.set8(VULNERABLE, 1);
            x.set16(0x30, 1);
            x.set16(TIMER, 0x14);
            strike(x, 0xa);
        }
        8 => {
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

/// 0x8681a (`+6` = 16): disappearance, sidestep, reappearance.
fn vanish<R: Bench>(x: &mut Cx<'_, R>) {
    match x.u8(f::SUB3) {
        0 => {
            addq8(x, f::SUB3, 2);
            x.set8(0, 2);
            key(x, 0x12);
            lib::sound(x, crate::sound::CMD_349);
            body(x, 4);
        }
        2 => {
            if x.anim_step() {
                addq8(x, f::SUB3, 2);
                aids::random_sidestep(x);
                x.set_bit(0x1, 1, true);
                x.set16(TIMER, 8);
                body(x, 5);
            }
        }
        4 => {
            if timer_done(x) {
                addq8(x, f::SUB3, 2);
                x.set_bit(0x1, 1, false);
                x.set8(0, 1);
                lib::sound(x, crate::sound::CMD_349);
            }
        }
        _ => finish(x),
    }
}

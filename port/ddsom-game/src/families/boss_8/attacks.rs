//! Boss 8, group 4: the attacks (`+6` = 0 to 18), each in steps `+7` = 0, 2, 4...
//! At the end of an attack, `aids::after_attack` chooses what follows.

use crate::families::cx::Bench;
use super::aids;
use super::*;
use crate::objects::Current;

/// Animation key `kind` (`+0xa2`, copied to `+0xa3`).
fn key<R: Bench>(x: &mut Cx<'_, R>, kind: u8) {
    x.set8(Warrior::ANIM_KIND, kind);
    x.set8(Warrior::ANIM_LAST, kind);
}

/// End of a countdown step: has the countdown of `+0xa0` elapsed?
fn timer_done<R: Bench>(x: &mut Cx<'_, R>) -> bool {
    sub16(x, Warrior::TIMER, 1).mi()
}

/// Common final step of the attacks (0x8880a): plays the animation to the end, then chooses what follows.
fn finish<R: Bench>(x: &mut Cx<'_, R>) {
    if x.anim_step() {
        aids::after_attack(x);
    }
}

pub(super) fn run<R: Bench>(x: &mut Cx<'_, R>) {
    match x.u8(f::SUB2) {
        0 => slash(x),
        2 => lunge(x, true),
        4 | 6 => throw_knives(x),
        8 => combo(x),
        0xa => charge(x),
        0xc => leap(x),
        0xe => rain(x),
        0x10 => vanish(x),
        _ => lunge(x, false),
    }
}


/// 0x887ce (`+6` = 0): a single hit.
fn slash<R: Bench>(x: &mut Cx<'_, R>) {
    if x.u8(f::SUB3) == 0 {
        addq8(x, f::SUB3, 2);
        x.set8(Enemy::STRIKE_POWER, 5);
        x.set8(Enemy::STRIKE_KIND, 0x44);
        key(x, 0xa);
        strike(x, 0);
    } else {
        finish(x)
    }
}

/// 0x8881a (`+6` = 8): three-beat hit (the boss only strikes at the key frames `+0x33`).
fn combo<R: Bench>(x: &mut Cx<'_, R>) {
    match x.u8(f::SUB3) {
        0 => {
            addq8(x, f::SUB3, 2);
            key(x, 0xb);
            x.set8(Warrior::PHASE_TIMER, 0x1e);
            strike(x, 1);
        }
        2 => {
            if x.u8(f::KEY_FRAME) != 0 {
                addq8(x, f::SUB3, 2);
                x.set8(f::KEY_FRAME, 0);
                lib::sound(x, crate::sound::CMD_342_342);
            }
            x.anim_step();
        }
        4 => {
            if x.u8(f::KEY_FRAME) != 0 {
                x.set8(Enemy::VULNERABLE, 1);
                addq8(x, f::SUB3, 2);
                x.set8(f::KEY_FRAME, 0);
                crate::families::eshl_creators::series_3b(&mut crate::families::effects_creators::leftovers::Pure, x.b, x.o);
            }
            x.anim_step();
        }
        _ => {
            if x.u8(Warrior::HIT_ACK) != 0 {
                x.set8(Enemy::VULNERABLE, 0);
                aids::after_attack(x);
                x.set8(Warrior::HIT_ACK, 0);
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
        x.set8(Warrior::PHASE_TIMER, 0xa);
        x.set8(Enemy::STRIKE_POWER, 5);
        x.set8(Enemy::STRIKE_KIND, 0x45);
    } else {
        x.set8(Enemy::STRIKE_KIND, 0x45);
    }
    key(x, kind);
    x.set8(Warrior::PICKED, 0);
    if extra {
        x.set8(Boss8::GRABBED, 2);
    }
    strike(x, 3);
}

/// First shot of a volley: sounds, effect, then one shot per living player.
fn volley_fire<'a, R: Bench>(x: &mut Cx<'a, R>, pick: fn(&mut Cx<'a, R>)) {
    lib::shake_58(x, 0x30);
    lib::sound(x, crate::sound::CMD_345);
    addq8(x, f::SUB3, 2);
    x.set16(Warrior::TIMER, 0x32);
    let n = var::PLAYERS_ALIVE.get(x.b);
    x.set8(Warrior::VOLLEY, n);
    sub8(x, Warrior::VOLLEY, 1);
    shoot(x, pick);
    strike(x, 4);
}

/// One more shot as long as there are players left to aim at.
fn volley_next<'a, R: Bench>(x: &mut Cx<'a, R>, pick: fn(&mut Cx<'a, R>)) {
    let f = sub8(x, Warrior::VOLLEY, 1);
    if !f.eq() && !f.mi() {
        shoot(x, pick);
    }
}

fn throw_knives<R: Bench>(x: &mut Cx<'_, R>) {
    match x.u8(f::SUB3) {
        0 => volley_start(x, 0xc, false),
        2 => {
            if x.anim_step() {
                volley_fire(x, aids::pick_new);
            }
        }
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

/// 0x88cae (`+6` = 14): rain of projectiles, repeated until `+0xaa` is exhausted.
fn rain<R: Bench>(x: &mut Cx<'_, R>) {
    match x.u8(f::SUB3) {
        0 => volley_start(x, 0x11, true),
        2 => {
            if x.anim_step() {
                volley_fire(x, aids::pick_any);
            }
        }
        4 => {
            volley_next(x, aids::pick_any);
            if !timer_done(x) {
                return;
            }
            if (sub8(x, Boss8::GRABBED, 1)).mi() {
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
    match x.u8(f::SUB3) {
        0 => {
            addq8(x, f::SUB3, 2);
            x.set8(Warrior::PHASE_TIMER, 0xa);
            x.set8(Enemy::STRIKE_POWER, 5);
            x.set8(Enemy::STRIKE_KIND, 0x46);
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
            x.set16(Enemy::VX, d0 as u16);
            x.set16(Enemy::AX, d1 as u16);
        }
        4 => {
            dust(x);
            x.move_x();
            if x.anim_step() {
                addq8(x, f::SUB3, 2);
                x.set16(Warrior::TIMER, 0x14);
                strike(x, 8);
            }
        }
        6 => {
            dust(x);
            if sub16(x, Warrior::TIMER, 1).mi() {
                addq8(x, f::SUB3, 2);
                x.set16(Warrior::TIMER, 1);
                let v = x.u16(Enemy::VX).wrapping_neg();
                x.set16(Enemy::VX, v);
            }
            x.move_x();
            x.anim_step();
        }
        _ => {
            if sub16(x, Warrior::TIMER, 1).mi() {
                x.set16(Warrior::TIMER, 0);
                x.set16(Enemy::VX, 0);
                x.set16(Enemy::AX, 0);
                x.set8(Enemy::ACTIVE, 1);
                return aids::after_attack(x);
            }
            x.move_x();
            x.anim_step();
        }
    }
}

/// Every 4 frames, a puff of dust under the feet (`andi.w #3,(0x6e,A5) ; beq`).
fn dust<R: Bench>(x: &mut Cx<'_, R>) {
    let v = var::FRAME_COUNTER.get(x.b) & 3;
    var::FRAME_COUNTER.set(x.b, v);
    if v == 0 {
        lib::shock(x, 1, 1, 0xffe0, 0xffff);
    }
}

/// 0x88b92 (`+6` = 12): leap with fall, landing and burst of effects.
fn leap<R: Bench>(x: &mut Cx<'_, R>) {
    match x.u8(f::SUB3) {
        0 => leap_start(x, -0x400, 0x10, 9),
        t => leap_rest(x, t),
    }
}

/// Start of the leap: horizontal momentum `vx` (reversed if the boss faces left), animation key `kind`.
pub(super) fn leap_start<R: Bench>(x: &mut Cx<'_, R>, vx: i16, kind: u8, anim: u32) {
    addq8(x, f::SUB3, 2);
    x.set8(Enemy::VULNERABLE, 1);
    key(x, kind);
    x.set8(Enemy::STRIKE_KIND, 0x45);
    x.set16(Enemy::VX, vx as u16);
    if x.u8(f::FLIP) != 0 {
        let v = x.u16(Enemy::VX).wrapping_neg();
        x.set16(Enemy::VX, v);
    }
    x.set16(Enemy::AX, 0);
    x.set16(Enemy::VZ, 0x900);
    x.set16(Enemy::GRAVITY, (-0x88i16) as u16);
    strike(x, anim);
}

/// Common steps of the leap (0x88bf8, 0x88c0e, 0x88c34, 0x88c7e, 0x88c9a).
pub(super) fn leap_rest<R: Bench>(x: &mut Cx<'_, R>, step: u8) {
    match step {
        2 => {
            if x.u8(f::KEY_FRAME) != 0 {
                addq8(x, f::SUB3, 2);
                x.set8(Enemy::VULNERABLE, 0);
            }
            x.anim_step();
        }
        4 => {
            x.move_xz();
            if !Fl::sub16(x.u16(Enemy::VZ), 0xfd00).gt() || true {
                // cmpi.w #-0x300,(0x18,A0) ; bgt: the vertical momentum shrinks
            }
            if (x.u16(Enemy::VZ) as i16) <= -0x300 {
                x.set16(Enemy::GRAVITY, (-0x98i16) as u16);
                addq8(x, f::SUB3, 2);
                x.set16(Obj::ANIM_TIMER, 1);
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
            x.set32(Obj::Z, 0);
            addq8(x, f::SUB3, 2);
            x.set8(Enemy::VULNERABLE, 1);
            x.set16(Obj::ANIM_TIMER, 1);
            x.set16(Warrior::TIMER, 0x14);
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
                x.set16(Warrior::TIMER, 0);
                aids::after_attack(x);
            }
        }
    }
}

/// 0x88d86 (`+6` = 16): disappearance, reappearance beside.
fn vanish<R: Bench>(x: &mut Cx<'_, R>) {
    match x.u8(f::SUB3) {
        0 => {
            addq8(x, f::SUB3, 2);
            x.set8(Warrior::PHASE_TIMER, 0x10);
            key(x, 0x12);
            lib::sound(x, crate::sound::CMD_349);
            body(x, 4);
        }
        2 => {
            if x.anim_step() {
                addq8(x, f::SUB3, 2);
                aids::random_sidestep(x);
                x.set_bit(Enemy::DISPLAY, 1, true);
                x.set16(Warrior::TIMER, 8);
                body(x, 5);
            }
        }
        4 => {
            if timer_done(x) {
                addq8(x, f::SUB3, 2);
                x.set_bit(Enemy::DISPLAY, 1, false);
                lib::sound(x, crate::sound::CMD_349);
            }
        }
        _ => finish(x),
    }
}

/// 0x88e0e (`+6` = 2, `first`) and 0x88e50 (`+6` = 18): forward leap then hit (0xa4e84 receives D4).
fn lunge<R: Bench>(x: &mut Cx<'_, R>, first: bool) {
    // jump tables of steps 0x88e1c (`first`) and 0x88e5e: they differ only by the second step
    match x.u8(f::SUB3) {
        0 => {
            addq8(x, f::SUB3, 2);
            x.set8(Warrior::PHASE_TIMER, 0xa);
            key(x, 0x13);
            strike(x, 0xc);
        }
        2 => {
            if x.anim_step() {
                addq8(x, f::SUB3, 2);
                x.set16(Warrior::TIMER, 0x20);
                // D4 = `first` (leftover re-read by the following handlers, `dispatch::Leftovers`)
                *x.c.d4() = first as u32;
                crate::families::eshl_creators::series_36(&mut crate::families::effects_creators::leftovers::Pure, x.b, x.o, first as u8);
                strike(x, 0xd);
            }
        }
        4 => {
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

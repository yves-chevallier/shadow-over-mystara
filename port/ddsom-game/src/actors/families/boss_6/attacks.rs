//! Boss 6, group `+5` = 4: the five attacks (`+6` = 0 to 8).
//!
//! | `+6` | Attack (h) |
//! |---|---|
//! | 0 | roar |
//! | 2 | dive: rises, plunges with a linked ESHL 0x2a, climbs back |
//! | 4 | dive from behind the camera: same flight, other approach |
//! | 6 | leap onto the target |
//! | 8 | low flight with an ESHL 0x2a (variant 2): the shot leaves at the end of the run |

use crate::actors::families::cx::Bench;
use super::decide;
use super::*;
use crate::world::objects::pools;
use crate::world::objects::Current;
use crate::world::objects::Obj;

/// Table of height increments (longs) of the low flight, indexed by the key frame `+0x33` (opcode space).
const LOW_FLIGHT: &str = "boss_6/low_flight";
/// Table of height increments of 0x8596c.
const SKIM: &str = "boss_6/skim";

pub(super) fn run<R: Bench>(x: &mut Cx<'_, R>) {
    match x.u8(f::SUB2) {
        0 => roar(x),
        2 => swoop_a(x),
        4 => swoop_b(x),
        6 => leap(x),
        8 => spit(x),
        _ => {}
    }
}

fn players_alive<R: Bench>(x: &mut Cx<'_, R>) -> bool {
    var::PLAYERS_ALIVE.get(x.b) != 0
}

fn resume_chase<R: Bench>(x: &mut Cx<'_, R>) {
    if !players_alive(x) {
        return decide::wait(x);
    }
    decide::chase(x);
}

fn resume_stand<R: Bench>(x: &mut Cx<'_, R>) {
    if !players_alive(x) {
        return decide::wait(x);
    }
    decide::stand(x);
}

/// Wing beat: the key frame has its bit 7 raised (set by the script): it is removed, `+0xb3` alternates
/// and the sound follows (0x11a36).
fn wing_beat<R: Bench>(x: &mut Cx<'_, R>) {
    if tst8(x, f::KEY_FRAME).pl() {
        return;
    }
    let k = x.u8(f::KEY_FRAME) & 0x7f;
    x.set8(f::KEY_FRAME, k);
    let flap = x.u8(Boss6::FLAP) ^ 1;
    x.set8(Boss6::FLAP, flap);
    lib::ground_debris(x, flap);
}

/// `(0xc).l += (word << 16) >> 8`: the height advances by `v` pixels per frame (8.8).
fn climb<R: Bench>(x: &mut Cx<'_, R>, v: u16) {
    let d0 = ((v as i16 as i32) << 8) as u32;
    let z = x.u32(Obj::Z).wrapping_add(d0);
    x.set32(Obj::Z, z);
}

// --- +6 = 0 : rugissement -----------------------------------------------------------------------

fn roar<R: Bench>(x: &mut Cx<'_, R>) {
    match x.u8(Enemy::SUB_STEP) {
        0 => {
            x.add8(Enemy::SUB_STEP, 2);
            lib::sound(x, crate::services::sound::CMD_30C_40C);
            x.set8(Boss::ANIM_KIND, 0x10);
            x.set8(Boss::ANIM_LAST, 0x10);
            fly(x, 0);
        }
        2 => {
            if lib::step(x) {
                resume_chase(x);
            }
        }
        _ => {}
    }
}

// --- linked shot ----------------------------------------------------------------------------------------

/// Creates the ESHL 0x2a linked to the boss (variant `v`); false if the pool is full.
fn make_shot<R: Bench>(x: &mut Cx<'_, R>, v: u8) -> bool {
    let Some(shot) = x.alloc(&pools::ESHL) else { return false };
    x.set16(Boss6::CHILD, shot.0 as u16);
    shot.set_u8(x.b, Enemy::ACTIVE, 1);
    shot.set_u8(x.b, Enemy::KIND, 0x2a);
    shot.set_u8(x.b, Enemy::VARIANT, v);
    shot.set_u16(x.b, Enemy::CREATOR, x.o.0 as u16);
    let target = x.u16(f::TARGET);
    shot.set_u16(x.b, f::TARGET, target);
    let side = x.u8(f::FLIP);
    shot.set_u8(x.b, f::FLIP, side);
    true
}

/// 0x84b9c: the boss rises (wings) once the shot is created.
fn take_off<R: Bench>(x: &mut Cx<'_, R>) {
    x.add8(Enemy::SUB_STEP, 2);
    x.set16(Enemy::VZ, 0x400);
    x.set16(Enemy::GRAVITY, 0);
    fly(x, 1);
}

/// 0x84b6e: creation of the shot then takeoff of the first dive attack.
fn start_swoop<R: Bench>(x: &mut Cx<'_, R>) {
    if make_shot(x, 1) {
        take_off(x);
    } else {
        resume_stand(x);
    }
}

// --- +6 = 2: dive A -----------------------------------------------------------------------------------

fn swoop_a<R: Bench>(x: &mut Cx<'_, R>) {
    match x.u8(Enemy::SUB_STEP) {
        0 => {
            if !pick_target(x) {
                return decide::wait(x);
            }
            x.set8(Boss::ANIM_KIND, 0x11);
            x.set8(Boss::ANIM_LAST, 0x11);
            start_swoop(x);
        }
        2 => swoop_flight(x),
        4 => {
            if sub16(x, Boss::TIMER, 1).le() {
                x.add8(Enemy::SUB_STEP, 2);
                x.set8(Boss::INVULNERABLE, 0);
                fly(x, 2);
                return;
            }
            if x.u16(Boss::TIMER) == 0xa {
                x.set_bit(Enemy::DISPLAY, 1, false);
                let target = x.linked(f::TARGET);
                let (tx, ty) = (target.u16(x.b, Obj::X), target.u16(x.b, Obj::Y));
                x.set16(Obj::X, tx);
                x.set16(Obj::Y, ty);
                let h = var::CAMERA1_Y.get(x.b).wrapping_add(0xf0).wrapping_sub(ty);
                x.set16(Obj::Z, h);
                x.set16(Enemy::VZ, (-0x500i16) as u16);
                x.set16(Enemy::GRAVITY, (-0x10i16) as u16);
            }
        }
        6 => {
            let v = x.u16(Enemy::VZ).wrapping_add(x.u16(Enemy::GRAVITY));
            x.set16(Enemy::VZ, v);
            climb(x, v);
            if tst16(x, 0xc).gt() {
                return;
            }
            x.add8(Enemy::SUB_STEP, 2);
            x.set16(Boss::TIMER, 0x1e);
            x.set16(Obj::Z, 0);
            lib::shake_58(x, 0x1e);
            lib::debris(x, 5);
            lib::sound(x, crate::services::sound::CMD_328_428_B);
            fly(x, 3);
        }
        8 => {
            if tst16(x, Boss::TIMER).gt() {
                sub16(x, Boss::TIMER, 1);
                return;
            }
            if x.anim_step() {
                resume_stand(x);
            }
        }
        _ => {}
    }
}

/// +7 = 2 of dives A and B (0x84bb8): flight up to the screen height, with the wing beats.
fn swoop_flight<R: Bench>(x: &mut Cx<'_, R>) {
    let ceiling = var::CAMERA1_Y.get(x.b).wrapping_add(0xf0);
    let height = x.u16(Obj::Y).wrapping_add(x.u16(Obj::Z));
    if Fl::sub16(height, ceiling).ge() {
        x.add8(Enemy::SUB_STEP, 2);
        x.set16(Boss::TIMER, 0x28);
        x.set8(Boss::INVULNERABLE, 1);
        x.set_bit(Enemy::DISPLAY, 1, true);
        let shot = x.linked(Boss6::CHILD);
        Salvo(shot).set_armed(x.b, 1);
        return;
    }
    lib::push_players(x);
    lib::step(x);
    let frame = x.u8(f::KEY_FRAME) & 0x7f;
    if !Fl::sub8(frame, 3).ge() {
        wing_beat(x);
        let v = x.u16(Enemy::VZ).wrapping_sub(0x28);
        x.set16(Enemy::VZ, v);
        climb(x, v);
        x.set16(Enemy::GRAVITY, v);
    } else {
        wing_beat(x);
        let v = x.u16(Enemy::GRAVITY).wrapping_sub(0x30);
        x.set16(Enemy::GRAVITY, v);
        climb(x, v);
        x.set16(Enemy::VZ, 0x400);
    }
}

// --- +6 = 4: dive B -----------------------------------------------------------------------------------

fn swoop_b<R: Bench>(x: &mut Cx<'_, R>) {
    match x.u8(Enemy::SUB_STEP) {
        0 => {
            if !pick_target(x) {
                return decide::wait(x);
            }
            lib::sound(x, crate::services::sound::CMD_30C_40C);
            x.set8(Boss::ANIM_KIND, 0x12);
            x.set8(Boss::ANIM_LAST, 0x12);
            start_swoop(x);
        }
        2 => swoop_flight(x),
        4 => {
            if sub16(x, Boss::TIMER, 1).gt() {
                return;
            }
            x.add8(Enemy::SUB_STEP, 2);
            x.set_bit(Enemy::DISPLAY, 1, false);
            let target = x.linked(f::TARGET);
            let ty = target.u16(x.b, Obj::Y);
            x.set16(Obj::Y, ty);
            x.set16(Obj::Z, 0x70);
            let cam = var::CAMERA1_X.get(x.b);
            let (px, vx) = if x.u8(f::FLIP) != 0 { (cam, 0x700i16) } else { (cam.wrapping_add(0x180), -0x700) };
            x.set16(Obj::X, px);
            x.set16(Enemy::VX, vx as u16);
            x.set16(Enemy::VZ, (-0x680i16) as u16);
            x.set16(Enemy::AX, 0);
            x.set16(Enemy::GRAVITY, 0);
            lib::sound(x, crate::services::sound::CMD_30C_40C);
            x.set8(Boss::INVULNERABLE, 0);
            fly(x, 5);
        }
        6 => {
            x.move_xz();
            if tst16(x, 0xc).gt() {
                return;
            }
            x.add8(Enemy::SUB_STEP, 2);
            x.set16(Boss::TIMER, 0x1e);
            x.set16(Boss6::COUNT, 0);
            x.set16(Obj::Z, 0);
            let vz: i16 = if x.u8(f::FLIP) != 0 { -0x38 } else { 0x38 };
            x.set16(Enemy::AX, vz as u16);
            lib::shake_54(x, 0x1e);
            lib::sound(x, crate::services::sound::CMD_328_428);
            lib::sound(x, crate::services::sound::CMD_329_429);
            fly(x, 6);
        }
        8 => {
            if !sub16(x, Boss6::COUNT, 1).gt() {
                x.set16(Boss6::COUNT, 4);
                lib::debris(x, 3);
            }
            if sub16(x, Boss::TIMER, 1).le() {
                return decide::stand(x);
            }
            x.move_x();
            lib::step(x);
        }
        _ => {}
    }
}

// --- +6 = 6: leap onto the target ----------------------------------------------------------------------

fn leap<R: Bench>(x: &mut Cx<'_, R>) {
    match x.u8(Enemy::SUB_STEP) {
        0 => {
            x.add8(Enemy::SUB_STEP, 2);
            x.set16(Enemy::AX, 0);
            let angle = if x.u8(f::FLIP) != 0 { 8 } else { 0x18 };
            x.set8(Obj::SPEED_INDEX, angle);
            speed(x, 0x40);
            let target = x.linked(f::TARGET);
            let mut gap = (x.u32(Obj::X) as i32).wrapping_sub(target.u32(x.b, Obj::X) as i32).wrapping_abs();
            let per_frame = ((x.u16(Enemy::VX) as i16 as i32) << 8).wrapping_abs();
            let mut vz: u16 = 0x200;
            for _ in 0..0x10000 {
                gap = gap.wrapping_sub(per_frame);
                if gap <= 0 {
                    break;
                }
                vz = vz.wrapping_add(0x10);
            }
            x.set16(Enemy::VZ, vz);
            x.set16(Enemy::GRAVITY, (-0x38i16) as u16);
            lib::sound(x, crate::services::sound::CMD_30C_40C);
            x.set8(Boss::ANIM_KIND, 0x13);
            x.set8(Boss::ANIM_LAST, 0x13);
            fly(x, 7);
        }
        2 => {
            lib::step(x);
            if x.u8(f::KEY_FRAME) != 0 {
                return;
            }
            x.move_xz();
            if cmp16(x, 0x18, 0x100).le() {
                x.add8(Enemy::SUB_STEP, 2);
                fly(x, 8);
            }
        }
        4 => {
            x.move_xz();
            if cmp16(x, 0x18, 0xff00).le() {
                x.add8(Enemy::SUB_STEP, 2);
                fly(x, 9);
            }
        }
        6 => {
            x.move_xz();
            if tst16(x, 0xc).gt() {
                return;
            }
            x.add8(Enemy::SUB_STEP, 2);
            x.set16(Obj::Z, 0);
            x.set16(Boss::TIMER, 0x14);
            x.set16(Boss6::COUNT, 0);
            x.set8(Boss::INVULNERABLE, 0);
            let vz: i16 = if x.u8(f::FLIP) == 0 { 0x5c } else { -0x5c };
            x.set16(Enemy::AX, vz as u16);
            lib::shake_54(x, 0x14);
            lib::sound(x, crate::services::sound::CMD_328_428);
            lib::sound(x, crate::services::sound::CMD_329_429);
            fly(x, 0xa);
        }
        8 => {
            if !sub16(x, Boss6::COUNT, 1).gt() {
                x.set16(Boss6::COUNT, 4);
                lib::debris(x, 3);
            }
            if sub16(x, Boss::TIMER, 1).le() {
                return resume_chase(x);
            }
            x.move_x();
            lib::step(x);
        }
        _ => {}
    }
}

// --- +6 = 8: low flight with shot ---------------------------------------------------------------------------

/// 0x8596c: the height advances by one increment of the `SKIM` table (sign according to `+0xa8`).
fn skim_step<R: Bench>(x: &mut Cx<'_, R>) {
    let i = (x.u8(f::KEY_FRAME) & 0x7f) as u32 * 4;
    let mut d0 = long(x, SKIM, i) as i32;
    if !cmp16(x, Boss6::COUNT, 0x24).gt() {
        d0 = d0.wrapping_neg();
    }
    let z = x.u32(Obj::Z).wrapping_add(d0 as u32);
    x.set32(Obj::Z, z);
    if !sub16(x, Boss6::COUNT, 1).gt() {
        x.set16(Boss6::COUNT, 0x48);
    }
}

fn spit<R: Bench>(x: &mut Cx<'_, R>) {
    match x.u8(Enemy::SUB_STEP) {
        0 => {
            if !pick_target(x) {
                return decide::wait(x);
            }
            x.set8(Boss::ANIM_KIND, 0x14);
            x.set8(Boss::ANIM_LAST, 0x14);
            if make_shot(x, 2) {
                take_off(x);
            } else {
                resume_chase(x);
            }
        }
        2 => {
            lib::push_players(x);
            wing_beat(x);
            let low = Fl::sub16(x.u16(Obj::Z), 0x20).lt();
            if low || x.u8(f::KEY_FRAME) != 4 {
                let i = (x.u8(f::KEY_FRAME) as u32) * 4;
                let inc = long(x, LOW_FLIGHT, i);
                let z = x.u32(Obj::Z).wrapping_add(inc);
                x.set32(Obj::Z, z);
                lib::step(x);
            } else {
                x.add8(Enemy::SUB_STEP, 2);
                x.set8(Boss::INVULNERABLE, 0);
                x.set16(Boss::TIMER, 0xa);
                x.set16(Boss6::COUNT, 0x24);
                lib::step(x);
            }
        }
        4 => {
            lib::step(x);
            skim_step(x);
            lib::push_players(x);
            wing_beat(x);
            if !sub16(x, Boss::TIMER, 1).gt() {
                x.add8(Enemy::SUB_STEP, 2);
                let shot = x.linked(Boss6::CHILD);
                Salvo(shot).set_armed(x.b, 1);
            }
        }
        6 => {
            lib::step(x);
            skim_step(x);
            lib::push_players(x);
            wing_beat(x);
            if x.u8(Enemy::ANIM_END_BYTE) != 0 {
                x.add8(Enemy::SUB_STEP, 2);
                let shot = x.linked(Boss6::CHILD);
                Salvo(shot).set_trigger(x.b, 1);
            }
        }
        8 => {
            wing_beat(x);
            lib::step(x);
            skim_step(x);
            lib::push_players(x);
            let shot = x.linked(Boss6::CHILD);
            if shot.u8(x.b, Enemy::ACTIVE) == 0 || (Salvo(shot).done(x.b) != 0 && x.u8(f::KEY_FRAME) == 4) {
                x.add8(Enemy::SUB_STEP, 2);
            }
        }
        10 => {
            wing_beat(x);
            let old = x.u32(Obj::Z);
            let flags = Fl::sub32(old, 0x18000);
            x.set32(Obj::Z, old.wrapping_sub(0x18000));
            if flags.le() {
                x.add8(Enemy::SUB_STEP, 2);
                x.set16(Obj::Z, 0);
                x.set16(Boss::TIMER, 0x10);
                body(x, 0);
            } else {
                lib::step(x);
            }
        }
        12 => {
            if sub16(x, Boss::TIMER, 1).le() {
                resume_chase(x);
            } else {
                lib::step(x);
            }
        }
        _ => {}
    }
}

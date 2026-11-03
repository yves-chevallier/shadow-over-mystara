//! Boss 3, group 4: the attacks (`+6` = 0 to 30, table 0x7fd0c). Each ends with `after_attack` (0x81462).
//!
//! | `+6` | Attack |
//! |---|---|
//! | 0, 6 to 14 | claw swipe (animation 3) |
//! | 2 | short claw swipe (animation 4) |
//! | 4 | double claw swipe, the boss advances at the key frames |
//! | 16 | leap: rise to 0x70, then dive forward |
//! | 18, 22 | cries (0xb3e4e), the last of which pushes back the players in front of it |
//! | 20 | low sweep: rush forward until it skims a player, hit, climb back up |
//! | 24 | rush along the ground at height 0x50, then dive |
//! | 26 | cry in place (0xa2d22) |
//! | 28 | sideways flight toward the middle of the screen pushing the players away, with cries |
//! | 30 | abduction: rises off screen, swoops on a player and carries them off (group 12) |

use crate::families::cx::Bench;
use super::body::push_players;
use super::*;
use crate::objects::Current;

pub(super) fn run<R: Bench>(x: &mut Cx<'_, R>) {
    match x.u8(f::SUB2) {
        2 => short_claw(x),
        4 => double_claw(x),
        16 => leap(x),
        18 | 22 => cries(x),
        20 => skim(x),
        24 => rush(x),
        26 => cry(x),
        28 => side_flight(x),
        30 => abduction(x),
        _ => claw(x),
    }
    if var::PLAYERS_ALIVE.get(x.b) == 0 {
        idle_alone(x);
    }
}

/// Start of a hit: speeds to zero, animation `n` of the attacks set, cry (0xebfdc).
fn swing<R: Bench>(x: &mut Cx<'_, R>, n: u32) {
    next7(x);
    x.set8(Enemy::STRIKE_KIND, 0);
    x.set8(Enemy::STRIKE_POWER, 0x10);
    stop(x);
    attack_anim(x, n);
    lib::sound(x, crate::sound::CMD_361);
}

/// Step forward at the key frames (`+0x33`): offset read from table `table` (PC-relative), toward the front.
fn key_step<R: Bench>(x: &mut Cx<'_, R>, table: &str, clear: bool) {
    let key = x.u8(f::KEY_FRAME) as u32;
    if clear {
        if key == 0 {
            return;
        }
        x.set8(f::KEY_FRAME, 0);
    }
    let d = byte(x, table, key) as i8 as i16 as u16;
    if d == 0 {
        return;
    }
    let d = if x.u8(f::FLIP) != 0 { d } else { d.wrapping_neg() };
    x.add16(Obj::X, d);
}

/// 0x7fdee : coup de griffes.
fn claw<R: Bench>(x: &mut Cx<'_, R>) {
    if x.u8(Enemy::SUB_STEP) == 0 {
        return swing(x, 3);
    }
    if !lib::step(x) {
        return key_step(x, "boss_3/claw_steps", true);
    }
    after_attack(x)
}

/// 0x7fe34 : coup de griffes court.
fn short_claw<R: Bench>(x: &mut Cx<'_, R>) {
    if x.u8(Enemy::SUB_STEP) == 0 {
        return swing(x, 4);
    }
    if lib::step(x) {
        after_attack(x)
    }
}

/// 0x7fd2c : double coup de griffes.
fn double_claw<R: Bench>(x: &mut Cx<'_, R>) {
    match x.u8(Enemy::SUB_STEP) {
        0 => swing(x, 3),
        2 => {
            if !lib::step(x) {
                return key_step(x, "boss_3/claw_steps", true);
            }
            next7(x);
            x.set8(Enemy::STRIKE_KIND, 0);
            x.set8(Enemy::STRIKE_POWER, 0x10);
            attack_anim(x, 9);
            lib::sound(x, crate::sound::CMD_361);
        }
        _ => {
            if lib::step(x) {
                return after_attack(x);
            }
            key_step(x, "boss_3/second_claw_steps", false)
        }
    }
}

/// 0x7fe7a: leap: rise to 0x70, landing forward, slide.
fn leap<R: Bench>(x: &mut Cx<'_, R>) {
    match x.u8(Enemy::SUB_STEP) {
        0 => {
            next7(x);
            stop(x);
            x.set16(Enemy::VZ, 0x800);
            x.set8(Enemy::STRIKE_KIND, 0);
            x.set8(Enemy::STRIKE_POWER, 0x10);
            attack_anim(x, 0);
        }
        2 => {
            lib::step(x);
            if x.bit(f::KEY_FRAME, 0) {
                return;
            }
            x.move_z();
            if (x.u16(Obj::Z) as i16) < 0x70 {
                return;
            }
            x.set16(Obj::Z, 0x70);
            next7(x);
            let v = (x.u16(Enemy::VZ) as i16) >> 1;
            x.set16(Enemy::VZ, v as u16);
            x.set8(Enemy::STRIKE_KIND, 0);
            x.set8(Enemy::STRIKE_POWER, 0x10);
            attack_anim(x, 1);
        }
        4 => {
            x.set8(Boss3::INVULNERABLE, 1);
            lib::step(x);
            if x.bit(f::KEY_FRAME, 0) {
                x.move_z();
                if (x.u16(Enemy::VZ) as i16) <= 0 {
                    x.set32(Enemy::VZ_LONG, 0);
                }
                return;
            }
            next7(x);
            x.set8(Boss3::INVULNERABLE, 8);
            x.set16(Enemy::VZ, (-0x500i16) as u16);
            let (mut vx, mut vz) = (0x600u16, 0x40u16);
            if x.u8(f::FLIP) == 0 {
                vx = vx.wrapping_neg();
                vz = vz.wrapping_neg();
            }
            x.set16(Enemy::VX, vx);
            x.set16(Enemy::AX, vz);
        }
        6 => {
            lib::step(x);
            x.move_xyz();
            if (x.u16(Obj::Z) as i16) > 0 {
                return;
            }
            x.set16(Obj::Z, 0);
            next7(x);
            let v = ((x.u16(Enemy::VX) as i16) >> 3) as u16;
            x.set16(Enemy::VX, v);
            x.set16(Enemy::AX, 0);
            let angle = if x.u8(f::FLIP) != 0 { 8 } else { 0x18 };
            x.set8(Obj::SPEED_INDEX, angle);
            attack_anim(x, 2);
        }
        _ => {
            x.move_xy();
            if lib::step(x) {
                after_attack(x)
            }
        }
    }
}

/// Marked key frame (bit 7 of `+0x33`): true and mark cleared.
fn marked_key<R: Bench>(x: &mut Cx<'_, R>) -> bool {
    if (x.u8(f::KEY_FRAME) as i8) >= 0 {
        return false;
    }
    x.set8(f::KEY_FRAME, 0);
    true
}

/// 0x7ffbc: a cry (animation 8), then four others that push back the players in front of it.
fn cries<R: Bench>(x: &mut Cx<'_, R>) {
    match x.u8(Enemy::SUB_STEP) {
        0 | 2 => {
            if x.u8(Enemy::SUB_STEP) == 0 {
                next7(x);
                x.set8(Boss3::TIMER, 1);
                attack_anim(x, 8);
            }
            lib::step(x);
            if !marked_key(x) || sub8(x, Boss3::TIMER, 1).gt() {
                return;
            }
            crate::families::effects_creators::shouts(x.b, x.o, false);
            next7(x);
            x.set8(Boss3::TIMER, 4);
            lib::sound(x, crate::sound::CMD_336_436);
        }
        _ => {
            push_players(x, 1);
            lib::step(x);
            if !marked_key(x) {
                return;
            }
            crate::families::effects_creators::shouts(x.b, x.o, false);
            if sub8(x, Boss3::TIMER, 1).gt() {
                return lib::sound(x, crate::sound::CMD_336_436);
            }
            after_attack(x)
        }
    }
}

/// 0x80130: X distance to the nearest present player (0x7fff with no player).
fn nearest_player_dx<R: Bench>(x: &mut Cx<'_, R>) -> u16 {
    let mut best = 0x7fffu16;
    for k in 0..4u32 {
        let p = Obj(0xff85de + 0x200 * k);
        if p.u8(x.b, Enemy::ACTIVE) == 0 {
            continue;
        }
        let mut d = p.u16(x.b, Obj::X).wrapping_sub(x.u16(Obj::X));
        if (d as i16) < 0 {
            d = d.wrapping_neg();
        }
        if (best as i16) > d as i16 {
            best = d;
        }
    }
    best
}

/// 0x801ca: launched at full speed (0x900 or more), it no longer drifts in depth.
fn no_drift_when_fast<R: Bench>(x: &mut Cx<'_, R>) {
    let mut v = x.u16(Enemy::VX);
    if (v as i16) < 0 {
        v = v.wrapping_neg();
    }
    if v as i16 >= 0x900 {
        x.set16(Enemy::AX, 0);
    }
}

/// 0x8003e: low sweep: rush forward, hit (animation 6) at the first player skimmed, climb back if the hit connects.
fn skim<R: Bench>(x: &mut Cx<'_, R>) {
    match x.u8(Enemy::SUB_STEP) {
        0 => {
            next7(x);
            x.set8(Boss3::DIVE, 0);
            x.set8(Enemy::STRIKE_KIND, 0);
            x.set8(Enemy::STRIKE_POWER, 0x10);
            x.set8(Boss::HIT_FLAGS, 0);
            stop(x);
            let (mut vx, mut vz) = (0x600u16, 0x20u16);
            if x.u8(f::FLIP) == 0 {
                vx = vx.wrapping_neg();
                vz = vz.wrapping_neg();
            }
            x.set16(Enemy::VX, vx);
            x.set16(Enemy::AX, vz);
            x.set8(Enemy::STRIKE_KIND, 0);
            x.set8(Enemy::STRIKE_POWER, 0x10);
            attack_anim(x, 5);
        }
        2 => {
            lib::step(x);
            if x.u8(f::KEY_FRAME) != 0 {
                return;
            }
            x.set8(Boss3::TIMER, 0x20);
            next7(x);
        }
        4 => {
            if (x.u8(Boss3::DIVE) as i8) < 4 && !sub8(x, Boss3::TIMER, 1).gt() {
                return skim_end(x);
            }
            match x.u8(Boss3::DIVE) {
                0 => {
                    lib::step(x);
                    x.move_xy();
                    no_drift_when_fast(x);
                    if nearest_player_dx(x) as i16 >= 0x30 {
                        return;
                    }
                    x.add8(Boss3::DIVE, 2);
                    attack_anim(x, 6);
                }
                2 => {
                    lib::step(x);
                    x.move_xy();
                    no_drift_when_fast(x);
                    if !x.bit(Boss::HIT_FLAGS, 2) {
                        return;
                    }
                    x.add8(Boss3::DIVE, 2);
                    stop(x);
                    let mut v = (-0x400i16) as u16;
                    if x.u8(f::FLIP) == 0 {
                        v = v.wrapping_neg();
                    }
                    x.set16(Enemy::VX, v);
                    x.set16(Enemy::VZ, 0x400);
                    x.set16(Enemy::GRAVITY, (-0x40i16) as u16);
                }
                _ => {
                    lib::step(x);
                    x.move_xz();
                    if (x.u16(Obj::Z) as i16) >= 0 {
                        return;
                    }
                    x.set16(Obj::Z, 0);
                    skim_end(x)
                }
            }
        }
        _ => {
            x.move_xz();
            if lib::step(x) {
                after_attack(x)
            }
        }
    }
}

/// 0x800e0: end of the low sweep (animation 7).
fn skim_end<R: Bench>(x: &mut Cx<'_, R>) {
    stop(x);
    let v = ((x.u16(Enemy::VX) as i16) >> 2) as u16;
    x.set16(Enemy::VX, v);
    x.set16(Enemy::AX, 0);
    next7(x);
    attack_anim(x, 7);
}

/// 0x8030e: X speed brought back to ±0x800 if it exceeds it (and no more drift in depth).
fn cap_speed<R: Bench>(x: &mut Cx<'_, R>) {
    let v = x.u16(Enemy::VX);
    let (cap, abs) = if (v as i16) < 0 { (0xf800u16, v.wrapping_neg()) } else { (0x800, v) };
    if abs as i16 <= 0x800 {
        return;
    }
    x.set16(Enemy::VX, cap);
    x.set16(Enemy::AX, 0);
}

/// 0x801de: rush: rises or descends to height 0x50, glides, then dives forward.
fn rush<R: Bench>(x: &mut Cx<'_, R>) {
    match x.u8(Enemy::SUB_STEP) {
        0 | 2 => {
            if x.u8(Enemy::SUB_STEP) == 0 {
                next7(x);
                x.set8(Enemy::STRIKE_KIND, 0);
                x.set8(Enemy::STRIKE_POWER, 0x10);
                x.set8(Boss3::HOVER, 0);
                body_anim(x, 1);
            }
            x.set8(Boss3::HOVER, 0);
            lib::step(x);
            x.set8(Boss3::INVULNERABLE, 1);
            let d = 0x50u16.wrapping_sub(x.u16(Obj::Z));
            if d == 0 {
                next7(x);
                x.set8(Boss3::TIMER, 0x10);
                return;
            }
            let v: i16 = if (d as i16) < 0 {
                if (d as i16) < -2 { -4 } else { -1 }
            } else if d as i16 > 2 {
                4
            } else {
                1
            };
            x.add16(Obj::Z, v as u16);
        }
        4 => {
            if sub8(x, Boss3::TIMER, 1).gt() {
                x.set8(Boss3::HOVER, 0);
                lib::step(x);
                return;
            }
            next7(x);
            stop(x);
            x.set16(Enemy::VZ, (-0x200i16) as u16);
            let (angle, mut vx, mut vz) = if x.u8(f::FLIP) != 0 { (8u8, 0x600u16, 0x40u16) } else { (0x18, 0x600, 0x40) };
            if x.u8(f::FLIP) == 0 {
                vx = vx.wrapping_neg();
                vz = vz.wrapping_neg();
            }
            x.set8(Obj::SPEED_INDEX, angle);
            x.set16(Enemy::VX, vx);
            x.set16(Enemy::AX, vz);
            speed(x, 0x1f);
            attack_anim(x, 0xa);
        }
        6 => {
            lib::step(x);
            x.move_xyz();
            cap_speed(x);
            if (x.u16(Obj::Z) as i16) > 8 {
                return;
            }
            x.set16(Obj::Z, 8);
            x.set32(Enemy::VZ_LONG, 0);
            next7(x);
            x.set8(Boss3::TIMER, 8);
            attack_anim(x, 0xb);
        }
        _ => {
            lib::step(x);
            x.move_xyz();
            cap_speed(x);
            if sub8(x, Boss3::TIMER, 1).gt() {
                return;
            }
            after_attack(x)
        }
    }
}

/// 0x80330: cry in place: rise, cry (0xa2d22, 0xedbf4) at the key frames.
fn cry<R: Bench>(x: &mut Cx<'_, R>) {
    match x.u8(Enemy::SUB_STEP) {
        0 | 2 => {
            if x.u8(Enemy::SUB_STEP) == 0 {
                next7(x);
                stop(x);
                x.set16(Enemy::VZ, 0x200);
                attack_anim(x, 0xc);
            }
            x.move_z();
            if !lib::step(x) {
                return;
            }
            next7(x);
            x.set16(Enemy::VZ, 0);
            attack_anim(x, 0xd);
        }
        _ => {
            if lib::step(x) {
                return after_attack(x);
            }
            if x.u8(f::KEY_FRAME) == 0 {
                return;
            }
            x.set8(f::KEY_FRAME, 0);
            lib::eshl_burst_26(x);
            lib::sound(x, crate::sound::CMD_355_455);
        }
    }
}

/// 0x804b0 : cri (0xb3ec6, 0xedb82).
fn shriek<R: Bench>(x: &mut Cx<'_, R>) {
    crate::families::effects_creators::shouts(x.b, x.o, true);
    lib::sound(x, crate::sound::CMD_336_436);
}

/// 0x803aa: sideways flight at height 0x60 toward the middle of the screen (body sideways: it pushes the players
/// aside).
fn side_flight<R: Bench>(x: &mut Cx<'_, R>) {
    match x.u8(Enemy::SUB_STEP) {
        0 => {
            next7(x);
            x.set8(Boss3::INVULNERABLE, 1);
            x.set8(Boss3::TIMER, 2);
            stop(x);
            let mut v = 0x400u16;
            let h = x.u16(Obj::Z);
            if h as i16 > 0x60 {
                v = v.wrapping_neg();
            }
            x.set16(Enemy::VZ, v);
            attack_anim(x, 0xe);
        }
        2 => {
            x.set8(Boss3::INVULNERABLE, 1);
            x.move_z();
            let mut d = x.u16(Obj::Z).wrapping_sub(0x60);
            if (x.u16(Enemy::VZ) as i16) < 0 {
                d = d.wrapping_neg();
            }
            if (d as i16) < 0 {
                return;
            }
            next7(x);
            x.set8(Boss3::DIVE, 0);
            x.set8(Boss3::TIMER, 4);
            shriek(x)
        }
        4 => {
            let cam = camera(x);
            let mut d = ((cam.u16(x.b, Obj::X).wrapping_add(0xc0).wrapping_sub(x.u16(Obj::X)) as i16) >> 4) as u16;
            let mut a = d;
            if (a as i16) < 0 {
                a = a.wrapping_neg();
            }
            if a as i16 > 6 {
                let mut m = 6u32;
                if (d as i16) < 0 {
                    m = m & 0xffff_0000 | (6u16).wrapping_neg() as u32;
                }
                d = m as u16;
            }
            x.add16(Obj::X, d);
            push_players(x, 2);
            lib::step(x);
            if x.u8(f::KEY_FRAME) == 0 {
                return;
            }
            x.set8(f::KEY_FRAME, 0);
            if sub8(x, Boss3::TIMER, 1).gt() {
                return shriek(x);
            }
            next7(x);
            stop(x);
            x.set16(Enemy::VZ, 0x200);
            x.set16(Enemy::GRAVITY, (-0x40i16) as u16);
            attack_anim(x, 0xf);
            descend_to_0x40(x)
        }
        _ => descend_to_0x40(x),
    }
}

/// 0x80496: descends back to height 0x40, then end of the attack.
fn descend_to_0x40<R: Bench>(x: &mut Cx<'_, R>) {
    x.move_z();
    if (x.u16(Obj::Z) as i16) > 0x40 {
        return;
    }
    x.set16(Obj::Z, 0x40);
    after_attack(x)
}

/// Height above the camera's ground: `z - camera + height`.
fn height_over_camera<R: Bench>(x: &mut Cx<'_, R>) -> i16 {
    let cam = camera(x);
    let h = x.u16(Obj::Y).wrapping_sub(cam.u16(x.b, Obj::Z)).wrapping_add(x.u16(Obj::Z));
    h as i16
}

/// 0x8069a: a player within range is grabbed (0xfc44, 0xfd64: group 12); true if grabbed.
fn try_grab<R: Bench>(x: &mut Cx<'_, R>) -> bool {
    let Some(player) = lib::attack_hits_a_player(x, 1) else {
        return false;
    };
    lib::seize_player(x, player, 2);
    true
}

/// 0x804ba: abduction: rises off screen, waits, swoops on the target grabbing on the way, climbs back up.
fn abduction<R: Bench>(x: &mut Cx<'_, R>) {
    match x.u8(Enemy::SUB_STEP) {
        0 | 2 => {
            if x.u8(Enemy::SUB_STEP) == 0 {
                next7(x);
                x.set8(Boss3::TIMER, 2);
                stop(x);
                x.set16(Enemy::VZ, 0x600);
                attack_anim(x, 0x10);
            }
            x.move_z();
            if height_over_camera(x) <= 0x100 {
                return;
            }
            next7(x);
            x.set8(Boss3::TIMER, 0x20);
            x.set16(f::IMMUNITY, 0xffff);
        }
        4 => {
            if sub8(x, Boss3::TIMER, 1).gt() {
                return;
            }
            next7(x);
            x.set_bit(Enemy::DISPLAY, 1, false);
            let cam = camera(x);
            stop(x);
            let mut pz = cam.u16(x.b, Obj::Y).wrapping_add(0x50);
            // the original overwrites X with the target's word before testing it: with no target, X = 0
            let mut px = x.u16(f::TARGET);
            if (px as i16) < 0 {
                let t = load_target(x);
                px = t.u16(x.b, Obj::X);
                pz = t.u16(x.b, Obj::Y);
            }
            x.set16(Obj::Y, pz);
            x.set16(Obj::X, px);
            let (mut side, mut dx, mut vx) = (0x20u32, (-0x80i16) as u16, 0x800u16);
            let rel = px.wrapping_sub(cam.u16(x.b, Obj::X));
            if (rel as i16) < 0x100 {
                side = 0;
                dx = dx.wrapping_neg();
                vx = vx.wrapping_neg();
            }
            x.set8(f::FLIP, side as u8);
            x.add16(Obj::X, dx);
            x.set16(Obj::Z, 0x80);
            x.set16(Enemy::VX, vx);
            x.set16(Enemy::VZ, (-0x800i16) as u16);
            x.set16(Enemy::GRAVITY, 0x20);
            attack_anim(x, 0x11);
            lib::set_immunities(x);
        }
        6 => {
            if try_grab(x) {
                return;
            }
            x.move_xz();
            if (x.u16(Obj::Z) as i16) > 0x10 {
                return;
            }
            x.set16(Obj::Z, 0x10);
            next7(x);
            x.set32(Enemy::VZ_LONG, 0);
            x.set8(Boss3::TIMER, 8);
        }
        8 => {
            if try_grab(x) {
                return;
            }
            x.move_xz();
            if sub8(x, Boss3::TIMER, 1).gt() {
                return;
            }
            next7(x);
            stop(x);
            let mut v = 0x600u16;
            if x.u8(f::FLIP) == 0 {
                v = v.wrapping_neg();
            }
            x.set16(Enemy::VX, v);
            x.set16(Enemy::VZ, 0x400);
            x.set16(Enemy::GRAVITY, 0x40);
            attack_anim(x, 0x12);
        }
        10 => {
            x.move_xz();
            if height_over_camera(x) <= 0x100 {
                return;
            }
            next7(x);
            x.set8(Boss3::TIMER, 0x20);
            stop(x);
            x.set16(Enemy::VZ, (-0x400i16) as u16);
            body_anim(x, 1);
        }
        _ => {
            if tst8(x, Boss3::TIMER).gt() {
                sub8(x, Boss3::TIMER, 1);
                return;
            }
            x.set8(Boss3::HOVER, 0);
            x.move_xz();
            if (x.u16(Obj::Z) as i16) > 0x40 {
                return;
            }
            x.set16(Obj::Z, 0x40);
            after_attack(x)
        }
    }
}

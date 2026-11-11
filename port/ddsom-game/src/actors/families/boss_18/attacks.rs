//! Boss 18, group `+5` = 4: the attacks (`+6` = 0 to 20), each in steps `+7`.
//!
//! | `+6` | Attack |
//! |---|---|
//! | 0 | simple hit (animation 0 of the hits) |
//! | 2 | sidestep (four beats, step speed `+0x16`) |
//! | 4 | gesture then follow-up drawn according to the number of players (attack 4 at most `+0xae` times) |
//! | 6 | magic pursuit (`+0xb8` frames, aiming every 2) |
//! | 8 | backward leap with landing |
//! | 10, 12 | ESHL 0x20 (ball) or 0x21 (beam) shot |
//! | 14 | shockwave: players on the ground are knocked down |
//! | 16 | teleportation (disappears, reappears elsewhere) |
//! | 18 | rain of 16 ESHL 0x23 shots |
//! | 20 | successive ESHL 0x24 shots (number drawn from a table) |

use crate::actors::families::cx::Bench;
use super::decide;
use super::*;
use crate::world::objects::pools;

pub(super) fn run<R: Bench>(x: &mut Cx<'_, R>) {
    match x.u8(f::SUB2) {
        0 => strike_0(x),
        2 => sidestep(x),
        4 => gesture(x),
        6 => chase_spell(x),
        8 => hop_back(x),
        10 => ball(x),
        12 => beam(x),
        14 => shockwave(x),
        16 => teleport(x),
        18 => rain(x),
        20 => volley(x),
        _ => {}
    }
}

fn strike_anim<R: Bench>(x: &mut Cx<'_, R>, kind: u8, n: u32) {
    x.set8(Boss::ANIM_KIND, kind);
    x.set8(Boss::ANIM_LAST, kind);
    strike(x, n);
}

/// End of attack: the absence of players takes precedence; otherwise `to`.
fn finish<'a, R: Bench>(x: &mut Cx<'a, R>, to: fn(&mut Cx<'a, R>)) {
    if !lib::players_alive(x) {
        return decide::wait(x);
    }
    to(x);
}

fn step_only<R: Bench>(x: &mut Cx<'_, R>) {
    lib::step(x);
}

/// Consumes bit 0 of the key frame `+0x33`.
fn key_bit0<R: Bench>(x: &mut Cx<'_, R>) -> bool {
    if x.u8(f::KEY_FRAME) & 1 == 0 {
        return false;
    }
    let k = x.u8(f::KEY_FRAME) & !1;
    x.set8(f::KEY_FRAME, k);
    true
}

/// End of a shot: the stage state is restored (`A5+0x43`), then `to`.
fn end_shot<'a, R: Bench>(x: &mut Cx<'a, R>, to: fn(&mut Cx<'a, R>)) {
    var::INPUT_MASK.set(x.b, 0xff);
    finish(x, to);
}

/// One animation step, plus one last effect per key frame (0xe5e2a).
fn step_with_spark<R: Bench>(x: &mut Cx<'_, R>) -> bool {
    if !lib::step(x) {
        if key_bit0(x) {
            lib::sound(x, crate::hw::sound::CMD_68_68);
        }
        return false;
    }
    true
}

// --- +6 = 0: single hit -------------------------------------------------------------------------

fn strike_0<R: Bench>(x: &mut Cx<'_, R>) {
    match x.u8(Enemy::SUB_STEP) {
        0 => {
            x.add8(Enemy::SUB_STEP, 2);
            lib::sound(x, crate::hw::sound::CMD_68_68);
            strike_anim(x, 0x10, 0);
        }
        2 => {
            if lib::step(x) {
                finish(x, decide::chase);
            }
        }
        _ => {}
    }
}

// --- +6 = 2: sidestep -----------------------------------------------------------------------

fn sidestep<R: Bench>(x: &mut Cx<'_, R>) {
    match x.u8(Enemy::SUB_STEP) {
        0 => {
            x.add8(Enemy::SUB_STEP, 2);
            x.set16(Boss::TIMER, 0x1e);
            let (angle, vz) = if x.u8(f::FLIP) == 0 { (8, -0x20i16) } else { (0x18, 0x20) };
            x.set16(Enemy::AX, vz as u16);
            x.set8(Obj::SPEED_INDEX, angle);
            speed(x, 0x21);
            strike_anim(x, 0x11, 1);
        }
        2 => {
            if sub16(x, Boss::TIMER, 1).gt() {
                x.move_x();
                return step_only(x);
            }
            x.add8(Enemy::SUB_STEP, 2);
            x.set16(Boss::TIMER, 0x14);
            x.set16(Enemy::AX, 0);
            lib::sound(x, crate::hw::sound::CMD_68_68);
            let a = x.u8(Obj::SPEED_INDEX) ^ 0x10;
            x.set8(Obj::SPEED_INDEX, a);
            speed(x, 0x35);
            strike(x, 2);
        }
        4 => {
            if sub16(x, Boss::TIMER, 1).gt() {
                x.move_x();
                return step_only(x);
            }
            x.add8(Enemy::SUB_STEP, 2);
            x.set16(Boss::TIMER, 0x1e);
            let mut v: i16 = 0x2c;
            if x.u8(Obj::SPEED_INDEX) != 0x18 {
                v = -v;
            }
            x.set16(Enemy::AX, v as u16);
            strike(x, 3);
            x.move_x();
        }
        6 => {
            if sub16(x, Boss::TIMER, 1).gt() {
                x.move_x();
                return step_only(x);
            }
            finish(x, decide::chase);
        }
        _ => {}
    }
}

// --- +6 = 4: gesture -----------------------------------------------------------------------------

fn gesture<R: Bench>(x: &mut Cx<'_, R>) {
    match x.u8(Enemy::SUB_STEP) {
        0 => {
            x.add8(Enemy::SUB_STEP, 2);
            strike_anim(x, 0x12, 4);
        }
        2 => {
            if !step_with_spark(x) {
                return;
            }
            if !lib::players_alive(x) {
                return decide::wait(x);
            }
            if (x.u8(Boss18::BUDGET_4) as i8) <= 0 {
                return decide::stand(x);
            }
            let players = var::PLAYERS_ALIVE.get(x.b) as u32;
            if players == 0 {
                return decide::wait(x);
            }
            let r = (x.random() as u32) & 0xf;
            let pick = byte(x, "boss_18/charge_picks", ((players - 1) << 4) + r);
            if pick == 0 {
                return decide::stand(x);
            }
            if pick == 1 {
                // 0x9900a: the charge
                x.set8(Enemy::VULNERABLE, 0);
                x.set8(Boss::SHOT, 0x2a);
                return x.set32(f::STATE, 0x0202_0800);
            }
            sub8(x, Boss18::BUDGET_4, 1);
            x.set32(f::STATE, 0x0204_0400);
        }
        _ => {}
    }
}

// --- +6 = 6: magic pursuit --------------------------------------------------------------------

fn chase_spell<R: Bench>(x: &mut Cx<'_, R>) {
    match x.u8(Enemy::SUB_STEP) {
        0 => {
            x.add8(Enemy::SUB_STEP, 2);
            x.set8(Boss::SHOT, 0x2b);
            lib::approach_target(x);
            speed(x, 0x21);
            x.set8(Boss::ANIM_KIND, 0x13);
            x.set8(Boss::ANIM_LAST, 0x13);
            body(x, 5);
        }
        2 => {
            if !sub16(x, Boss18::CHASE_TIME, 1).gt() {
                return end_chase(x);
            }
            if !lib::facing_target(x) {
                return decide::interrupt(x);
            }
            if lib::target_in_box(x, -0x50, 0x20, 8) {
                x.add8(Enemy::SUB_STEP, 2);
                return strike(x, 5);
            }
            if !sub8(x, Boss18::AIM_TICK, 1).gt() {
                x.set8(Boss18::AIM_TICK, 2);
                lib::approach_target(x);
                speed(x, 0x21);
            }
            x.move_by_table_xy();
            step_only(x);
        }
        4 => {
            if (x.u16(Boss18::CHASE_TIME) as i16) > 0 {
                sub16(x, Boss18::CHASE_TIME, 1);
            }
            if !step_with_spark(x) {
                return;
            }
            if (x.u16(Boss18::CHASE_TIME) as i16) <= 0 {
                return end_chase(x);
            }
            if !lib::players_alive(x) {
                return decide::wait(x);
            }
            x.set8(Enemy::SUB_STEP, 2);
            body(x, 5);
        }
        _ => {}
    }
}

fn end_chase<R: Bench>(x: &mut Cx<'_, R>) {
    x.set8(Boss18::AIM_ON, 0);
    finish(x, decide::chase);
}

// --- +6 = 8: backward leap --------------------------------------------------------------------

fn hop_back<R: Bench>(x: &mut Cx<'_, R>) {
    match x.u8(Enemy::SUB_STEP) {
        0 => {
            x.add8(Enemy::SUB_STEP, 2);
            x.set16(Enemy::VZ, 0x580);
            x.set16(Enemy::GRAVITY, (-0x38i16) as u16);
            let (angle, vz) = if x.u8(f::FLIP) == 0 { (0x18, 0x10i16) } else { (8, -0x10) };
            x.set8(Obj::SPEED_INDEX, angle);
            x.set16(Enemy::AX, vz as u16);
            speed(x, 0x21);
            strike_anim(x, 0x14, 6);
        }
        2 => {
            x.move_xz();
            if tst16(x, Enemy::VZ).gt() {
                return step_only(x);
            }
            x.add8(Enemy::SUB_STEP, 2);
            x.set16(Enemy::AX, 0);
            lib::sound(x, crate::hw::sound::CMD_68_68);
            strike(x, 7);
        }
        4 => {
            x.move_xz();
            if tst16(x, Obj::Z).gt() {
                return;
            }
            x.add8(Enemy::SUB_STEP, 2);
            x.set16(Boss::TIMER, 0x1e);
            x.set16(Obj::Z, 0);
            let mut v: i16 = 0x10;
            if x.u8(f::FLIP) != 0 {
                v = -v;
            }
            x.set16(Enemy::AX, v as u16);
            speed(x, 0xd);
            strike(x, 8);
        }
        6 => {
            if sub16(x, Boss::TIMER, 1).gt() {
                x.move_x();
                return step_only(x);
            }
            finish(x, decide::stand);
        }
        _ => {}
    }
}

// --- +6 = 10: ball (ESHL 0x20) ------------------------------------------------------------------

fn ball<R: Bench>(x: &mut Cx<'_, R>) {
    match x.u8(Enemy::SUB_STEP) {
        0 => {
            x.add8(Enemy::SUB_STEP, 2);
            var::INPUT_MASK.set(x.b, 0x7f);
            sub8(x, Boss18::BUDGET_A, 1);
            x.set16(Boss::TIMER, 1);
            strike_anim(x, 0x15, 9);
        }
        2 => {
            if sub16(x, Boss::TIMER, 1).gt() {
                return step_only(x);
            }
            x.add8(Enemy::SUB_STEP, 2);
            x.set16(Boss::TIMER, 0x14);
            let Some(shot) = x.alloc(&pools::ESHL) else { return end_shot(x, decide::chase) };
            x.set16(Boss18::CHILD, shot.0 as u16);
            shot.set_u8(x.b, Obj::ACTIVE, 1);
            shot.set_u8(x.b, Obj::KIND, 0x20);
            shot.set_u8(x.b, Obj::VARIANT, 0);
            shot.set_u16(x.b, ShotB::HOST, x.o.0 as u16);
            let target = x.u16(f::TARGET);
            shot.set_u16(x.b, f::TARGET, target);
            let side = x.u8(f::FLIP);
            shot.set_u8(x.b, f::FLIP, side);
            for off in [Obj::X, Obj::Z, Obj::Y] {
                let v = x.u16(off);
                shot.set_u16(x.b, off, v);
            }
            let dx: i16 = if side == 0 { -0x30 } else { 0x30 };
            let nx = shot.u16(x.b, Obj::X).wrapping_add(dx as u16);
            shot.set_u16(x.b, Obj::X, nx);
            shot.set_u16(x.b, Obj::Z, 0x5a);
            step_only(x);
        }
        4 => {
            if sub16(x, Boss::TIMER, 1).gt() {
                return step_only(x);
            }
            x.add8(Enemy::SUB_STEP, 2);
            x.set16(Boss::TIMER, 0xa);
            let shot = x.linked(Boss18::CHILD);
            shot.set_u8(x.b, Shot32::ARMED, 1);
            lib::sound(x, crate::hw::sound::CMD_6C_6C);
            step_only(x);
        }
        6 => {
            let shot = x.linked(Boss18::CHILD);
            if shot.u8(x.b, Obj::ACTIVE) == 0 || shot.u8(x.b, Shot32::DONE) != 0 {
                return end_shot(x, decide::chase);
            }
            step_only(x);
        }
        _ => {}
    }
}

// --- +6 = 12: ray (ESHL 0x21) -------------------------------------------------------------------

fn beam<R: Bench>(x: &mut Cx<'_, R>) {
    match x.u8(Enemy::SUB_STEP) {
        0 => {
            x.add8(Enemy::SUB_STEP, 2);
            var::INPUT_MASK.set(x.b, 0x7f);
            sub8(x, Boss18::BUDGET_B, 1);
            strike_anim(x, 0x16, 0xa);
        }
        2 => {
            step_only(x);
            if x.u8(f::KEY_FRAME) == 0 {
                return;
            }
            x.add8(Enemy::SUB_STEP, 2);
            x.set16(Boss::TIMER, 0xa);
            x.set8(f::KEY_FRAME, 0);
            let Some(shot) = x.alloc(&pools::ESHL) else { return end_shot(x, decide::stand) };
            x.set16(Boss18::CHILD, shot.0 as u16);
            shot.set_u8(x.b, Obj::ACTIVE, 1);
            shot.set_u8(x.b, Obj::KIND, 0x21);
            shot.set_u16(x.b, ShotB::HOST, x.o.0 as u16);
            let side = x.u8(f::FLIP);
            shot.set_u8(x.b, f::FLIP, side ^ 0x20);
            for off in [Obj::X, Obj::Z, Obj::Y] {
                let v = x.u16(off);
                shot.set_u16(x.b, off, v);
            }
            let dx: i16 = if side == 0 { -0x5e } else { 0x5e };
            let nx = shot.u16(x.b, Obj::X).wrapping_add(dx as u16);
            shot.set_u16(x.b, Obj::X, nx);
            let nz = shot.u16(x.b, Obj::Z).wrapping_add(0x28);
            shot.set_u16(x.b, Obj::Z, nz);
        }
        4 => {
            if sub16(x, Boss::TIMER, 1).gt() {
                return step_only(x);
            }
            x.add8(Enemy::SUB_STEP, 2);
            x.set16(Boss::TIMER, 0x5a);
            let shot = x.linked(Boss18::CHILD);
            shot.set_u8(x.b, Shot33::FIRST, 1);
            lib::sound(x, crate::hw::sound::CMD_6D_6D);
            step_only(x);
        }
        6 => {
            if sub16(x, Boss::TIMER, 1).gt() {
                return step_only(x);
            }
            x.add8(Enemy::SUB_STEP, 2);
            let shot = x.linked(Boss18::CHILD);
            shot.set_u8(x.b, Shot33::SECOND, 1);
            step_only(x);
        }
        8 => {
            let shot = x.linked(Boss18::CHILD);
            if shot.u8(x.b, Obj::ACTIVE) == 0 || shot.u8(x.b, Shot33::DONE) != 0 {
                x.add8(Enemy::SUB_STEP, 2);
                x.set16(Boss::TIMER, 0xa);
            }
            step_only(x);
        }
        10 => {
            if sub16(x, Boss::TIMER, 1).gt() {
                return step_only(x);
            }
            end_shot(x, decide::stand);
        }
        _ => {}
    }
}

// --- +6 = 14: shock wave -------------------------------------------------------------------------

/// 0x9931a: a cloud (effect 0x32, variant 15) behind the Lich.
fn dust<R: Bench>(x: &mut Cx<'_, R>) {
    let Some(e) = x.alloc(&pools::EFFCT) else { return };
    e.set_u8(x.b, Obj::ACTIVE, 1);
    e.set_u8(x.b, Obj::KIND, 0x32);
    e.set_u8(x.b, Obj::VARIANT, 0xf);
    let (px, pz) = (x.u16(Obj::X), x.u16(Obj::Z));
    e.set_u16(x.b, Obj::X, px);
    e.set_u16(x.b, Obj::Z, pz);
    let py = x.u32(Obj::Y);
    e.set_u32(x.b, Obj::Y, py);
    let mut d: i16 = 4;
    // the flip read as a word (with the heading byte that follows), like the original
    if x.u16(Enemy::FLIP) != 0 {
        d = -d;
    }
    let nx = e.u16(x.b, Obj::X).wrapping_add(d as u16);
    e.set_u16(x.b, Obj::X, nx);
    let nz = e.u16(x.b, Obj::Z).wrapping_add(0x40);
    e.set_u16(x.b, Obj::Z, nz);
    let ny = e.u32(x.b, Obj::Y).wrapping_sub(1);
    e.set_u32(x.b, Obj::Y, ny);
}

/// 0x9921c: on the third pulse, players on the ground who are not already down are knocked over.
fn topple_all<R: Bench>(x: &mut Cx<'_, R>) {
    if x.u16(Boss::TIMER2) != 3 {
        return;
    }
    for k in 0..4u32 {
        let p = crate::world::ram::PLAYERS.nth(k);
        if p.u8(x.b, Obj::ACTIVE) != 1 || (p.u16(x.b, Obj::Z) as i16) > 0 || p.u8(x.b, Player::SLOW) != 0 {
            continue;
        }
        let state = p.u32(x.b, Player::MAIN) & 0xffff_ff00;
        if state == 0x0200_2200 || (state == 0x0200_1400 && p.u8(x.b, Player::CARRY) == 4) || state == 0x0204_0e00 {
            continue;
        }
        p.set_u32(x.b, Player::MAIN, 0x0204_0e00);
    }
}

/// 0x99274: drops projectiles around the camera (0x5f764) as long as none is in play.
fn spawn_blast<R: Bench>(x: &mut Cx<'_, R>) {
    if (x.u8(Boss18::SPAWNED) as i8) > 0 {
        return;
    }
    lib::sound(x, crate::hw::sound::CMD_6F_6F);
    x.set8(Boss18::SPAWNED, 0);
    let level = (var::RANK.get(x.b) & 0x1e) as u32 * 2;
    let players = var::PLAYERS_ALIVE.get(x.b) as u32;
    let mut d5 = byte(x, "boss_18/blast_counts", level + players.saturating_sub(1)) as i8 as i16 as u16;
    loop {
        let i = (d5 as u32).wrapping_mul(4) & 0xffff;
        let dx = word(x, "boss_18/blast_spots", i);
        let dy = word(x, "boss_18/blast_spots", i + 2);
        let px = dx.wrapping_add(var::CAMERA1_X.get(x.b));
        let py = dy.wrapping_add(var::CAMERA1_Y.get(x.b));
        // (D0, D1, D2) = (x, 0, y); D2 (low word) remains
        *x.c.d2() = *x.c.d2() & 0xffff_0000 | py as u32;
        if crate::actors::families::creators::enemy_5_at_pure(x.b, x.o, (px, 0, py)).is_some() {
            x.add8(Boss18::SPAWNED, 1);
        }
        if d5 == 0 {
            break;
        }
        d5 = d5.wrapping_sub(1);
    }
    // `dbf D5`: D5.w = -1 on exit (leftover; the high word is the caller's)
    *x.c.d5() = *x.c.d5() & 0xffff_0000 | 0xffff;
}

fn shockwave<R: Bench>(x: &mut Cx<'_, R>) {
    match x.u8(Enemy::SUB_STEP) {
        0 => {
            x.add8(Enemy::SUB_STEP, 2);
            lib::sound(x, crate::hw::sound::CMD_69_69);
            var::INPUT_MASK.set(x.b, 0x7f);
            sub8(x, Boss18::BUDGET_C, 1);
            x.set16(Boss::TIMER, 6);
            strike_anim(x, 0x17, 0xb);
        }
        2 => {
            if sub16(x, Boss::TIMER, 1).gt() {
                return step_only(x);
            }
            x.add8(Enemy::SUB_STEP, 2);
            dust(x);
            x.set16(Boss::TIMER, 6);
            x.set16(Boss::TIMER2, 4);
            step_only(x);
        }
        4 => {
            if !sub16(x, Boss::TIMER, 1).gt() {
                let n = x.u16(Boss::TIMER2).wrapping_sub(1);
                x.set16(Boss::TIMER2, n);
                if n & 0x8000 != 0 {
                    x.add8(Enemy::SUB_STEP, 2);
                    return strike(x, 0xc);
                }
                match n {
                    3 => {
                        x.set16(Boss::TIMER, 0x20);
                        return lib::shake_54(x, 0x20);
                    }
                    2 => {
                        x.set16(Boss::TIMER, 0x14);
                        lib::shake_5c(x, 0x14);
                    }
                    1 => {
                        x.set8(Boss::INVULNERABLE, 0);
                        x.set16(Boss::TIMER, 0x14);
                        lib::shake_58(x, 0x14);
                    }
                    _ => {
                        x.set16(Boss::TIMER, 0xa);
                        return lib::shake_54(x, 0xa);
                    }
                }
            }
            topple_all(x);
            step_only(x);
        }
        6 => {
            if !lib::step(x) {
                return;
            }
            x.add8(Enemy::SUB_STEP, 2);
            var::INPUT_MASK.set(x.b, 0xff);
            spawn_blast(x);
            x.set16(Boss::TIMER, 0x14);
            body(x, 2);
        }
        8 => {
            if sub16(x, Boss::TIMER, 1).gt() {
                return step_only(x);
            }
            finish(x, decide::stand);
        }
        _ => {}
    }
}

// --- +6 = 16: teleportation ----------------------------------------------------------------------

/// 0x99184: reappearance at one of the nine points around the camera, depending on the Lich's position.
fn place_after_teleport<R: Bench>(x: &mut Cx<'_, R>) {
    let cam = var::CAMERA1_X.get(x.b).wrapping_add(0xc0);
    let here = x.u16(Obj::X);
    let mut d5 = 0u32;
    if Fl::sub16(cam, here).lt() {
        if !Fl::sub16(cam.wrapping_add(0x70), here).lt() {
            d5 = 0x10;
        }
    } else if !Fl::sub16(cam.wrapping_sub(0x70), here).ge() {
        d5 = 0x20;
    }
    let camy = var::CAMERA1_Y.get(x.b).wrapping_add(0x54);
    let y = x.u16(Obj::Y);
    if Fl::sub16(camy, y).lt() {
        if !Fl::sub16(camy.wrapping_add(0x16), y).lt() {
            d5 |= 4;
        }
    } else if !Fl::sub16(camy.wrapping_sub(0x16), y).ge() {
        d5 |= 8;
    }
    let nx = word(x, "boss_18/teleport_spots", d5);
    let ny = word(x, "boss_18/teleport_spots", d5 + 2);
    x.set16(Obj::X, nx);
    x.set16(Obj::Y, ny);
    x.set16(Obj::Z, 0);
}

fn teleport<R: Bench>(x: &mut Cx<'_, R>) {
    match x.u8(Enemy::SUB_STEP) {
        0 => {
            x.add8(Enemy::SUB_STEP, 2);
            lib::sound(x, crate::hw::sound::CMD_6E_6E);
            strike_anim(x, 0x18, 0xd);
        }
        2 => {
            if !lib::step(x) {
                return;
            }
            x.add8(Enemy::SUB_STEP, 2);
            x.set16(Boss::TIMER, 4);
            x.set_bit(Enemy::DISPLAY, 1, true);
        }
        4 => {
            if sub16(x, Boss::TIMER, 1).gt() {
                return;
            }
            x.add8(Enemy::SUB_STEP, 2);
            place_after_teleport(x);
            x.set_bit(Enemy::DISPLAY, 1, false);
            lib::sound(x, crate::hw::sound::CMD_331);
            strike(x, 0xe);
        }
        6 => {
            if !lib::step(x) {
                return;
            }
            x.add8(Enemy::SUB_STEP, 2);
            x.set16(Boss::TIMER, 4);
            body(x, 2);
        }
        8 => {
            if sub16(x, Boss::TIMER, 1).gt() {
                return;
            }
            finish(x, decide::chase);
        }
        _ => {}
    }
}

// --- +6 = 18: rain of effects ----------------------------------------------------------------------

fn rain<R: Bench>(x: &mut Cx<'_, R>) {
    match x.u8(Enemy::SUB_STEP) {
        0 => {
            x.add8(Enemy::SUB_STEP, 2);
            var::INPUT_MASK.set(x.b, 0x7f);
            sub8(x, Boss18::BUDGET_E, 1);
            x.set16(Boss::TIMER, 0xa);
            lib::sound(x, crate::hw::sound::CMD_69_69);
            strike_anim(x, 0x19, 0xb);
        }
        2 => {
            if sub16(x, Boss::TIMER, 1).gt() {
                return step_only(x);
            }
            x.add8(Enemy::SUB_STEP, 2);
            x.set8(Boss::INVULNERABLE, 0);
            x.set16(Boss::TIMER, 0x5a);
            for n in (0..=0xfu8).rev() {
                let Some(e) = x.alloc(&pools::ESHL) else { break };
                e.set_u8(x.b, Obj::ACTIVE, 1);
                e.set_u8(x.b, Obj::KIND, 0x23);
                e.set_u8(x.b, Obj::VARIANT, n);
                e.set_u16(x.b, Enemy::CREATOR, x.o.0 as u16);
                let side = x.u8(f::FLIP);
                e.set_u8(x.b, f::FLIP, side);
                let (px, py) = (x.u16(Obj::X), x.u16(Obj::Y));
                e.set_u16(x.b, Obj::X, px);
                e.set_u16(x.b, Obj::Y, py);
            }
            step_only(x);
        }
        4 => {
            if sub16(x, Boss::TIMER, 1).gt() {
                return step_only(x);
            }
            x.add8(Enemy::SUB_STEP, 2);
            strike(x, 0xc);
        }
        6 => {
            if !lib::step(x) {
                return;
            }
            x.add8(Enemy::SUB_STEP, 2);
            x.set16(Boss::TIMER, 4);
            body(x, 2);
        }
        8 => {
            if sub16(x, Boss::TIMER, 1).gt() {
                return step_only(x);
            }
            end_shot(x, decide::chase);
        }
        _ => {}
    }
}

// --- +6 = 20: successive shots ----------------------------------------------------------------------

fn volley<R: Bench>(x: &mut Cx<'_, R>) {
    match x.u8(Enemy::SUB_STEP) {
        0 => {
            x.add8(Enemy::SUB_STEP, 2);
            var::INPUT_MASK.set(x.b, 0x7f);
            sub8(x, Boss18::BUDGET_F, 1);
            let level = (var::RANK.get(x.b) & 0x1e) as u32 * 4;
            let players = var::PLAYERS_ALIVE.get(x.b) as u32;
            let i = level + 2 * players.saturating_sub(1);
            let n = word(x, "boss_18/pulse_counts", i);
            x.set16(Boss::TIMER2, n);
            x.set16(Boss::TIMER, 0xe);
            dust(x);
            lib::sound(x, crate::hw::sound::CMD_69_69);
            strike_anim(x, 0x1a, 0xb);
        }
        2 => {
            if sub16(x, Boss::TIMER, 1).gt() {
                return step_only(x);
            }
            let n = x.u16(Boss::TIMER2).wrapping_sub(1);
            x.set16(Boss::TIMER2, n);
            if n & 0x8000 != 0 {
                x.add8(Enemy::SUB_STEP, 2);
                x.set8(Boss::INVULNERABLE, 0);
                return strike(x, 0xc);
            }
            if let Some(e) = x.alloc(&pools::ESHL) {
                e.set_u8(x.b, Obj::ACTIVE, 1);
                e.set_u8(x.b, Obj::KIND, 0x24);
                e.set_u16(x.b, Enemy::CREATOR, x.o.0 as u16);
                let side = x.u8(f::FLIP) ^ 0x20;
                e.set_u8(x.b, f::FLIP, side);
            }
            x.set16(Boss::TIMER, 0x14);
            step_only(x);
        }
        4 => {
            if !lib::step(x) {
                return;
            }
            x.add8(Enemy::SUB_STEP, 2);
            x.set16(Boss::TIMER, 0x14);
            body(x, 2);
        }
        6 => {
            if sub16(x, Boss::TIMER, 1).gt() {
                return step_only(x);
            }
            end_shot(x, decide::stand);
        }
        _ => {}
    }
}

use crate::world::objects::Obj;
use crate::world::objects::Current;

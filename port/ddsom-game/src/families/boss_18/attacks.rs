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

use crate::families::cx::Bench;
use super::decide;
use super::*;
use crate::objects::pools;

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
    x.set8(ANIM_KIND, kind);
    x.set8(ANIM_LAST, kind);
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
    set_abs8(x, 0xff8043, 0xff);
    finish(x, to);
}

/// One animation step, plus one last effect per key frame (0xe5e2a).
fn step_with_spark<R: Bench>(x: &mut Cx<'_, R>) -> bool {
    if !lib::step(x) {
        if key_bit0(x) {
            lib::sound(x, 0xe5e2a);
        }
        return false;
    }
    true
}

// --- +6 = 0 : coup simple -----------------------------------------------------------------------

fn strike_0<R: Bench>(x: &mut Cx<'_, R>) {
    match x.u8(0x7) {
        0 => {
            x.add8(0x7, 2);
            lib::sound(x, 0xe5e2a);
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
    match x.u8(0x7) {
        0 => {
            x.add8(0x7, 2);
            x.set16(TIMER, 0x1e);
            let (angle, vz) = if x.u8(f::FLIP) == 0 { (8, -0x20i16) } else { (0x18, 0x20) };
            x.set16(0x16, vz as u16);
            x.set8(Obj::SPEED_INDEX, angle);
            speed(x, 0x21);
            strike_anim(x, 0x11, 1);
        }
        2 => {
            if sub16(x, TIMER, 1).gt() {
                x.move_x();
                return step_only(x);
            }
            x.add8(0x7, 2);
            x.set16(TIMER, 0x14);
            x.set16(0x16, 0);
            lib::sound(x, 0xe5e2a);
            let a = x.u8(Obj::SPEED_INDEX) ^ 0x10;
            x.set8(Obj::SPEED_INDEX, a);
            speed(x, 0x35);
            strike(x, 2);
        }
        4 => {
            if sub16(x, TIMER, 1).gt() {
                x.move_x();
                return step_only(x);
            }
            x.add8(0x7, 2);
            x.set16(TIMER, 0x1e);
            let mut v: i16 = 0x2c;
            if x.u8(Obj::SPEED_INDEX) != 0x18 {
                v = -v;
            }
            x.set16(0x16, v as u16);
            strike(x, 3);
            x.move_x();
        }
        6 => {
            if sub16(x, TIMER, 1).gt() {
                x.move_x();
                return step_only(x);
            }
            finish(x, decide::chase);
        }
        _ => {}
    }
}

// --- +6 = 4 : geste ------------------------------------------------------------------------------

fn gesture<R: Bench>(x: &mut Cx<'_, R>) {
    match x.u8(0x7) {
        0 => {
            x.add8(0x7, 2);
            strike_anim(x, 0x12, 4);
        }
        2 => {
            if !step_with_spark(x) {
                return;
            }
            if !lib::players_alive(x) {
                return decide::wait(x);
            }
            if (x.u8(BUDGET_4) as i8) <= 0 {
                return decide::stand(x);
            }
            let players = var::PLAYERS_ALIVE.get(x.b) as u32;
            if players == 0 {
                return decide::wait(x);
            }
            let r = random(x) & 0xf;
            let pick = x.rom8(0x97d9c + ((players - 1) << 4) + r);
            if pick == 0 {
                return decide::stand(x);
            }
            if pick == 1 {
                // 0x9900a: the charge
                x.set8(VULNERABLE, 0);
                x.set8(SHOT, 0x2a);
                return x.set32(f::STATE, 0x0202_0800);
            }
            sub8(x, BUDGET_4, 1);
            x.set32(f::STATE, 0x0204_0400);
        }
        _ => {}
    }
}

// --- +6 = 6 : poursuite magique ------------------------------------------------------------------

fn chase_spell<R: Bench>(x: &mut Cx<'_, R>) {
    match x.u8(0x7) {
        0 => {
            x.add8(0x7, 2);
            x.set8(SHOT, 0x2b);
            lib::approach_target(x);
            speed(x, 0x21);
            x.set8(ANIM_KIND, 0x13);
            x.set8(ANIM_LAST, 0x13);
            body(x, 5);
        }
        2 => {
            if !sub16(x, CHASE_TIME, 1).gt() {
                return end_chase(x);
            }
            if !lib::facing_target(x) {
                return decide::interrupt(x);
            }
            if lib::target_in_box(x, -0x50, 0x20, 8) {
                x.add8(0x7, 2);
                return strike(x, 5);
            }
            if !sub8(x, AIM_TICK, 1).gt() {
                x.set8(AIM_TICK, 2);
                lib::approach_target(x);
                speed(x, 0x21);
            }
            x.move_by_table_xy();
            step_only(x);
        }
        4 => {
            if (x.u16(CHASE_TIME) as i16) > 0 {
                sub16(x, CHASE_TIME, 1);
            }
            if !step_with_spark(x) {
                return;
            }
            if (x.u16(CHASE_TIME) as i16) <= 0 {
                return end_chase(x);
            }
            if !lib::players_alive(x) {
                return decide::wait(x);
            }
            x.set8(0x7, 2);
            body(x, 5);
        }
        _ => {}
    }
}

fn end_chase<R: Bench>(x: &mut Cx<'_, R>) {
    x.set8(AIM_ON, 0);
    finish(x, decide::chase);
}

// --- +6 = 8: backward leap --------------------------------------------------------------------

fn hop_back<R: Bench>(x: &mut Cx<'_, R>) {
    match x.u8(0x7) {
        0 => {
            x.add8(0x7, 2);
            x.set16(0x18, 0x580);
            x.set16(0x1a, (-0x38i16) as u16);
            let (angle, vz) = if x.u8(f::FLIP) == 0 { (0x18, 0x10i16) } else { (8, -0x10) };
            x.set8(Obj::SPEED_INDEX, angle);
            x.set16(0x16, vz as u16);
            speed(x, 0x21);
            strike_anim(x, 0x14, 6);
        }
        2 => {
            x.move_xz();
            if tst16(x, 0x18).gt() {
                return step_only(x);
            }
            x.add8(0x7, 2);
            x.set16(0x16, 0);
            lib::sound(x, 0xe5e2a);
            strike(x, 7);
        }
        4 => {
            x.move_xz();
            if tst16(x, 0xc).gt() {
                return;
            }
            x.add8(0x7, 2);
            x.set16(TIMER, 0x1e);
            x.set16(0xc, 0);
            let mut v: i16 = 0x10;
            if x.u8(f::FLIP) != 0 {
                v = -v;
            }
            x.set16(0x16, v as u16);
            speed(x, 0xd);
            strike(x, 8);
        }
        6 => {
            if sub16(x, TIMER, 1).gt() {
                x.move_x();
                return step_only(x);
            }
            finish(x, decide::stand);
        }
        _ => {}
    }
}

// --- +6 = 10 : boule (ESHL 0x20) ----------------------------------------------------------------

fn ball<R: Bench>(x: &mut Cx<'_, R>) {
    match x.u8(0x7) {
        0 => {
            x.add8(0x7, 2);
            set_abs8(x, 0xff8043, 0x7f);
            sub8(x, BUDGET_A, 1);
            x.set16(TIMER, 1);
            strike_anim(x, 0x15, 9);
        }
        2 => {
            if sub16(x, TIMER, 1).gt() {
                return step_only(x);
            }
            x.add8(0x7, 2);
            x.set16(TIMER, 0x14);
            let Some(shot) = x.alloc(&pools::ESHL) else { return end_shot(x, decide::chase) };
            x.set16(CHILD, shot.0 as u16);
            shot.set_u8(x.b, 0, 1);
            shot.set_u8(x.b, 2, 0x20);
            shot.set_u8(x.b, 3, 0);
            shot.set_u16(x.b, 0x42, x.o.0 as u16);
            let target = x.u16(f::TARGET);
            shot.set_u16(x.b, f::TARGET, target);
            let side = x.u8(f::FLIP);
            shot.set_u8(x.b, f::FLIP, side);
            for off in [0x8u32, 0xc, 0x10] {
                let v = x.u16(off);
                shot.set_u16(x.b, off, v);
            }
            let dx: i16 = if side == 0 { -0x30 } else { 0x30 };
            let nx = shot.u16(x.b, 0x8).wrapping_add(dx as u16);
            shot.set_u16(x.b, 0x8, nx);
            shot.set_u16(x.b, 0xc, 0x5a);
            step_only(x);
        }
        4 => {
            if sub16(x, TIMER, 1).gt() {
                return step_only(x);
            }
            x.add8(0x7, 2);
            x.set16(TIMER, 0xa);
            let shot = x.linked(CHILD);
            shot.set_u8(x.b, 0x58, 1);
            lib::sound(x, 0xe5ffa);
            step_only(x);
        }
        6 => {
            let shot = x.linked(CHILD);
            if shot.u8(x.b, 0) == 0 || shot.u8(x.b, 0x5a) != 0 {
                return end_shot(x, decide::chase);
            }
            step_only(x);
        }
        _ => {}
    }
}

// --- +6 = 12 : rayon (ESHL 0x21) ----------------------------------------------------------------

fn beam<R: Bench>(x: &mut Cx<'_, R>) {
    match x.u8(0x7) {
        0 => {
            x.add8(0x7, 2);
            set_abs8(x, 0xff8043, 0x7f);
            sub8(x, BUDGET_B, 1);
            strike_anim(x, 0x16, 0xa);
        }
        2 => {
            step_only(x);
            if x.u8(f::KEY_FRAME) == 0 {
                return;
            }
            x.add8(0x7, 2);
            x.set16(TIMER, 0xa);
            x.set8(f::KEY_FRAME, 0);
            let Some(shot) = x.alloc(&pools::ESHL) else { return end_shot(x, decide::stand) };
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
            let dx: i16 = if side == 0 { -0x5e } else { 0x5e };
            let nx = shot.u16(x.b, 0x8).wrapping_add(dx as u16);
            shot.set_u16(x.b, 0x8, nx);
            let nz = shot.u16(x.b, 0xc).wrapping_add(0x28);
            shot.set_u16(x.b, 0xc, nz);
        }
        4 => {
            if sub16(x, TIMER, 1).gt() {
                return step_only(x);
            }
            x.add8(0x7, 2);
            x.set16(TIMER, 0x5a);
            let shot = x.linked(CHILD);
            shot.set_u8(x.b, 0x58, 1);
            lib::sound(x, 0xe606e);
            step_only(x);
        }
        6 => {
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
            if shot.u8(x.b, 0) == 0 || shot.u8(x.b, 0x5a) != 0 {
                x.add8(0x7, 2);
                x.set16(TIMER, 0xa);
            }
            step_only(x);
        }
        10 => {
            if sub16(x, TIMER, 1).gt() {
                return step_only(x);
            }
            end_shot(x, decide::stand);
        }
        _ => {}
    }
}

// --- +6 = 14 : onde de choc ----------------------------------------------------------------------

/// 0x9931a: a cloud (effect 0x32, variant 15) behind the Lich.
fn dust<R: Bench>(x: &mut Cx<'_, R>) {
    let Some(e) = x.alloc(&pools::EFFCT) else { return };
    e.set_u8(x.b, 0, 1);
    e.set_u8(x.b, 2, 0x32);
    e.set_u8(x.b, 3, 0xf);
    let (px, pz) = (x.u16(0x8), x.u16(0xc));
    e.set_u16(x.b, 0x8, px);
    e.set_u16(x.b, 0xc, pz);
    let py = x.u32(0x10);
    e.set_u32(x.b, 0x10, py);
    let mut d: i16 = 4;
    if x.u16(0x2c) != 0 {
        d = -d;
    }
    let nx = e.u16(x.b, 0x8).wrapping_add(d as u16);
    e.set_u16(x.b, 0x8, nx);
    let nz = e.u16(x.b, 0xc).wrapping_add(0x40);
    e.set_u16(x.b, 0xc, nz);
    let ny = e.u32(x.b, 0x10).wrapping_sub(1);
    e.set_u32(x.b, 0x10, ny);
}

/// 0x9921c: on the third pulse, players on the ground who are not already down are knocked over.
fn topple_all<R: Bench>(x: &mut Cx<'_, R>) {
    if x.u16(COUNT) != 3 {
        return;
    }
    for k in 0..4u32 {
        let p = Obj(0xff85de + 0x200 * k);
        if p.u8(x.b, 0) != 1 || (p.u16(x.b, 0xc) as i16) > 0 || p.u8(x.b, 0x66) != 0 {
            continue;
        }
        let state = p.u32(x.b, 0x4) & 0xffff_ff00;
        if state == 0x0200_2200 || (state == 0x0200_1400 && p.u8(x.b, 0x8b) == 4) || state == 0x0204_0e00 {
            continue;
        }
        p.set_u32(x.b, 0x4, 0x0204_0e00);
    }
}

/// 0x99274: drops projectiles around the camera (0x5f764) as long as none is in play.
fn spawn_blast<R: Bench>(x: &mut Cx<'_, R>) {
    if (x.u8(SPAWNED) as i8) > 0 {
        return;
    }
    lib::sound(x, 0xe6156);
    x.set8(SPAWNED, 0);
    let level = (var::RANK.get(x.b) & 0x1e) as u32 * 2;
    let players = var::PLAYERS_ALIVE.get(x.b) as u32;
    let mut d5 = x.rom8(0x992ca + level + players.saturating_sub(1)) as i8 as i16 as u16;
    loop {
        let i = (d5 as u32).wrapping_mul(4) & 0xffff;
        let dx = x.rom16(0x9930a + i);
        let dy = x.rom16(0x9930c + i);
        let px = dx.wrapping_add(var::CAMERA1_X.get(x.b));
        let py = dy.wrapping_add(var::CAMERA1_Y.get(x.b));
        // (D0, D1, D2) = (x, 0, y) ; D2 (mot bas) reste
        *x.c.d2() = *x.c.d2() & 0xffff_0000 | py as u32;
        if crate::families::creators::enemy_5_at_pure(x.b, x.o, (px, 0, py)).is_some() {
            x.add8(SPAWNED, 1);
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
    match x.u8(0x7) {
        0 => {
            x.add8(0x7, 2);
            lib::sound(x, 0xe5e9e);
            set_abs8(x, 0xff8043, 0x7f);
            sub8(x, BUDGET_C, 1);
            x.set16(TIMER, 6);
            strike_anim(x, 0x17, 0xb);
        }
        2 => {
            if sub16(x, TIMER, 1).gt() {
                return step_only(x);
            }
            x.add8(0x7, 2);
            dust(x);
            x.set16(TIMER, 6);
            x.set16(COUNT, 4);
            step_only(x);
        }
        4 => {
            if !sub16(x, TIMER, 1).gt() {
                let n = x.u16(COUNT).wrapping_sub(1);
                x.set16(COUNT, n);
                if n & 0x8000 != 0 {
                    x.add8(0x7, 2);
                    return strike(x, 0xc);
                }
                match n {
                    3 => {
                        x.set16(TIMER, 0x20);
                        return lib::shake_54(x, 0x20);
                    }
                    2 => {
                        x.set16(TIMER, 0x14);
                        lib::shake_5c(x, 0x14);
                    }
                    1 => {
                        x.set8(INVULNERABLE, 0);
                        x.set16(TIMER, 0x14);
                        lib::shake_58(x, 0x14);
                    }
                    _ => {
                        x.set16(TIMER, 0xa);
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
            x.add8(0x7, 2);
            set_abs8(x, 0xff8043, 0xff);
            spawn_blast(x);
            x.set16(TIMER, 0x14);
            body(x, 2);
        }
        8 => {
            if sub16(x, TIMER, 1).gt() {
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
    let here = x.u16(0x8);
    let mut d5 = 0u32;
    if Fl::sub16(cam, here).lt() {
        if !Fl::sub16(cam.wrapping_add(0x70), here).lt() {
            d5 = 0x10;
        }
    } else if !Fl::sub16(cam.wrapping_sub(0x70), here).ge() {
        d5 = 0x20;
    }
    let camy = var::CAMERA1_Y.get(x.b).wrapping_add(0x54);
    let y = x.u16(0x10);
    if Fl::sub16(camy, y).lt() {
        if !Fl::sub16(camy.wrapping_add(0x16), y).lt() {
            d5 |= 4;
        }
    } else if !Fl::sub16(camy.wrapping_sub(0x16), y).ge() {
        d5 |= 8;
    }
    let nx = x.rom16(0x991ec + d5);
    let ny = x.rom16(0x991ee + d5);
    x.set16(0x8, nx);
    x.set16(0x10, ny);
    x.set16(0xc, 0);
}

fn teleport<R: Bench>(x: &mut Cx<'_, R>) {
    match x.u8(0x7) {
        0 => {
            x.add8(0x7, 2);
            lib::sound(x, 0xe60e2);
            strike_anim(x, 0x18, 0xd);
        }
        2 => {
            if !lib::step(x) {
                return;
            }
            x.add8(0x7, 2);
            x.set16(TIMER, 4);
            x.set_bit(0x1, 1, true);
        }
        4 => {
            if sub16(x, TIMER, 1).gt() {
                return;
            }
            x.add8(0x7, 2);
            place_after_teleport(x);
            x.set_bit(0x1, 1, false);
            lib::sound(x, 0xecf10);
            strike(x, 0xe);
        }
        6 => {
            if !lib::step(x) {
                return;
            }
            x.add8(0x7, 2);
            x.set16(TIMER, 4);
            body(x, 2);
        }
        8 => {
            if sub16(x, TIMER, 1).gt() {
                return;
            }
            finish(x, decide::chase);
        }
        _ => {}
    }
}

// --- +6 = 18 : pluie d'effets ----------------------------------------------------------------------

fn rain<R: Bench>(x: &mut Cx<'_, R>) {
    match x.u8(0x7) {
        0 => {
            x.add8(0x7, 2);
            set_abs8(x, 0xff8043, 0x7f);
            sub8(x, BUDGET_E, 1);
            x.set16(TIMER, 0xa);
            lib::sound(x, 0xe5e9e);
            strike_anim(x, 0x19, 0xb);
        }
        2 => {
            if sub16(x, TIMER, 1).gt() {
                return step_only(x);
            }
            x.add8(0x7, 2);
            x.set8(INVULNERABLE, 0);
            x.set16(TIMER, 0x5a);
            for n in (0..=0xfu8).rev() {
                let Some(e) = x.alloc(&pools::ESHL) else { break };
                e.set_u8(x.b, 0, 1);
                e.set_u8(x.b, 2, 0x23);
                e.set_u8(x.b, 3, n);
                e.set_u16(x.b, 0x42, x.o.0 as u16);
                let side = x.u8(f::FLIP);
                e.set_u8(x.b, f::FLIP, side);
                let (px, py) = (x.u16(0x8), x.u16(0x10));
                e.set_u16(x.b, 0x8, px);
                e.set_u16(x.b, 0x10, py);
            }
            step_only(x);
        }
        4 => {
            if sub16(x, TIMER, 1).gt() {
                return step_only(x);
            }
            x.add8(0x7, 2);
            strike(x, 0xc);
        }
        6 => {
            if !lib::step(x) {
                return;
            }
            x.add8(0x7, 2);
            x.set16(TIMER, 4);
            body(x, 2);
        }
        8 => {
            if sub16(x, TIMER, 1).gt() {
                return step_only(x);
            }
            end_shot(x, decide::chase);
        }
        _ => {}
    }
}

// --- +6 = 20 : tirs successifs ----------------------------------------------------------------------

fn volley<R: Bench>(x: &mut Cx<'_, R>) {
    match x.u8(0x7) {
        0 => {
            x.add8(0x7, 2);
            set_abs8(x, 0xff8043, 0x7f);
            sub8(x, BUDGET_F, 1);
            let level = (var::RANK.get(x.b) & 0x1e) as u32 * 4;
            let players = var::PLAYERS_ALIVE.get(x.b) as u32;
            let i = level + 2 * players.saturating_sub(1);
            let n = x.rom16(0x9852e + i);
            x.set16(COUNT, n);
            x.set16(TIMER, 0xe);
            dust(x);
            lib::sound(x, 0xe5e9e);
            strike_anim(x, 0x1a, 0xb);
        }
        2 => {
            if sub16(x, TIMER, 1).gt() {
                return step_only(x);
            }
            let n = x.u16(COUNT).wrapping_sub(1);
            x.set16(COUNT, n);
            if n & 0x8000 != 0 {
                x.add8(0x7, 2);
                x.set8(INVULNERABLE, 0);
                return strike(x, 0xc);
            }
            if let Some(e) = x.alloc(&pools::ESHL) {
                e.set_u8(x.b, 0, 1);
                e.set_u8(x.b, 2, 0x24);
                e.set_u16(x.b, 0x42, x.o.0 as u16);
                let side = x.u8(f::FLIP) ^ 0x20;
                e.set_u8(x.b, f::FLIP, side);
            }
            x.set16(TIMER, 0x14);
            step_only(x);
        }
        4 => {
            if !lib::step(x) {
                return;
            }
            x.add8(0x7, 2);
            x.set16(TIMER, 0x14);
            body(x, 2);
        }
        6 => {
            if sub16(x, TIMER, 1).gt() {
                return step_only(x);
            }
            end_shot(x, decide::stand);
        }
        _ => {}
    }
}

use crate::objects::Obj;
use crate::objects::Current;

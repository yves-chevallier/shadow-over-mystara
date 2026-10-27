//! Boss 18, groupe `+5` = 4 : les attaques (`+6` = 0 à 20), chacune en étapes `+7`.
//!
//! | `+6` | Attaque |
//! |---|---|
//! | 0 | coup simple (animation 0 des coups) |
//! | 2 | pas de côté (quatre temps, vitesse du pas `+0x16`) |
//! | 4 | geste puis suite tirée selon le nombre de joueurs (attaque 4 au plus `+0xae` fois) |
//! | 6 | poursuite magique (`+0xb8` images, visée toutes les 2) |
//! | 8 | bond en arrière avec retombée |
//! | 10, 12 | tir ESHL 0x20 (boule) ou 0x21 (rayon) |
//! | 14 | onde de choc : les joueurs au sol sont renversés |
//! | 16 | téléportation (disparaît, réapparaît ailleurs) |
//! | 18 | pluie de 16 tirs ESHL 0x23 |
//! | 20 | tirs successifs ESHL 0x24 (nombre tiré d'une table) |

use super::decide;
use super::*;
use crate::objects::pools;

pub(super) fn run(x: &mut Cx) {
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

fn strike_anim(x: &mut Cx, kind: u8, n: u32) {
    x.set8(ANIM_KIND, kind);
    x.set8(ANIM_LAST, kind);
    strike(x, n);
}

/// Fin d'attaque : l'absence de joueurs l'emporte ; sinon `to`.
fn finish(x: &mut Cx, to: fn(&mut Cx)) {
    if !lib::any_player_alive(x) {
        return decide::wait(x);
    }
    to(x);
}

fn step_only(x: &mut Cx) {
    lib::step(x);
}

/// Consomme le bit 0 de l'image-clé `+0x33`.
fn key_bit0(x: &mut Cx) -> bool {
    if x.u8(f::KEY_FRAME) & 1 == 0 {
        return false;
    }
    let k = x.u8(f::KEY_FRAME) & !1;
    x.set8(f::KEY_FRAME, k);
    true
}

/// Fin d'un tir : l'état de la scène est rendu (`A5+0x43`), puis `to`.
fn end_shot(x: &mut Cx, to: fn(&mut Cx)) {
    set_abs8(x, 0xff8043, 0xff);
    finish(x, to);
}

/// Un pas de l'animation, plus un dernier effet par image-clé (0xe5e2a).
fn step_with_spark(x: &mut Cx) -> bool {
    if !lib::step(x) {
        if key_bit0(x) {
            lib::sound(x, 0xe5e2a);
        }
        return false;
    }
    true
}

// --- +6 = 0 : coup simple -----------------------------------------------------------------------

fn strike_0(x: &mut Cx) {
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

// --- +6 = 2 : pas de côté -----------------------------------------------------------------------

fn sidestep(x: &mut Cx) {
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

fn gesture(x: &mut Cx) {
    match x.u8(0x7) {
        0 => {
            x.add8(0x7, 2);
            strike_anim(x, 0x12, 4);
        }
        2 => {
            if !step_with_spark(x) {
                return;
            }
            if !lib::any_player_alive(x) {
                return decide::wait(x);
            }
            if (x.u8(BUDGET_4) as i8) <= 0 {
                return decide::stand(x);
            }
            let players = x.a5_u8(var::PLAYERS_ALIVE) as u32;
            if players == 0 {
                return decide::wait(x);
            }
            let r = random(x) & 0xf;
            let pick = x.rom8(0x97d9c + ((players - 1) << 4) + r);
            if pick == 0 {
                return decide::stand(x);
            }
            if pick == 1 {
                // 0x9900a : la charge
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

fn chase_spell(x: &mut Cx) {
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
            if !lib::faces_target(x) {
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
            if !lib::any_player_alive(x) {
                return decide::wait(x);
            }
            x.set8(0x7, 2);
            body(x, 5);
        }
        _ => {}
    }
}

fn end_chase(x: &mut Cx) {
    x.set8(AIM_ON, 0);
    finish(x, decide::chase);
}

// --- +6 = 8 : bond en arrière --------------------------------------------------------------------

fn hop_back(x: &mut Cx) {
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

fn ball(x: &mut Cx) {
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

fn beam(x: &mut Cx) {
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

/// 0x9931a : un nuage (effet 0x32, variante 15) derrière le Lich.
fn dust(x: &mut Cx) {
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

/// 0x9921c : à la troisième pulsation, les joueurs au sol qui ne sont pas déjà à terre sont renversés.
fn topple_all(x: &mut Cx) {
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

/// 0x99274 : lâche des projectiles autour de la caméra (0x5f764) tant qu'aucun n'est en jeu.
fn spawn_blast(x: &mut Cx) {
    if (x.u8(SPAWNED) as i8) > 0 {
        return;
    }
    lib::sound(x, 0xe6156);
    x.set8(SPAWNED, 0);
    let level = (x.a5_u16(var::DIFFICULTY) & 0x1e) as u32 * 2;
    let players = x.a5_u8(var::PLAYERS_ALIVE) as u32;
    let mut d5 = x.rom8(0x992ca + level + players.saturating_sub(1)) as i8 as i16 as u16;
    loop {
        let i = (d5 as u32).wrapping_mul(4) & 0xffff;
        let dx = x.rom16(0x9930a + i);
        let dy = x.rom16(0x9930c + i);
        let px = dx.wrapping_add(x.a5_u16(var::CAMERA1_X));
        let py = dy.wrapping_add(x.a5_u16(var::CAMERA1_Y));
        set_dw(x, 0, px);
        set_dw(x, 2, py);
        x.c.d[1] = 0;
        x.call(0x5f764);
        if !carry(x) {
            x.add8(SPAWNED, 1);
        }
        if d5 == 0 {
            break;
        }
        d5 = d5.wrapping_sub(1);
    }
}

fn shockwave(x: &mut Cx) {
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

// --- +6 = 16 : téléportation ----------------------------------------------------------------------

/// 0x99184 : réapparition dans l'un des neuf points autour de la caméra, selon la position du Lich.
fn place_after_teleport(x: &mut Cx) {
    let cam = x.a5_u16(var::CAMERA1_X).wrapping_add(0xc0);
    let here = x.u16(0x8);
    let mut d5 = 0u32;
    if Fl::sub16(cam, here).lt() {
        if !Fl::sub16(cam.wrapping_add(0x70), here).lt() {
            d5 = 0x10;
        }
    } else if !Fl::sub16(cam.wrapping_sub(0x70), here).ge() {
        d5 = 0x20;
    }
    let camy = x.a5_u16(var::CAMERA1_Y).wrapping_add(0x54);
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

fn teleport(x: &mut Cx) {
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

fn rain(x: &mut Cx) {
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

fn volley(x: &mut Cx) {
    match x.u8(0x7) {
        0 => {
            x.add8(0x7, 2);
            set_abs8(x, 0xff8043, 0x7f);
            sub8(x, BUDGET_F, 1);
            let level = (x.a5_u16(var::DIFFICULTY) & 0x1e) as u32 * 4;
            let players = x.a5_u8(var::PLAYERS_ALIVE) as u32;
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
use crate::objects::Courant;

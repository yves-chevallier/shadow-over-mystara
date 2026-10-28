//! Boss 3, groupe 4 : les attaques (`+6` = 0 à 30, table 0x7fd0c). Chacune finit par `after_attack` (0x81462).
//!
//! | `+6` | Attaque |
//! |---|---|
//! | 0, 6 à 14 | coup de griffes (animation 3) |
//! | 2 | coup de griffes court (animation 4) |
//! | 4 | double coup de griffes, le boss avance aux images-clés |
//! | 16 | bond : montée à 0x70, puis plongée en avant |
//! | 18, 22 | cris (0xb3e4e) dont le dernier repousse les joueurs devant lui |
//! | 20 | rase-mottes : ruée en avant jusqu'à frôler un joueur, coup, remontée |
//! | 24 | ruée au sol à hauteur 0x50, puis plongée |
//! | 26 | cri sur place (0xa2d22) |
//! | 28 | vol de côté vers le milieu de l'écran en repoussant les joueurs, avec cris |
//! | 30 | enlèvement : monte hors de l'écran, fond sur un joueur et l'emporte (groupe 12) |

use crate::families::cx::Banc;
use super::body::push_players;
use super::*;
use crate::objects::Courant;

pub(super) fn run(x: &mut Cx) {
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

/// Début d'un coup : vitesses à zéro, animation `n` du jeu des attaques, cri (0xebfdc).
fn swing<R: Banc>(x: &mut Cx<'_, R>, n: u32) {
    next7(x);
    x.set8(0x44, 0);
    x.set8(0x45, 0x10);
    stop(x);
    attack_anim(x, n);
    lib::sound(x, 0xebfdc);
}

/// Pas en avant aux images-clés (`+0x33`) : décalage lu dans la table `table` (relative au PC), vers l'avant.
fn key_step<R: Banc>(x: &mut Cx<'_, R>, table: u32, clear: bool) {
    let key = x.u8(f::KEY_FRAME) as u32;
    if clear {
        if key == 0 {
            return;
        }
        x.set8(f::KEY_FRAME, 0);
    }
    let d = x.rom8(table + key) as i8 as i16 as u16;
    if d == 0 {
        return;
    }
    let d = if x.u8(f::FLIP) != 0 { d } else { d.wrapping_neg() };
    x.add16(0x8, d);
}

/// 0x7fdee : coup de griffes.
fn claw(x: &mut Cx) {
    if x.u8(0x7) == 0 {
        return swing(x, 3);
    }
    if !lib::step(x) {
        return key_step(x, 0x7fdea, true);
    }
    after_attack(x)
}

/// 0x7fe34 : coup de griffes court.
fn short_claw(x: &mut Cx) {
    if x.u8(0x7) == 0 {
        return swing(x, 4);
    }
    if lib::step(x) {
        after_attack(x)
    }
}

/// 0x7fd2c : double coup de griffes.
fn double_claw(x: &mut Cx) {
    match x.u8(0x7) {
        0 => swing(x, 3),
        2 => {
            if !lib::step(x) {
                return key_step(x, 0x7fdea, true);
            }
            next7(x);
            x.set8(0x44, 0);
            x.set8(0x45, 0x10);
            attack_anim(x, 9);
            lib::sound(x, 0xebfdc);
        }
        _ => {
            if lib::step(x) {
                return after_attack(x);
            }
            key_step(x, 0x7fdbc, false)
        }
    }
}

/// 0x7fe7a : bond : montée à 0x70, retombée en avant, glissade.
fn leap(x: &mut Cx) {
    match x.u8(0x7) {
        0 => {
            next7(x);
            stop(x);
            x.set16(0x18, 0x800);
            x.set8(0x44, 0);
            x.set8(0x45, 0x10);
            attack_anim(x, 0);
        }
        2 => {
            lib::step(x);
            if x.bit(f::KEY_FRAME, 0) {
                return;
            }
            x.move_z();
            if (x.u16(0xc) as i16) < 0x70 {
                return;
            }
            x.set16(0xc, 0x70);
            next7(x);
            let v = (x.u16(0x18) as i16) >> 1;
            x.set16(0x18, v as u16);
            x.set8(0x44, 0);
            x.set8(0x45, 0x10);
            attack_anim(x, 1);
        }
        4 => {
            x.set8(INVULNERABLE, 1);
            lib::step(x);
            if x.bit(f::KEY_FRAME, 0) {
                x.move_z();
                if (x.u16(0x18) as i16) <= 0 {
                    x.set32(0x18, 0);
                }
                return;
            }
            next7(x);
            x.set8(INVULNERABLE, 8);
            x.set16(0x18, (-0x500i16) as u16);
            let (mut vx, mut vz) = (0x600u16, 0x40u16);
            if x.u8(f::FLIP) == 0 {
                vx = vx.wrapping_neg();
                vz = vz.wrapping_neg();
            }
            x.set16(0x14, vx);
            x.set16(0x16, vz);
        }
        6 => {
            lib::step(x);
            x.move_xyz();
            if (x.u16(0xc) as i16) > 0 {
                return;
            }
            x.set16(0xc, 0);
            next7(x);
            let v = ((x.u16(0x14) as i16) >> 3) as u16;
            x.set16(0x14, v);
            x.set16(0x16, 0);
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

/// Image-clé marquée (bit 7 de `+0x33`) : vrai et marque effacée.
fn marked_key<R: Banc>(x: &mut Cx<'_, R>) -> bool {
    if (x.u8(f::KEY_FRAME) as i8) >= 0 {
        return false;
    }
    x.set8(f::KEY_FRAME, 0);
    true
}

/// 0x7ffbc : un cri (animation 8), puis quatre autres qui repoussent les joueurs devant lui.
fn cries(x: &mut Cx) {
    match x.u8(0x7) {
        0 | 2 => {
            if x.u8(0x7) == 0 {
                next7(x);
                x.set8(TIMER, 1);
                attack_anim(x, 8);
            }
            lib::step(x);
            if !marked_key(x) || sub8(x, TIMER, 1).gt() {
                return;
            }
            crate::families::effects_creators::shouts(x.b, x.o, false);
            next7(x);
            x.set8(TIMER, 4);
            lib::sound(x, 0xedb82);
        }
        _ => {
            push_players(x, 1);
            lib::step(x);
            if !marked_key(x) {
                return;
            }
            crate::families::effects_creators::shouts(x.b, x.o, false);
            if sub8(x, TIMER, 1).gt() {
                return lib::sound(x, 0xedb82);
            }
            after_attack(x)
        }
    }
}

/// 0x80130 : distance en X au joueur présent le plus proche (0x7fff sans joueur).
fn nearest_player_dx<R: Banc>(x: &mut Cx<'_, R>) -> u16 {
    let mut best = 0x7fffu16;
    for k in 0..4u32 {
        let p = Obj(0xff85de + 0x200 * k);
        if p.u8(x.b, 0) == 0 {
            continue;
        }
        let mut d = p.u16(x.b, 0x8).wrapping_sub(x.u16(0x8));
        if (d as i16) < 0 {
            d = d.wrapping_neg();
        }
        if (best as i16) > d as i16 {
            best = d;
        }
    }
    best
}

/// 0x801ca : lancé à pleine vitesse (0x900 ou plus), il ne dérive plus en profondeur.
fn no_drift_when_fast<R: Banc>(x: &mut Cx<'_, R>) {
    let mut v = x.u16(0x14);
    if (v as i16) < 0 {
        v = v.wrapping_neg();
    }
    if v as i16 >= 0x900 {
        x.set16(0x16, 0);
    }
}

/// 0x8003e : rase-mottes : ruée en avant, coup (animation 6) au premier joueur frôlé, remontée si le coup porte.
fn skim(x: &mut Cx) {
    match x.u8(0x7) {
        0 => {
            next7(x);
            x.set8(DIVE, 0);
            x.set8(0x44, 0);
            x.set8(0x45, 0x10);
            x.set8(0x57, 0);
            stop(x);
            let (mut vx, mut vz) = (0x600u16, 0x20u16);
            if x.u8(f::FLIP) == 0 {
                vx = vx.wrapping_neg();
                vz = vz.wrapping_neg();
            }
            x.set16(0x14, vx);
            x.set16(0x16, vz);
            x.set8(0x44, 0);
            x.set8(0x45, 0x10);
            attack_anim(x, 5);
        }
        2 => {
            lib::step(x);
            if x.u8(f::KEY_FRAME) != 0 {
                return;
            }
            x.set8(TIMER, 0x20);
            next7(x);
        }
        4 => {
            if (x.u8(DIVE) as i8) < 4 && !sub8(x, TIMER, 1).gt() {
                return skim_end(x);
            }
            match x.u8(DIVE) {
                0 => {
                    lib::step(x);
                    x.move_xy();
                    no_drift_when_fast(x);
                    if nearest_player_dx(x) as i16 >= 0x30 {
                        return;
                    }
                    x.add8(DIVE, 2);
                    attack_anim(x, 6);
                }
                2 => {
                    lib::step(x);
                    x.move_xy();
                    no_drift_when_fast(x);
                    if !x.bit(0x57, 2) {
                        return;
                    }
                    x.add8(DIVE, 2);
                    stop(x);
                    let mut v = (-0x400i16) as u16;
                    if x.u8(f::FLIP) == 0 {
                        v = v.wrapping_neg();
                    }
                    x.set16(0x14, v);
                    x.set16(0x18, 0x400);
                    x.set16(0x1a, (-0x40i16) as u16);
                }
                _ => {
                    lib::step(x);
                    x.move_xz();
                    if (x.u16(0xc) as i16) >= 0 {
                        return;
                    }
                    x.set16(0xc, 0);
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

/// 0x800e0 : fin du rase-mottes (animation 7).
fn skim_end<R: Banc>(x: &mut Cx<'_, R>) {
    stop(x);
    let v = ((x.u16(0x14) as i16) >> 2) as u16;
    x.set16(0x14, v);
    x.set16(0x16, 0);
    next7(x);
    attack_anim(x, 7);
}

/// 0x8030e : vitesse en X ramenée à ±0x800 si elle la dépasse (et plus de dérive en profondeur).
fn cap_speed<R: Banc>(x: &mut Cx<'_, R>) {
    let v = x.u16(0x14);
    let (cap, abs) = if (v as i16) < 0 { (0xf800u16, v.wrapping_neg()) } else { (0x800, v) };
    if abs as i16 <= 0x800 {
        return;
    }
    x.set16(0x14, cap);
    x.set16(0x16, 0);
}

/// 0x801de : ruée : monte ou descend à hauteur 0x50, plane, puis plonge vers l'avant.
fn rush(x: &mut Cx) {
    match x.u8(0x7) {
        0 | 2 => {
            if x.u8(0x7) == 0 {
                next7(x);
                x.set8(0x44, 0);
                x.set8(0x45, 0x10);
                x.set8(HOVER, 0);
                body_anim(x, 1);
            }
            x.set8(HOVER, 0);
            lib::step(x);
            x.set8(INVULNERABLE, 1);
            let d = 0x50u16.wrapping_sub(x.u16(0xc));
            if d == 0 {
                next7(x);
                x.set8(TIMER, 0x10);
                return;
            }
            let v: i16 = if (d as i16) < 0 {
                if (d as i16) < -2 { -4 } else { -1 }
            } else if d as i16 > 2 {
                4
            } else {
                1
            };
            x.add16(0xc, v as u16);
        }
        4 => {
            if sub8(x, TIMER, 1).gt() {
                x.set8(HOVER, 0);
                lib::step(x);
                return;
            }
            next7(x);
            stop(x);
            x.set16(0x18, (-0x200i16) as u16);
            let (angle, mut vx, mut vz) = if x.u8(f::FLIP) != 0 { (8u8, 0x600u16, 0x40u16) } else { (0x18, 0x600, 0x40) };
            if x.u8(f::FLIP) == 0 {
                vx = vx.wrapping_neg();
                vz = vz.wrapping_neg();
            }
            x.set8(Obj::SPEED_INDEX, angle);
            x.set16(0x14, vx);
            x.set16(0x16, vz);
            speed(x, 0x1f);
            attack_anim(x, 0xa);
        }
        6 => {
            lib::step(x);
            x.move_xyz();
            cap_speed(x);
            if (x.u16(0xc) as i16) > 8 {
                return;
            }
            x.set16(0xc, 8);
            x.set32(0x18, 0);
            next7(x);
            x.set8(TIMER, 8);
            attack_anim(x, 0xb);
        }
        _ => {
            lib::step(x);
            x.move_xyz();
            cap_speed(x);
            if sub8(x, TIMER, 1).gt() {
                return;
            }
            after_attack(x)
        }
    }
}

/// 0x80330 : cri sur place : montée, cri (0xa2d22, 0xedbf4) aux images-clés.
fn cry(x: &mut Cx) {
    match x.u8(0x7) {
        0 | 2 => {
            if x.u8(0x7) == 0 {
                next7(x);
                stop(x);
                x.set16(0x18, 0x200);
                attack_anim(x, 0xc);
            }
            x.move_z();
            if !lib::step(x) {
                return;
            }
            next7(x);
            x.set16(0x18, 0);
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
            lib::sound(x, 0xedbf4);
        }
    }
}

/// 0x804b0 : cri (0xb3ec6, 0xedb82).
fn shriek<R: Banc>(x: &mut Cx<'_, R>) {
    crate::families::effects_creators::shouts(x.b, x.o, true);
    lib::sound(x, 0xedb82);
}

/// 0x803aa : vol de côté à hauteur 0x60 vers le milieu de l'écran (corps de côté : il écarte les joueurs).
fn side_flight(x: &mut Cx) {
    match x.u8(0x7) {
        0 => {
            next7(x);
            x.set8(INVULNERABLE, 1);
            x.set8(TIMER, 2);
            stop(x);
            let mut v = 0x400u16;
            let h = x.u16(0xc);
            if h as i16 > 0x60 {
                v = v.wrapping_neg();
            }
            set_dw(x, 0, v);
            x.set16(0x18, v);
            attack_anim(x, 0xe);
        }
        2 => {
            x.set8(INVULNERABLE, 1);
            x.move_z();
            let mut d = x.u16(0xc).wrapping_sub(0x60);
            if (x.u16(0x18) as i16) < 0 {
                d = d.wrapping_neg();
            }
            set_dw(x, 0, d);
            if (d as i16) < 0 {
                return;
            }
            next7(x);
            x.set8(DIVE, 0);
            x.set8(TIMER, 4);
            shriek(x)
        }
        4 => {
            let cam = camera(x);
            let mut d = ((cam.u16(x.b, 0x8).wrapping_add(0xc0).wrapping_sub(x.u16(0x8)) as i16) >> 4) as u16;
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
            set_dw(x, 0, d);
            x.add16(0x8, d);
            push_players(x, 2);
            lib::step(x);
            if x.u8(f::KEY_FRAME) == 0 {
                return;
            }
            x.set8(f::KEY_FRAME, 0);
            if sub8(x, TIMER, 1).gt() {
                return shriek(x);
            }
            next7(x);
            stop(x);
            x.set16(0x18, 0x200);
            x.set16(0x1a, (-0x40i16) as u16);
            attack_anim(x, 0xf);
            descend_to_0x40(x)
        }
        _ => descend_to_0x40(x),
    }
}

/// 0x80496 : redescend à hauteur 0x40, puis fin de l'attaque.
fn descend_to_0x40(x: &mut Cx) {
    x.move_z();
    if (x.u16(0xc) as i16) > 0x40 {
        return;
    }
    x.set16(0xc, 0x40);
    after_attack(x)
}

/// Hauteur au-dessus du sol de la caméra : `z - caméra + hauteur`.
fn height_over_camera<R: Banc>(x: &mut Cx<'_, R>) -> i16 {
    let cam = camera(x);
    let h = x.u16(0x10).wrapping_sub(cam.u16(x.b, 0xc)).wrapping_add(x.u16(0xc));
    h as i16
}

/// 0x8069a : un joueur à portée est saisi (0xfc44, 0xfd64 : groupe 12) ; vrai si saisi.
fn try_grab<R: Banc>(x: &mut Cx<'_, R>) -> bool {
    let Some(player) = lib::attack_hits_a_player(x, 1) else {
        return false;
    };
    lib::seize_player(x, player, 2);
    true
}

/// 0x804ba : enlèvement : monte hors de l'écran, attend, fond sur la cible en saisissant au passage, remonte.
fn abduction(x: &mut Cx) {
    match x.u8(0x7) {
        0 | 2 => {
            if x.u8(0x7) == 0 {
                next7(x);
                x.set8(TIMER, 2);
                stop(x);
                x.set16(0x18, 0x600);
                attack_anim(x, 0x10);
            }
            x.move_z();
            if height_over_camera(x) <= 0x100 {
                return;
            }
            next7(x);
            x.set8(TIMER, 0x20);
            x.set16(f::IMMUNITY, 0xffff);
        }
        4 => {
            if sub8(x, TIMER, 1).gt() {
                return;
            }
            next7(x);
            x.set_bit(0x1, 1, false);
            let cam = camera(x);
            stop(x);
            let mut pz = cam.u16(x.b, 0x10).wrapping_add(0x50);
            // l'original écrase X par le mot de la cible avant de la tester : sans cible, X = 0
            let mut px = x.u16(f::TARGET);
            if (px as i16) < 0 {
                let t = load_target(x);
                px = t.u16(x.b, 0x8);
                pz = t.u16(x.b, 0x10);
            }
            x.set16(0x10, pz);
            x.set16(0x8, px);
            let (mut side, mut dx, mut vx) = (0x20u32, (-0x80i16) as u16, 0x800u16);
            let rel = px.wrapping_sub(cam.u16(x.b, 0x8));
            if (rel as i16) < 0x100 {
                side = 0;
                dx = dx.wrapping_neg();
                vx = vx.wrapping_neg();
            }
            x.set8(f::FLIP, side as u8);
            x.add16(0x8, dx);
            x.set16(0xc, 0x80);
            x.set16(0x14, vx);
            x.set16(0x18, (-0x800i16) as u16);
            x.set16(0x1a, 0x20);
            attack_anim(x, 0x11);
            lib::set_immunities(x);
        }
        6 => {
            if try_grab(x) {
                return;
            }
            x.move_xz();
            if (x.u16(0xc) as i16) > 0x10 {
                return;
            }
            x.set16(0xc, 0x10);
            next7(x);
            x.set32(0x18, 0);
            x.set8(TIMER, 8);
        }
        8 => {
            if try_grab(x) {
                return;
            }
            x.move_xz();
            if sub8(x, TIMER, 1).gt() {
                return;
            }
            next7(x);
            stop(x);
            let mut v = 0x600u16;
            if x.u8(f::FLIP) == 0 {
                v = v.wrapping_neg();
            }
            x.set16(0x14, v);
            x.set16(0x18, 0x400);
            x.set16(0x1a, 0x40);
            attack_anim(x, 0x12);
        }
        10 => {
            x.move_xz();
            if height_over_camera(x) <= 0x100 {
                return;
            }
            next7(x);
            x.set8(TIMER, 0x20);
            stop(x);
            x.set16(0x18, (-0x400i16) as u16);
            body_anim(x, 1);
        }
        _ => {
            if tst8(x, TIMER).gt() {
                sub8(x, TIMER, 1);
                return;
            }
            x.set8(HOVER, 0);
            x.move_xz();
            if (x.u16(0xc) as i16) > 0x40 {
                return;
            }
            x.set16(0xc, 0x40);
            after_attack(x)
        }
    }
}

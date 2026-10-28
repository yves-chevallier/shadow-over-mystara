//! Boss 4, groupe 4 : les attaques (`+6` = 0 à 12), chacune en étapes `+7` = 0, 2, 4...
//!
//! | `+6` | Rôle |
//! |---|---|
//! | 0 | cri (animation de vol 0) |
//! | 2 | envol hors de l'écran, puis piqué sur la cible |
//! | 4 | envol, puis traversée de l'écran au ras du sol |
//! | 6 | bond par-dessus la cible |
//! | 8 | envol, souffle en vol (jets ESHL 0x32 en éventail), descente |
//! | 10 | geste du premier double (deux signaux) |
//! | 12 | souffle au sol (jets ESHL 0x32 variante 1) |

use crate::families::cx::Banc;
use super::decide;
use super::*;
use crate::objects::pools;
use crate::objects::Courant;

pub(super) fn run(x: &mut Cx) {
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

/// Fin commune : s'il reste un joueur, le boss reprend la marche.
fn back_to_walk<R: Banc>(x: &mut Cx<'_, R>) {
    if alive(x) {
        decide::walk(x)
    } else {
        decide::wait(x)
    }
}

/// 0x8232e (`+6` = 0) : cri.
fn roar<R: Banc>(x: &mut Cx<'_, R>) {
    if x.u8(0x7) == 0 {
        x.add8(0x7, 2);
        lib::sound(x, 0xe3f76);
        kind(x, 0x10);
        return air(x, 0);
    }
    if lib::step(x) {
        back_to_walk(x);
    }
}

/// 0x823a6 : départ de l'envol (animation de vol 1, vitesse verticale 0x400).
fn take_off<R: Banc>(x: &mut Cx<'_, R>) {
    x.add8(0x7, 2);
    x.set16(0x18, 0x400);
    x.set16(0x1a, 0);
    air(x, 1);
}

/// 0x823c2 : montée jusqu'au-dessus de l'écran, au rythme des battements d'ailes ; puis le boss disparaît
/// (invulnérable) 0x28 images.
fn climb(x: &mut Cx) {
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

/// 0x8237a (`+6` = 2) : envol, puis piqué à la verticale de la cible.
fn dive(x: &mut Cx) {
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
                x.c.d[0] = 2;
                return air(x, 2);
            }
            // dix images avant la fin, le boss reparaît au-dessus de la cible
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
            lib::sound(x, 0xecc0a);
            x.c.d[0] = 3;
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

/// 0x82546 (`+6` = 4) : envol, puis traversée de l'écran au ras du sol à hauteur de la cible.
fn swoop(x: &mut Cx) {
    match x.u8(0x7) {
        0 => {
            if !pick_target(x) {
                return decide::wait(x);
            }
            lib::sound(x, 0xe3f76);
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
            // entrée par le bord opposé à celui que le boss regarde
            let (dx, vx) = if x.u8(f::FLIP) != 0 { (0u16, 0x900u16) } else { (0x180, (-0x900i16) as u16) };
            let px = dx.wrapping_add(var::CAMERA1_X.get(x.b));
            x.set16(0x8, px);
            x.set16(0x14, vx);
            x.set16(0x18, 0xf700);
            x.set16(0x16, 0);
            x.set16(0x1a, 0);
            lib::sound(x, 0xe3f76);
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

/// Atterrissage : secousse de `shake` images, poussière, animation de vol `n`.
fn landing<R: Banc>(x: &mut Cx<'_, R>, shake: u16, n: u32) {
    lib::shake_54(x, shake);
    lib::sound(x, 0xec976);
    lib::sound(x, 0xecdba);
    air(x, n);
}

/// 0x82660 (`+6` = 6) : bond par-dessus la cible ; l'élan vertical dépend de la distance à parcourir.
fn leap(x: &mut Cx) {
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
            // 0x200, plus 0x10 par pas de `v` jusqu'à couvrir la distance
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
            lib::sound(x, 0xe3f76);
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

/// Les étapes en vol : un pas d'animation, l'oscillation (0x833b6), le vent des ailes, un battement.
fn hover(x: &mut Cx) {
    lib::step(x);
    bob(x);
    lib::push_players(x);
    flap(x);
}

/// 0x827c6 (`+6` = 8) : envol à faible hauteur, souffle en éventail (jets ESHL 0x32 tant que le premier double
/// garde la gueule ouverte), puis descente.
fn breath_in_flight(x: &mut Cx) {
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
            x.c.d[0] = v;
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

/// Un jet du souffle (ESHL 0x32, variante `variant`) : son angle suit la table `angles` (16 crans, retournés
/// quand le boss regarde à gauche). Faux si le pool est plein.
fn breath_shot<R: Banc>(x: &mut Cx<'_, R>, angles: u32, variant: Option<u8>) -> bool {
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

/// Attente de `+0xa6` images, puis étape suivante avec la minuterie `next` (et un signal au premier double).
fn timed(x: &mut Cx, next: Option<u16>, cue: bool) {
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

/// Dernière étape commune : attente, puis la marche.
fn last(x: &mut Cx) {
    if !expired(x, TIMER) {
        lib::step(x);
        return;
    }
    back_to_walk(x);
}

/// 0x82a0c (`+6` = 10) : deux signaux au premier double, puis attente.
fn double_gesture(x: &mut Cx) {
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

/// 0x82ab6 (`+6` = 12) : souffle au sol, jets ESHL 0x32 variante 1 toutes les 3 images pendant 0x64 images.
fn breath(x: &mut Cx) {
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

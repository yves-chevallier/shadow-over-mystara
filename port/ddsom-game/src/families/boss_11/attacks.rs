//! Boss 11/12, groupe 4 : les huit attaques (`+6` = 0 à 14), étapes `+7` = 0, 2, 4...

use super::behave::life_rank;
use super::*;
use crate::objects::Courant;
use crate::machine::Machine;

fn step(x: &mut Cx, table: u32) -> u32 {
    dispatch0(x, table, f::SUB3)
}

fn cam_x(x: &mut Cx) -> u16 {
    if var::CAMERA_SELECT.get(x.b) != 0 { var::CAMERA_ALT_X.get(x.b) } else { var::SCROLL_X.get(x.b) }
}

fn cam_y(x: &mut Cx) -> u16 {
    if var::CAMERA_SELECT.get(x.b) != 0 { var::CAMERA_ALT_Y.get(x.b) } else { var::SCROLL_Y.get(x.b) }
}

pub(super) fn run(x: &mut Cx) {
    match dispatch0(x, 0x8dbde, f::SUB2) {
        0x8dbee => strike(x),
        0x8dc44 => spit(x),
        0x8dd1c => leap(x),
        0x8ddcc => lunge(x),
        0x8de64 => charge(x),
        0x8e00c => approach(x),
        0x8e20a => spit_c(x),
        _ => leap_c(x),
    }
}

/// 0x8dc30 : coup de queue (le effet dépend du type).
fn sting(x: &mut Cx) {
    if is_12(x) {
        lib::sound(x, 0xe4de6)
    } else {
        lib::sound(x, 0xe4d72)
    }
}

/// 0x8dbee (`+6` = 0) : un coup.
fn strike(x: &mut Cx) {
    set44(x, 1);
    if step(x, 0x8dc02) == 0x8dc06 {
        x.add8(f::SUB3, 2);
        act(x, 1);
        return;
    }
    if x.anim_step() {
        return to_chase(x);
    }
    sync(x);
    if x.u8(f::KEY_FRAME) != 0 {
        x.set8(f::KEY_FRAME, 0);
        sting(x);
    }
}

/// 0x8dc44 (`+6` = 2) : crachat (types 11 et 12 : projectiles différents).
fn spit(x: &mut Cx) {
    match step(x, 0x8dc52) {
        0x8dc5c => {
            x.add8(f::SUB3, 2);
            x.set16(TIMER, 0x10);
            act(x, 0)
        }
        0x8dc6e => {
            x.anim_step();
            sync(x);
            if sub16(x, TIMER, 1).gt() {
                return;
            }
            x.add8(f::SUB3, 2);
            act(x, 0x24)
        }
        0x8dc8e => {
            if x.anim_step() {
                x.add8(f::SUB3, 2);
                act(x, 0x25);
                return lib::sound(x, 0xe4b4c);
            }
            sync(x);
            if x.u8(f::KEY_FRAME) == 0 {
                return;
            }
            x.set8(f::KEY_FRAME, 0);
            let alea = x.random();
            if is_12(x) {
                let r = alea & 3;
                setw(x, 0, r);
                if r == 0 {
                    return x.call(0xa7da2);
                }
                return x.call(0xa7b0a);
            }
            let r = alea & 7;
            setw(x, 0, r);
            x.call(0xa7570)
        }
        0x8dcec => {
            if x.anim_step() {
                x.add8(f::SUB3, 2);
                return act(x, 0x26);
            }
            sync(x);
        }
        _ => {
            if x.anim_step() {
                return to_chase(x);
            }
            sync(x);
        }
    }
}

/// Orientation vers la caméra : D1b = 0 ou 0x20, D2w = ±vitesse (la tête saute vers le centre de l'écran).
fn jump_side(x: &mut Cx, speed: i16, z: u16, rise: u16) {
    stop(x);
    let mut d0 = cam_x(x);
    setb(x, 1, 0);
    let mut d2 = speed as u16;
    d0 = d0.wrapping_add(0xe0);
    if !Fl::sub16(d0, x.u16(0x8)).le() {
        setb(x, 1, 0x20);
        d2 = d2.wrapping_neg();
    }
    let side = x.c.d[1] as u8;
    x.set8(f::FLIP, side);
    x.set16(0x14, d2);
    x.set16(0x18, z);
    x.set16(0x1a, rise);
    body(x, 5);
}

/// Atterrissage d'un bond : la hauteur redevient nulle, le serpent s'immobilise (bruit et poussière).
fn landing(x: &mut Cx) -> bool {
    x.anim_step();
    sync(x);
    x.move_xz();
    if !tst16(x, 0xc).mi() {
        return false;
    }
    x.add8(f::SUB3, 2);
    x.set16(0xc, 0);
    body(x, 7);
    lib::shake_58(x, 8);
    lib::sound(x, 0xecc0a);
    true
}

/// 0x8dd1c (`+6` = 4) : bond sur l'adversaire.
fn leap(x: &mut Cx) {
    x.set8(0xb2, 1);
    match step(x, 0x8dd30) {
        0x8dd36 => {
            x.add8(f::SUB3, 2);
            jump_side(x, -0x400, 0x600, (-0x40i16) as u16);
            lib::sound(x, 0xe4cfe)
        }
        0x8dd84 => {
            landing(x);
        }
        _ => {
            if !x.anim_step() {
                return to_chase(x);
            }
            sync(x);
        }
    }
}

/// 0x8e2ea (`+6` = 14) : bond plus long (vitesse -0x100).
fn leap_c(x: &mut Cx) {
    x.set8(0xb2, 1);
    match step(x, 0x8e2fe) {
        0x8e304 => {
            x.add8(f::SUB3, 2);
            jump_side(x, -0x100, 0x800, (-0x80i16) as u16);
            lib::sound(x, 0xe4cfe)
        }
        0x8e352 => {
            landing(x);
        }
        _ => {
            if !x.anim_step() {
                return to_chase(x);
            }
            sync(x);
        }
    }
}

/// 0x8ddcc (`+6` = 6) : élan puis chute.
fn lunge(x: &mut Cx) {
    match step(x, 0x8ddda) {
        0x8dde0 => {
            x.add8(f::SUB3, 2);
            stop(x);
            x.set16(0x18, 0x200);
            x.set16(0x1a, (-0x80i16) as u16);
            x.set16(TIMER, 0x20);
            act(x, 0x20);
            sync(x);
            lib::sound(x, 0xe4cfe)
        }
        0x8de0c => {
            x.anim_step();
            sync(x);
            if x.u16(0x1a) == 0 {
                if sub16(x, TIMER, 1).gt() {
                    return;
                }
                x.add8(f::SUB3, 2);
                x.set16(0xc, 0);
                return act(x, 0x22);
            }
            x.move_xz();
            if !tst16(x, 0xc).mi() {
                return;
            }
            x.set16(0xc, 0);
            stop(x)
        }
        _ => {
            x.set8(0xb2, 1);
            if !x.anim_step() {
                return to_chase(x);
            }
            sync(x);
        }
    }
}

/// 0x8de64 (`+6` = 8) : charge sur un joueur (cap pris dans la table de 0x1376).
fn charge(x: &mut Cx) {
    x.set8(0xb2, 1);
    let mut t = step(x, 0x8de78);
    if t == 0x8de82 {
        x.add8(f::SUB3, 2);
        act(x, 0x27);
        sync(x);
        x.set16(TIMER, 0x20);
        t = 0x8de9a;
    }
    if t == 0x8de9a {
        x.anim_step();
        sync(x);
        if sub16(x, TIMER, 1).gt() {
            return;
        }
        x.add8(f::SUB3, 2);
        t = 0x8deb2;
    }
    match t {
        0x8deb2 => {
            x.add8(f::SUB3, 2);
            seg_choice(x);
            if !aim_target(x) {
                return to_chase(x);
            }
            let d0 = if x.u8(f::FLIP) != 0 { 8 } else { 0x18 };
            x.c.d[0] = d0;
            x.set8(0x2d, d0 as u8);
            stop(x);
            speed(x, 0x34);
            lib::sound(x, 0xe4cfe)
        }
        0x8dee2 => {
            if !sub8(x, 0xb1, 1).gt() {
                if sub8(x, 0xb0, 1).le() {
                    return charge_end(x);
                }
                if !aim_target(x) {
                    return charge_end(x);
                }
                pause_tick(x);
                lib::sound(x, 0xe4cfe);
            }
            let mut d0 = 0u32;
            x.c.d[0] = d0;
            let diff = x.u8(0xb3).wrapping_sub(x.u8(0x2d));
            setb(x, 0, diff);
            if diff != 0 {
                let mut w = x.c.d[0] as u16 & 0x1f;
                if !Fl::sub16(w, 0x10).lt() {
                    w = w.wrapping_sub(0x20);
                }
                let cur = x.u8(0x2d).wrapping_sub(w as u8) & 0x1f;
                x.set8(0x2d, cur);
            }
            d0 = 0;
            let _ = d0;
            x.move_by_table_xy();
            x.anim_step();
            sync(x);
        }
        _ => {
            x.move_xy();
            if x.anim_step() {
                to_chase(x);
            }
        }
    }
}

/// 0x8df38 : fin de la charge, freinage.
fn charge_end(x: &mut Cx) {
    x.add8(f::SUB3, 2);
    let a = asr_w(x.u16(0x14), 2);
    x.set16(0x14, a);
    let b = asr_w(x.u16(0x1c), 2);
    x.set16(0x1c, b);
    act(x, 0x29);
    sync(x);
}

/// 0x8df70 : choisit une cible (0x1376 donne le cap) ; faux sans joueur.
fn aim_target(x: &mut Cx) -> bool {
    let Some(target) = lib::target_by_criterion(x, lib::LOWEST_HP) else {
        x.c.d[0] = 0;
        return false;
    };
    lib::aim_to(x, target);
    let a = x.c.d[0] as u8;
    x.set8(0xb3, a);
    true
}

/// 0x8df8e : durée de la charge (`+0xb0`, table 0x8dfcc selon le rang de vie) puis `pause_tick`.
fn seg_choice(x: &mut Cx) {
    life_rank(x);
    let i = x.c.d[0] as u16 & 0x30;
    setw(x, 0, i);
    let a4 = 0x8dfcc + i as u32;
    let alea = x.random();
    let r = alea & 0xf;
    setw(x, 0, r);
    let v = x.data8(a4 + r as u32);
    x.set8(0xb0, v);
    pause_tick(x)
}

/// 0x8dfaa : pas entre deux visées (`+0xb1`, table 0x8dfbc).
fn pause_tick(x: &mut Cx) {
    let alea = x.random();
    let r = alea & 0xf;
    let v = x.rom8(0x8dfbc + r as u32);
    x.set8(0xb1, v);
}

/// 0x8e00c (`+6` = 10) : approche en suivant un tracé (table 0x8e0f8).
fn approach(x: &mut Cx) {
    x.set8(0xb2, 1);
    match step(x, 0x8e020) {
        0x8e026 => {
            x.add8(f::SUB3, 2);
            stop(x);
            speed(x, 0x2b);
            if !route_start(x) {
                return to_chase(x);
            }
            act(x, 0x27);
            sync(x);
        }
        0x8e04e => {
            if sub16(x, TIMER, 1).mi() && !route_step(x) {
                x.add8(f::SUB3, 2);
                let a = asr_w(x.u16(0x14), 2);
                x.set16(0x14, a);
                let b = asr_w(x.u16(0x1c), 2);
                x.set16(0x1c, b);
                act(x, 0x29);
                return sync(x);
            }
            x.move_by_table_xy();
            x.anim_step();
            sync(x);
        }
        _ => {
            if x.anim_step() {
                to_chase(x);
            }
        }
    }
}

/// 0x8e0a2 : tracé tiré selon la distance à la caméra (trois tables identiques dans la ROM).
fn route_start(x: &mut Cx) -> bool {
    let d1 = cam_x(x);
    let d0 = x.u16(0x8).wrapping_sub(d1);
    setw(x, 0, d0);
    let alea = x.random();
    let r = alea & 0xf;
    setw(x, 0, r);
    let v = x.data8(0x8e1e8 + r as u32);
    setb(x, 0, v);
    x.set8(0xb4, v);
    x.set8(0xb5, 0);
    x.set16(TIMER, 0);
    route_step(x)
}

/// 0x8e0e8 : point suivant du tracé ; vrai s'il reste du chemin (la durée du pas va en `+0xb0`).
fn route_step(x: &mut Cx) -> bool {
    x.c.d[0] = x.u8(0xb4) as u32;
    let off = x.rom16(0x8e0f8 + x.c.d[0]);
    setw(x, 0, off);
    let a1 = 0x8e0f8u32.wrapping_add(off as i16 as i32 as u32);
    x.c.a[1] = a1;
    x.c.d[0] = x.u8(0xb5) as u32;
    let count = x.data16(a1.wrapping_sub(2));
    if Fl::sub16(x.c.d[0] as u16, count).gt() {
        x.c.d[0] = 0;
        return false;
    }
    addq8(x, 0xb5, 1);
    let i = (x.c.d[0] as u16).wrapping_mul(4);
    setw(x, 0, i);
    let a1 = a1.wrapping_add(i as i16 as i32 as u32);
    x.c.a[1] = a1;
    let cam = if var::CAMERA_SELECT.get(x.b) != 0 { A5 + 0x54e } else { A5 + 0x4fe };
    x.c.a[2] = cam;
    let d1 = rd16(x.b, cam + 8).wrapping_add(x.data16(a1));
    let d2 = rd16(x.b, cam + 0xc).wrapping_add(x.data16(a1 + 2));
    setw(x, 1, d1);
    setw(x, 2, d2);
    let (px, py) = (x.u16(0x8), x.u16(0x10));
    setw(x, 3, px);
    setw(x, 4, py);
    lib::aim(x, (x.c.d[3] as u16, x.c.d[4] as u16), (x.c.d[1] as u16, x.c.d[2] as u16));
    let angle = x.c.d[0] as u8;
    x.set8(0x2d, angle);
    let i = (x.c.d[0] as u16).wrapping_mul(4);
    setw(x, 0, i);
    let a2 = x.u32(0x4c);
    let mut v = x.data16(a2.wrapping_add(i as i16 as i32 as u32));
    setw(x, 0, v);
    let (mut d0, mut d1l): (u32, u32);
    if v != 0 {
        if (v as i16) < 0 {
            v = v.wrapping_neg();
        }
        d0 = (v as u32) << 16;
        let mut w = cam_x(x).wrapping_add(x.data16(a1)).wrapping_sub(x.u16(0x8));
        if (w as i16) < 0 {
            w = w.wrapping_neg();
        }
        d1l = (w as u32) << 16;
    } else {
        v = x.data16(a2.wrapping_add(2));
        setw(x, 0, v);
        if v == 0 {
            x.c.d[0] = 0;
            return false;
        }
        if (v as i16) < 0 {
            v = v.wrapping_neg();
        }
        d0 = (v as u32) << 16;
        let mut w = cam_y(x).wrapping_add(x.data16(a1 + 2)).wrapping_sub(x.u16(0x10));
        if (w as i16) < 0 {
            w = w.wrapping_neg();
        }
        d1l = (w as u32) << 16;
    }
    // la durée : combien de fois la vitesse entre dans l'écart (centaines, seizièmes, unités)
    let mut d2: u16 = 0;
    let mut d3;
    loop {
        d3 = d1l;
        d1l = d1l.wrapping_sub(d0);
        if (d1l as i32) < 0 {
            break;
        }
        d2 = d2.wrapping_add(0x100);
    }
    d1l = d3;
    d0 = ((d0 as i32) >> 4) as u32;
    if d0 != 0 {
        loop {
            d3 = d1l;
            d1l = d1l.wrapping_sub(d0);
            if (d1l as i32) < 0 {
                break;
            }
            d2 = d2.wrapping_add(0x10);
        }
        d1l = d3;
        d0 = d0.rotate_left(16);
        let w = asr_w(d0 as u16, 4);
        d0 = d0 & 0xffff_0000 | w as u32;
        if w != 0 {
            d1l = d1l.rotate_left(16);
            let mut d1w = d1l as u16;
            loop {
                d3 = d3 & 0xffff_0000 | d1w as u32;
                let r = Fl::sub16(d1w, w);
                d1w = d1w.wrapping_sub(w);
                if r.mi() {
                    break;
                }
                d2 = d2.wrapping_add(1);
            }
        }
    }
    x.c.d[0] = d0;
    x.set16(TIMER, d2);
    x.c.d[0] = 1;
    true
}

/// 0x8e20a (`+6` = 12, type 12 seulement) : crachat de projectile à tête chercheuse.
fn spit_c(x: &mut Cx) {
    match step(x, 0x8e218) {
        0x8e222 => {
            if !is_12(x) {
                return to_chase(x);
            }
            x.add8(f::SUB3, 2);
            x.set16(TIMER, 0x10);
            x.c.d[0] = 0;
            act(x, 0)
        }
        0x8e23e => {
            x.anim_step();
            sync(x);
            if sub16(x, TIMER, 1).gt() {
                return;
            }
            x.add8(f::SUB3, 2);
            act(x, 0x24)
        }
        0x8e25e => {
            if x.anim_step() {
                x.add8(f::SUB3, 2);
                act(x, 0x25);
                return lib::sound(x, 0xe4b4c);
            }
            sync(x);
            if x.u8(f::KEY_FRAME) == 0 {
                return;
            }
            x.set8(f::KEY_FRAME, 0);
            x.set16(0x5c, 0);
            x.call(0xa817c);
            if zero(x) {
                return;
            }
            let a1 = x.c.a[1] as u16;
            x.set16(0x5c, a1);
        }
        0x8e2a0 => {
            if !x.anim_step() {
                return sync(x);
            }
            x.add8(f::SUB3, 2);
            let w = x.u16(0x5c);
            if (w as i16) < 0 {
                let a1 = w as i16 as i32 as u32;
                if rd8(x.b, a1) != 0 && rd8(x.b, a1 + 2) == 0x47 {
                    wr8(x.b, a1 + 0x58, 1);
                }
            }
            act(x, 0x26)
        }
        _ => {
            if x.anim_step() {
                return to_chase(x);
            }
            sync(x);
        }
    }
}

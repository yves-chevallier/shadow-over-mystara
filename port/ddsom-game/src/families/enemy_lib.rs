//! Petites routines communes à plusieurs ennemis (bibliothèque 0x103d6-0x105b6 du programme), réécrites en
//! fonctions Rust : jeux d'animation avec recopie du type de coup, portes de décision de l'IA, tests de position.
//! Les originaux restent en 68000 pour leurs autres appelants.

use super::cx::Cx;
use super::enemy_common::*;

/// 0x1046c : le type de coup `+0x44` suit la clé d'animation `+0x3e`.
fn copy_strike_kind(x: &mut Cx) {
    let k = x.u8(0x3e);
    x.set8(0x44, k);
}

/// 0x103d6 : démarre le script `n` du jeu `set`.
pub fn start(x: &mut Cx, set: u32, n: u32) {
    x.anim_start(set, n);
    copy_strike_kind(x);
}

/// 0x103e0 : reprend l'animation de même clé (`+0x33`) dans le jeu `table`, en gardant la durée (`+0x30`).
pub fn switch_keeping_time(x: &mut Cx, table: u32) {
    let time = x.u16(0x30);
    let key = x.u8(0x33) & 0x7f;
    let mut at = table;
    while x.data8(at + 3) & 0x7f != key {
        at += 0x10;
    }
    x.c.a[4] = at;
    x.c.d[2] = x.c.d[2] & !0xffff | time as u32;
    x.c.d[0] = x.c.d[0] & !0xff | key as u32;
    x.call(0x1a3c);
    x.set16(0x30, time);
    copy_strike_kind(x);
}

/// 0x1040c : un pas d'animation ; vrai à la fin du script.
pub fn step(x: &mut Cx) -> bool {
    let end = x.anim_step();
    copy_strike_kind(x);
    end
}

/// 0x10474 : au moins un joueur en vie (`A5+0x35`).
pub fn players_alive(x: &mut Cx) -> bool {
    x.a5_u8(0x35) != 0
}

/// 0x104a2 : l'objet est hors de la zone `[caméra + gauche, caméra + droite]` (selon le mode deux écrans).
pub fn outside(x: &mut Cx, left: i16, right: i16) -> bool {
    let cam = if x.a5_u8(0x36) != 0 { x.a5_u16(0x5ae) } else { x.a5_u16(0x5a6) };
    let pos = x.u16(0x8) as i16;
    let (lo, hi) = (cam.wrapping_add(left as u16) as i16, cam.wrapping_add(right as u16) as i16);
    x.c.d[0] = x.c.d[0] & !0xffff | lo as u16 as u32;
    x.c.d[1] = x.c.d[1] & !0xffff | hi as u16 as u32;
    lo >= pos || hi <= pos
}

/// 0x1050c : vrai si l'ennemi regarde déjà vers sa cible (ou n'en a pas).
pub fn facing_target(x: &mut Cx) -> bool {
    if x.u16(f::TARGET) as i16 >= 0 {
        x.c.d[0] &= !0xff;
        return true;
    }
    let target = x.linked(f::TARGET);
    let dx = x.u16(0x8).wrapping_sub(target.u16(x.b, 0x8));
    let want = if (dx as i16) < 0 { 0x20 } else { 0 };
    x.c.a[1] = target.0 | 0xff00_0000;
    x.c.d[0] = x.c.d[0] & !0xff | want;
    x.c.d[1] = x.c.d[1] & !0xffff | dx as u32;
    want as u8 == x.u8(f::FLIP)
}

/// 0x10560 : le sens de la vitesse `+0x14` est celui de l'orientation.
pub fn moving_forward(x: &mut Cx) -> bool {
    let flip = (x.u8(f::FLIP) as u16).rotate_right(6);
    let sign = x.u16(0x14) & 0x8000;
    x.c.d[1] = flip as u32;
    x.c.d[2] = x.c.d[2] & !0xffff | sign as u32;
    flip == sign
}

/// 0x10574 : tourne l'angle `+0x2d` d'un cran tiré au hasard.
pub fn jitter_angle(x: &mut Cx) {
    let r = rand_and(x, 0xf);
    let d = x.rom8(0x1058e + r);
    x.c.d[0] = x.c.d[0] & !0xff | d as u32;
    let a = x.u8(0x2d).wrapping_add(d) & 0x1f;
    x.set8(0x2d, a);
}

/// 0x1059e : rang de PV (`+0xac`) selon le nombre de joueurs et la difficulté.
pub fn init_hp_rank(x: &mut Cx) {
    let players = x.a5_u8(0x35).wrapping_sub(1) << 5;
    let level = x.a5_u8(0x7f93) & 0x1e;
    x.c.d[0] = x.c.d[0] & !0xff | players.wrapping_add(level) as u32;
    x.set8(0xac, players.wrapping_add(level));
}

/// 0x105b6 : franchit le palier de PV suivant (table `A1`, un palier par rang) ; vrai si un palier est passé.
pub fn hp_threshold(x: &mut Cx, table: u32) -> bool {
    let n = x.u8(0xaa) as i8;
    if n >= x.u8(0xab) as i8 {
        x.c.d[0] &= !0xff;
        return false;
    }
    let rank = x.u8(0xac) as i8 as i16 as u16;
    let at = (n as i16 as u16).wrapping_shl(7).wrapping_add(rank);
    let limit = x.data16(table + at as u32);
    if (limit as i16) < (x.u16(0x62) as i16) {
        return false;
    }
    x.add8(0xaa, 1);
    true
}

/// 0x104e0 (non Z) : la cible regarde du même côté que l'ennemi, qui lui tourne le dos.
pub fn back_to_target(x: &mut Cx) -> bool {
    if x.u16(f::TARGET) as i16 >= 0 {
        return false;
    }
    let target = x.linked(f::TARGET);
    if x.u8(f::FLIP) != target.u8(x.b, f::FLIP) {
        return false;
    }
    let dx = x.u16(0x8).wrapping_sub(target.u16(x.b, 0x8));
    let want = if (dx as i16) < 0 { 0 } else { 0x20 };
    want != x.u8(f::FLIP)
}

/// 0x1052e : la cible regarde vers l'ennemi alors qu'elle est de son côté.
pub fn target_faces_me(x: &mut Cx) -> bool {
    if x.u16(f::TARGET) as i16 >= 0 {
        return false;
    }
    let target = x.linked(f::TARGET);
    let dx = x.u16(0x8).wrapping_sub(target.u16(x.b, 0x8)) & 0x8000;
    let d1 = (x.u8(f::FLIP) as u16).rotate_right(6) & dx;
    let want = if (d1 as i16) < 0 { 0 } else { 0x20 };
    want == target.u8(x.b, f::FLIP)
}

/// 0x1047a : décision d'attaque de l'IA : 0 rien, 1 ou 2 geste choisi selon le côté de la cible.
pub fn ai_gate(x: &mut Cx) -> u32 {
    let n = if target_faces_me(x) {
        x.call(0xa4b4);
        if x.c.cond(7) { 0 } else { 1 }
    } else if back_to_target(x) {
        x.call(0xa4c2);
        if x.c.cond(7) { 0 } else { 2 }
    } else {
        0
    };
    x.c.d[0] = n;
    n
}

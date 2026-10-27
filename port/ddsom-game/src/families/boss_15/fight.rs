//! Boss 15 / 20, groupe `+5` = 2 : le combat. Le dragon se tient autour du centre de l'écran (`steer`), se retourne ou
//! s'éloigne quand il est trop loin, et choisit une attaque (`+5` = 4) selon la distance du joueur visé et sa santé.
//!
//! | `+6` | Rôle |
//! |---|---|
//! | 0 | surveille (tirages, sondes de portée) |
//! | 2 | bond vers un point calculé (vitesses de saut tirées d'une distance) |
//! | 4 | rejoint le joueur visé |

use super::*;
use crate::objects::Obj;
use crate::objects::Courant;

/// Octets de choix d'attaque (4 lignes de 16 : 0x91a28, 0x91a38, 0x91a48, 0x91a58) et seuil de patience.
const PICKS: [u32; 4] = [0x91a28, 0x91a38, 0x91a48, 0x91a58];
const COOLDOWNS: u32 = 0x91a18;
/// Table de bits (un long par quart de PV) qui décide d'un tir de 0x43ae.
const FAR_MASKS: u32 = 0x91998;
/// Gestes déclenchés par la riposte spéciale (`+6` de l'attaque 4).
const SPECIAL_MOVES: u32 = 0x91ab0;

pub(super) fn run(x: &mut Cx) {
    match x.u8(f::SUB2) {
        0 => watch(x),
        2 => jump(x),
        4 => approach(x),
        _ => {}
    }
    if var::PLAYERS_ALIVE.get(x.b) == 0 {
        wait(x);
    }
}

// --- +6 = 0 : surveille ---------------------------------------------------------------------------------

fn watch(x: &mut Cx) {
    x.set8(HURT_BOX, 0);
    x.set8(SHOT, 0x20);
    if x.u8(0x7) == 0 {
        x.add8(0x7, 2);
        x.set8(VULNERABLE, 1);
        speed(x, 0x11);
        lib::draw_stance(x);
        return rest(x);
    }
    x.anim_step();
    if far_from_center(x) {
        return wander(x);
    }
    if !sub8(x, WALK_DELAY, 1).gt() {
        lib::draw_stance(x);
        if !sub8(x, WALK_STEPS, 1).pl() {
            lib::draw_6b(x);
        }
    }
    if special_due(x) {
        return;
    }
    if probe(x, 0, 0x20, 0x100) {
        return pick_attack(x, 0);
    }
    if far_roll(x) {
        if probe(x, 0, 0x50, 0x100) {
            return pick_attack(x, 1);
        }
        if probe(x, 0x88, 0x38, 0x100) {
            return pick_attack(x, 2);
        }
        if probe(x, -0x88, 0x38, 0x100) {
            return pick_attack(x, 3);
        }
    }
    steer(x);
}

/// 0x91ab8 : le dragon est à plus de 0x80 pixels du centre de la zone (caméra + 0xc0).
fn far_from_center(x: &mut Cx) -> bool {
    let d = var::SCROLL_X.get(x.b).wrapping_add(0xc0).wrapping_sub(x.u16(0x8)) as i16;
    d.wrapping_abs() >= 0x80
}

/// Un joueur est-il dans la boîte `(d0, d1, d2)` devant le dragon ?
fn probe(x: &mut Cx, d0: i16, d1: u16, d2: u16) -> bool {
    lib::target_in_box(x, d0, d1, d2)
}

/// 0x919b0 : numéro (0 à 3) du quart de PV restant, d'après `+0x62` rapporté à `+0x60`.
fn quarter(x: &mut Cx) -> u32 {
    let d1 = x.u16(0x60) as u32;
    let d3 = (x.u16(0x62) as u32) << 2;
    let (mut d0, mut d2) = (-1i32, 0u32);
    // l'original boucle sans fin si `+0x60` est nul
    for _ in 0..0x20000 {
        d2 = d2.wrapping_add(d1);
        d0 += 1;
        if (d2 as i32) >= (d3 as i32) {
            break;
        }
    }
    d0 as u16 as u32
}

/// 0x91950 : le dragon est à son altitude de garde (0x40), sa patience est épuisée et un tirage favorable (un bit du
/// long de PV 0x91998) l'autorise à viser un joueur éloigné (0x43ae).
fn far_roll(x: &mut Cx) -> bool {
    if x.u16(0xc) != 0x40 {
        return false;
    }
    if (x.u8(COOLDOWN) as i8) > 0 {
        sub8(x, COOLDOWN, 1);
        return false;
    }
    let q = quarter(x);
    let mask = rom32(x, FAR_MASKS + (((q as u16) << 2) as u32));
    let bit = random(x) & 0x1f;
    if mask >> bit & 1 == 0 {
        return false;
    }
    x.c.d[0] = 2;
    lib::pick_target_any(x)
}

/// 0x91a68 : une riposte spéciale est-elle due ? Les PV passent sous `SPECIALS` quarts de `+0x60` : le dragon lance
/// l'attaque 4 (geste tiré dans 0x91ab0).
fn special_due(x: &mut Cx) -> bool {
    let left = x.u8(SPECIALS);
    if left == 0 {
        return false;
    }
    let quarter_hp = x.u16(0x60) >> 2;
    let mut sum = 0u16;
    let mut n = left as i16;
    loop {
        sum = sum.wrapping_add(quarter_hp);
        n -= 1;
        if n <= 0 {
            break;
        }
    }
    if Fl::sub16(sum, x.u16(f::HP)).le() {
        return false;
    }
    sub8(x, SPECIALS, 1);
    set_abs8(x, 0xff8043, 0x7f);
    x.set8(SHIELD, 8);
    x.set32(f::STATE, ATTACK_4);
    let r = random(x) & 3;
    let which = x.rom8(SPECIAL_MOVES + r);
    x.set8(f::SUB2, which);
    true
}

/// 0x919cc..0x91a16 : choisit une attaque dans la ligne `row` des tables ; un octet négatif renvoie à la ligne de
/// patience (0x91a18), qui peut aussi renvoyer au hasard (`wander`).
fn pick_attack(x: &mut Cx, row: usize) {
    let r = random(x) & 0xf;
    let b = x.data8(PICKS[row] + r) as i8;
    if b >= 0 {
        x.set32(f::STATE, 0x0204_0000);
        x.set8(f::SUB2, b as u8);
        return;
    }
    let r = random(x) & 0xf;
    let c = x.rom8(COOLDOWNS + r) as i8;
    if c < 0 {
        return wander(x);
    }
    x.set8(COOLDOWN, c as u8);
}

/// 0x91ad8 : se tient au centre de la zone (caméra + 0xc0), à la profondeur d'un joueur (ou de la caméra) et à l'altitude 0x40.
pub(super) fn steer(x: &mut Cx) {
    let target_x = var::SCROLL_X.get(x.b).wrapping_add(0xc0);
    let d = target_x.wrapping_sub(x.u16(0x8)) as i16;
    if d != 0 {
        let step: i16 = if d < 0 { -1 } else { 1 };
        x.add16(0x8, step as u16);
    }
    let mut y = var::SCROLL_Y.get(x.b).wrapping_add(0x60);
    if lib::pick_target_any(x) {
        y = x.linked(f::TARGET).u16(x.b, Obj::Y);
    }
    let d = y.wrapping_sub(x.u16(0x10)) as i16;
    if d != 0 {
        let step: i16 = if d < 0 { -1 } else { 1 };
        x.add16(0x10, step as u16);
    }
    let z = x.u16(0xc) as i16;
    if z != 0x40 {
        let step: i16 = if z > 0x40 { -1 } else { 1 };
        x.add16(0xc, step as u16);
    }
    x.set8(HOVER, 1);
}

// --- +6 = 2 : bond ---------------------------------------------------------------------------------------

fn jump(x: &mut Cx) {
    if x.u8(0x7) == 0 {
        x.add8(0x7, 2);
        jump_speeds(x);
        rest(x);
        x.set8(0x38, 0);
        return;
    }
    x.move_xz();
    if x.u16(0xc) as i16 >= 0x2c0 {
        x.set16(0xc, 0x2c0);
    }
    if x.u16(0xc) as i16 > 0x30 {
        return;
    }
    x.set16(0xc, 0x30);
    stand(x);
}

/// 0x91b54 : vitesses du bond pour atteindre le centre de la zone : vitesse horizontale 0x100 dans le bon sens,
/// vitesse verticale 0x100 × (distance × 16 / 0x100).
fn jump_speeds(x: &mut Cx) {
    let d1 = x.rom16(0x91b94);
    let mut d2 = d1 as i16;
    let mut d0 = var::SCROLL_X.get(x.b).wrapping_add(0xc0).wrapping_sub(x.u16(0x8)) as i16;
    if d0 < 0 {
        d0 = d0.wrapping_neg();
        d2 = d2.wrapping_neg();
    }
    x.set16(0x14, d2 as u16);
    x.set16(0x16, 0);
    let az = x.rom16(0x91b96);
    x.set16(0x1a, az);
    let half = (az as i16).wrapping_neg() >> 1;
    let mut product = (d0 as u16 as u32).wrapping_mul(half as u16 as u32) as u16;
    let mut d6: i16 = -1;
    loop {
        d6 = d6.wrapping_add(1);
        product = product.wrapping_sub(d1);
        if product & 0x8000 != 0 {
            break;
        }
    }
    x.set16(0x18, (d6 as u16) << 8);
}

// --- +6 = 4 : rejoint le joueur visé -------------------------------------------------------------------

fn approach(x: &mut Cx) {
    let t = x.u16(TIMER).wrapping_sub(1);
    x.set16(TIMER, t);
    if t & 0x8000 != 0 {
        return stand(x);
    }
    if x.u16(f::TARGET) == 0 {
        return wait(x);
    }
    let p = x.linked(f::TARGET);
    if p.u8(x.b, 0) == 0 {
        return wait(x);
    }
    let px = p.u16(x.b, 0x8);
    if !close_x(x, 0x40, px) {
        return;
    }
    stand(x);
}

/// 0x91c22 : rejoint par tiers l'altitude `z` (1/4 de l'écart par image) et la profondeur `y` (1/4) ; rend vrai
/// quand les deux sont atteintes.
pub(super) fn close_in_zy(x: &mut Cx, z: i16, y: i16) -> bool {
    let mut reached = 0;
    let dz = z.wrapping_sub(x.u16(0xc) as i16);
    let mut s = dz >> 2;
    if s == 0 {
        s = 1;
    }
    x.add16(0xc, s as u16);
    if dz.wrapping_abs().wrapping_sub(1) <= 0 {
        reached += 1;
    }
    let dy = y.wrapping_sub(x.u16(0x10) as i16);
    let mut s = dy >> 2;
    if s == 0 {
        s = 1;
    }
    x.add16(0x10, s as u16);
    if dy.wrapping_abs().wrapping_sub(1) <= 0 {
        reached += 1;
        if reached >= 2 {
            return true;
        }
    }
    false
}

/// 0x91c6a : idem pour l'altitude `z` (1/4) et l'abscisse `px` (1/16).
fn close_x(x: &mut Cx, z: i16, px: u16) -> bool {
    let mut reached = 0;
    let dz = z.wrapping_sub(x.u16(0xc) as i16);
    let mut s = dz >> 2;
    if s == 0 {
        s = 1;
    }
    x.add16(0xc, s as u16);
    if dz.wrapping_abs().wrapping_sub(1) <= 0 {
        reached += 1;
    }
    let dx = (px as i16).wrapping_sub(x.u16(0x8) as i16);
    let mut s = dx >> 4;
    if s == 0 {
        s = 1;
    }
    x.add16(0x8, s as u16);
    if dx.wrapping_abs().wrapping_sub(1) <= 0 {
        reached += 1;
        if reached >= 2 {
            return true;
        }
    }
    false
}

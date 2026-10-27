//! Boss 15 / 20, groupe `+5` = 4 : les attaques (`+6` = 0 à 18), chacune en étapes `+7`. Les montées (`rise`), descentes
//! (`fall`) et attentes (`pause`) sont partagées par plusieurs attaques.
//!
//! | `+6` | Attaque |
//! |---|---|
//! | 0 | rejoint le joueur, bat des ailes (le pousse), se pose |
//! | 2 | monte, entre par un bord de l'écran, vole en travers et sort (souffle de feu) |
//! | 4 | monte, plonge en tournant et se pose |
//! | 6, 16 | coup de griffe à droite (6) ou à gauche (16) |
//! | 8, 10, 18 | pose de 0x100 images (boîtes de coups 6, 8, 7) |
//! | 12 | monte, rugit (0xa7234 / 0xa727c), descend |
//! | 14 | plonge sur un joueur et le saisit |

use super::*;
use crate::mem::{rd16, rts};
use crate::objects::Obj;
use crate::{regs, Native};
use cps2::Cps2Bus;
use m68k::Cpu;
use crate::objects::Courant;

pub(super) fn run(x: &mut Cx) {
    match x.u8(f::SUB2) {
        0 => attack_0(x),
        2 => attack_2(x),
        4 => attack_4(x),
        6 => claw(x, 4, 6),
        8 => pose(x, 6),
        10 => pose(x, 8),
        12 => attack_12(x),
        14 => attack_14(x),
        16 => claw(x, 5, -6),
        18 => pose(x, 7),
        _ => {}
    }
}

// --- étapes communes ----------------------------------------------------------------------------------------

/// 0x91cce : début de montée.
fn rise_start(x: &mut Cx) {
    x.add8(0x7, 2);
    x.set32(0x14, 0);
    x.set32(0x1c, 0);
    x.set16(0x18, 0x400);
    x.set16(0x1a, 0x40);
    set_anim(x, 2, 0);
}

/// 0x91cec : monte jusqu'à l'altitude 0x300.
fn rise_up(x: &mut Cx) {
    x.move_z();
    if (x.u16(0xc) as i16) < 0x300 {
        return;
    }
    x.set16(TIMER, 0xf0);
    x.add8(0x7, 2);
}

/// 0x91d08, 0x91d6e : attend la fin du minuteur.
fn pause(x: &mut Cx) {
    if sub16(x, TIMER, 1).gt() {
        return;
    }
    x.set16(TIMER, 0);
    x.add8(0x7, 2);
}

/// 0x91d30 : début de descente.
fn fall_start(x: &mut Cx) {
    x.add8(0x7, 2);
    x.set32(0x14, 0);
    x.set32(0x1c, 0);
    x.set16(0x18, (-0x400i16) as u16);
    x.set16(0x1a, (-0x40i16) as u16);
    set_anim(x, 2, 0);
}

/// 0x91d4e : descend jusqu'à l'altitude 0x20, puis attend 0x20 images.
fn fall_down(x: &mut Cx) {
    x.move_z();
    if (x.u16(0xc) as i16) > 0x20 {
        return;
    }
    x.set16(0xc, 0x20);
    x.add8(0x7, 2);
    x.set16(TIMER, 0x20);
    pause(x);
}

/// Le joueur visé (`+0x50`) s'il existe, sinon le dragon attend (0x91748).
fn target(x: &mut Cx) -> Option<Obj> {
    if x.u16(f::TARGET) == 0 {
        wait(x);
        return None;
    }
    let p = x.linked(f::TARGET);
    if p.u8(x.b, 0) == 0 {
        wait(x);
        return None;
    }
    Some(p)
}

/// Rejoint par quarts l'altitude 0x40 et la profondeur du joueur visé ; passe à l'étape suivante à l'arrivée.
fn reach_player(x: &mut Cx) {
    let Some(p) = target(x) else { return };
    let y = p.u16(x.b, 0x10) as i16;
    if fight_close(x, y) {
        x.add8(0x7, 2);
    }
}

fn fight_close(x: &mut Cx, y: i16) -> bool {
    super::fight::close_in_zy(x, 0x40, y)
}

// --- +6 = 0 -----------------------------------------------------------------------------------------------

fn attack_0(x: &mut Cx) {
    x.set8(SHIELD, 2);
    match x.u8(0x7) {
        0 => reach_player(x),
        2 => {
            x.add8(0x7, 2);
            lib::sound(x, 0xe571a);
            x.set16(TIMER, 0xa0);
            set_anim(x, 0, 2);
        }
        4 => {
            if !sub16(x, TIMER, 1).gt() {
                x.add8(0x7, 2);
                x.call(0x1ac86);
                x.set_a5_u16(0x4be, 8);
                return settle(x);
            }
            push_players(x);
            x.add16(0xc, 1);
            if x.u16(TIMER) & 3 == 0 {
                let z = x.u16(0xc).wrapping_sub(3);
                x.set16(0xc, z);
            }
            x.call(0xb3fd4);
        }
        6 => settle(x),
        8 => {
            step(x);
            if x.u8(0x33) == 0 {
                return;
            }
            x.set8(0x33, 0);
            x.add8(0x7, 2);
            lib::sound(x, 0xe571a);
            eshl_shot_34(x, 0);
            lib::sound(x, 0xe54dc);
            x.set16(TIMER, 0x100);
            flap(x);
        }
        10 => flap(x),
        12 => {
            if step(x) {
                stand(x);
            }
        }
        _ => {}
    }
}

/// 0x91e16 : descend de 8 par image jusqu'à l'altitude 0x20.
fn settle(x: &mut Cx) {
    let z = x.u16(0xc).wrapping_sub(8);
    x.set16(0xc, z);
    if (z as i16) > 0x20 {
        return;
    }
    x.set16(0xc, 0x20);
    x.add8(0x7, 2);
    set_anim(x, 2, 1);
}

/// 0x91e62 : bat des ailes : l'altitude et la profondeur oscillent.
fn flap(x: &mut Cx) {
    step(x);
    if !sub16(x, TIMER, 1).gt() {
        x.add8(0x7, 2);
        return set_anim(x, 2, 2);
    }
    x.add16(0xc, 1);
    if x.u16(TIMER) & 3 == 0 {
        let z = x.u16(0xc).wrapping_sub(3);
        x.set16(0xc, z);
    }
    let mut d: i16 = 4;
    if x.u16(TIMER) & 1 != 0 {
        d = -d;
    }
    x.add16(0x10, d as u16);
}

// --- +6 = 2 : souffle en travers de l'écran ----------------------------------------------------------------------

fn attack_2(x: &mut Cx) {
    x.set8(SHIELD, 2);
    match x.u8(0x7) {
        0 => rise_start(x),
        2 => rise_up(x),
        4 => pause(x),
        6 => {
            x.add8(0x7, 2);
            x.set8(0x42, 1);
            x.set_a5_u16(0x51e, 4);
            let r = random(x) as u8 & 0x20;
            x.set8(f::FLIP, r);
            let d1: i16 = if r != 0 { -0xe0 } else { 0x260 };
            let px = x.a5_u16(var::SCROLL_X).wrapping_add(d1 as u16);
            x.set16(0x8, px);
            x.set16(0xc, 0x30);
            let py = x.a5_u16(var::SCROLL_Y).wrapping_add(0x50);
            x.set16(0x10, py);
            x.set16(TIMER, 0x40);
            set_anim(x, 2, 0x13);
            lib::sound(x, 0xe571a);
            push_side(x);
        }
        8 => push_side(x),
        10 => align(x),
        12 => sweep(x),
        14 => sweep_end(x),
        16 => leave(x),
        18 => pause(x),
        20 => fall_start(x),
        22 => fall_down(x),
        24 => stand(x),
        _ => {}
    }
}

/// 0x91f36 : pousse les joueurs de 2 pixels du côté où le dragon entre pendant que le minuteur décompte.
fn push_side(x: &mut Cx) {
    let d: i16 = if x.u8(f::FLIP) != 0 { -2 } else { 2 };
    for k in 0..4u32 {
        let p = Obj(0xff85de + 0x200 * k);
        if p.u8(x.b, 0) != 0 {
            let nx = p.u16(x.b, 0x8).wrapping_add(d as u16);
            p.set_u16(x.b, 0x8, nx);
        }
    }
    x.call(0xba18a);
    let t = x.u16(TIMER).wrapping_sub(1);
    x.set16(TIMER, t);
    if (t as i16) >= 0 {
        return;
    }
    x.add8(0x7, 2);
    align(x);
}

/// 0x91f6c : rejoint la position d'attaque (0x138 ou 0x48 devant la caméra).
fn align(x: &mut Cx) {
    x.set8(0x42, 1);
    let edge: u16 = if x.u8(f::FLIP) != 0 { 0x48 } else { 0x138 };
    let d = edge.wrapping_add(x.a5_u16(var::SCROLL_X)).wrapping_sub(x.u16(0x8)) as i16;
    if d != 0 {
        let mut s = d >> 4;
        if s == 0 {
            s = 1;
        }
        x.add16(0x8, s as u16);
        return;
    }
    x.add8(0x7, 2);
    set_anim(x, 2, 0x14);
    x.set32(0x4c, 0);
    sweep(x);
}

/// 0x91fa6 : souffle : la profondeur suit la table de vecteurs pointée par `+0x4c` (posée par le script d'animation).
fn sweep(x: &mut Cx) {
    x.set8(0x42, 1);
    let table = x.u32(0x4c);
    if table != 0 {
        let k = (x.u8(0x2d) as u32) << 2;
        let w = x.data16(table + 2 + k);
        let mut y = x.u32(0x10).wrapping_add((((w as u32) << 16) as i32 >> 8) as u32);
        x.set32(0x10, y);
        y = y >> 16;
        let lo = x.a5_u16(var::SCROLL_Y).wrapping_add(0x20);
        let mut yw = y as u16;
        if !Fl::sub16(yw, lo).ge() {
            yw = lo;
        }
        if !Fl::sub16(yw, lo.wrapping_add(0x60)).le() {
            yw = lo.wrapping_add(0x60);
        }
        x.set16(0x10, yw);
        if x.a5_u16(0x6e) & 1 != 0 {
            let a = x.u8(0x2d).wrapping_add(1) & 0x1f;
            x.set8(0x2d, a);
        }
    }
    if step(x) {
        x.add8(0x7, 2);
        set_anim(x, 2, 0x22);
        return sweep_end(x);
    }
    if x.u8(0x33) == 0 {
        return;
    }
    x.set8(0x33, 0);
    eshl_shot_34(x, 1);
    lib::sound(x, 0xe54dc);
    x.set8(0x2d, 0);
    x.select_speed(&crate::objects::motion::SPEEDS_A, 0x18);
}

/// 0x92036 : fin du souffle, puis lance le vol de sortie.
fn sweep_end(x: &mut Cx) {
    x.set8(0x42, 1);
    if !step(x) {
        return;
    }
    x.add8(0x7, 2);
    let mut vx: i16 = 0x400;
    if x.u8(f::FLIP) != 0 {
        vx = -vx;
    }
    x.set16(0x14, vx as u16);
    x.set16(0x16, 0);
    x.set32(0x1c, 0);
    x.set32(0x18, 0);
    leave(x);
}

/// 0x92066 : s'éloigne jusqu'à sortir de l'écran, puis se replace en haut à droite.
fn leave(x: &mut Cx) {
    x.set8(0x42, 1);
    x.move_x();
    let d = x.u16(0x8).wrapping_sub(x.a5_u16(var::SCROLL_X)) as i16;
    if !(d < -0x80) && d < 0x200 {
        return;
    }
    x.add8(0x7, 2);
    x.set_a5_u16(0x51e, 0);
    x.set16(TIMER, 0x20);
    x.set16(0xc, 0x380);
    let px = x.a5_u16(var::SCROLL_X).wrapping_add(0xc0);
    x.set16(0x8, px);
    let py = x.a5_u16(var::SCROLL_Y).wrapping_add(0x7a);
    x.set16(0x10, py);
}

// --- +6 = 4 : plongeon ------------------------------------------------------------------------------------------

fn attack_4(x: &mut Cx) {
    x.set8(SHIELD, 2);
    match x.u8(0x7) {
        0 => rise_start(x),
        2 => rise_up(x),
        4 => pause(x),
        6 => {
            x.add8(0x7, 2);
            x.set8(0x42, 1);
            x.set8(f::FLIP, 0);
            set_anim(x, 2, 0x15);
            x.c.d[0] = 0xa000;
            x.set_a5_u8(0x48e, 4);
            x.call(0x168e);
            dive_a(x);
        }
        8 => dive_a(x),
        10 => dive_b(x),
        12 => pause(x),
        14 => fall_start(x),
        16 => fall_down(x),
        18 => stand(x),
        _ => {}
    }
}

/// 0x92180 : place le dragon au-dessus de l'écran, selon son type.
fn dive_position(x: &mut Cx) {
    let big = x.u8(TYPE) == TYPE_BIG;
    let px = x.a5_u16(var::SCROLL_X).wrapping_add(if big { 0x96 } else { 0xc0 });
    x.set16(0x8, px);
    x.set16(0x10, 0x170);
    let z = x.a5_u16(var::SCROLL_Y).wrapping_sub(if big { 0xf0 } else { 0xd8 });
    x.set16(0xc, z);
}

fn dive_a(x: &mut Cx) {
    x.set8(0x42, 1);
    x.set8(FOREGROUND, 1);
    dive_position(x);
    if step(x) {
        x.add8(0x7, 2);
        set_anim(x, 2, 0x16);
        x.call(0xb4026);
        return dive_b(x);
    }
    if x.u8(0x33) == 0 {
        return;
    }
    x.set8(0x33, 0);
    eshl_shot_34(x, 2);
    lib::sound(x, 0xe54dc);
}

fn dive_b(x: &mut Cx) {
    x.set8(0x42, 1);
    x.set8(FOREGROUND, 1);
    x.c.d[0] = 0xf000;
    x.set_a5_u8(0x48e, 4);
    x.call(0x168e);
    dive_position(x);
    if !step(x) {
        return;
    }
    x.add8(0x7, 2);
    x.set16(TIMER, 0x20);
    x.set16(0xc, 0x380);
}

// --- +6 = 6 et 16 : coup de griffe --------------------------------------------------------------------------

/// `boxes` : boîtes de coups (`+0xa2`) ; `dx` : déplacement par image du coup (6 à droite, -6 à gauche).
fn claw(x: &mut Cx, boxes: u8, dx: i16) {
    match x.u8(0x7) {
        0 => reach_player(x),
        2 => {
            x.set8(HURT_BOX, boxes);
            x.add8(0x7, 2);
            x.set16(TIMER, 0x18);
        }
        4 => {
            x.set8(HURT_BOX, boxes);
            if sub16(x, TIMER, 1).gt() {
                return;
            }
            x.add8(0x7, 2);
            x.set16(TIMER, 0x20);
            claw_sweep(x, boxes, dx);
        }
        6 => claw_sweep(x, boxes, dx),
        _ => {}
    }
}

fn claw_sweep(x: &mut Cx, boxes: u8, dx: i16) {
    x.set8(HURT_BOX, boxes);
    x.add16(0x8, dx as u16);
    if sub16(x, TIMER, 1).gt() {
        return;
    }
    x.set16(TIMER, 0);
    stand(x);
}

// --- +6 = 8, 10, 18 : pose -----------------------------------------------------------------------------------

fn pose(x: &mut Cx, boxes: u8) {
    x.set8(HURT_BOX, boxes);
    match x.u8(0x7) {
        0 => {
            x.add8(0x7, 2);
            x.set16(TIMER, 0x100);
        }
        2 => {
            if sub16(x, TIMER, 1).gt() {
                return;
            }
            x.set16(TIMER, 0);
            stand(x);
        }
        _ => {}
    }
}

// --- +6 = 12 : rugissement -----------------------------------------------------------------------------------------

fn attack_12(x: &mut Cx) {
    match x.u8(0x7) {
        0 => rise_start(x),
        2 => rise_up(x),
        4 => {
            x.add8(0x7, 2);
            x.set16(TIMER, 0x50);
            lib::shake_5c(x, 0x10);
            let routine = if x.u8(TYPE) == TYPE_BIG { 0xa7234 } else { 0xa727c };
            x.call(routine);
        }
        6 => {
            if sub16(x, TIMER, 1).gt() {
                return;
            }
            x.add8(0x7, 2);
        }
        8 => pause(x),
        10 => fall_start(x),
        12 => fall_down(x),
        14 => stand(x),
        _ => {}
    }
}

// --- +6 = 14 : plongeon sur un joueur ---------------------------------------------------------------------------

fn attack_14(x: &mut Cx) {
    match x.u8(0x7) {
        0 => {
            x.add8(0x7, 2);
            x.set16(0x18, 0x300);
            x.set16(0x1a, (-0x20i16) as u16);
            set_anim(x, 2, 3);
        }
        2 => {
            step(x);
            x.move_z();
            follow(x);
            if (x.u16(0xc) as i16) >= 0x80 {
                x.add8(0x7, 2);
                x.set16(TIMER, 8);
                return set_anim(x, 2, 0x26);
            }
            if (x.u16(0x18) as i16) >= 0x100 {
                return;
            }
            x.set16(0x18, 0x100);
            x.set16(0x1a, 0);
        }
        4 => {
            follow(x);
            step(x);
            if sub16(x, TIMER, 1).gt() {
                return;
            }
            x.add8(0x7, 2);
            x.set16(0x1c, (-0x200i16) as u16);
            x.set16(0x1e, 0);
            x.set16(0x18, (-0x1000i16) as u16);
            x.set16(0x1a, 0);
            x.set32(0x14, 0);
        }
        6 => {
            follow(x);
            if try_seize(x) {
                return;
            }
            x.move_y();
            x.move_z();
            if (x.u16(0xc) as i16) > 0 {
                let low = x.u16(0xc).wrapping_add(x.u16(0x10));
                let floor = x.a5_u16(var::SCROLL_Y).wrapping_add(0x60);
                if (low as i16) > floor as i16 {
                    return;
                }
            } else {
                x.set16(0xc, 0);
            }
            x.add8(0x7, 2);
            stand(x);
        }
        _ => {}
    }
}

/// 0x9249e : un joueur à portée (0xfc44) est saisi ; rend vrai.
fn try_seize(x: &mut Cx) -> bool {
    let Some(player) = lib::attack_hits_a_player(x, 7) else { return false };
    lib::seize_player(x, player, 0);
    true
}

/// 0x931ea : se rapproche du joueur visé de 2 pixels par image en X et en Y (sans le dépasser).
fn follow(x: &mut Cx) {
    if x.u16(f::TARGET) == 0 {
        return;
    }
    let p = x.linked(f::TARGET);
    let (tx, ty) = (p.u16(x.b, 0x8) as i16, p.u16(x.b, 0x10) as i16);
    for (off, t) in [(0x8, tx), (0x10, ty)] {
        let here = x.u16(off) as i16;
        if t == here {
            continue;
        }
        let mut n = if t > here { here.wrapping_add(2) } else { here.wrapping_sub(2) };
        if (t > here && n >= t) || (t < here && n <= t) {
            n = t;
        }
        x.set16(off, n as u16);
    }
}

/// 0x930e2 : repousse les joueurs trop proches (moins de 0x10 pixels en X + Y) le long de l'axe dragon-joueur.
fn push_players(x: &mut Cx) {
    for k in 0..4u32 {
        let p = Obj(0xff85de + 0x200 * k);
        if p.u8(x.b, 0) == 0 {
            continue;
        }
        if (p.u8(x.b, 3) >> 1) == 5 && p.u8(x.b, 0x6a) == 0x1a {
            continue;
        }
        let dx = (x.u16(0x8).wrapping_sub(p.u16(x.b, 0x8)) as i16).wrapping_abs();
        let dy = (x.u16(0x10).wrapping_sub(p.u16(x.b, 0x10)) as i16).wrapping_abs();
        if dx.wrapping_add(dy) <= 0x10 {
            continue;
        }
        x.c.d[1] = x.u16(0x8) as u32;
        x.c.d[2] = x.u16(0x10) as u32;
        x.c.d[3] = p.u16(x.b, 0x8) as u32;
        x.c.d[4] = p.u16(x.b, 0x10) as u32;
        shove(x.c, x.b, p);
    }
}

/// Pente (en 1/256 de pixel) des pas de `shove` : 32 directions, (dx, dy) en mots.
const SHOVE_STEPS: u32 = 0x1c6636;

/// 0x93158 puis l'ajout à la position : déplace `o` d'un pas dans la direction de (D3, D4) vers (D1, D2) (0x1386).
/// D0 et D1 rendent le pas (longs, 8 bits de fraction), A2 la table.
fn shove(c: &mut Cpu, b: &mut Cps2Bus, o: Obj) {
    crate::call(c, b, 0x1386);
    let a = (c.d[0] as u16 & 0x1f) * 4;
    c.d[0] = c.d[0] & 0xffff_0000 | a as u32;
    c.a[2] = SHOVE_STEPS;
    let dy = ((rd16(b, SHOVE_STEPS + 2 + a as u32) as i16 as i32) << 8) as u32;
    let dx = ((rd16(b, SHOVE_STEPS + a as u32) as i16 as i32) << 8) as u32;
    c.d[0] = dx;
    c.d[1] = dy;
    let nx = o.u32(b, 0x8).wrapping_add(dx);
    o.set_u32(b, 0x8, nx);
    let ny = o.u32(b, 0x10).wrapping_add(dy);
    o.set_u32(b, 0x10, ny);
}

/// 0x930a4 (appelé par un effet, 0xb482e) : l'objet A0 se rapproche de l'objet A1 d'un pas tant qu'il en est à
/// plus de 0x10 pixels (X + Y).
fn approach(c: &mut Cpu, b: &mut Cps2Bus) {
    let (me, other) = (Obj(c.a[0] & 0xff_ffff), Obj(c.a[1] & 0xff_ffff));
    let dx = (me.u16(b, 0x8).wrapping_sub(other.u16(b, 0x8)) as i16).wrapping_abs() as u16;
    let dy = (me.u16(b, 0x10).wrapping_sub(other.u16(b, 0x10)) as i16).wrapping_abs() as u16;
    let sum = dx.wrapping_add(dy);
    c.d[0] = c.d[0] & 0xffff_0000 | sum as u32;
    c.d[1] = c.d[1] & 0xffff_0000 | dy as u32;
    if (sum as i16) > 0x10 {
        for (n, v) in [(1, other.u16(b, 0x8)), (2, other.u16(b, 0x10)), (3, me.u16(b, 0x8)), (4, me.u16(b, 0x10))] {
            c.d[n] = c.d[n] & 0xffff_0000 | v as u32;
        }
        shove(c, b, me);
    }
    rts(c, b);
}

pub(super) const APPROACH: Native = Native {
    entry: 0x930a4,
    name: "boss_15_rapprochement",
    run: approach,
    touches_sound: false,
    outputs: regs::d(0) | regs::d(1) | regs::d(2) | regs::d(3) | regs::d(4) | regs::a(2),
};

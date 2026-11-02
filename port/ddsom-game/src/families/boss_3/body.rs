//! Boss 3 : le corps repousse ce qui le touche. Deux formes (`+0xa4`) : 1 « devant » (pendant le cri, l'objet devant
//! le boss est poussé vers l'avant) et 2 « de côté » (pendant le vol bas, l'objet est écarté du boss). Une table par
//! forme donne, par tranche de 16 pixels de distance, la hauteur maximale, la demi-profondeur et la poussée.
//!
//! Les joueurs sont poussés par le boss lui-même (`push_players`) ; les autres objets appellent 0x81aa4
//! (`boss_common`), qui appelle ces formes par `pousser`.

use cps2::Cps2Bus;
use m68k::decode::Size;
use m68k::Cpu;

use super::*;
use crate::families::cx::Bench;
use crate::mem::{rd16, rts};

/// Une forme : tranches (octets, relatives au PC) et lignes (hauteur, demi-profondeur, poussée ; données).
struct Shape {
    slices: u32,
    rows: u32,
}

const FRONT: Shape = Shape { slices: 0x81950, rows: 0x81970 };
const SIDE: Shape = Shape { slices: 0x81a44, rows: 0x81a64 };

/// 0x81916 / 0x81a0a : poussée pour l'objet `other` à la distance `d` du boss `me`, 0 hors de la forme (hauteur
/// de l'objet au-dessus de la tranche, ou trop loin en profondeur).
fn shove_amount(b: &mut Cps2Bus, shape: &Shape, me: Obj, other: Obj, d: u16) -> u16 {
    if (d as i16) >= 0x200 {
        return 0;
    }
    let slice = crate::mem::prog8(b, shape.slices + (d >> 4) as u32) as u32;
    let row = shape.rows + (slice << 3);
    let height = rd16(b, row);
    if (height as i16) < other.u16(b, 0xc) as i16 {
        return 0;
    }
    let depth = rd16(b, row + 2);
    let mut dz = other.u16(b, 0x10).wrapping_sub(me.u16(b, 0x10));
    if (dz as i16) < 0 {
        dz = dz.wrapping_neg();
    }
    if dz as i16 >= depth as i16 { 0 } else { rd16(b, row + 4) }
}

/// 0x818e6 : forme 1 : l'objet `other` devant le boss `me` (jusqu'à 0x20 derrière lui) est poussé vers l'avant.
fn shove_front(b: &mut Cps2Bus, me: Obj, other: Obj) -> bool {
    let ahead = me.u8(b, 0x2c) != 0;
    let mut d = other.u16(b, 0x8).wrapping_sub(me.u16(b, 0x8));
    if !ahead {
        d = d.wrapping_neg();
    }
    d = d.wrapping_add(0x20);
    if (d as i16) < 0 {
        return false;
    }
    let mut p = shove_amount(b, &FRONT, me, other, d);
    if p == 0 {
        return false;
    }
    if !ahead {
        p = p.wrapping_neg();
    }
    let ox = other.u16(b, 0x8);
    other.set_u16(b, 0x8, ox.wrapping_add(p));
    true
}

/// 0x819e2 : forme 2 : l'objet `other` est écarté du boss `me`, du côté où il se trouve.
fn shove_side(b: &mut Cps2Bus, me: Obj, other: Obj) -> bool {
    let mut d = other.u16(b, 0x8).wrapping_sub(me.u16(b, 0x8));
    if (d as i16) < 0 {
        d = d.wrapping_neg();
    }
    let mut p = shove_amount(b, &SIDE, me, other, d);
    if p == 0 {
        return false;
    }
    let ox = other.u16(b, 0x8);
    if (ox as i16) < me.u16(b, 0x8) as i16 {
        p = p.wrapping_neg();
    }
    other.set_u16(b, 0x8, ox.wrapping_add(p));
    true
}

/// 0x818b2 (`shape` = 1) et 0x819b0 (2) : le boss pousse chaque joueur présent, sauf l'elfe (classe 5) dans son
/// état 0x1a.
pub(super) fn push_players<R: Bench>(x: &mut Cx<'_, R>, shape: u8) {
    x.set8(BODY_SHAPE, shape);
    for k in 0..4u32 {
        let p = Obj(0xff85de + 0x200 * k);
        if p.u8(x.b, 0) == 0 {
            continue;
        }
        let class = p.u8(x.b, 3) >> 1;
        if class == 5 && p.u8(x.b, 0x6a) == 0x1a {
            continue;
        }
        if shape == 1 {
            shove_front(x.b, x.o, p);
        } else {
            shove_side(x.b, x.o, p);
        }
    }
}

/// Forme du corps `routine` (0x818e6 devant, 0x819e2 de côté) appliquée à l'objet `other` par le boss `me` ; vrai si
/// l'objet a été poussé (pour 0x81aa4 de `boss_common`).
pub fn pousser(b: &mut Cps2Bus, me: Obj, other: Obj, routine: u32) -> bool {
    match routine {
        0x818e6 => shove_front(b, me, other),
        0x819e2 => shove_side(b, me, other),
        _ => unreachable!("forme du corps du boss 3 en {routine:#x} : la table 0x81ad4 n'a que 0x818e6 et 0x819e2"),
    }
}

/// Adaptateur des natives (fuzz contre l'original) : A0 = le boss, A1 = l'objet ; D0 = 1 / 0 et ses drapeaux. Le
/// seul appelant d'origine, 0x81aa4 (`boss_common::push_out_of_boss`), ne relit ni D1 ni A2, que l'original laisse.
fn native(c: &mut Cpu, b: &mut Cps2Bus, f: fn(&mut Cps2Bus, Obj, Obj) -> bool) {
    c.d[0] = f(b, Obj(c.a[0]), Obj(c.a[1])) as u32;
    c.flags_logic(c.d[0], Size::Long);
    rts(c, b);
}

pub const SHOVE_FRONT: crate::Native = crate::Native {
    entry: 0x818e6,
    name: "boss_3_pousse_devant",
    run: crate::Body::Processeur(|c, b| native(c, b, shove_front)),
    touches_sound: false,
    outputs: crate::regs::d(0) | crate::regs::CCR,
};

pub const SHOVE_SIDE: crate::Native = crate::Native {
    entry: 0x819e2,
    name: "boss_3_pousse_cote",
    run: crate::Body::Processeur(|c, b| native(c, b, shove_side)),
    touches_sound: false,
    outputs: crate::regs::d(0) | crate::regs::CCR,
};

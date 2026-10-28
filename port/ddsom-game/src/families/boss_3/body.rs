//! Boss 3 : le corps repousse ce qui le touche. Deux formes (`+0xa4`) : 1 « devant » (pendant le cri, l'objet devant
//! le boss est poussé vers l'avant) et 2 « de côté » (pendant le vol bas, l'objet est écarté du boss). Une table par
//! forme donne, par tranche de 16 pixels de distance, la hauteur maximale, la demi-profondeur et la poussée.
//!
//! Les joueurs sont poussés par le boss lui-même (`push_players`) ; les autres objets appellent 0x81aa4
//! (`boss_common`), qui appelle ces formes par leur adresse d'origine (natives `SHOVE_FRONT`, `SHOVE_SIDE`).

use cps2::Cps2Bus;
use m68k::decode::Size;
use m68k::Cpu;

use super::*;
use crate::mem::{rd16, rd8, rts, wr16};

/// Une forme : tranches (octets, relatives au PC) et lignes (hauteur, demi-profondeur, poussée ; données).
struct Shape {
    slices: u32,
    rows: u32,
}

const FRONT: Shape = Shape { slices: 0x81950, rows: 0x81970 };
const SIDE: Shape = Shape { slices: 0x81a44, rows: 0x81a64 };

/// 0x81916 / 0x81a0a : poussée pour l'objet A1 à la distance `d` (D0) du boss A0, 0 hors de la forme.
/// Registres comme l'original : D0 = la poussée, D1, A2.
fn shove_amount(c: &mut Cpu, b: &mut Cps2Bus, shape: &Shape, d: u16) -> u16 {
    let (me, other) = (c.a[0], c.a[1]);
    let push = if (d as i16) < 0x200 {
        let slice = crate::mem::prog8(b, shape.slices + (d >> 4) as u32) as u32;
        let row = shape.rows + (slice << 3);
        let height = rd16(b, row);
        if (height as i16) < rd16(b, other + 0xc) as i16 {
            c.a[2] = row + 2;
            c.d[1] = slice << 3;
            0
        } else {
            c.a[2] = row + 4;
            let depth = rd16(b, row + 2);
            let mut dz = rd16(b, other + 0x10).wrapping_sub(rd16(b, me + 0x10));
            if (dz as i16) < 0 {
                dz = dz.wrapping_neg();
            }
            c.d[1] = dz as u32;
            if dz as i16 >= depth as i16 { 0 } else { rd16(b, row + 4) }
        }
    } else {
        0
    };
    c.d[0] = push as u32;
    push
}

/// 0x818e6 : forme 1 : l'objet A1 devant le boss A0 (jusqu'à 0x20 derrière lui) est poussé vers l'avant.
fn shove_front(c: &mut Cpu, b: &mut Cps2Bus) -> bool {
    let (me, other) = (c.a[0], c.a[1]);
    let ahead = rd8(b, me + 0x2c) != 0;
    let mut d = rd16(b, other + 0x8).wrapping_sub(rd16(b, me + 0x8));
    if !ahead {
        d = d.wrapping_neg();
    }
    d = d.wrapping_add(0x20);
    let moved = (d as i16) >= 0 && {
        let mut p = shove_amount(c, b, &FRONT, d);
        p != 0 && {
            if !ahead {
                p = p.wrapping_neg();
            }
            let ox = rd16(b, other + 0x8);
            wr16(b, other + 0x8, ox.wrapping_add(p));
            true
        }
    };
    c.d[0] = moved as u32;
    moved
}

/// 0x819e2 : forme 2 : l'objet A1 est écarté du boss A0, du côté où il se trouve.
fn shove_side(c: &mut Cpu, b: &mut Cps2Bus) -> bool {
    let (me, other) = (c.a[0], c.a[1]);
    let mut d = rd16(b, other + 0x8).wrapping_sub(rd16(b, me + 0x8));
    if (d as i16) < 0 {
        d = d.wrapping_neg();
    }
    let mut p = shove_amount(c, b, &SIDE, d);
    let moved = p != 0 && {
        let ox = rd16(b, other + 0x8);
        c.d[1] = c.d[1] & 0xffff_0000 | ox as u32;
        if (ox as i16) < rd16(b, me + 0x8) as i16 {
            p = p.wrapping_neg();
        }
        wr16(b, other + 0x8, ox.wrapping_add(p));
        true
    };
    c.d[0] = moved as u32;
    moved
}

/// 0x818b2 (`shape` = 1) et 0x819b0 (2) : le boss pousse chaque joueur présent, sauf l'elfe (classe 5) dans son
/// état 0x1a.
pub(super) fn push_players(x: &mut Cx, shape: u8) {
    x.set8(BODY_SHAPE, shape);
    for k in 0..4u32 {
        let p = 0xff85de + 0x200 * k;
        x.c.d[6] = x.c.d[6] & 0xffff_0000 | (3 - k);
        if rd8(x.b, p) == 0 {
            continue;
        }
        let class = rd8(x.b, p + 3) >> 1;
        x.c.d[0] = x.c.d[0] & !0xff | class as u32;
        if class == 5 && rd8(x.b, p + 0x6a) == 0x1a {
            continue;
        }
        x.c.a[0] = x.o.0;
        x.c.a[1] = p;
        if shape == 1 {
            shove_front(x.c, x.b);
        } else {
            shove_side(x.c, x.b);
        }
    }
    x.c.d[6] = x.c.d[6] & 0xffff_0000 | 0xffff;
    x.c.a[1] = 0xff85de + 0x800;
}

fn native(c: &mut Cpu, b: &mut Cps2Bus, f: fn(&mut Cpu, &mut Cps2Bus) -> bool) {
    f(c, b);
    c.flags_logic(c.d[0], Size::Long);
    rts(c, b);
}

pub const SHOVE_FRONT: crate::Native = crate::Native {
    entry: 0x818e6,
    name: "boss_3_pousse_devant",
    run: crate::Corps::Processeur(|c, b| native(c, b, shove_front)),
    touches_sound: false,
    outputs: crate::regs::d(0) | crate::regs::d(1) | crate::regs::a(2) | crate::regs::CCR,
};

pub const SHOVE_SIDE: crate::Native = crate::Native {
    entry: 0x819e2,
    name: "boss_3_pousse_cote",
    run: crate::Corps::Processeur(|c, b| native(c, b, shove_side)),
    touches_sound: false,
    outputs: crate::regs::d(0) | crate::regs::d(1) | crate::regs::a(2) | crate::regs::CCR,
};

//! Boss 3: the body pushes back whatever touches it. Two shapes (`+0xa4`): 1 "front" (during the cry, the object in
//! front of the boss is pushed forward) and 2 "sideways" (during the low flight, the object is pushed away from the
//! boss). A table per shape gives, per 16-pixel distance slice, the maximum height, the half-depth and the push.
//!
//! The players are pushed by the boss itself (`push_players`); the other objects call 0x81aa4
//! (`boss_common`), which calls these shapes through `push`.

use cps2::Cps2Bus;
use m68k::decode::Size;
use m68k::Cpu;

use super::*;
use crate::families::cx::Bench;
use crate::mem::{rd16, rts};

/// A shape: slices (bytes, PC-relative) and rows (height, half-depth, push; data).
struct Shape {
    slices: u32,
    rows: u32,
}

const FRONT: Shape = Shape { slices: 0x81950, rows: 0x81970 };
const SIDE: Shape = Shape { slices: 0x81a44, rows: 0x81a64 };

/// 0x81916 / 0x81a0a: push for object `other` at distance `d` from boss `me`, 0 outside the shape (object height
/// above the slice, or too far in depth).
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

/// 0x818e6: shape 1: object `other` in front of boss `me` (up to 0x20 behind it) is pushed forward.
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

/// 0x819e2: shape 2: object `other` is pushed away from boss `me`, on the side where it is.
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

/// 0x818b2 (`shape` = 1) and 0x819b0 (2): the boss pushes each present player, except the elf (class 5) in its
/// state 0x1a.
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

/// Body shape `routine` (0x818e6 front, 0x819e2 sideways) applied to object `other` by boss `me`; true if
/// the object was pushed (for 0x81aa4 of `boss_common`).
pub fn push(b: &mut Cps2Bus, me: Obj, other: Obj, routine: u32) -> bool {
    match routine {
        0x818e6 => shove_front(b, me, other),
        0x819e2 => shove_side(b, me, other),
        _ => unreachable!("boss 3 body shape at {routine:#x}: table 0x81ad4 only has 0x818e6 and 0x819e2"),
    }
}

/// Adapter for the natives (fuzz against the original): A0 = the boss, A1 = the object; D0 = 1 / 0 and its flags. The
/// only original caller, 0x81aa4 (`boss_common::push_out_of_boss`), re-reads neither D1 nor A2, which the original
/// leaves.
fn native(c: &mut Cpu, b: &mut Cps2Bus, f: fn(&mut Cps2Bus, Obj, Obj) -> bool) {
    c.d[0] = f(b, Obj(c.a[0]), Obj(c.a[1])) as u32;
    c.flags_logic(c.d[0], Size::Long);
    rts(c, b);
}

pub const SHOVE_FRONT: crate::Native = crate::Native {
    entry: 0x818e6,
    name: "boss_3_pushes_front",
    run: crate::Body::WithCpu(|c, b| native(c, b, shove_front)),
    touches_sound: false,
    outputs: crate::regs::d(0) | crate::regs::CCR,
};

pub const SHOVE_SIDE: crate::Native = crate::Native {
    entry: 0x819e2,
    name: "boss_3_pushes_side",
    run: crate::Body::WithCpu(|c, b| native(c, b, shove_side)),
    touches_sound: false,
    outputs: crate::regs::d(0) | crate::regs::CCR,
};

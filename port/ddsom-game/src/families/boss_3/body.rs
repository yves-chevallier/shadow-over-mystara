//! Boss 3: the body pushes back whatever touches it. Two shapes (`+0xa4`): 1 "front" (during the cry, the object in
//! front of the boss is pushed forward) and 2 "sideways" (during the low flight, the object is pushed away from the
//! boss). A table per shape gives, per 16-pixel distance slice, the maximum height, the half-depth and the push.
//!
//! The players are pushed by the boss itself (`push_players`); the other objects call 0x81aa4
//! (`boss_common`), which calls these shapes through `push`.

use cps2::Cps2Bus;

use super::*;
use crate::families::cx::Bench;

/// A shape: its tables of slices (row per 16-pixel slice of distance) and rows (height, half-depth, push).
struct Shape {
    slices: &'static str,
    rows: &'static str,
}

const FRONT: Shape = Shape { slices: "boss_3/front_slices", rows: "boss_3/front_rows" };
const SIDE: Shape = Shape { slices: "boss_3/side_slices", rows: "boss_3/side_rows" };

/// 0x81916 / 0x81a0a: push for object `other` at distance `d` from boss `me`, 0 outside the shape (object height
/// above the slice, or too far in depth).
fn shove_amount(b: &mut Cps2Bus, shape: &Shape, me: Obj, other: Obj, d: u16) -> u16 {
    if (d as i16) >= 0x200 {
        return 0;
    }
    let slice = tables::table(shape.slices).at8(b, (d >> 4) as u32) as u32;
    let rows = tables::table(shape.rows);
    let row = slice << 3;
    let height = rows.at16(b, row);
    if (height as i16) < other.u16(b, Obj::Z) as i16 {
        return 0;
    }
    let depth = rows.at16(b, row + 2);
    let mut dz = other.u16(b, Obj::Y).wrapping_sub(me.u16(b, Obj::Y));
    if (dz as i16) < 0 {
        dz = dz.wrapping_neg();
    }
    if dz as i16 >= depth as i16 { 0 } else { rows.at16(b, row + 4) }
}

/// 0x818e6: shape 1: object `other` in front of boss `me` (up to 0x20 behind it) is pushed forward.
pub(crate) fn shove_front(b: &mut Cps2Bus, me: Obj, other: Obj) -> bool {
    let ahead = me.u8(b, Enemy::FLIP) != 0;
    let mut d = other.u16(b, Obj::X).wrapping_sub(me.u16(b, Obj::X));
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
    let ox = other.u16(b, Obj::X);
    other.set_u16(b, Obj::X, ox.wrapping_add(p));
    true
}

/// 0x819e2: shape 2: object `other` is pushed away from boss `me`, on the side where it is.
pub(crate) fn shove_side(b: &mut Cps2Bus, me: Obj, other: Obj) -> bool {
    let mut d = other.u16(b, Obj::X).wrapping_sub(me.u16(b, Obj::X));
    if (d as i16) < 0 {
        d = d.wrapping_neg();
    }
    let mut p = shove_amount(b, &SIDE, me, other, d);
    if p == 0 {
        return false;
    }
    let ox = other.u16(b, Obj::X);
    if (ox as i16) < me.u16(b, Obj::X) as i16 {
        p = p.wrapping_neg();
    }
    other.set_u16(b, Obj::X, ox.wrapping_add(p));
    true
}

/// 0x818b2 (`shape` = 1) and 0x819b0 (2): the boss pushes each present player, except the elf (class 5) in its
/// state 0x1a.
pub(super) fn push_players<R: Bench>(x: &mut Cx<'_, R>, shape: u8) {
    x.set8(Boss3::BODY_SHAPE, shape);
    for k in 0..4u32 {
        let p = Obj(0xff85de + 0x200 * k);
        if p.u8(x.b, Enemy::ACTIVE) == 0 {
            continue;
        }
        let class = p.u8(x.b, Enemy::VARIANT) >> 1;
        if class == 5 && p.u8(x.b, Enemy::AI_WANDER_ROW) == 0x1a {
            continue;
        }
        if shape == 1 {
            shove_front(x.b, x.o, p);
        } else {
            shove_side(x.b, x.o, p);
        }
    }
}

/// Body shape `shape` (1 front, 0x818e6; 2 sideways, 0x819e2) applied to object `other` by boss `me`; true if the
/// object was pushed (for 0x81aa4 of `boss_common`, which jumps through the table 0x81ad4 of the two shapes).
pub fn push(b: &mut Cps2Bus, me: Obj, other: Obj, shape: u8) -> bool {
    match shape {
        1 => shove_front(b, me, other),
        2 => shove_side(b, me, other),
        _ => bad_index(shape),
    }
}

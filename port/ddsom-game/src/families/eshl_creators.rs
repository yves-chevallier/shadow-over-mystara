//! Enemy shot (ESHL) creators called by the enemy and boss handlers: each allocates one or more ESHL objects
//! (`0x22ea`), sets the type, the variant and the link to the creator.
//!
//! Each creator is an ordinary Rust function (creator and settings as arguments; the last object created returned,
//! `None` if the pool is full), written once on a [`Regs`] like those of `effects_creators`: the core ([`Pure`])
//! touches no register; the native, for the remaining calls by address, passes the processor, which receives what the
//! original leaves (A1 = the last object allocated, carry set if the pool is full, flags of the last write, loop
//! counters: `dbf` leaves the low word at -1).

use cps2::Cps2Bus;

use super::effects_creators::leftovers::Regs;
use crate::mem::{rd16, wr16, wr32, wr8, A5};
use super::shell::tables::{held8, tables, Read};
use ddsom_data::effects::Table;
use crate::objects::pools;
use crate::objects::Obj;
use crate::ops::Size;

mod f {
    pub const ACTIVE: u32 = 0x00;
    pub const KIND: u32 = 0x02;
    pub const VARIANT: u32 = 0x03;
    pub const X: u32 = 0x08;
    pub const Z: u32 = 0x0c;
    pub const Y: u32 = 0x10;
    pub const VX: u32 = 0x14;
    pub const VZ: u32 = 0x18;
    pub const SWING: u32 = 0x1c;
    pub const MODE: u32 = 0x2a;
    pub const FLIP: u32 = 0x2c;
    pub const DIRECTION: u32 = 0x2d;
    pub const INDEX: u32 = 0x2f;
    pub const CREATOR: u32 = 0x42;
    pub const PARENT: u32 = 0x46;
    pub const OWNER: u32 = 0x50;
    pub const LINK: u32 = 0x5c;
}

/// Dynamic rank (`A5+0x7f92`, word): chooses the number of bursts of the rows.
const RANK: u32 = A5 + 0x7f92;
/// Number of type 5 shots beyond which 0x9e426 creates no more (`A5+0x7e64`, word).
const LIMIT_TWELVE: u32 = A5 + 0x7e64;

/// Allocates an ESHL of type `kind` (active); `None` (carry set) if the pool is full.
fn spawn(r: &mut impl Regs, b: &mut Cps2Bus, kind: u8) -> Option<Obj> {
    let o = r.allocated(b, &pools::ESHL)?;
    o.set_u8(b, f::ACTIVE, 1);
    o.set_u8(b, f::KIND, kind);
    Some(o)
}

/// Copies the creator's position (words X, Z, Y).
fn copy_position(b: &mut Cps2Bus, from: Obj, to: Obj) {
    for off in [f::X, f::Z, f::Y] {
        let v = from.u16(b, off);
        to.set_u16(b, off, v);
    }
}

/// Numbered fragments: `count` objects of type `kind`, variant `variant`, index (`+0x2f`) from `count - 1` down to 0
/// (except `skip`), linked to the creator (`+0x46`), at its position; orientation: the creator's, or `flip` imposed.
/// D1.w serves as counter. False if the pool is full before the end.
fn fragments(r: &mut impl Regs, b: &mut Cps2Bus, me: Obj, kind: u8, variant: u8, count: u16, skip: Option<u8>, flip: Option<u8>, creator_kind: bool) -> bool {
    let mut i = count.wrapping_sub(1);
    loop {
        r.d_word(1, i);
        if skip != Some(i as u8) {
            let Some(o) = spawn(r, b, kind) else { return false };
            o.set_u8(b, f::VARIANT, variant);
            o.set_u8(b, f::INDEX, i as u8);
            o.set_u16(b, f::PARENT, me.word());
            copy_position(b, me, o);
            if creator_kind {
                let k = me.u8(b, f::KIND);
                o.set_u8(b, f::CREATOR, k);
            }
            let v = flip.unwrap_or_else(|| me.u8(b, f::FLIP));
            o.set_u8(b, f::FLIP, v);
            r.flags(v as u32, Size::Byte);
        }
        i = i.wrapping_sub(1);
        if i == 0xffff {
            r.d_word(1, i);
            return true;
        }
    }
}

/// 0xa7b0a: bursts of boss 11 (type 0x44); a draw chooses their number and the one that is missing (table 0xa7b64:
/// pairs number, absent).
pub(crate) fn boss11_shards(r: &mut impl Regs, b: &mut Cps2Bus, me: Obj) {
    let i = crate::rng::random(b) & 0xe;
    r.d_word(0, i);
    let count = tables().boss11_shard_counts.at8(b, i as u32);
    let missing = tables().boss11_shard_counts.at8(b, i as u32 + 1);
    r.d(1, count as u32);
    r.d_byte(2, missing);
    fragments(r, b, me, 0x44, 0, count as u16, Some(missing), None, false);
}

/// 0xa7570: fragments of type 0x42 of variant `variant`, their number read from table 0xa75b6.
pub(crate) fn fragments_42(r: &mut impl Regs, b: &mut Cps2Bus, me: Obj, variant: u16) {
    let count = tables().fragments_42_counts.at8(b, variant as i16 as i32 as u32);
    r.d(1, count as u32);
    fragments(r, b, me, 0x42, variant as u8, count as u16, None, None, false);
}

/// 0xa7da2: two rows of fragments of type 0x45 (number read at 0xa7d92), turned right (with the creator's type in
/// `+0x42`) then left.
pub(crate) fn fragments_45(r: &mut impl Regs, b: &mut Cps2Bus, me: Obj) {
    r.d(0, 0);
    let count = tables().fragments_45_count.at8(b, 0) as u16;
    r.d(2, count as u32);
    r.d(1, count as u32);
    if fragments(r, b, me, 0x45, 0, count, None, Some(0), true) {
        r.d_word(1, count);
        fragments(r, b, me, 0x45, 0, count, None, Some(0x20), false);
    }
}

/// Series of objects linked by `+0x5c`: variants `last` down to 0 (D5), `+0x2f` = `index`.
fn series(r: &mut impl Regs, b: &mut Cps2Bus, me: Obj, kind: u8, last: u8, index: u8) {
    let mut n = last as u16;
    r.d(5, n as u32);
    while let Some(o) = spawn(r, b, kind) {
        o.set_u8(b, f::VARIANT, n as u8);
        o.set_u8(b, f::INDEX, index);
        o.set_u16(b, f::LINK, me.word());
        r.flags(me.word() as u32, Size::Word);
        n = n.wrapping_sub(1);
        r.d_word(5, n);
        if n == 0xffff {
            break;
        }
    }
}

/// 0xa4e84: 17 objects of type 0x36 (variants 16 down to 0), `+0x2f` = `index`.
pub(crate) fn series_36(r: &mut impl Regs, b: &mut Cps2Bus, me: Obj, index: u8) {
    series(r, b, me, 0x36, 0x10, index);
}

/// 0xa5c9a: 16 objects of type 0x3b (variants 15 down to 0), `+0x2f` = 1.
pub(crate) fn series_3b(r: &mut impl Regs, b: &mut Cps2Bus, me: Obj) {
    series(r, b, me, 0x3b, 0xf, 1);
}

/// An object linked to the creator by `+0x5c`, of type `kind`, variant `variant` (None: not set), `+0x2f` = `index`.
fn linked(r: &mut impl Regs, b: &mut Cps2Bus, me: Obj, kind: u8, variant: Option<u8>, index: u8) -> Option<Obj> {
    let o = spawn(r, b, kind)?;
    if let Some(v) = variant {
        o.set_u8(b, f::VARIANT, v);
    }
    o.set_u8(b, f::INDEX, index);
    o.set_u16(b, f::LINK, me.word());
    r.flags(me.word() as u32, Size::Word);
    Some(o)
}

/// 0xa5c7c: an object of type 0x3b, `+0x2f` = 0.
pub(crate) fn single_3b(r: &mut impl Regs, b: &mut Cps2Bus, me: Obj) -> Option<Obj> {
    linked(r, b, me, 0x3b, None, 0)
}

/// 0xa6e6a: an object of type 0x40, variant 0, `+0x2f` = 5.
pub(crate) fn single_40(r: &mut impl Regs, b: &mut Cps2Bus, me: Obj) -> Option<Obj> {
    linked(r, b, me, 0x40, Some(0), 5)
}

/// The creator's orientation is copied into the object A1 (`move.b D0b,(0x2c,A1)`). Original glitch: when the pool is
/// full, A1 is the leftover of the allocation (first slot or end of the pool): the byte is written there anyway (type 1
/// of `thrown`, `sweeping_8`).
fn copy_flip(r: &mut impl Regs, b: &mut Cps2Bus, me: Obj, a1: Obj) {
    let flip = me.u8(b, f::FLIP);
    r.d_byte(0, flip);
    wr8(b, a1.0 + f::FLIP, flip);
    r.flags(flip as u32, Size::Byte);
}

/// Shot launched in front of the creator (types 1 and 2): `+0x2a` = 2, angle 8 (0x18 and speed reversed if it faces
/// right), orientation of the creator. `always_flip`: the orientation is copied into A1 even when the pool is full
/// (type 1, see `copy_flip`).
fn thrown(r: &mut impl Regs, b: &mut Cps2Bus, me: Obj, kind: u8, vx: Option<u32>, always_flip: bool) -> Option<Obj> {
    let a1 = match r.alloc_or_rest(b, &pools::ESHL) {
        Ok(o) => {
            o.set_u8(b, f::ACTIVE, 1);
            o.set_u8(b, f::KIND, kind);
            if kind == 1 {
                o.set_u8(b, f::INDEX, 0);
            }
            o.set_u8(b, f::MODE, 2);
            o.set_u16(b, f::OWNER, me.word());
            if let Some(v) = vx {
                o.set_u32(b, f::VX, v);
            }
            o.set_u32(b, f::VZ, 0);
            o.set_u8(b, f::DIRECTION, 8);
            if me.u8(b, f::FLIP) == 0 {
                o.set_u8(b, f::DIRECTION, 0x18);
                let v = o.u16(b, f::VX).wrapping_neg();
                o.set_u16(b, f::VX, v);
            }
            o
        }
        Err(_) if !always_flip => return None,
        Err(remainder) => remainder,
    };
    copy_flip(r, b, me, a1);
    Some(a1)
}

/// 0x9dda8: type 1 shot (speed left at zero by the allocation).
pub(crate) fn thrown_1(r: &mut impl Regs, b: &mut Cps2Bus, me: Obj) -> Option<Obj> {
    thrown(r, b, me, 1, None, true)
}

/// 0x9e062: type 2 shot, speed 5 pixels per frame.
pub(crate) fn thrown_2(r: &mut impl Regs, b: &mut Cps2Bus, me: Obj) -> Option<Obj> {
    thrown(r, b, me, 2, Some(0x0500_0000), false)
}

/// Cores of 0x9dda8 and 0x9e062 for the handlers that receive the shot to launch as a parameter.
pub(crate) fn throw_1(b: &mut Cps2Bus, me: Obj) {
    thrown_1(&mut super::effects_creators::leftovers::Pure, b, me);
}
pub(crate) fn throw_2(b: &mut Cps2Bus, me: Obj) {
    thrown_2(&mut super::effects_creators::leftovers::Pure, b, me);
}

/// 0x9ed78: type 8 shot that sweeps: its angle (`+0x1c`, ±8 per shot between -0x40 and 0x40, direction in `+0x42`) is
/// taken from the creator, advanced then returned to the creator.
pub(crate) fn sweeping_8(r: &mut impl Regs, b: &mut Cps2Bus, me: Obj) {
    let a1 = match r.alloc_or_rest(b, &pools::ESHL) {
        Ok(o) => {
            o.set_u8(b, f::ACTIVE, 1);
            o.set_u8(b, f::KIND, 8);
            o.set_u8(b, f::MODE, 2);
            o.set_u16(b, f::OWNER, me.word());
            let swing = me.u16(b, f::SWING);
            o.set_u16(b, f::SWING, swing);
            let back = me.u8(b, f::CREATOR);
            o.set_u8(b, f::CREATOR, back);
            if back == 0 {
                let s = swing.wrapping_add(8);
                o.set_u16(b, f::SWING, s);
                if s as i16 > 0x40 {
                    o.set_u8(b, f::CREATOR, back.wrapping_add(1));
                }
            } else {
                let s = swing.wrapping_sub(8);
                o.set_u16(b, f::SWING, s);
                if (s as i16) < -0x40 {
                    o.set_u8(b, f::CREATOR, 0);
                }
            }
            let s = o.u16(b, f::SWING);
            me.set_u16(b, f::SWING, s);
            let d = o.u8(b, f::CREATOR);
            me.set_u8(b, f::CREATOR, d);
            o.set_u32(b, f::VX, 0x0280_0000);
            o.set_u32(b, f::VZ, 0);
            if me.u8(b, f::FLIP) == 0 {
                let v = o.u16(b, f::VX).wrapping_neg();
                o.set_u16(b, f::VX, v);
            }
            o
        }
        Err(remainder) => remainder,
    };
    copy_flip(r, b, me, a1);
}

/// 0x9eba6: type 6 shot, variant `variant`, orientation of the creator.
pub(crate) fn shot_6(r: &mut impl Regs, b: &mut Cps2Bus, me: Obj, variant: u8) -> Option<Obj> {
    let o = spawn(r, b, 6)?;
    o.set_u8(b, f::VARIANT, variant);
    let flip = me.u8(b, f::FLIP);
    r.d_byte(0, flip);
    o.set_u8(b, f::FLIP, flip);
    o.set_u16(b, f::OWNER, me.word());
    r.flags(me.word() as u32, Size::Word);
    Some(o)
}

/// Objects of type `kind` linked to the creator (`link`: `+0x5c`, otherwise `+0x50`), variants `count - 1` down to 0
/// (D1); stops if the pool is full (false, carry set).
fn ring(r: &mut impl Regs, b: &mut Cps2Bus, me: Obj, kind: u8, count: u16, link: u32) -> bool {
    let mut n = count - 1;
    r.d(1, n as u32);
    loop {
        let Some(o) = spawn(r, b, kind) else { return false };
        o.set_u8(b, f::VARIANT, n as u8);
        o.set_u16(b, link, me.word());
        r.flags(me.word() as u32, Size::Word);
        n = n.wrapping_sub(1);
        r.d_word(1, n);
        if n == 0xffff {
            return true;
        }
    }
}

/// 0x9e194: four objects of type 3 (variants 3 down to 0, D1), linked by `+0x50`; false if the pool is full.
pub(crate) fn four_3(r: &mut impl Regs, b: &mut Cps2Bus, me: Obj) -> bool {
    ring(r, b, me, 3, 4, f::OWNER)
}

/// 0x9e2a0: object of type 4 above the creator (Z = 0x28), variant = `+0x2f` of the creator, linked by a long in
/// `+0x46`; the creator receives 0x8000 in `+0x12`.
pub(crate) fn above_4(r: &mut impl Regs, b: &mut Cps2Bus, me: Obj) -> Option<Obj> {
    let o = spawn(r, b, 4)?;
    wr32(b, o.0 + f::PARENT, me.0);
    let v = me.u8(b, f::INDEX);
    o.set_u8(b, f::VARIANT, v);
    let (x, y) = (me.u32(b, f::X), me.u32(b, f::Y));
    o.set_u32(b, f::X, x);
    o.set_u32(b, f::Y, y);
    o.set_u16(b, f::Z, 0x28);
    wr16(b, me.0 + 0x12, 0x8000);
    let flip = me.u8(b, f::FLIP);
    o.set_u8(b, f::FLIP, flip);
    r.flags(flip as u32, Size::Byte);
    Some(o)
}

/// 0x9eede: two objects of type 9 (variants 1 and 2), the second linked to the first; D0 = 1.
pub(crate) fn pair_9(r: &mut impl Regs, b: &mut Cps2Bus, me: Obj) -> Option<Obj> {
    let first = spawn(r, b, 9)?;
    first.set_u8(b, f::VARIANT, 1);
    first.set_u16(b, f::PARENT, me.word());
    r.a(2, first.word() as i16 as i32 as u32);
    let second = spawn(r, b, 9)?;
    second.set_u8(b, f::VARIANT, 2);
    second.set_u16(b, f::PARENT, first.word());
    r.d(0, 1);
    r.flags(1, Size::Long);
    Some(second)
}

/// 0x9dbd6: object of type 0, variant = `+0x2f` of the creator, linked by `+0x42` and `+0x50`.
pub(crate) fn shot_0(r: &mut impl Regs, b: &mut Cps2Bus, me: Obj) -> Option<Obj> {
    let o = spawn(r, b, 0)?;
    o.set_u16(b, f::CREATOR, me.word());
    o.set_u16(b, f::OWNER, me.word());
    let v = me.u8(b, f::INDEX);
    o.set_u8(b, f::VARIANT, v);
    let flip = me.u8(b, f::FLIP);
    o.set_u8(b, f::FLIP, flip);
    r.flags(flip as u32, Size::Byte);
    Some(o)
}

/// 0x9e426: twelve objects of type 5 (`+0x2f` = 0 to 11, linked by `+0x46`) if `count` is less than the word
/// `A5+0x7e64`; true (D0 = 1) if they are created.
pub(crate) fn twelve_5(r: &mut impl Regs, b: &mut Cps2Bus, me: Obj, count: u16) -> bool {
    r.d(1, 12);
    let limit = rd16(b, LIMIT_TWELVE);
    if (count as i16) >= limit as i16 {
        r.d(0, 0);
        r.flags(0, Size::Long);
        return false;
    }
    r.d_byte(1, 11);
    r.d(2, 0);
    for i in 0..12u16 {
        // Original glitch: the allocation failure is not tested (see `Pool::alloc_unchecked`).
        let o = r.alloc_without_test(b, &pools::ESHL);
        o.set_u8(b, f::ACTIVE, 1);
        o.set_u8(b, f::KIND, 5);
        o.set_u8(b, f::INDEX, i as u8);
        o.set_u16(b, f::PARENT, me.word());
        r.d_word(2, i + 1);
        r.d_word(1, 10u16.wrapping_sub(i));
    }
    r.d(0, 1);
    r.flags(1, Size::Long);
    true
}

/// Row of bursts (types 0x19 and 0x1a): `count` + 1 objects (D5.w) numbered D3 = 0, 1..., `+0x58` = 1, 9, 17... (D4),
/// with the creator's `+0x5c`; `directions`: byte `+0x2d` of each (type 0x1a), read from a table that advances (A2).
fn row(r: &mut impl Regs, b: &mut Cps2Bus, me: Obj, kind: u8, mut count: u16, mut directions: Option<u32>) -> Option<Obj> {
    let (mut n3, mut n4) = (0u8, 1u16);
    r.d(3, 0);
    r.d(4, 1);
    loop {
        let o = spawn(r, b, kind)?;
        o.set_u8(b, f::VARIANT, n3);
        if let Some(at) = directions.as_mut() {
            let d = held8(b, *at);
            *at += 1;
            r.a(2, *at);
            o.set_u8(b, f::DIRECTION, d);
        }
        o.set_u16(b, 0x58, n4);
        n3 = n3.wrapping_add(1);
        n4 = n4.wrapping_add(8);
        r.d_byte(3, n3);
        r.d_word(4, n4);
        let link = me.u16(b, f::LINK);
        o.set_u16(b, f::LINK, link);
        r.flags(link as u32, Size::Word);
        count = count.wrapping_sub(1);
        r.d_word(5, count);
        if count == 0xffff {
            return Some(o);
        }
    }
}

/// Number of bursts minus one, read from a (program) table indexed by rank / 2.
fn count_by_rank(r: &mut impl Regs, b: &mut Cps2Bus, table: &Table) -> u8 {
    let i = rd16(b, RANK) >> 1;
    r.d_word(0, i);
    table.at8(b, i as i16 as i32 as u32)
}

/// 0xa1670: row of bursts of type 0x19 (number drawn from table 0xa16ac).
pub(crate) fn row_19(r: &mut impl Regs, b: &mut Cps2Bus, me: Obj) -> Option<Obj> {
    let n = count_by_rank(r, b, &tables().bursts_19_counts);
    r.d(5, n as u32);
    row(r, b, me, 0x19, n as u16, None)
}

/// 0xa17d0: row of bursts of type 0x1a (`kind`, stored in `+0x56`); their directions come from a table (in the clear)
/// chosen by the kind, or by the position of the target `+0x6c`, then by chance. Original glitch: `move.b (table),D5.b`
/// without `moveq #0,D5` (unlike 0xa1670): the high byte of the word D5 left by the caller (`d5`, the crate
/// `boxes::effect_special`) is added to the count of the `dbf D5.w` loop, which can then fill the ESHL pool.
pub(crate) fn row_1a(r: &mut impl Regs, b: &mut Cps2Bus, me: Obj, kind: u16, d5: u16) -> Option<Obj> {
    me.set_u16(b, 0x56, kind);
    let n = count_by_rank(r, b, &tables().bursts_1a_counts);
    r.d_byte(5, n);
    // the four direction tables of `bursts_1a_directions`, 0x20 bytes each; the row reads them by address (A2)
    let directions = tables().bursts_1a_directions.rom;
    let table = directions + match kind as u8 {
        1 => 0,
        2 => 0x20,
        _ => {
            let target = me.u16(b, 0x6c) as i16 as i32 as u32;
            r.a(1, target);
            let tx = rd16(b, target + f::X);
            r.d_word(1, tx);
            if tx <= me.u16(b, f::X) { 0x40 } else { 0x60 }
        }
    };
    let rng = (crate::rng::random(b) & 3) << 3;
    r.d_word(0, rng);
    row(r, b, me, 0x1a, d5 & 0xff00 | n as u16, Some(table + rng as u32))
}

/// 0xa1c6a: four objects of type 0x1c (variants 0 to 3, `+0x5c` of the creator), then the sound 0xebef8.
pub(crate) fn four_1c(r: &mut impl Regs, b: &mut Cps2Bus, me: Obj) {
    r.d(4, 0);
    r.d(5, 3);
    for k in 0..4u16 {
        let Some(o) = spawn(r, b, 0x1c) else { break };
        o.set_u8(b, f::VARIANT, k as u8);
        r.d_byte(4, k as u8 + 1);
        let link = me.u16(b, f::LINK);
        o.set_u16(b, f::LINK, link);
        r.d_word(5, 2u16.wrapping_sub(k));
    }
    crate::sound::play(b, crate::sound::CMD_13D_23D, me);
}

/// 0xa31c2: three objects of type 0x28.
pub(crate) fn ring_28(r: &mut impl Regs, b: &mut Cps2Bus, me: Obj) {
    ring(r, b, me, 0x28, 3, f::LINK);
}

/// 0xa2f96: sixteen objects of type 0x27.
pub(crate) fn ring_27(r: &mut impl Regs, b: &mut Cps2Bus, me: Obj) {
    ring(r, b, me, 0x27, 16, f::LINK);
}

/// 0xa6824: two objects of type 0x3f linked to the creator, turned right then left (variant 0).
pub(crate) fn pair_3f(r: &mut impl Regs, b: &mut Cps2Bus, me: Obj) {
    for flip in [0, 0x20] {
        let Some(o) = spawn(r, b, 0x3f) else { break };
        o.set_u16(b, f::LINK, me.word());
        o.set_u8(b, f::FLIP, flip);
        o.set_u8(b, f::VARIANT, 0);
        r.flags(0, Size::Byte);
    }
}

/// 0xa3354: an object of type 0x29 linked to the creator.
pub(crate) fn single_29(r: &mut impl Regs, b: &mut Cps2Bus, me: Obj) -> Option<Obj> {
    let o = spawn(r, b, 0x29)?;
    o.set_u16(b, f::LINK, me.word());
    r.flags(me.word() as u32, Size::Word);
    Some(o)
}

/// 0x9ddee: a type 1 shot (`+0x2f` = 1) of variant `variant`, linked to the creator by `+0x50`.
pub(crate) fn single_1(r: &mut impl Regs, b: &mut Cps2Bus, me: Obj, variant: u8) -> Option<Obj> {
    let o = spawn(r, b, 1)?;
    o.set_u8(b, f::INDEX, 1);
    o.set_u8(b, f::VARIANT, variant);
    o.set_u16(b, f::OWNER, me.word());
    r.flags(me.word() as u32, Size::Word);
    Some(o)
}

/// 0xa460a, 0xa464e, 0xa4676 and 0xa462c: an object of type 0x34 (`+0x2f` = 0) of variant 2, 1 (with the creator's
/// orientation), 0 and 3, linked by `+0x46`.
pub(crate) fn linked_34(r: &mut impl Regs, b: &mut Cps2Bus, me: Obj, variant: u8) -> Option<Obj> {
    let o = spawn(r, b, 0x34)?;
    o.set_u8(b, f::VARIANT, variant);
    o.set_u8(b, f::INDEX, 0);
    if variant == 1 {
        let flip = me.u8(b, f::FLIP);
        o.set_u8(b, f::FLIP, flip);
    }
    o.set_u16(b, f::PARENT, me.word());
    r.flags(me.word() as u32, Size::Word);
    Some(o)
}

/// 0xa5f38: an object of type 0x3c, variant 2, `+0x2f` and links of the creator (`+0x2f` copied, `+0x5c` = itself).
pub(crate) fn single_3c(r: &mut impl Regs, b: &mut Cps2Bus, me: Obj) -> Option<Obj> {
    let o = spawn(r, b, 0x3c)?;
    o.set_u8(b, f::VARIANT, 2);
    let v = me.u8(b, f::INDEX);
    o.set_u8(b, f::INDEX, v);
    o.set_u16(b, f::LINK, me.word());
    r.flags(me.word() as u32, Size::Word);
    Some(o)
}

/// 0xa2b4a: an object of type 0x25 linked to the creator (`+0x5c`).
pub(crate) fn single_25(r: &mut impl Regs, b: &mut Cps2Bus, me: Obj) -> Option<Obj> {
    let o = spawn(r, b, 0x25)?;
    o.set_u16(b, f::LINK, me.word());
    r.flags(me.word() as u32, Size::Word);
    Some(o)
}

/// 0xa1360: an object of type 0x18 that inherits the creator's `+0x5c`, then the sound 0xebe14.
pub(crate) fn single_18(r: &mut impl Regs, b: &mut Cps2Bus, me: Obj) {
    if let Some(o) = spawn(r, b, 0x18) {
        let link = me.u16(b, f::LINK);
        o.set_u16(b, f::LINK, link);
        r.flags(link as u32, Size::Word);
    }
    crate::sound::play(b, crate::sound::CMD_34B_44B, me);
}

/// 0xa1a40: an object of type 0x1b; the creator keeps `kind` in `+0x56`, the object takes its `+0x5c` and its target
/// (`+0x6c` -> `+0x50`).
pub(crate) fn single_1b(r: &mut impl Regs, b: &mut Cps2Bus, me: Obj, kind: u16) -> Option<Obj> {
    let o = spawn(r, b, 0x1b)?;
    me.set_u16(b, 0x56, kind);
    let link = me.u16(b, f::LINK);
    o.set_u16(b, f::LINK, link);
    let target = me.u16(b, 0x6c);
    o.set_u16(b, f::OWNER, target);
    r.flags(target as u32, Size::Word);
    Some(o)
}

/// 0xa44bc: three objects of type 0x33 (variant `variant`) around the position of `target`, at the offsets of table
/// 0xa4516 (two words per object) and numbered 2 down to 0; the last one created (D0 = 1, A1), `None` (D0 = 0) if the
/// pool is full before the end.
pub(crate) fn chasers(r: &mut impl Regs, b: &mut Cps2Bus, me: Obj, variant: u8, target: Obj) -> Option<Obj> {
    let mut d4 = 1u16;
    r.d(4, 1);
    r.d(5, 2);
    let mut last = None;
    for n in (0..3u16).rev() {
        let Some(o) = spawn(r, b, 0x33) else {
            r.d(0, 0);
            r.flags(0, Size::Long);
            return None;
        };
        last = Some(o);
        o.set_u8(b, f::VARIANT, variant);
        o.set_u8(b, f::INDEX, n as u8);
        let (x, y) = (target.u16(b, f::X), target.u16(b, f::Y));
        let i = n << 2;
        r.d_word(3, i);
        let (dx, dy) = (tables().chasers_offsets.at16(b, i as u32), tables().chasers_offsets.at16(b, 2 + i as u32));
        r.d_word(1, dx);
        r.d_word(2, dy);
        o.set_u16(b, f::X, x.wrapping_add(dx));
        o.set_u16(b, f::Y, y.wrapping_add(dy));
        o.set_u16(b, f::CREATOR, d4);
        d4 = d4.wrapping_add(8);
        r.d_word(4, d4);
        o.set_u16(b, f::OWNER, target.word());
        o.set_u16(b, f::LINK, me.word());
        r.flags(me.word() as u32, Size::Word);
        r.d_word(5, n.wrapping_sub(1));
    }
    r.d(0, 1);
    r.flags(1, Size::Long);
    last
}

use super::*;
use crate::mem::{wr16, wr32};
use crate::families::set::tables as set_tables;
use crate::objects::Obj;
use crate::player::Player;
use crate::families::set::fields::SetObj;
use crate::families::set_b::fields::{LifeBarObj, LifeOwner};

const COLUMN_A: u32 = 0x91215c;
const COLUMN_B: u32 = 0x912160;
const COLUMN_C: u32 = 0x912164;
const ROW: u32 = 0x80;
/// Column reset to empty when the bar falls back below 0x160.
const CURSOR_AFTER_RESET: u32 = 0x913760;
const ROWS: u32 = 44;
const MAX_LEN: i16 = 0x2c0;
const SECOND_COLUMN_FROM: i16 = 0x160;
/// Full cell, empty cell.
const FULL: u32 = 0x1318_000d;
const BLANK: u32 = 0x0020_0000;
/// Attribute of the pieces.
const PIECE_ATTR: u16 = 0xd;
/// Partial pieces: word table indexed by the number of points (1 to 8) * 2.
const PIECES: u32 = 0xd61b2;
/// Pointers (data space) to the trim (bytes, end 0x40) according to the player's type.
const FRAMES: u32 = 0xd6232;
const FRAME_SHARED: u32 = 0xd628c;
const FRAME_END: u8 = 0x40;
const FRAME_ATTR: u16 = 2;
const CAP: u32 = 0x003f_0002;

pub fn run<R: Bench>(x: &mut Cx<'_, R>) {
    let owner = x.linked(LifeBarObj::OWNER);
    let (hp, max) = (owner.u16(x.b, LifeOwner::HP), owner.u16(x.b, LifeOwner::HP_MAX));
    x.set16(LifeBarObj::LIFE, hp);
    x.set16(LifeBarObj::MAX, max);
    if x.u8(Obj::VARIANT) != 0 {
        let second = x.linked(LifeBarObj::SECOND);
        let (hp2, max2) = (second.u16(x.b, LifeOwner::HP), second.u16(x.b, LifeOwner::HP_MAX));
        x.add16(LifeBarObj::LIFE, hp2);
        x.add16(LifeBarObj::MAX, max2);
    }
    match x.u8(Obj::STATE) {
        0 => {
            x.add8(Obj::STATE, 2);
            draw_frame(x);
            bar(x);
        }
        2 => bar(x),
        4 => dismiss(x),
        _ => x.free(&pools::SET),
    }
}

fn bar<R: Bench>(x: &mut Cx<'_, R>) {
    match x.u8(SetObj::SUBSTATE) {
        0 => draw_full(x),
        2 => animate(x),
        _ => {}
    }
}

fn clear_column<R: Bench>(x: &mut Cx<'_, R>, mut at: u32) {
    for _ in 0..ROWS {
        wr32(x.b, at, BLANK);
        at += ROW;
    }
}

/// End: columns emptied, object freed.
fn dismiss<R: Bench>(x: &mut Cx<'_, R>) {
    x.set8(Obj::STATE, 6);
    for c in [COLUMN_A, COLUMN_B, COLUMN_C] {
        clear_column(x, c);
    }
    x.free(&pools::SET);
}

/// Trim of the left column (cells of the byte list of the player type), and cap of the next column when the life
/// exceeds the maximum length.
fn draw_frame<R: Bench>(x: &mut Cx<'_, R>) {
    let player = x.linked(LifeBarObj::OWNER);
    let kind = player.u8(x.b, Player::INDEX);
    let mut list = set_tables::long(x.b, FRAMES + 4 * kind as u32);
    if x.u8(Obj::VARIANT) != 0 && kind == 0 {
        list = FRAME_SHARED;
    }
    let mut at = COLUMN_A;
    loop {
        let tile = set_tables::byte(x.b, list);
        list += 1;
        if tile == FRAME_END {
            break;
        }
        wr16(x.b, at, tile as u16);
        wr16(x.b, at + 2, FRAME_ATTR);
        at += ROW;
    }
    if x.u16(LifeBarObj::LIFE) as i16 <= MAX_LEN {
        return;
    }
    let mut at = COLUMN_B;
    for _ in 0..5 {
        wr32(x.b, at, CAP);
        at += ROW;
    }
}

/// Cell of the piece of `d1` points (at most 8), offset by `d0`.
fn piece<R: Bench>(x: &mut Cx<'_, R>, at: u32, d0: i16, d1: i16) {
    let points = d1.min(8);
    let base = set_tables::pc_word(x.b, PIECES.wrapping_add((points.wrapping_mul(2)) as i32 as u32));
    wr16(x.b, at, base.wrapping_add(d0 as u16));
    wr16(x.b, at + 2, PIECE_ATTR);
}

/// Substate 0: draws the whole bar (the displayed life takes the real life).
fn draw_full<R: Bench>(x: &mut Cx<'_, R>) {
    if x.u16(LifeBarObj::LIFE) as i16 > MAX_LEN {
        return;
    }
    x.add8(SetObj::SUBSTATE, 2);
    let mut at = COLUMN_B;
    let mut d0 = x.u16(LifeBarObj::LIFE) as i16;
    x.set16(LifeBarObj::SHOWN, d0 as u16);
    let mut d1 = x.u16(LifeBarObj::MAX) as i16;
    if d1 == 0 {
        return;
    }
    d1 = d1.min(MAX_LEN);
    x.set16(LifeBarObj::MAX_FULL, d1 as u16 & !7);
    x.set8(LifeBarObj::LAST_PIECE, d1 as u8 & 7);
    let mut columns_left = 2;
    loop {
        let mut d2: i16 = (ROWS - 1) as i16;
        loop {
            d1 -= 8;
            d0 -= 8;
            if d0 <= 0 {
                d1 += 8;
                d0 += 8;
                if d0 == 0 {
                    return tail(x, at, d0, d1, d2, true);
                }
                return partial(x, at, d0, d1, d2);
            }
            wr32(x.b, at, FULL);
            at += ROW;
            d2 -= 1;
            if d2 == -1 {
                break;
            }
        }
        if d0 == 0 {
            return tail(x, at - ROW, 0, d1, d2, true);
        }
        columns_left -= 1;
        if columns_left == 0 {
            return tail(x, at, d0, d1, d2, true);
        }
        at = COLUMN_C;
    }
}

/// Last piece of the full part, then the empty part.
fn partial<R: Bench>(x: &mut Cx<'_, R>, at: u32, d0: i16, d1: i16, d2: i16) {
    piece(x, at, d0, d1);
    if d1 <= 8 {
        return tail(x, at, d0, d1, d2, true);
    }
    x.set32(LifeBarObj::CURSOR, at);
    tail(x, at + ROW, d0, d1, d2 - 1, false)
}

/// Remembers the cursor (`+0x22`) and the sub-step (`+0x26`), then draws the remaining part (empty pieces) until
/// `d1` is exhausted.
fn tail<R: Bench>(x: &mut Cx<'_, R>, mut at: u32, d0: i16, mut d1: i16, mut d2: i16, set_cursor: bool) {
    if set_cursor {
        x.set32(LifeBarObj::CURSOR, at);
    }
    x.set8(LifeBarObj::PIECE, d0 as u8);
    if x.u8(LifeBarObj::PIECE) == 0 {
        x.set8(LifeBarObj::PIECE, 8);
    }
    loop {
        d1 -= 8;
        if d1 <= 0 {
            return;
        }
        piece(x, at, 0, d1);
        at += ROW;
        d2 -= 1;
        if d2 == -1 {
            d2 = (ROWS - 1) as i16;
            at = COLUMN_C;
        }
    }
}

/// Substate 2: the displayed life catches up with the real life by one point per frame.
fn animate<R: Bench>(x: &mut Cx<'_, R>) {
    let shown = x.u16(LifeBarObj::SHOWN) as i16;
    if shown == 0 {
        return dismiss(x);
    }
    let target = x.u16(LifeBarObj::LIFE) as i16;
    if shown == target {
        return;
    }
    let mut at = x.u32(LifeBarObj::CURSOR);
    let full_len = x.u16(LifeBarObj::MAX_FULL) as i16;
    if shown > target {
        let d0 = shown - 1;
        x.set16(LifeBarObj::SHOWN, d0 as u16);
        if d0 == SECOND_COLUMN_FROM {
            clear_column(x, COLUMN_C);
            x.set8(LifeBarObj::PIECE, 0);
            x.set32(LifeBarObj::CURSOR, CURSOR_AFTER_RESET);
            return;
        }
        let n = x.u8(LifeBarObj::PIECE).wrapping_sub(1);
        x.set8(LifeBarObj::PIECE, n);
        if (n as i8) < 0 {
            x.set8(LifeBarObj::PIECE, 7);
            at -= ROW;
        }
        let d1 = if d0 < full_len { 8 } else { x.u8(LifeBarObj::LAST_PIECE) as i16 };
        let sub_step = x.u8(LifeBarObj::PIECE) as i16;
        piece(x, at, sub_step, d1);
        x.set32(LifeBarObj::CURSOR, at);
    } else {
        let d0 = shown + 1;
        x.set16(LifeBarObj::SHOWN, d0 as u16);
        if d0 == SECOND_COLUMN_FROM + 1 {
            x.set8(LifeBarObj::PIECE, 1);
            let d1 = (x.u16(LifeBarObj::MAX) as i16).min(MAX_LEN) - SECOND_COLUMN_FROM;
            partial(x, COLUMN_C, d0 - SECOND_COLUMN_FROM, d1, (ROWS - 1) as i16);
            x.set32(LifeBarObj::CURSOR, COLUMN_C);
            return;
        }
        let n = x.u8(LifeBarObj::PIECE) + 1;
        x.set8(LifeBarObj::PIECE, n);
        if n == 9 {
            x.set8(LifeBarObj::PIECE, 1);
            at += ROW;
        }
        let d1 = if d0 <= full_len { 8 } else { x.u8(LifeBarObj::LAST_PIECE) as i16 };
        let sub_step = x.u8(LifeBarObj::PIECE) as i16;
        piece(x, at, sub_step, d1);
        x.set32(LifeBarObj::CURSOR, at);
    }
}

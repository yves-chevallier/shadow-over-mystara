//! Variant 7 of the SET 15 type (0xce714): the end-of-level dialogue window (h).
//!
//! It opens for the player designated by `+0x34` (16-bit RAM address): picks from the ROM tables the message that
//! matches its character (`+3 >> 1`), its progress level (`+0x10c` of the player, capped) and the language (`A5+0x72`),
//! applies the bonus that the message describes (player statistics `+0x151..`, equipment `+0x10d`, `+0x10e`, `+0x140..`
//! then recomputation by 0x3552), draws the window frame in the tilemap and composes the text in the buffer 0xff2072
//! (player name, then message) that the text engine then displays. State 2 recomposes the rest of the message page by
//! page (`+0x10`: delay between two pages).

use crate::families::cx::Bench;
use super::*;
use crate::mem::{rd8, wr16, wr32, wr8};

/// ROM tables (program space: PC-relative accesses).
const LEVEL_RANGE: u32 = 0xce728;
const FRAME_BY_LEVEL: u32 = 0xce7ac;
const FRAMES: u32 = 0xce7e4;
const MESSAGE_TABLES: u32 = 0xceaa2;
/// ROM tables (data space).
const HEADER_PAGES: u32 = 0xce9a4;
const GLYPHS_JP: u32 = 0xd7e8a;
const GLYPHS_LATIN: u32 = 0xc9216;
const PALETTE_SOURCES: [(u32, u32, u32); 2] = [(0x292260, 0x91c1e0, 1), (0x292200, 0x91c300, 2)];

/// Composed text buffer and its variables (A5+).
const TEXT_BUFFER: u32 = 0xff2072;
const TEXT_POINTER: u32 = 0x5c6;
const TEXT_WIDTH: u32 = 0x5ca;
const BOX_TOP: u32 = 0x5cc;
/// Language (2: Japanese) and message page number.
const LANGUAGE: u32 = 0x73;
const MESSAGE_PAGE: u32 = 0x72;

pub fn status_window<R: Bench>(x: &mut Cx<'_, R>) {
    match x.u8(0x4) {
        0 => {
            next_state(x);
            open(x);
        }
        2 => {
            x.add8(0x10, 0xff);
            if x.u8(0x10) != 0 {
                return;
            }
            if x.u8(0x5) != 0 {
                return next_state(x);
            }
            x.set8(0x10, 0x1e);
            compose_message(x, 0);
        }
        _ => {}
    }
}

fn next_state<R: Bench>(x: &mut Cx<'_, R>) {
    x.add8(0x4, 2);
}

/// State 0: computes the player's tier, creates the window object, draws the frame and displays the first message.
fn open<R: Bench>(x: &mut Cx<'_, R>) {
    let player = adr(x.u16(0x34));
    let half = rd8(x.b, player + 3) >> 1;
    x.set8(0x37, half);
    let at = LEVEL_RANGE + 2 * half as u32;
    let (min, max) = (x.rom8(at) as i8, x.rom8(at + 1) as i8);
    let mut level = rd8(x.b, player + 0x10c).wrapping_add(1) as i8;
    if level > max {
        level = max;
    }
    wr8(x.b, player + 0x10c, level as u8);
    let step = level.wrapping_sub(min) as u8;
    x.set8(0x36, step);
    let base = x.rom16(FRAME_BY_LEVEL + 2 * half as u32);
    let low = (base as u8).wrapping_add(step);
    let frame = x.rom8(FRAME_BY_LEVEL + (base & 0xff00 | low as u16) as i16 as i32 as u32);
    x.set_a5_u16(BOX_TOP, 0xb8u16.wrapping_sub(0x18 * frame as u16));
    // 0xddeaa: scenery 6 of variant 0xd, which keeps the frame
    if let Ok(scenery) = crate::effects_aids::scenery_6(x.b, 0xd) {
        scenery.set_u8(x.b, 0x2f, frame);
    }
    draw_frame(x, frame);
    pick_message(x, player);
}

/// RAM address stored on 16 bits (`movea.w`: sign extension).
fn adr(v: u16) -> u32 {
    v as i16 as i32 as u32
}

/// Window frame no. `frame` (0xce7fc): 8-byte record (GFX RAM address, width) in table 0xce7e4; a top edge, two side
/// columns (42 lines of 0x80 bytes) and a background.
fn draw_frame<R: Bench>(x: &mut Cx<'_, R>, frame: u8) {
    let entry = FRAMES + 8 * frame as u32;
    let origin = (x.rom16(entry) as u32) << 16 | x.rom16(entry + 2) as u32;
    let width = x.rom16(entry + 4);
    let longs = width as u32 + 1;
    let row = |x: &mut Cx<'_, R>, at: u32, value: u32, count: u32, stride: u32| {
        for i in 0..count {
            wr32(x.b, at + i * stride, value);
        }
    };
    wr16(x.b, origin, 0x145);
    wr16(x.b, origin + 2, 2);
    let mut at = origin + 4;
    row(x, at, 0x14b_0002, longs, 4);
    at += 4 * longs;
    wr32(x.b, at, 0x147_0002);
    row(x, at + 0x80, 0x14a_0002, 42, 0x80);
    row(x, origin + 0x80, 0x149_0002, 42, 0x80);
    let at = origin + 0x80 + 0x80 * 42;
    wr32(x.b, at, 0x146_0002);
    row(x, at + 4, 0x14c_0002, longs, 4);
    wr32(x.b, at + 4 + 4 * longs, 0x148_0002);
    let mut column = origin + 0x80;
    for _ in 0..longs {
        row(x, column + 4, 0x431_000f, 42, 0x80);
        column += 4;
    }
}

/// Chooses the message (0xceaba): table of records per character, page rank, tier.
fn pick_message<R: Bench>(x: &mut Cx<'_, R>, player: u32) {
    let half = x.u8(0x37) as u32;
    let tables = {
        let at = MESSAGE_TABLES + 4 * half;
        (x.rom16(at) as u32) << 16 | x.rom16(at + 2) as u32
    };
    let page = (x.a5_u16(MESSAGE_PAGE).wrapping_sub(2)).wrapping_mul(2);
    let messages = x.data32(tables.wrapping_add(page as i16 as i32 as u32));
    let step = x.u8(0x36) as u16;
    let record = tables + 0xc + step.wrapping_mul(20) as i16 as i32 as u32;
    let hp = x.data16(record);
    wr16(x.b, player + 0x60, hp);
    let choice = if rd8(x.b, player + 3) & 1 != 0 { x.data16(record + 4) } else { x.data16(record + 2) };
    let pointer = x.data32(messages.wrapping_add(choice as i16 as i32 as u32));
    x.set32(0x8, pointer);
    let next = merge_statistics(x, player, record + 6);
    equip(x, player, next);
    let count = compose_header(x, player);
    x.set8(0x10, 0x1e);
    compose_message(x, count);
}

/// 0xceb20: takes the maximum, byte by byte, of nine ROM values and of the player's statistics `+0x151..` (unless the
/// record starts with a negative byte). Returns the continuation.
fn merge_statistics<R: Bench>(x: &mut Cx<'_, R>, player: u32, at: u32) -> u32 {
    let skip = x.data8(at) as i8 <= -1;
    if !skip {
        for i in 0..9 {
            let v = x.data8(at + 1 + i) as i8;
            let current = rd8(x.b, player + 0x151 + i) as i8;
            if v > current {
                wr8(x.b, player + 0x151 + i, v as u8);
            }
        }
    }
    at + 10
}

/// 0xceb6c: changes a piece of the player's equipment. The record gives (new value, old value) for `+0x10d` or,
/// otherwise, for the slot of `+0x140..` that holds the old value; then 0x3552 recomputes the player's characteristics.
fn equip<R: Bench>(x: &mut Cx<'_, R>, player: u32, at: u32) {
    if x.data8(at) as i8 <= -1 {
        return;
    }
    let (new, old) = (x.data8(at), x.data8(at + 1));
    let kind = rd8(x.b, player + 0x10e).wrapping_sub(4);
    if kind <= 3 || old != rd8(x.b, player + 0x10d) {
        let (new, old) = (x.data8(at + 2), x.data8(at + 3));
        let mut slot = player + 0x140;
        loop {
            let found = rd8(x.b, slot) == old;
            slot += 1;
            if found {
                break;
            }
        }
        wr8(x.b, slot - 1, new);
    } else {
        wr8(x.b, player + 0x10d, new);
    }
    // 0x3552: the player's weapon power is recomputed (its output registers, D0 and A4, are not read)
    crate::player::init::init_weapon_stats(&mut crate::player::Ctx::new(x.b), crate::player::Player(player));
}

/// Window palette and text header (0xce88a): copies three palette blocks, sets the text buffer and composes the
/// player's name in it. Returns the number of characters written.
fn compose_header<R: Bench>(x: &mut Cx<'_, R>, player: u32) -> u16 {
    for (from, to, blocks) in PALETTE_SOURCES {
        for i in 0..8 * blocks {
            let v = x.data32(from + 4 * i);
            wr32(x.b, to + 4 * i, v);
        }
    }
    x.set_a5_u32(TEXT_POINTER, TEXT_BUFFER);
    x.set_a5_u16(TEXT_WIDTH, 0x60);
    let (end, count) = write_name(x, player, TEXT_BUFFER);
    x.set32(0xc, end);
    count
}

/// Player name (0xce8de): glyph codes of the player's record `+0x118`, six at most; a code >= 0xf9 refers to a list of
/// table 0xce9a4. Returns (end of buffer, number of glyphs).
fn write_name<R: Bench>(x: &mut Cx<'_, R>, player: u32, buffer: u32) -> (u32, u16) {
    let japanese = x.a5_u8(LANGUAGE) == 2;
    let mut src = player + 0x118;
    let mut out = buffer;
    let mut count = 0u16;
    let mut left = 5i32;
    if japanese {
        wr32(x.b, out, 0xfffe_0018);
        out += 4;
    } else {
        wr8(x.b, out, 0xfe);
        wr8(x.b, out + 1, 0x18);
        out += 2;
    }
    // a code >= 0xf9 replaces the source with a list of the table (its codes then 0xff)
    let first = rd8(x.b, src).wrapping_add(1);
    if first >= 0xfa {
        let index = if japanese {
            (first.wrapping_neg().wrapping_sub(2)).wrapping_mul(4)
        } else {
            (rd8(x.b, src).wrapping_neg().wrapping_sub(3)).wrapping_mul(4)
        };
        src = x.data32(HEADER_PAGES + index as u32);
        left = x.data16(src) as i32;
        src += 2;
    }
    while left >= 0 {
        left -= 1;
        let code = x.data8(src);
        src += 1;
        if code == 0xff {
            break;
        }
        if japanese {
            let glyph = x.data16(GLYPHS_JP + 2 * code as u32);
            wr16(x.b, out, glyph);
            out += 2;
        } else {
            let index = code.wrapping_sub(0x51) as u32;
            let table = GLYPHS_LATIN;
            let at = table.wrapping_add(x.data16(table + 2 * index) as i16 as i32 as u32);
            let mut p = at;
            if x.data8(p) <= 0xf {
                let byte = x.data8(p);
                wr8(x.b, out, byte);
                out += 1;
                p += 1;
            }
            let byte = x.data8(p);
            wr8(x.b, out, byte);
            out += 1;
        }
        count = count.wrapping_add(1);
    }
    if japanese {
        wr32(x.b, out, 0xfffe_000f);
        out += 4;
    } else {
        wr8(x.b, out, 0xfe);
        wr8(x.b, out + 1, 0x0f);
        out += 2;
    }
    (out, count)
}

/// Composition of the rest of the message (0xce9ea): copies into the buffer (`+0xc`) the characters of the source
/// (`+8`) up to a page end mark; `count` = glyphs already written on the line.
fn compose_message<R: Bench>(x: &mut Cx<'_, R>, mut count: u16) {
    let mut src = x.u32(0x8);
    let mut out = x.u32(0xc);
    if x.a5_u8(LANGUAGE) == 2 {
        loop {
            let w = x.data16(src);
            src += 2;
            if (w as i16) < 0 {
                if w == 0xfffe {
                    wr16(x.b, out, w);
                    let next = x.data16(src);
                    wr16(x.b, out + 2, next);
                    out += 4;
                    src += 2;
                    continue;
                }
                wr16(x.b, out, w);
                wr16(x.b, out + 2, count.wrapping_shl(3).wrapping_neg());
                out += 4;
                src += 2;
                let tail = x.data16(src);
                src += 2;
                wr16(x.b, out, tail);
                out += 2;
                x.set32(0x8, src);
                x.set32(0xc, out);
                wr16(x.b, out, 0);
                return;
            }
            if w == 0 {
                wr16(x.b, out, 0);
                next_substate(x);
                x.set16(0x10, 0xb4);
                return;
            }
            wr16(x.b, out, w);
            out += 2;
            count = count.wrapping_add(1);
        }
    } else {
        loop {
            let c = x.data8(src);
            src += 1;
            if c >= 0xfe {
                if c == 0xfe {
                    wr8(x.b, out, c);
                    let next = x.data8(src);
                    src += 1;
                    wr8(x.b, out + 1, next);
                    out += 2;
                    continue;
                }
                wr8(x.b, out, c);
                wr8(x.b, out + 1, count as u8);
                out += 2;
                src += 1;
                let tail = x.data8(src);
                src += 1;
                wr8(x.b, out, tail);
                out += 1;
                x.set32(0x8, src);
                x.set32(0xc, out);
                wr8(x.b, out, 0xfd);
                return;
            }
            if c == 0xfd {
                wr8(x.b, out, c);
                next_substate(x);
                x.set16(0x10, 0xb4);
                return;
            }
            if c > 0xf {
                wr8(x.b, out, c);
                out += 1;
            } else {
                wr8(x.b, out, c);
                let next = x.data8(src);
                src += 1;
                wr8(x.b, out + 1, next);
                out += 2;
            }
            count = count.wrapping_add(1);
        }
    }
}

fn next_substate<R: Bench>(x: &mut Cx<'_, R>) {
    x.add8(0x5, 2);
}

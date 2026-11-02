//! Type SET 3 (0xc8352, reused by type 33): text window.
//!
//! A window is described by an 18-byte ROM record (pointer table indexed
//! by variant `+3`, one table per language): position in the scroll 1 tilemap, size, text,
//! text position, *style* (`+0x2f`), display duration, parameter, then the next window.
//! The object draws the frame in the tilemap (opening line by line), composes the text in buffer
//! 0xff2072 according to the style (player names, equipment messages...), hands it to the text engine
//! through one of the entries 0xff85c6 (pointer, x, y), waits for the duration or a button, closes the frame line by
//! line, then chains the next window or disappears.
//!
//! States: `+4` 0 (loading), 2 (alive), 4/6 (release); when alive, `+5`: 0 start of opening,
//! 2 opening, 4 display, 6 closing.
//!
//! The composed text is a sequence of bytes: character (> 15), command (≤ 15) followed by its argument,
//! 0xfe + argument, 0xff + number of characters written since the previous 0xff + argument, 0xfd end.

use crate::families::cx::Bench;
use crate::objects::Current;
use crate::families::cx::Cx;
use crate::mem::{prog32, rd16, rd32, rd8, wr16, wr32, wr8, A5};
use crate::objects::{pools, Obj};

/// Champs de l'objet.
mod f {
    pub const STATE: u32 = 0x04;
    pub const SUB: u32 = 0x05;
    pub const X: u32 = 0x08;
    pub const Y: u32 = 0x0c;
    /// Address in the tilemap of the current corner (long); the low word advances by one line (4 bytes).
    pub const TILES: u32 = 0x14;
    pub const TILES_LOW: u32 = 0x16;
    /// "Large" frame (other tiles, height 11 instead of 15).
    pub const BIG: u32 = 0x18;
    /// Number of lines of the frame, and lines left to open or close.
    pub const ROWS: u32 = 0x19;
    pub const ROWS_LEFT: u32 = 0x21;
    /// Negative: last window; otherwise the next window (`NEXT`) is loaded on closing.
    pub const CHAIN: u32 = 0x1a;
    /// Delay between two lines, and its reload value.
    pub const TICK: u32 = 0x1b;
    pub const TICK_RELOAD: u32 = 0x36;
    pub const NEXT: u32 = 0x1c;
    /// Frame width (in columns of 8 pixels, minus one; 0xff: no frame).
    pub const WIDTH: u32 = 0x20;
    pub const CLOSED: u32 = 0x22;
    pub const INDEX: u32 = 0x23;
    /// Text (or style parameter), long.
    pub const TEXT: u32 = 0x24;
    pub const STYLE: u32 = 0x2f;
    /// Display duration in frames (0xffff: until the end imposed by another object).
    pub const DURATION: u32 = 0x32;
    pub const PARAM: u32 = 0x34;
    /// Player whose buttons cut the display short (negative: all).
    pub const PLAYER: u32 = 0x37;
    /// Text engine entry: offset (byte) from `TEXT_SLOTS`, then address (word).
    pub const SLOT: u32 = 0x38;
    pub const SLOT_PTR: u32 = 0x3a;
    pub const CHOICE: u32 = 0x42;
    pub const PLAYER_ALT: u32 = 0x43;
    /// Linked object (address word).
    pub const LINKED: u32 = 0x46;
}

/// Game variables (A5+).
mod ram {
    pub const BUTTONS: u32 = 0x0c;
    pub const PLAYING: u32 = 0x30;
    pub const SCENE: u32 = 0x34;
    pub const PLAYER_COUNT: u32 = 0x35;
    pub const FROZEN: u32 = 0x42;
    pub const PAUSED: u32 = 0x50;
    pub const WINDOWS: u32 = 0x51;
    pub const FRAME_PARITY: u32 = 0x6f;
    /// Language (word): 4 picks the first table of windows and messages.
    pub const LANGUAGE: u32 = 0x72;
    pub const DIM: u32 = 0x48e;
    pub const DIM_ON: u32 = 0x48f;
    pub const TEXT_SLOTS: u32 = 0x5c6;
    pub const PLAYERS: u32 = 0x5de;
}

/// Absolute addresses of the work RAM.
const TEXT_BUFFER: u32 = 0xff2072;
const WINDOW_COUNT: u32 = 0xff2472;
const SKIP_FLAG: u32 = 0xff2042;
const SAVED_NAME: u32 = 0xff0fd2;
const PLAYER_SIZE: u32 = 0x200;
/// Fields of a player: name (6 coded characters), equipment.
const NAME: u32 = 0x118;
const EQUIP_A: u32 = 0x10d;
const EQUIP_B: u32 = 0x10e;
const ITEMS: u32 = 0x140;

/// Window tables (data), according to the language.
const WINDOWS_LANG4: u32 = 0x1cd000;
const WINDOWS_OTHER: u32 = 0x1e78da;
/// Scroll 1 tilemap and window palettes.
const TILEMAP: u32 = 0x912000;
const PALETTE_SRC: u32 = 0x292260;
const PALETTE_SRC_B: u32 = 0x292200;
const PALETTE: u32 = 0x91c1e0;
const PALETTE_ALT: u32 = 0x91c380;
const PALETTE_B: u32 = 0x91c300;
const PALETTE_C: u32 = 0x91c180;
/// Color whose brightness is copied when the screen is darkened.
const DIM_REFERENCE: u32 = 0x91c2a0;

/// Code tables. Program space (PC-relative reads):
const STYLE5_OFFSETS: u32 = 0xc851c;
const MESSAGES_LANG4: u32 = 0xc8e34;
const MESSAGES_OTHER: u32 = 0xc8e48;
/// Data space (reads through address register):
const CLASS_TITLES: u32 = 0xc91c6;
const DEFAULT_NAMES: u32 = 0xc8948;
const GLYPHS: u32 = 0xc9216;
const SEPARATOR: u32 = 0xc91c4;
const EQUIP_LISTS: [(u32, u32); 2] = [(0xc8e0c, 0xc8e12), (0xc8e18, 0xc8e1e)];

/// Tile words (number << 16 | attribute) of the frame.
const BLANK: u32 = 0x0020_0000;
const FLIP: u32 = 0x20;
const ROW_STRIDE: u32 = 0x80;

pub fn run<R: Bench>(x: &mut Cx<'_, R>) {
    match x.u8(f::STATE) {
        0 => start(x),
        2 => match x.u8(f::SUB) {
            0 => open_start(x),
            2 => opening(x),
            4 => shown(x),
            6 => closing(x),
            _ => {}
        },
        _ => free(x),
    }
}

/// 0xc8368: loads the variant's window and sets its palette.
fn start<R: Bench>(x: &mut Cx<'_, R>) {
    x.add8(f::STATE, 2);
    x.set8(f::TICK_RELOAD, 3);
    let table = if x.a5_u16(ram::LANGUAGE) == 4 { WINDOWS_LANG4 } else { WINDOWS_OTHER };
    let variant = x.u8(0x3) as u32;
    let desc = rd32(x.b, table + 4 * variant);
    load(x, desc);
    let scene = x.a5_u8(ram::SCENE);
    let palette = if (0x45..0x5a).contains(&scene) { PALETTE_ALT } else { PALETTE };
    match x.u8(f::STYLE) {
        17 => {}
        21 => {
            if rd8(x.b, SKIP_FLAG) != 0 {
                x.set8(f::STYLE, 0);
            }
        }
        _ => {
            copy_palette(x, PALETTE_SRC, palette);
            dim_palette(x, palette);
        }
    }
}

/// 0xc83ea: darkened screen, the palette takes the brightness of the reference color. Returns
/// the address following the processed palette (register A1 of the original).
fn dim_palette<R: Bench>(x: &mut Cx<'_, R>, palette: u32) -> u32 {
    if x.a5_u8(ram::DIM_ON) == 0 || x.a5_u8(ram::DIM) & 1 == 0 {
        return palette;
    }
    let level = rd32(x.b, DIM_REFERENCE) & 0xf000_f000;
    for i in 0..8 {
        let at = palette + 4 * i;
        let v = rd32(x.b, at) & 0x0fff_0fff | level;
        wr32(x.b, at, v);
    }
    palette + 32
}

/// Seize couleurs (`movem.l` de 8 longs).
fn copy_palette<R: Bench>(x: &mut Cx<'_, R>, src: u32, dst: u32) {
    for i in 0..8 {
        let v = rd32(x.b, src + 4 * i);
        wr32(x.b, dst + 4 * i, v);
    }
}

/// 0xc8a82 : seconde palette.
fn copy_palette_b<R: Bench>(x: &mut Cx<'_, R>) {
    copy_palette(x, PALETTE_SRC_B, PALETTE_B);
}

/// Frameless window: direct display (substate `sub`).
fn frameless<R: Bench>(x: &mut Cx<'_, R>, sub: u8) {
    x.set8(f::SUB, sub);
    x.set8(f::ROWS, 0);
    x.set8(f::ROWS_LEFT, 0);
    x.set8(f::WIDTH, 0xff);
    x.set8(f::TICK, 1);
}

/// Registers left by the loading: A1, and D6 and A6 when a style overwrites them (palette copied by
/// `movem`, search for a player). Closing compares them to the width (D6) and the place (A6) of
/// the old window, then clears from A6 over the width D6.
struct Loaded {
    a1: u32,
    d6: Option<u32>,
    a6: Option<u32>,
}

/// 0xc8430: loads record `desc`; some styles replace it with another (chosen
/// according to the players, the number of players...). Returns the registers of the original that closing
/// relit (voir `Loaded`).
fn load<R: Bench>(x: &mut Cx<'_, R>, mut desc: u32) -> Loaded {
    let mut regs = Loaded { a1: 0, d6: None, a6: None };
    let done = |regs: Loaded, a1: u32| Loaded { a1, ..regs };
    loop {
        let col = rd8(x.b, desc) as u16;
        let row = rd8(x.b, desc + 1) as u16;
        let v = rd8(x.b, desc + 2);
        x.set8(f::BIG, v);
        let v = rd8(x.b, desc + 3);
        x.set8(f::ROWS, v);
        let v = rd32(x.b, desc + 4);
        x.set32(f::TEXT, v);
        let v = rd16(x.b, desc + 8);
        x.set16(f::X, v);
        let v = rd16(x.b, desc + 10);
        x.set16(f::Y, v);
        let v = rd8(x.b, desc + 12);
        x.set8(f::STYLE, v);
        let v = rd8(x.b, desc + 13);
        x.set8(f::CHAIN, v);
        let v = rd16(x.b, desc + 14);
        x.set16(f::DURATION, v);
        let v = rd16(x.b, desc + 16);
        x.set16(f::PARAM, v);
        x.set32(f::NEXT, desc + 18);
        let offset = (col << 7).wrapping_add(row << 2) as i16 as i32 as u32;
        let tiles = TILEMAP.wrapping_add(offset);
        x.set32(f::TILES, tiles);
        let width = if x.u8(f::BIG) != 0 { 11 } else { 15 };
        x.set8(f::WIDTH, width);
        let rows = x.u8(f::ROWS);
        x.set8(f::ROWS_LEFT, rows);
        let text = x.u32(f::TEXT);
        match x.u8(f::STYLE) {
            // a window depending on the number of players
            1 => {
                let k = if x.a5_u8(ram::PLAYER_COUNT) == 1 { 0 } else { 4 };
                desc = rd32(x.b, text + k);
            }
            // enlarged frame depending on the number of players
            2 | 4 => {
                let extra = x.a5_u8(ram::PLAYER_COUNT).wrapping_sub(1).wrapping_mul(2);
                x.add8(f::ROWS, extra);
                let rows = x.u8(f::ROWS);
                x.set8(f::ROWS_LEFT, rows);
                return done(regs, tiles);
            }
            5 => {
                let i = x.u8(f::CHOICE).wrapping_mul(2).wrapping_add(x.a5_u8(ram::FRAME_PARITY) & 1);
                desc = text + x.rom8(STYLE5_OFFSETS + i as u32) as u32;
            }
            9 => {
                frameless(x, 2);
                x.set8(f::STYLE, 0);
                return done(regs, tiles);
            }
            // (the search for a class 5 player has no effect, except on A1)
            10 => return done(regs, find_player(x, |p| p == 5, false).map_or(A5 + ram::PLAYERS + 4 * PLAYER_SIZE, |p| p.0)),
            11 => {
                let k = x.u8(f::INDEX).wrapping_mul(4) as u32;
                desc = rd32(x.b, text + k);
            }
            13 => {
                copy_palette_b(x);
                regs.d6 = Some(rd32(x.b, PALETTE_SRC_B + 24));
                return done(regs, PALETTE_B + 0x20);
            }
            14 => {
                x.set8(f::TICK_RELOAD, 1);
                x.set8(f::STYLE, 0);
                return done(regs, tiles);
            }
            17 => {
                frameless(x, 2);
                copy_palette_b(x);
                x.set16(f::DURATION, 0xffff);
                copy_palette(x, PALETTE_SRC, PALETTE);
                for base in [PALETTE, PALETTE_B] {
                    for i in 0..8 {
                        let v = rd32(x.b, base + 4 * i) & 0x0fff_0fff;
                        wr32(x.b, base + 4 * i, v);
                    }
                }
                regs.d6 = Some(rd32(x.b, PALETTE_SRC + 24));
                return done(regs, PALETTE + 0x20);
            }
            19 => {
                x.set8(f::SLOT, 0x10);
                desc = text + 18 * x.u8(f::CHOICE) as u32;
            }
            // second window if a class 3 player is in game
            20 => {
                desc = text;
                let found = find_player(x, |p| p == 3, false);
                regs.a6 = Some(found.map_or(A5 + ram::PLAYERS + 4 * PLAYER_SIZE, |p| p.0));
                if found.is_some() {
                    desc += 36;
                }
            }
            21 => {
                if rd8(x.b, SKIP_FLAG) != 0 {
                    frameless(x, 6);
                    return done(regs, tiles);
                }
                frameless(x, 2);
                copy_palette(x, PALETTE_SRC, PALETTE_C);
                regs.d6 = Some(rd32(x.b, PALETTE_SRC + 24));
                let a1 = dim_palette(x, PALETTE_C);
                return done(regs, a1);
            }
            22 => {
                x.set8(f::TICK_RELOAD, 1);
                x.set8(f::STYLE, 1);
                return done(regs, tiles);
            }
            23 => {
                frameless(x, 2);
                return done(regs, tiles);
            }
            _ => return done(regs, tiles),
        }
    }
}

/// First player (in slot order) present, whose state `+4` is below 4 (or
/// different from 4 if `not_four`), and whose class (`+3 >> 1`) satisfies `class`.
fn find_player<R: Bench>(x: &mut Cx<'_, R>, class: impl Fn(u8) -> bool, not_four: bool) -> Option<Obj> {
    (0..4).map(|k| Obj(A5 + ram::PLAYERS + k * PLAYER_SIZE)).find(|&p| {
        let state = p.u8(x.b, 4);
        p.u8(x.b, 0) != 0 && (if not_four { state != 4 } else { (state as i8) < 4 }) && class(p.u8(x.b, 3) >> 1)
    })
}

// ---- cadre ----------------------------------------------------------------------------------------

fn big<R: Bench>(x: &mut Cx<'_, R>) -> bool {
    x.u8(f::BIG) != 0
}

/// Writes `count` tiles of a row, column by column, the tile changing by `step` each time.
fn strip<R: Bench>(x: &mut Cx<'_, R>, at: &mut u32, tile: &mut u32, step: u32, count: u32) {
    for _ in 0..count {
        wr32(x.b, *at, *tile);
        *tile = tile.wrapping_add(step);
        *at += ROW_STRIDE;
    }
}

/// A symmetric row: `width + 1` rising tiles, then the same ones flipped, descending.
fn mirrored<R: Bench>(x: &mut Cx<'_, R>, mut at: u32, mut tile: u32) {
    let n = x.u8(f::WIDTH) as u32 + 1;
    strip(x, &mut at, &mut tile, 0x1_0000, n);
    tile = (tile | FLIP).wrapping_sub(0x1_0000);
    strip(x, &mut at, &mut tile, 0x1_0000u32.wrapping_neg(), n);
}

/// 0xc90ce: top edge (two rows).
fn draw_top<R: Bench>(x: &mut Cx<'_, R>, at: u32) {
    let (mut a, mut b) = if big(x) { (0x0434_000f, 0x0444_000f) } else { (0x0400_000f, 0x0410_000f) };
    let n = x.u8(f::WIDTH) as u32 + 1;
    let mut at = at;
    for (step, last) in [(0x1_0000u32, false), (0x1_0000u32.wrapping_neg(), true)] {
        if last {
            a = (a | FLIP).wrapping_add(step);
            b = (b | FLIP).wrapping_add(step);
        }
        for _ in 0..n {
            wr32(x.b, at, a);
            wr32(x.b, at + 4, b);
            a = a.wrapping_add(step);
            b = b.wrapping_add(step);
            at += ROW_STRIDE;
        }
    }
}

/// 0xc9128: middle row (left and right edges, background).
fn draw_middle<R: Bench>(x: &mut Cx<'_, R>, at: u32) {
    let edge = if big(x) { 0x0432_000f } else { 0x0430_000f };
    wr32(x.b, at, edge);
    let inner = (x.u8(f::WIDTH) as u16 * 2).wrapping_sub(1) as u32 + 1;
    let mut at = at + ROW_STRIDE;
    for _ in 0..inner {
        wr32(x.b, at, 0x0431_000f);
        at += ROW_STRIDE;
    }
    wr32(x.b, at, edge | FLIP);
}

/// 0xc9164 : bord bas.
fn draw_bottom<R: Bench>(x: &mut Cx<'_, R>, at: u32) {
    let tile = if big(x) { 0x0454_000f } else { 0x0420_000f };
    mirrored(x, at, tile);
}

/// 0xc91a8: clears a row.
fn clear_row<R: Bench>(x: &mut Cx<'_, R>, mut at: u32) {
    let n = x.u8(f::WIDTH) as u32 * 2 + 2;
    for _ in 0..n {
        wr32(x.b, at, BLANK);
        at += ROW_STRIDE;
    }
}

/// 0xc9036: clears three rows starting at `at`, over the width `width` (register D6: nothing if its
/// byte is negative, its word gives the width).
fn clear_window<R: Bench>(x: &mut Cx<'_, R>, mut at: u32, width: u32) {
    if (width as u8 as i8) < 0 {
        return;
    }
    for _ in 0..(width as u16).wrapping_mul(2).wrapping_add(1) as u32 + 1 {
        for i in 0..3 {
            wr32(x.b, at + 4 * i, BLANK);
        }
        at += ROW_STRIDE;
    }
}

/// Delay between two rows; true when it has elapsed (it is then reloaded).
fn tick<R: Bench>(x: &mut Cx<'_, R>) -> bool {
    let t = x.u8(f::TICK);
    x.set8(f::TICK, t.wrapping_sub(1));
    // `subq.b #1 ; bgt`: strictly positive after the decrement, overflow included
    if t as i8 > 1 {
        return false;
    }
    let reload = x.u8(f::TICK_RELOAD);
    x.set8(f::TICK, reload);
    true
}

// ---- life of the window --------------------------------------------------------------------

/// 0xc870c: top edge and first row.
fn open_start<R: Bench>(x: &mut Cx<'_, R>) {
    x.add8(f::SUB, 2);
    let reload = x.u8(f::TICK_RELOAD);
    x.set8(f::TICK, reload);
    let at = x.u32(f::TILES);
    draw_top(x, at);
    x.add16(f::TILES_LOW, 8);
    let at = x.u32(f::TILES);
    draw_bottom(x, at);
}

/// 0xc872a: one more row at each delay; at the end, the text.
fn opening<R: Bench>(x: &mut Cx<'_, R>) {
    if !tick(x) {
        return;
    }
    let left = x.u8(f::ROWS_LEFT);
    if left == 0 {
        return show_text(x);
    }
    x.set8(f::ROWS_LEFT, left - 1);
    let at = x.u32(f::TILES);
    draw_middle(x, at);
    x.add16(f::TILES_LOW, 4);
    let at = x.u32(f::TILES);
    draw_bottom(x, at);
}

/// Window text engine entry (16-bit address).
fn slot<R: Bench>(x: &mut Cx<'_, R>) -> u32 {
    x.u16(f::SLOT_PTR) as i16 as i32 as u32
}

/// 0xc875a: frame open; hands the text to the text engine, composed according to the style.
fn show_text<R: Bench>(x: &mut Cx<'_, R>) {
    x.add8(f::SUB, 2);
    let rows = x.u8(f::ROWS);
    x.set8(f::ROWS_LEFT, rows);
    let slot = A5 + ram::TEXT_SLOTS + x.u8(f::SLOT) as u32;
    x.set16(f::SLOT_PTR, slot as u16);
    let (px, py) = (x.u16(f::X), x.u16(f::Y));
    wr16(x.b, slot + 4, px);
    wr16(x.b, slot + 6, py);
    let style = x.u8(f::STYLE);
    if style == 0 {
        let text = x.u32(f::TEXT);
        wr32(x.b, slot, text);
        return;
    }
    if style != 12 {
        wr32(x.b, slot, TEXT_BUFFER);
    }
    let mut t = Text { at: TEXT_BUFFER, count: 0 };
    compose(x, &mut t, style, slot);
}

/// 0xc8f76: display during the duration (a button cuts it short), then clears the text.
fn shown<R: Bench>(x: &mut Cx<'_, R>) {
    if x.u16(f::DURATION) == 0xffff {
        return;
    }
    if buttons(x) != 0 {
        x.set16(f::DURATION, 0);
    }
    let left = x.u16(f::DURATION);
    if left != 0 {
        x.set16(f::DURATION, left - 1);
        return;
    }
    x.add8(f::SUB, 2);
    if x.u8(f::STYLE) == 12 {
        return;
    }
    let slot = slot(x);
    wr32(x.b, slot, 0);
}

/// 0xc9068: newly pressed buttons (high nibble) of the players in game concerned.
fn buttons<R: Bench>(x: &mut Cx<'_, R>) -> u8 {
    let filter = if x.u8(0x3) == 0x8b {
        x.u8(f::PLAYER_ALT)
    } else {
        let p = x.u8(f::PLAYER);
        if (p as i8) >= 0 { p } else { 0xff }
    };
    if x.a5_u8(ram::PLAYING) == 0 {
        return 0;
    }
    let mut pressed = 0;
    for k in 0..4 {
        let p = Obj(A5 + ram::PLAYERS + k * PLAYER_SIZE);
        if p.u8(x.b, 0) != 0 && (p.u8(x.b, 4) as i8) < 4 && ((filter as i8) < 0 || filter == p.u8(x.b, 2)) {
            pressed |= x.a5_u8(ram::BUTTONS + k);
        }
    }
    pressed & 0xf0
}

/// 0xc8fb0: clears one row at each delay; at the end, next window or disappearance.
fn closing<R: Bench>(x: &mut Cx<'_, R>) {
    if !tick(x) {
        return;
    }
    let left = x.u8(f::ROWS_LEFT);
    if left == 0 {
        return closed(x);
    }
    x.set8(f::ROWS_LEFT, left - 1);
    let at = x.u32(f::TILES);
    clear_row(x, at);
    x.add16(f::TILES_LOW, 0xfffc);
    let at = x.u32(f::TILES);
    draw_bottom(x, at);
}

/// 0xc8fe0: clears the top edge, unless the next window has the same place and the same width.
fn closed<R: Bench>(x: &mut Cx<'_, R>) {
    let top = x.u32(f::TILES).wrapping_sub(8);
    let width = x.u8(f::WIDTH);
    if (x.u8(f::CHAIN) as i8) < 0 {
        x.set8(f::CLOSED, 1);
        x.set32(f::STATE, 0x0600_0000);
        return clear_window(x, top, width as u32);
    }
    x.set8(f::SUB, 0);
    let next = x.u32(f::NEXT);
    let regs = load(x, next);
    let (d6, a6) = (regs.d6.unwrap_or(width as u32), regs.a6.unwrap_or(top));
    if d6 as u8 != x.u8(f::WIDTH) || a6 != regs.a1 {
        clear_window(x, a6, d6);
    }
}

/// 0xc9056: release.
fn free<R: Bench>(x: &mut Cx<'_, R>) {
    let n = x.a5_u8(ram::WINDOWS).wrapping_sub(1);
    x.set_a5_u8(ram::WINDOWS, n);
    x.set8(f::CLOSED, 0);
    let n = rd8(x.b, WINDOW_COUNT).wrapping_sub(1);
    wr8(x.b, WINDOW_COUNT, n);
    x.free(&pools::SET);
}


/// Text composition: names, titles, equipment and linked messages written into the text buffer.
mod text;
use text::{compose, Text};

use super::*;
use crate::world::mem::{rd16, rd32, wr16, wr32};
use crate::actors::families::set::tables as set_tables;

/// Window definitions per variant (pointers, data space).
const DEFINITIONS: u32 = 0x1d78d0;
const SCREEN: u32 = 0x912000;
/// Token buffer (A5-0x5f8e) and text engine cells (A5+0x5c6, 8 bytes each).
const BUFFER: u32 = 0xff_2072;
const SLOTS: u32 = A5 + vars::TEXT_SLOTS;
/// Counter of open windows (A5-0x5b8e) and flag (A5-0x5fbe).
const OPEN_COUNT: u32 = 0xff_2472;
const FLAG_SKIP: u32 = 0xff_2042;
const PALETTE_SRC: u32 = 0x292260;
const PALETTE_ALT: u32 = 0x292200;
const CHARS: u32 = 0xd7e8a;
const SEPARATOR: u32 = 0xd7e26;
const NAME_TABLE: u32 = 0xd7604;
const PLAYER_TITLES: u32 = 0xd7db6;
const PLAYERS_ONE_TWO: u32 = 0xd7d96;
const PLAYERS_ONE_TWO_B: u32 = 0xd7da6;
const TOK_LINK_A: u16 = 0xfffe;
/// Opening and closing words of a player name.
const NAME_OPEN: u32 = 0xfffe_0018;
const NAME_CLOSE: u32 = 0xfffe_000f;

pub fn run<R: Bench>(x: &mut Cx<'_, R>) {
    match x.u8(Obj::STATE) {
        0 => open(x),
        2 => step(x),
        _ => close(x),
    }
}

/// Construction of the token list of a window (texts, names, kinds).
mod text;
use text::build;
use crate::actors::families::set::vars;
use crate::world::objects::Obj;
use crate::actors::player::Player;
use crate::actors::families::set::fields::SetObj;
use crate::actors::families::set_b::fields::DialogObj;

/// `movem.l (A2)+,D0-D6/A3 ; movem.l D0-D6/A3,(A1)`: the eight longs remain in D0-D6 and A3. Returns the seventh
/// (D6), which the closing of a window rereads (`shrink`: the number of lines). Original glitch: D2-D5 remain for
/// the following handlers (`dispatch::Leftovers`).
fn copy_palette<R: Bench>(x: &mut Cx<'_, R>, src: u32, dst: u32) -> u32 {
    copy_longs(x, src, dst, 8);
    let [d2, d3, d4, d5, d6] = [2, 3, 4, 5, 6].map(|i| set_tables::long(x.b, src + 4 * i));
    (*x.c.d2(), *x.c.d3(), *x.c.d4(), *x.c.d5()) = (d2, d3, d4, d5);
    d6
}

fn copy_longs<R: Bench>(x: &mut Cx<'_, R>, from: u32, to: u32, n: u32) {
    for i in 0..n {
        let v = set_tables::long(x.b, from + 4 * i);
        wr32(x.b, to + 4 * i, v);
    }
}


// --- loading a window definition -------------------------------------------------------------

/// Routine 0xd7124: reads the definition at `def`, computes the screen address and applies the kind's
/// initialization. Returns the screen address (A1 at the routine's exit) and, if a palette was copied, the D6 that
/// it leaves (`copy_palette`).
fn load<R: Bench>(x: &mut Cx<'_, R>, mut def: u32) -> (u32, Option<u32>) {
    x.set16(DialogObj::TOKENS, 0);
    wr16(x.b, BUFFER, 0);
    let col = set_tables::byte(x.b, def) as u16;
    let row = set_tables::byte(x.b, def + 1) as u16;
    let v = set_tables::byte(x.b, def + 2);
    x.set8(DialogObj::SMALL, v);
    let v = set_tables::byte(x.b, def + 3);
    x.set8(DialogObj::LINES, v);
    let v = set_tables::long(x.b, def + 4);
    x.set32(DialogObj::TEXT, v);
    let v = set_tables::word(x.b, def + 8);
    x.set16(Obj::X, v);
    let v = set_tables::word(x.b, def + 10);
    x.set16(Obj::Z, v);
    let v = set_tables::byte(x.b, def + 12);
    x.set8(DialogObj::KIND, v);
    let v = set_tables::byte(x.b, def + 13);
    x.set8(DialogObj::NEXT, v);
    let v = set_tables::word(x.b, def + 14);
    x.set16(DialogObj::WAIT, v);
    let v = set_tables::word(x.b, def + 16);
    x.set16(DialogObj::SETTING, v);
    def += 18;
    x.set32(DialogObj::NEXT_DEF, def);
    let offset = (col << 7).wrapping_add(row << 2) as i16 as i32 as u32;
    let a1 = SCREEN.wrapping_add(offset);
    x.set32(DialogObj::SCREEN_AT, a1);
    let height = if x.u8(DialogObj::SMALL) != 0 { 0xb } else { 0xf };
    x.set8(DialogObj::HEIGHT, height);
    let v = x.u8(DialogObj::LINES);
    x.set8(DialogObj::LINES_LEFT, v);
    kind_init(x, a1)
}

/// Initialization specific to the kind (jump table 0xd7196).
fn kind_init<R: Bench>(x: &mut Cx<'_, R>, a1: u32) -> (u32, Option<u32>) {
    match x.u8(DialogObj::KIND) {
        1 => {
            let script = x.u32(DialogObj::TEXT);
            let pick = if x.a5_u8(vars::PLAYERS_ALIVE) == 1 { 0 } else { 4 };
            let next = set_tables::long(x.b, script + pick);
            load(x, next)
        }
        2 | 4 => {
            let more = x.a5_u8(vars::PLAYERS_ALIVE).wrapping_sub(1).wrapping_mul(2);
            x.add8(DialogObj::LINES, more);
            let v = x.u8(DialogObj::LINES);
            x.set8(DialogObj::LINES_LEFT, v);
            (a1, None)
        }
        5 => {
            let mut d0 = (x.u8(DialogObj::CHOICE) as u32 * 2) as u8;
            d0 = d0.wrapping_add(x.a5_u8(vars::FRAME_PARITY) & 1);
            let step = set_tables::pc_byte(x.b, 0xd7218 + d0 as u32) as u32;
            let script = x.u32(DialogObj::TEXT).wrapping_add(step);
            load(x, script)
        }
        9 | 17 | 21 => {
            x.set8(SetObj::SUBSTATE, 2);
            x.set8(DialogObj::LINES, 0);
            x.set8(DialogObj::LINES_LEFT, 0);
            x.set8(DialogObj::HEIGHT, 0xff);
            match x.u8(DialogObj::KIND) {
                9 => {
                    x.set8(Obj::VARIANT, 1);
                    x.set8(DialogObj::KIND, 0);
                    (a1, None)
                }
                17 => {
                    x.set8(DialogObj::TICK, 1);
                    copy_palette(x, PALETTE_ALT, 0x91c300);
                    x.set16(DialogObj::WAIT, 0xffff);
                    let d6 = copy_palette(x, PALETTE_SRC, 0x91c1e0);
                    for base in [0x91c1e0, 0x91c300] {
                        for i in 0..8 {
                            let v = rd32(x.b, base + 4 * i) & 0x0fff_0fff;
                            wr32(x.b, base + 4 * i, v);
                        }
                    }
                    (0x91c200, Some(d6))
                }
                _ => {
                    if rd8(x.b, FLAG_SKIP) != 0 {
                        x.set8(SetObj::SUBSTATE, 6);
                        x.set8(DialogObj::TICK, 1);
                        return (a1, None);
                    }
                    x.set8(DialogObj::TICK, 1);
                    let d6 = copy_palette(x, PALETTE_SRC, 0x91c180);
                    (palette_tail(x, 0x91c180), Some(d6))
                }
            }
        }
        11 => {
            let script = x.u32(DialogObj::TEXT);
            let kind = x.u8(DialogObj::PLAYER_TEXT).wrapping_mul(4) as u32;
            let next = set_tables::long(x.b, script + kind);
            load(x, next)
        }
        13 => {
            let d6 = copy_palette(x, PALETTE_ALT, 0x91c300);
            (0x91c320, Some(d6))
        }
        14 => {
            x.set8(DialogObj::SPEED, 1);
            x.set8(DialogObj::KIND, 0);
            (a1, None)
        }
        19 => {
            x.set8(DialogObj::SLOT_OFFSET, 0x10);
            let mut script = x.u32(DialogObj::TEXT);
            for _ in 0..x.u8(DialogObj::CHOICE) {
                script += 0x12;
            }
            load(x, script)
        }
        20 => {
            let mut script = x.u32(DialogObj::TEXT);
            for p in players() {
                if p.u8(x.b, pl::ACTIVE) == 0 || (p.u8(x.b, Player::MAIN) as i8) >= 4 {
                    continue;
                }
                if p.u8(x.b, Player::CHARACTER) >> 1 == 3 {
                    script += 0x24;
                    break;
                }
            }
            load(x, script)
        }
        22 => {
            x.set8(DialogObj::SPEED, 1);
            x.set8(DialogObj::KIND, 1);
            (a1, None)
        }
        23 => {
            x.set32(Obj::STATE, 0x0400_0000);
            (a1, None)
        }
        _ => (a1, None),
    }
}

/// Window light: if a game is in progress and its bit 0 is set (`A5+0x48e`), the brightness of the palette 0x91c2a0
/// is carried over to the eight longs starting at `a1` (0x91c1e0 or 0x91c380). Returns the address after the eight
/// longs.
fn palette_tail<R: Bench>(x: &mut Cx<'_, R>, a1: u32) -> u32 {
    if x.a5_u8(vars::FADE_STEPS) == 0 || x.a5_u8(vars::FADE_PAGES) & 1 == 0 {
        return a1;
    }
    let bright = rd32(x.b, 0x91c2a0) & 0xf000_f000;
    for i in 0..8 {
        let v = rd32(x.b, a1 + 4 * i) & 0x0fff_0fff | bright;
        wr32(x.b, a1 + 4 * i, v);
    }
    a1 + 0x20
}

// --- states -----------------------------------------------------------------------------

fn open<R: Bench>(x: &mut Cx<'_, R>) {
    x.add8(Obj::STATE, 2);
    x.set8(DialogObj::SPEED, 3);
    let variant = x.u8(Obj::VARIANT) as u32;
    let def = set_tables::long(x.b, DEFINITIONS + 4 * variant);
    load(x, def);
    let stage = x.a5_u8(vars::SCENE);
    let a1 = if (0x45..0x5a).contains(&stage) { 0x91c380 } else { 0x91c1e0 };
    match x.u8(DialogObj::KIND) {
        0x11 => return,
        0x15 => {
            if rd8(x.b, FLAG_SKIP) == 0 {
                return;
            }
            return x.set8(DialogObj::KIND, 0);
        }
        _ => {}
    }
    copy_palette(x, PALETTE_SRC, a1);
    palette_tail(x, a1);
}

fn step<R: Bench>(x: &mut Cx<'_, R>) {
    match x.u8(SetObj::SUBSTATE) {
        0 => {
            x.add8(SetObj::SUBSTATE, 2);
            let v = x.u8(DialogObj::SPEED);
            x.set8(DialogObj::TICK, v);
            let a1 = x.u32(DialogObj::SCREEN_AT);
            frame_top(x, a1);
            x.add16(DialogObj::SCREEN_AT_LOW, 8);
            let a1 = x.u32(DialogObj::SCREEN_AT);
            frame_middle(x, a1);
        }
        2 => grow(x),
        4 => wait_key(x),
        6 => shrink(x),
        _ => {}
    }
}

fn rd8(b: &mut crate::world::World, a: u32) -> u8 {
    crate::world::mem::rd8(b, a)
}

/// Substate 2: the frame grows by one line every `+0x36` frames.
fn grow<R: Bench>(x: &mut Cx<'_, R>) {
    let n = x.u8(DialogObj::TICK).wrapping_sub(1);
    x.set8(DialogObj::TICK, n);
    if (n as i8) > 0 {
        return;
    }
    let v = x.u8(DialogObj::SPEED);
    x.set8(DialogObj::TICK, v);
    if x.u8(DialogObj::LINES_LEFT) != 0 {
        x.add8(DialogObj::LINES_LEFT, 0xff);
        let a1 = x.u32(DialogObj::SCREEN_AT);
        frame_side(x, a1);
        x.add16(DialogObj::SCREEN_AT_LOW, 4);
        let a1 = x.u32(DialogObj::SCREEN_AT);
        frame_middle(x, a1);
        return;
    }
    x.add8(SetObj::SUBSTATE, 2);
    let v = x.u8(DialogObj::LINES);
    x.set8(DialogObj::LINES_LEFT, v);
    let slot = SLOTS + x.u8(DialogObj::SLOT_OFFSET) as u32;
    x.set16(DialogObj::SLOT, slot as u16);
    let v = x.u16(Obj::X);
    wr16(x.b, slot + 4, v);
    let v = x.u16(Obj::Z);
    wr16(x.b, slot + 6, v);
    let kind = x.u8(DialogObj::KIND);
    if kind == 0 {
        let v = x.u32(DialogObj::TEXT);
        wr32(x.b, slot, v);
        return;
    }
    if kind != 0xc {
        wr32(x.b, slot, BUFFER);
    }
    build(x, kind);
}

/// Substate 4: waits for a player to press (countdown `+0x32`; -1 = endless).
fn wait_key<R: Bench>(x: &mut Cx<'_, R>) {
    if x.u16(DialogObj::WAIT) == 0xffff {
        return;
    }
    if poll_keys(x) != 0 {
        x.set16(DialogObj::WAIT, 0);
    }
    if x.u16(DialogObj::WAIT) == 0 {
        x.add8(SetObj::SUBSTATE, 2);
        if x.u8(DialogObj::KIND) == 0xc {
            return;
        }
        let slot = x.linked(DialogObj::SLOT);
        slot.set_u32(x.b, Obj::ACTIVE, 0);
        return;
    }
    x.add16(DialogObj::WAIT, 0xffff);
}

/// Keys pressed by the players concerned (byte `A5+0xc+n`, bits 4 to 7).
fn poll_keys<R: Bench>(x: &mut Cx<'_, R>) -> u8 {
    let mut who: i8 = -1;
    if x.u8(Obj::VARIANT) == 0x8b {
        who = x.u8(DialogObj::SPEAKER) as i8;
    } else if (x.u8(DialogObj::LISTENER) as i8) >= 0 {
        who = x.u8(DialogObj::LISTENER) as i8;
    }
    if x.a5_u8(vars::GAME) == 0 {
        return 0;
    }
    let mut keys = 0u8;
    for n in (0..4u32).rev() {
        let p = Obj(PLAYERS + n * PLAYER_SIZE);
        if p.u8(x.b, pl::ACTIVE) == 0 || (p.u8(x.b, Player::MAIN) as i8) >= 4 {
            continue;
        }
        if who >= 0 && p.u8(x.b, Player::INDEX) as i8 != who {
            continue;
        }
        keys |= x.a5_u8(vars::INPUTS_NEW + n);
    }
    keys & 0xf0
}

/// Substate 6: the frame closes by one line per step; then the object is deleted.
fn shrink<R: Bench>(x: &mut Cx<'_, R>) {
    let n = x.u8(DialogObj::TICK).wrapping_sub(1);
    x.set8(DialogObj::TICK, n);
    if (n as i8) > 0 {
        return;
    }
    let v = x.u8(DialogObj::SPEED);
    x.set8(DialogObj::TICK, v);
    let mut a6;
    // D6: number of lines of the window (byte), which the loading of a definition may overwrite
    let d6;
    if x.u8(DialogObj::LINES_LEFT) != 0 {
        x.add8(DialogObj::LINES_LEFT, 0xff);
        let a1 = x.u32(DialogObj::SCREEN_AT);
        erase_column(x, a1);
        x.add16(DialogObj::SCREEN_AT_LOW, 0xfffc);
        let a1 = x.u32(DialogObj::SCREEN_AT);
        return frame_middle(x, a1);
    }
    if (x.u8(DialogObj::NEXT) as i8) >= 0 {
        x.set8(SetObj::SUBSTATE, 0);
        a6 = x.u32(DialogObj::SCREEN_AT).wrapping_sub(8);
        let lines = x.u8(DialogObj::HEIGHT) as u32;
        let def = x.u32(DialogObj::NEXT_DEF);
        let (a1, palette_d6) = load(x, def);
        d6 = palette_d6.unwrap_or(lines);
        if x.u8(DialogObj::HEIGHT) == d6 as u8 && a1 == a6 {
            return;
        }
    } else {
        x.add8(SetObj::SUBSTATE, 2);
        x.set8(DialogObj::CLOSING, 1);
        x.set32(Obj::STATE, 0x0600_0000);
        a6 = x.u32(DialogObj::SCREEN_AT).wrapping_sub(8);
        d6 = x.u8(DialogObj::HEIGHT) as u32;
    }
    if (d6 as u8 as i8) < 0 {
        return;
    }
    let count = (d6 as u16).wrapping_mul(2).wrapping_add(1);
    for _ in 0..=count {
        for k in 0..3 {
            wr32(x.b, a6 + 4 * k, 0x20_0000);
        }
        a6 += 0xc + 0x74;
    }
}

fn close<R: Bench>(x: &mut Cx<'_, R>) {
    let n = x.a5_u8(vars::SCENES).wrapping_sub(1);
    x.set_a5_u8(vars::SCENES, n);
    x.set8(DialogObj::CLOSING, 0);
    let v = crate::world::mem::rd8(x.b, OPEN_COUNT).wrapping_sub(1);
    crate::world::mem::wr8(x.b, OPEN_COUNT, v);
    x.free(&pools::SET);
}

// --- drawing the frame --------------------------------------------------------------------

fn frame_top<R: Bench>(x: &mut Cx<'_, R>, mut a1: u32) {
    let tiles: [u32; 2] = if x.u8(DialogObj::SMALL) != 0 { [0x0434_000f, 0x0444_000f] } else { [0x0400_000f, 0x0410_000f] };
    let (mut d0, mut d1) = (tiles[0], tiles[1]);
    let rows = x.u8(DialogObj::HEIGHT) as i16;
    for _ in 0..=rows {
        wr32(x.b, a1, d0);
        wr32(x.b, a1 + 4, d1);
        d0 = d0.wrapping_add(0x1_0000);
        d1 = d1.wrapping_add(0x1_0000);
        a1 += 0x80;
    }
    d0 = (d0 & !0xff) | (d0 as u8 | 0x20) as u32;
    d1 = (d1 & !0xff) | (d1 as u8 | 0x20) as u32;
    d0 = d0.wrapping_add(0xffff_0000);
    d1 = d1.wrapping_add(0xffff_0000);
    let rows = x.u8(DialogObj::HEIGHT) as i16;
    for _ in 0..=rows {
        wr32(x.b, a1, d0);
        wr32(x.b, a1 + 4, d1);
        d0 = d0.wrapping_add(0xffff_0000);
        d1 = d1.wrapping_add(0xffff_0000);
        a1 += 0x80;
    }
}

fn frame_side<R: Bench>(x: &mut Cx<'_, R>, mut a1: u32) {
    let mut d0 = if x.u8(DialogObj::SMALL) != 0 { 0x0432_000f } else { 0x0430_000f };
    wr32(x.b, a1, d0);
    a1 += 0x80;
    d0 = (d0 & !0xff) | (d0 as u8 | 0x20) as u32;
    let count = (x.u8(DialogObj::HEIGHT) as u16).wrapping_mul(2).wrapping_sub(1) as u32 + 1;
    for _ in 0..count {
        wr32(x.b, a1, 0x0431_000f);
        a1 += 0x80;
    }
    wr32(x.b, a1, d0);
}

fn frame_middle<R: Bench>(x: &mut Cx<'_, R>, mut a1: u32) {
    let mut d0 = if x.u8(DialogObj::SMALL) != 0 { 0x0454_000f } else { 0x0420_000f };
    let rows = x.u8(DialogObj::HEIGHT) as i16;
    for _ in 0..=rows {
        wr32(x.b, a1, d0);
        d0 = d0.wrapping_add(0x1_0000);
        a1 += 0x80;
    }
    d0 = (d0 & !0xff) | (d0 as u8 | 0x20) as u32;
    d0 = d0.wrapping_add(0xffff_0000);
    let rows = x.u8(DialogObj::HEIGHT) as i16;
    for _ in 0..=rows {
        wr32(x.b, a1, d0);
        d0 = d0.wrapping_add(0xffff_0000);
        a1 += 0x80;
    }
}

fn erase_column<R: Bench>(x: &mut Cx<'_, R>, mut a1: u32) {
    let rows = (x.u8(DialogObj::HEIGHT) as u16).wrapping_mul(2).wrapping_add(1);
    for _ in 0..=rows {
        wr32(x.b, a1, 0x20_0000);
        a1 += 0x80;
    }
}

use super::*;
use crate::actors::families::set::tables as set_tables;
use crate::world::objects::player::Player;
use crate::actors::families::set::fields::SetObj;
use crate::actors::families::set_b::fields::WindowEffect;

/// Write into the buffer (register A4 of the original) and number of characters written (D6).
pub(super) struct Text {
    pub(super) at: u32,
    pub(super) count: u16,
}

impl Text {
    fn byte<R>(&mut self, x: &mut Cx<'_, R>, v: u8) {
        wr8(x.b, self.at, v);
        self.at += 1;
    }
    fn word<R>(&mut self, x: &mut Cx<'_, R>, v: u16) {
        wr16(x.b, self.at, v);
        self.at += 2;
    }
    /// Final byte, without advancing.
    fn end<R>(&mut self, x: &mut Cx<'_, R>, v: u8) {
        wr8(x.b, self.at, v);
    }
    fn char(&mut self) {
        self.count = self.count.wrapping_add(1);
    }
    /// Character (> 15) or command and its argument, read at `src`.
    fn plain<R>(&mut self, x: &mut Cx<'_, R>, c: u8, src: &mut u32) {
        self.byte(x, c);
        if c <= 15 {
            let arg = set_tables::byte(x.b, *src);
            *src += 1;
            self.byte(x, arg);
        }
        self.char();
    }
    /// 0xc8cda: 0xfe + argument, or 0xff + count + argument (the byte after 0xff is skipped).
    fn escape<R>(&mut self, x: &mut Cx<'_, R>, c: u8, src: &mut u32) {
        self.byte(x, c);
        if c != 0xfe {
            let n = self.count as u8;
            self.byte(x, n);
            *src += 1;
            self.count = 0;
        }
        let arg = set_tables::byte(x.b, *src);
        *src += 1;
        self.byte(x, arg);
    }
    /// 0xc88d8 : 0xff, compte, 0x10.
    fn counted<R>(&mut self, x: &mut Cx<'_, R>) {
        self.byte(x, 0xff);
        let n = self.count as u8;
        self.byte(x, n);
        self.byte(x, 0x10);
    }
}

/// Way of writing a command glyph (≤ 15): its byte and the next one, or the same one twice
/// (0xc88e4, variante de l'original).
#[derive(Clone, Copy, PartialEq)]
enum Glyph {
    Pair,
    Twice,
}

/// Way of recognizing a default name from the first byte of the name.
#[derive(Clone, Copy)]
enum Defaults {
    /// 0xf9 to 0xfe: default name no. `-byte - 3`.
    High,
    /// ≥ 130: default name according to the byte (136, 144, 130, 160, other).
    Codes,
}

/// Name of 6 coded characters (0x51 = first glyph, 0xff end) at `src`, or default name, written
/// with the glyphs of table 0xc9216.
fn name<R: Bench>(x: &mut Cx<'_, R>, t: &mut Text, mut src: u32, defaults: Defaults, glyph: Glyph) {
    let first = set_tables::byte(x.b, src);
    let default = match defaults {
        Defaults::High if first.wrapping_add(1) >= 250 => Some(first.wrapping_neg().wrapping_sub(3).wrapping_mul(4)),
        Defaults::Codes if first >= 130 => Some(match first {
            136 => 0,
            144 => 4,
            130 => 8,
            160 => 12,
            _ => 16,
        }),
        _ => None,
    };
    let mut n: u16 = 5;
    if let Some(index) = default {
        let table = set_tables::long(x.b, DEFAULT_NAMES + index as u32);
        n = set_tables::word(x.b, table);
        src = table + 2;
    }
    loop {
        let c = set_tables::byte(x.b, src);
        src += 1;
        if c == 0xff {
            return;
        }
        let index = c.wrapping_sub(0x51) as u32 * 2;
        let mut g = GLYPHS.wrapping_add(set_tables::word(x.b, GLYPHS + index) as i16 as i32 as u32);
        let v = set_tables::byte(x.b, g);
        if v <= 15 {
            t.byte(x, v);
            if glyph == Glyph::Pair {
                g += 1;
            }
        }
        let v = set_tables::byte(x.b, g);
        t.byte(x, v);
        t.char();
        n = n.wrapping_sub(1);
        if n == 0xffff {
            return;
        }
    }
}

/// Copy of a text up to 0xfd: `special(c, src)` first handles a byte (true if it did).
fn copy_until_end<R: Bench, F>(x: &mut Cx<'_, R>, t: &mut Text, mut src: u32, mut special: F) -> u32
where
    F: FnMut(&mut Cx<'_, R>, &mut Text, u8, &mut u32) -> bool,
{
    loop {
        let c = set_tables::byte(x.b, src);
        src += 1;
        if c == 0xfd {
            t.end(x, c);
            return src;
        }
        if special(x, t, c, &mut src) {
            continue;
        }
        if c >= 0xfe {
            t.escape(x, c, &mut src);
        } else {
            t.plain(x, c, &mut src);
        }
    }
}

/// 0xc87de: the text as is.
fn copy_text<R: Bench>(x: &mut Cx<'_, R>, t: &mut Text) {
    let src = x.u32(f::TEXT);
    copy_until_end(x, t, src, |_, _, _, _| false);
}

/// Code 7 followed by `arg`: insertion (name...) in its place.
fn is_insert<R: Bench>(x: &mut Cx<'_, R>, c: u8, src: u32, arg: u8) -> bool {
    c == 7 && set_tables::byte(x.b, src) == arg
}

pub(super) fn compose<R: Bench>(x: &mut Cx<'_, R>, t: &mut Text, style: u8, slot: u32) {
    match style {
        1 | 5 | 9 | 11 | 14 | 19 | 20 | 22 => copy_text(x, t),
        // 0xc8816: class title and name of each player present
        2 => {
            copy_palette_b(x);
            for k in 0..4 {
                let p = Obj(A5 + ram::PLAYERS + k * PLAYER_SIZE);
                if p.u8(x.b, Player::ACTIVE) != 0 && p.u8(x.b, Player::MAIN) != 4 {
                    t.count = 0;
                    title_and_name(x, t, p);
                }
            }
            t.end(x, 0xfd);
        }
        3 => linked_effect(x, t),
        // 0xc8ac0: names of the players in game, separated
        4 => {
            copy_palette_b(x);
            let mut left = x.a5_u8(ram::PLAYER_COUNT);
            for k in 0..4 {
                let p = Obj(A5 + ram::PLAYERS + k * PLAYER_SIZE);
                if p.u8(x.b, Player::ACTIVE) == 0 || (p.u8(x.b, Player::MAIN) as i8) >= 4 {
                    continue;
                }
                t.count = 0;
                prefixed_name(x, t, p.0 + NAME);
                left = left.wrapping_sub(1);
                if left != 0 {
                    let sep = set_tables::byte(x.b, SEPARATOR);
                    t.byte(x, sep);
                    t.char();
                    t.counted(x);
                }
            }
            let sep = set_tables::byte(x.b, SEPARATOR);
            t.byte(x, sep);
            t.end(x, 0xfd);
        }
        6 | 7 => words(x, t, slot),
        // 0xc8b58: characters written as a word (code 0x7003 of the original cannot occur)
        8 => {
            let mut src = x.u32(f::TEXT);
            loop {
                let c = set_tables::byte(x.b, src);
                src += 1;
                if c == 0xfd {
                    return t.end(x, c);
                }
                if c.wrapping_add(3) & 0x80 != 0 {
                    t.escape(x, c, &mut src);
                } else {
                    t.word(x, c as u16);
                    t.char();
                }
            }
        }
        // 0xc8b9e: text with the name of the class 5 player, otherwise second text as is
        10 => {
            let text = x.u32(f::TEXT);
            let Some(p) = find_player(x, |c| c == 5, false) else {
                let v = set_tables::long(x.b, text);
                let slot = self::slot(x);
                return wr32(x.b, slot, v);
            };
            let src = set_tables::long(x.b, text + 4);
            copy_until_end(x, t, src, |x, t, c, src| {
                if !is_insert(x, c, *src, 4) {
                    return false;
                }
                *src += 1;
                name(x, t, p.0 + NAME, Defaults::High, Glyph::Pair);
                true
            });
        }
        12 => x.set16(f::DURATION, 0xffff),
        // 0xc8c86: text with the saved name, framed
        13 => {
            let src = x.u32(f::TEXT);
            copy_until_end(x, t, src, |x, t, c, src| {
                if !is_insert(x, c, *src, 5) {
                    return false;
                }
                *src += 1;
                t.byte(x, 0xfe);
                t.byte(x, 0x18);
                name(x, t, SAVED_NAME, Defaults::Codes, Glyph::Pair);
                t.byte(x, 0xfe);
                t.byte(x, 0x0f);
                true
            });
        }
        // 0xc8cf6: text depending on the number of players, then like style 3
        15 => {
            let table = x.u32(f::TEXT);
            let k = if x.a5_u8(ram::PLAYER_COUNT) == 1 { 0 } else { 4 };
            let v = set_tables::long(x.b, table + k);
            x.set32(f::TEXT, v);
            linked_effect(x, t);
        }
        // 0xc8d12: text as is, without composition
        16 => {
            x.set8(f::INDEX, 1);
            let text = x.u32(f::TEXT);
            let slot = self::slot(x);
            wr32(x.b, slot, text);
        }
        17 => linked_message(x, t),
        // 0xc8ece: text as is and a linked SOBJ 6 object (variant 19)
        18 => {
            copy_text(x, t);
            if let Some(o) = spawn(x, &pools::SOBJ, 6, 19) {
                o.set_u16(x.b, f::LINKED, x.o.0 as u16);
                let v = x.u8(f::PARAM);
                o.set_u8(x.b, SetObj::PARAM, v);
            }
        }
        // 0xc8eea: text preceded by a color command, with the saved name
        21 => {
            t.byte(x, 0xfe);
            t.byte(x, 0x0c);
            if x.a5_u8(ram::PAUSED) != 0 && x.a5_u8(ram::FROZEN) != 0 {
                t.at -= 4;
                copy_palette(x, PALETTE_SRC, PALETTE);
            }
            t.count = 0;
            let mut src = x.u32(f::TEXT);
            loop {
                let c = set_tables::byte(x.b, src);
                src += 1;
                match c {
                    0xfd => return t.end(x, c),
                    0xff => t.escape(x, c, &mut src),
                    7 if set_tables::byte(x.b, src) == 5 => {
                        src += 1;
                        name(x, t, SAVED_NAME, Defaults::Codes, Glyph::Pair);
                    }
                    _ => t.plain(x, c, &mut src),
                }
            }
        }
        23 => wr32(x.b, slot, 0),
        _ => {}
    }
}

/// 0xc8844: title of the player's class, their framed name, and the count.
fn title_and_name<R: Bench>(x: &mut Cx<'_, R>, t: &mut Text, p: Obj) {
    let class = (p.u8(x.b, Player::CHARACTER) >> 1).wrapping_mul(4);
    let mut src = set_tables::long(x.b, CLASS_TITLES + class as u32);
    loop {
        let c = set_tables::byte(x.b, src);
        if c == 0xfd {
            break;
        }
        src += 1;
        t.byte(x, c);
        if c <= 15 {
            let arg = set_tables::byte(x.b, src);
            src += 1;
            t.byte(x, arg);
        }
        t.char();
    }
    t.byte(x, 0xfe);
    t.byte(x, 0x18);
    name(x, t, p.0 + NAME, Defaults::High, Glyph::Pair);
    t.byte(x, 0xfe);
    t.byte(x, 0x0f);
    t.counted(x);
}

/// 0xc88e8: name preceded by the command 0xfe 0x18.
fn prefixed_name<R: Bench>(x: &mut Cx<'_, R>, t: &mut Text, src: u32) {
    t.byte(x, 0xfe);
    t.byte(x, 0x18);
    name(x, t, src, Defaults::High, Glyph::Twice);
}

/// Creates an object in `pool`: active, type `kind`, variant `variant`.
fn spawn<R: Bench>(x: &mut Cx<'_, R>, pool: &pools::Pool, kind: u8, variant: u8) -> Option<Obj> {
    let o = pool.alloc(x.b)?;
    o.set_u8(x.b, Obj::ACTIVE, 1);
    o.set_u8(x.b, Obj::KIND, kind);
    o.set_u8(x.b, Obj::VARIANT, variant);
    Some(o)
}

/// 0xc8a9c: an EFFCT 23 effect (set by `PARAM`) tied to the window, which will close it; text as is.
/// With no room, retries on the next frame.
fn linked_effect<R: Bench>(x: &mut Cx<'_, R>, t: &mut Text) {
    let setting = x.u8(f::PARAM);
    let Some(o) = spawn(x, &pools::EFFCT, 23, 0) else {
        return x.add8(f::SUB, 0xfe);
    };
    o.set_u8(x.b, SetObj::PARAM, setting);
    o.set_u16(x.b, WindowEffect::WINDOW, x.o.0 as u16);
    x.set16(f::DURATION, 0xffff);
    copy_text(x, t);
}

/// 0xc8b08: text in words; 0x7001 and 0x7002 insert the word list designated by the text
/// engine entry according to the number of players; a negative word is a 0xfe/0xff command.
fn words<R: Bench>(x: &mut Cx<'_, R>, t: &mut Text, slot: u32) {
    let mut src = x.u32(f::TEXT);
    let mut list = slot;
    loop {
        let w = set_tables::word(x.b, src);
        src += 2;
        if w == 0 {
            return wr16(x.b, t.at, 0);
        }
        if w & 0x8000 != 0 {
            t.escape(x, w as u8, &mut src);
        } else if w == 0x7001 || w == 0x7002 {
            let k = x.a5_u8(ram::PLAYER_COUNT).wrapping_sub(1) as u32 * 4;
            list = set_tables::long(x.b, list + k);
            loop {
                let v = set_tables::word(x.b, list);
                if v == 0 {
                    break;
                }
                list += 2;
                t.word(x, v);
                t.char();
            }
        } else {
            t.word(x, w);
            t.char();
        }
    }
}

/// 0xc8d22: text that talks about the linked object (`LINKED`): code 7 followed by 6 to 9 (name, equipment
/// item); at the end, a linked SET 15 (variant 17), set by `PARAM`. With no room, it
/// starts over on the next frame.
fn linked_message<R: Bench>(x: &mut Cx<'_, R>, t: &mut Text) {
    let src = x.u32(f::TEXT);
    copy_until_end(x, t, src, |x, t, c, src| {
        if c != 7 || (set_tables::byte(x.b, *src) as i8) < 6 {
            return false;
        }
        let k = set_tables::byte(x.b, *src);
        *src += 1;
        let linked = x.linked(f::LINKED);
        match k {
            6 => prefixed_name(x, t, linked.0 + NAME),
            7 | 8 => equipment_message(x, t, linked),
            9 => {
                if x.u16(f::LINKED) != 0 {
                    prefixed_name(x, t, linked.0 + NAME);
                }
            }
            _ => {}
        }
        true
    });
    let Some(o) = spawn(x, &pools::SET, 15, 17) else {
        return x.add8(f::SUB, 0xfe);
    };
    o.set_u16(x.b, f::LINKED, x.o.0 as u16);
    let v = x.u16(f::PARAM);
    o.set_u16(x.b, f::CHOICE, v);
}

/// 0xc8d88: message of the first piece of equipment of player `p` recognized in the lists of their
/// class (two weapon slots, then six items).
fn equipment_message<R: Bench>(x: &mut Cx<'_, R>, t: &mut Text, p: Obj) {
    let (main, items) = EQUIP_LISTS[(p.u8(x.b, Player::CHARACTER) >> 1 != 0) as usize];
    let candidates = [(main, p.u8(x.b, EQUIP_A)), (main, p.u8(x.b, EQUIP_B))]
        .into_iter()
        .chain((0..6).map(|i| (items, p.u8(x.b, ITEMS + i))));
    for (list, item) in candidates.collect::<Vec<_>>() {
        if let Some(i) = find_in_list(x, list, item) {
            return message(x, t, i);
        }
    }
}

/// 0xc8e24: rank of `item` in the byte list `list` (ended by a negative byte).
fn find_in_list<R: Bench>(x: &mut Cx<'_, R>, list: u32, item: u8) -> Option<u32> {
    (0..).map_while(|i| {
        let c = set_tables::byte(x.b, list + i);
        ((c as i8) >= 0).then_some((i, c))
    })
    .find(|&(_, c)| c == item)
    .map(|(i, _)| i)
}

/// 0xc8e5c: message no. `i` (two tables depending on the language), up to 0xfd excluded.
fn message<R: Bench>(x: &mut Cx<'_, R>, t: &mut Text, i: u32) {
    let table = if x.a5_u16(ram::LANGUAGE) == 4 { MESSAGES_LANG4 } else { MESSAGES_OTHER };
    let mut src = set_tables::pc_long(x.b, table + 4 * i);
    loop {
        let c = set_tables::byte(x.b, src);
        src += 1;
        if c == 0xfd {
            return;
        }
        t.plain(x, c, &mut src);
    }
}

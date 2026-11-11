use super::*;
use crate::actors::families::set::tables as set_tables;
use crate::actors::families::set::vars;
use crate::world::objects::Obj;
use crate::actors::player::Player;
use crate::actors::families::set::fields::SetObj;
use crate::actors::families::set_b::fields::{DialogObj, WindowEffect};

// --- token output ----------------------------------------------------------------------

/// Write pointer (A4) and number of characters (D6) of the token list.
struct Out {
    at: u32,
    n: u16,
}

impl Out {
    fn put<R: Bench>(&mut self, x: &mut Cx<'_, R>, w: u16) {
        wr16(x.b, self.at, w);
        self.at += 2;
    }
    fn put32<R: Bench>(&mut self, x: &mut Cx<'_, R>, v: u32) {
        self.put(x, (v >> 16) as u16);
        self.put(x, v as u16);
    }
    /// Terminator, not advanced.
    fn end<R: Bench>(&mut self, x: &mut Cx<'_, R>) {
        wr16(x.b, self.at, 0);
    }
}

/// Command (negative token): copies the token, `-8 * n`, then the word that follows the next word (0xfffe: the next
/// word alone).
fn command<R: Bench>(x: &mut Cx<'_, R>, o: &mut Out, a6: &mut u32, tok: u16) {
    o.put(x, tok);
    if tok == TOK_LINK_A {
        let w = set_tables::word(x.b, *a6);
        *a6 += 2;
        return o.put(x, w);
    }
    o.put(x, (o.n << 3).wrapping_neg());
    o.n = 0;
    *a6 += 2;
    let w = set_tables::word(x.b, *a6);
    *a6 += 2;
    o.put(x, w);
}

/// Reads the next word of the text.
fn next<R: Bench>(x: &mut Cx<'_, R>, a6: &mut u32) -> u16 {
    let w = set_tables::word(x.b, *a6);
    *a6 += 2;
    w
}

/// Raw text: tokens copied, 0x7000 (number of players) replaced according to the number of players.
fn plain<R: Bench>(x: &mut Cx<'_, R>, o: &mut Out, a6: &mut u32) {
    loop {
        let tok = next(x, a6);
        if tok == 0 {
            return o.end(x);
        }
        if (tok as i16) < 0 {
            command(x, o, a6, tok);
        } else if tok == 0x7000 {
            if x.a5_u8(vars::PLAYERS_ALIVE) != 1 {
                o.put(x, 0x18f3);
                o.n += 1;
            }
        } else {
            o.put(x, tok);
            o.n += 1;
        }
    }
}

/// Character string of a name (up to 6 bytes, index into the tile table 0xd7e8a; 0xff terminates). `special`: bytes
/// >= 0xf9 designate a predefined name of the table 0xd7604.
fn name<R: Bench>(x: &mut Cx<'_, R>, o: &mut Out, mut a1: u32) {
    let mut remaining: u16 = 5;
    let first = set_tables::byte(x.b, a1);
    if first.wrapping_add(1) >= 0xfa {
        let index = (0u8.wrapping_sub(first).wrapping_sub(3)).wrapping_mul(4) as u32;
        a1 = set_tables::long(x.b, NAME_TABLE + index);
        remaining = set_tables::word(x.b, a1);
        a1 += 2;
    }
    loop {
        let c = set_tables::byte(x.b, a1);
        a1 += 1;
        if c == 0xff {
            return;
        }
        let tile = set_tables::word(x.b, CHARS + 2 * c as u32);
        o.put(x, tile);
        o.n += 1;
        remaining = remaining.wrapping_sub(1);
        if remaining == 0xffff {
            return;
        }
    }
}

/// Variant for names with a category (starting byte >= 0x82: 0x88, 0x90, 0x82, 0xa0 or other, each with its own
/// text in the table 0xd7604).
fn name_category<R: Bench>(x: &mut Cx<'_, R>, o: &mut Out, mut a1: u32) {
    let mut remaining: u16 = 5;
    let first = set_tables::byte(x.b, a1);
    if first >= 0x82 {
        let mut d0 = 0u32;
        for probe in [0x88, 0x90, 0x82, 0xa0] {
            if first == probe {
                break;
            }
            d0 += 4;
        }
        a1 = set_tables::long(x.b, NAME_TABLE + d0);
        remaining = set_tables::word(x.b, a1);
        a1 += 2;
    }
    loop {
        let c = set_tables::byte(x.b, a1);
        a1 += 1;
        if c == 0xff {
            return;
        }
        let tile = set_tables::word(x.b, CHARS + 2 * c as u32);
        o.put(x, tile);
        o.n += 1;
        remaining = remaining.wrapping_sub(1);
        if remaining == 0xffff {
            return;
        }
    }
}

/// End of a name block: word -1, `-8 * n`, 0x10 (routine 0xd75a6).
fn name_tail<R: Bench>(x: &mut Cx<'_, R>, o: &mut Out) {
    o.put(x, 0xffff);
    o.put(x, (o.n << 3).wrapping_neg());
    o.put(x, 0x10);
}

/// Name of player `p` between the openings 0xfffe0018 and 0xfffe000f (routine 0xd75b6 without the end).
fn player_name<R: Bench>(x: &mut Cx<'_, R>, o: &mut Out, p: Obj) {
    o.put32(x, NAME_OPEN);
    name(x, o, p.0 + 0x118);
}

/// Complete block of a kind 1 player (routine 0xd7536): title, name, end.
fn player_block<R: Bench>(x: &mut Cx<'_, R>, o: &mut Out, p: Obj) {
    let index = ((p.u8(x.b, Player::CHARACTER) >> 1).wrapping_mul(4)) as u32;
    let mut a1 = set_tables::long(x.b, PLAYER_TITLES + index);
    loop {
        let w = set_tables::word(x.b, a1);
        if w == 0 {
            break;
        }
        o.put(x, w);
        a1 += 2;
        o.n += 1;
    }
    player_name(x, o, p);
    o.put32(x, NAME_CLOSE);
    name_tail(x, o);
}

// --- text construction, according to the kind ----------------------------------------------

/// Builds the token list of the kind `kind` (table 0xd7464: the index is shifted by one notch, kind `k` jumps to
/// the `k`-th entry of the table 0xd7466; kind 0 is handled before).
pub(super) fn build<R: Bench>(x: &mut Cx<'_, R>, kind: u8) {
    let mut o = Out { at: BUFFER, n: 0 };
    let mut a6 = x.u32(DialogObj::TEXT);
    match kind {
        2 => {
            copy_palette(x, PALETTE_ALT, 0x91c300);
            for p in players() {
                if p.u8(x.b, pl::ACTIVE) != 0 && p.u8(x.b, Player::MAIN) != 4 {
                    o.n = 0;
                    player_block(x, &mut o, p);
                }
            }
            o.end(x);
        }
        3 => {
            let setting = x.u8(DialogObj::SETTING);
            let Some(other) = crate::actors::families::effects_creators::vote(x.b, 0, setting) else {
                x.add8(SetObj::SUBSTATE, 0xfe);
                return;
            };
            other.set_u16(x.b, WindowEffect::WINDOW, x.o.0 as u16);
            x.set16(DialogObj::WAIT, 0xffff);
            build(x, 1);
        }
        4 => {
            copy_palette(x, PALETTE_ALT, 0x91c300);
            let mut left = x.a5_u8(vars::PLAYERS_ALIVE);
            for p in players() {
                if p.u8(x.b, pl::ACTIVE) == 0 || (p.u8(x.b, Player::MAIN) as i8) >= 4 {
                    continue;
                }
                o.n = 0;
                player_name(x, &mut o, p);
                left = left.wrapping_sub(1);
                if left == 0 {
                    continue;
                }
                let sep = set_tables::word(x.b, SEPARATOR);
                o.put(x, sep);
                o.n += 1;
                name_tail(x, &mut o);
            }
            let sep = set_tables::word(x.b, SEPARATOR);
            o.put(x, sep);
            o.end(x);
        }
        6 | 7 => {
            loop {
                let tok = next(x, &mut a6);
                if tok == 0 {
                    break;
                }
                if (tok as i16) < 0 {
                    command(x, &mut o, &mut a6, tok);
                    continue;
                }
                let table = match tok {
                    0x7001 => PLAYERS_ONE_TWO,
                    0x7002 => PLAYERS_ONE_TWO_B,
                    _ => {
                        o.put(x, tok);
                        o.n += 1;
                        continue;
                    }
                };
                let index = ((x.a5_u8(vars::PLAYERS_ALIVE).wrapping_sub(1)) as u16).wrapping_mul(4) as u32;
                let mut a1 = set_tables::long(x.b, table + index);
                loop {
                    let w = set_tables::word(x.b, a1);
                    if w == 0 {
                        break;
                    }
                    o.put(x, w);
                    a1 += 2;
                    o.n += 1;
                }
            }
            o.end(x);
        }
        8 => {
            loop {
                let tok = next(x, &mut a6);
                if tok == 0 {
                    break;
                }
                if (tok as i16) < 0 {
                    command(x, &mut o, &mut a6, tok);
                } else if tok == 0x7003 {
                    let w = if x.a5_u8(vars::PLAYERS_ALIVE) == 1 { 0x2013 } else { 0x2018 };
                    o.put(x, w);
                    o.n += 1;
                } else {
                    o.put(x, tok);
                    o.n += 1;
                }
            }
            o.end(x);
        }
        10 => build_9(x, &mut o, a6),
        12 => x.set16(DialogObj::WAIT, 0xffff),
        13 => text_with_name(x, &mut o, a6, 0x7005, false),
        15 => {
            let a1 = x.u32(DialogObj::TEXT);
            let first = set_tables::long(x.b, a1);
            x.set32(DialogObj::TEXT, first);
            if x.a5_u8(vars::PLAYERS_ALIVE) != 1 {
                let second = set_tables::long(x.b, a1 + 4);
                x.set32(DialogObj::TEXT, second);
            }
            build(x, 3);
        }
        16 => {
            x.set8(DialogObj::PLAYER_TEXT, 1);
            let slot = x.linked(DialogObj::SLOT);
            let v = x.u32(DialogObj::TEXT);
            slot.set_u32(x.b, Obj::ACTIVE, v);
        }
        17 => build_16(x, &mut o, a6),
        18 => {
            build(x, 1);
            if let Ok(child) = crate::services::effects_aids::scenery_6(x.b, 0x13) {
                child.set_u16(x.b, SetObj::OWNER, x.o.0 as u16);
                let v = x.u8(DialogObj::SETTING);
                child.set_u8(x.b, SetObj::PARAM, v);
            }
        }
        21 => build_20(x, &mut o, a6),
        1 | 5 | 9 | 11 | 14 | 19 | 20 | 22 | 23 => {
            plain(x, &mut o, &mut a6);
            let v = rd16(x.b, BUFFER);
            x.set16(DialogObj::TOKENS, v);
        }
        _ => {}
    }
}

/// Text whose token `special` calls the name of a player located at 0xff0fd2 (kinds 12, 20).
fn text_with_name<R: Bench>(x: &mut Cx<'_, R>, o: &mut Out, mut a6: u32, special: u16, _alt: bool) {
    loop {
        let tok = next(x, &mut a6);
        if tok == 0 {
            return o.end(x);
        }
        if (tok as i16) < 0 {
            command(x, o, &mut a6, tok);
        } else if tok == special {
            o.put32(x, NAME_OPEN);
            name_category(x, o, 0xff_0fd2);
            o.put32(x, NAME_CLOSE);
        } else {
            o.put(x, tok);
            o.n += 1;
        }
    }
}

/// Kind 9: the class 5 player (byte `+3` >> 1) gives its name in place of the token 0x7004; without this player,
/// the text is only a link to the other text.
fn build_9<R: Bench>(x: &mut Cx<'_, R>, o: &mut Out, a6: u32) {
    let mut found = None;
    for p in players() {
        if p.u8(x.b, pl::ACTIVE) == 0 || (p.u8(x.b, Player::MAIN) as i8) >= 4 {
            continue;
        }
        if p.u8(x.b, Player::CHARACTER) >> 1 == 5 {
            found = Some(p);
            break;
        }
    }
    let Some(player) = found else {
        let slot = x.linked(DialogObj::SLOT);
        let v = set_tables::long(x.b, a6);
        slot.set_u32(x.b, Obj::ACTIVE, v);
        return;
    };
    let mut a6 = set_tables::long(x.b, a6 + 4);
    loop {
        let tok = next(x, &mut a6);
        if tok == 0 {
            return o.end(x);
        }
        if (tok as i16) < 0 {
            command(x, o, &mut a6, tok);
        } else if tok == 0x7004 {
            name(x, o, player.0 + 0x118);
        } else {
            o.put(x, tok);
            o.n += 1;
        }
    }
}

/// Kind 16: tokens 0x7006 to 0x7009 = names of the owner `+0x46`, equipped objects...
fn build_16<R: Bench>(x: &mut Cx<'_, R>, o: &mut Out, mut a6: u32) {
    loop {
        let tok = next(x, &mut a6);
        if tok == 0 {
            // end: looks for a test piece (0xce00a, type 17) to link to this text
            wr16(x.b, o.at, tok);
            if let Ok(created) = crate::actors::families::creators::set_15_pure(x.b, 0x11) {
                created.set_u16(x.b, SetObj::OWNER, x.o.0 as u16);
                let v = x.u16(DialogObj::SETTING);
                created.set_u16(x.b, SetObj::ARG, v);
            } else {
                x.add8(SetObj::SUBSTATE, 0xfe);
            }
            return;
        }
        if (tok as i16) < 0 {
            command(x, o, &mut a6, tok);
            continue;
        }
        if (tok as i16) < 0x7006 {
            o.put(x, tok);
            o.n += 1;
            continue;
        }
        let entry = set_tables::pc_word(x.b, 0xd7988 + 2 * (tok - 0x7006) as u32) as u32;
        match 0xd7988 + entry {
            0xd7990 => owner_name(x, o),
            0xd799c => equipped_item(x, o),
            0xd7a72 => {
                if x.u16(DialogObj::OWNER) != 0 {
                    owner_name(x, o)
                }
            }
            _ => {}
        }
    }
}

/// Name of the window's owner (`+0x46`) with its openings (routine 0xd75ba).
fn owner_name<R: Bench>(x: &mut Cx<'_, R>, o: &mut Out) {
    let owner = x.linked(DialogObj::OWNER);
    o.put32(x, NAME_OPEN);
    name(x, o, owner.0 + 0x118);
}

/// First equipped object of the owner that appears in one of the lists of kind 16 (routines 0xd799c, 0xd7a38 and
/// 0xd7a5c): copies the object's text.
fn equipped_item<R: Bench>(x: &mut Cx<'_, R>, o: &mut Out) {
    let owner = x.linked(DialogObj::OWNER);
    let (a, b, c) = if owner.u8(x.b, Obj::VARIANT) >> 1 == 0 {
        (0xd7a20, 0xd7a20, 0xd7a26)
    } else {
        (0xd7a2c, 0xd7a2c, 0xd7a32)
    };
    let mut candidates = vec![(a, owner.u8(x.b, Player::WEAPON_A)), (b, owner.u8(x.b, Player::WEAPON_B))];
    for i in 0..6 {
        candidates.push((c, owner.u8(x.b, Player::INVENTORY + i)));
    }
    for (list, value) in candidates {
        let mut index = None;
        let mut at = list;
        let mut i = 0;
        loop {
            let e = set_tables::byte(x.b, at);
            at += 1;
            if (e as i8) < 0 {
                break;
            }
            if e == value {
                index = Some(i);
                break;
            }
            i += 1;
        }
        if let Some(i) = index {
            let text = set_tables::pc_word(x.b, 0xd7a48 + 4 * i);
            let mut text = (text as u32) << 16 | set_tables::pc_word(x.b, 0xd7a48 + 4 * i + 2) as u32;
            loop {
                let w = set_tables::word(x.b, text);
                if w == 0 {
                    return;
                }
                o.put(x, w);
                o.n += 1;
                text += 2;
            }
        }
    }
}

/// Kind 20: like kind 12 (token 0x7005), but preceded by a palette change in a one-player game; token 0x7000
/// skipped in solo or replaced by 0x2003.
fn build_20<R: Bench>(x: &mut Cx<'_, R>, o: &mut Out, mut a6: u32) {
    o.put32(x, 0xfffe_000c);
    if x.a5_u8(vars::EXIT_LOCK) != 0 && x.a5_u8(vars::FROZEN) != 0 {
        o.at -= 4;
        copy_palette(x, 0x292260, 0x91c260);
    }
    loop {
        let tok = next(x, &mut a6);
        if tok == 0 {
            return o.end(x);
        }
        if (tok as i16) < 0 {
            command(x, o, &mut a6, tok);
        } else if tok == 0x7000 {
            if x.a5_u8(vars::PLAYERS_ALIVE) != 1 {
                o.put(x, 0x2003);
                o.n += 1;
            }
        } else if tok == 0x7005 {
            name_category(x, o, 0xff_0fd2);
            // the rest of the text is read as for kind 12
            return text_with_name(x, o, a6, 0x7005, true);
        } else {
            o.put(x, tok);
            o.n += 1;
        }
    }
}

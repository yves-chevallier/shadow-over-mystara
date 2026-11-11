use super::*;
use crate::actors::families::set::tables as set_tables;
use crate::actors::families::set::vars;
use crate::world::objects::Obj;
use crate::actors::families::set::fields::SetObj;
use crate::actors::families::set_c::fields::ScreenObj;
use crate::actors::families::set_c::fields::CreditsObj;
use crate::actors::families::set_c::fields::FlickerObj;
use crate::actors::families::set_c::fields::NameShowObj;
use crate::actors::families::set_c::fields::ResultsObj;
use crate::actors::families::set_c::fields::Conductor;

// --- variant 20: credits scroll ---------------------------------------------------------------------------------

/// Variant 20 (0xd00c2): scrolling text (h: credits) written line by line into the tilemap
/// 0x910000 from the ROM text 0xd01de: one byte per cell (0xfe: end, 0xff n: attribute n,
/// 0x2f: end of page). The background (`A5+0x4ba`) scrolls by 0x4000/65536 pixel per frame.
pub fn credits_scroll<R: Bench>(x: &mut Cx<'_, R>) {
    const TEXT: u32 = 0xd01de;
    if x.u8(Obj::STATE) == 6 {
        return x.free(&pools::SET);
    }
    if x.u8(Obj::STATE) == 0 {
        next_state(x);
        crate::services::video::clear_layer(x.b, crate::services::video::Layer::One);
        x.set_a5_u8(vars::FADE_PAGES, 2);
        crate::services::palette::full_brightness(x.b);
        x.set32(CreditsObj::SCROLL, 0);
        x.set32(CreditsObj::TEXT, TEXT);
        x.set8(CreditsObj::COLUMN, 0);
        x.set8(CreditsObj::PHASE, 0);
        x.set16(CreditsObj::DEST, 0xf00);
        x.set8(CreditsObj::ATTR, 0);
    }
    let v = x.u32(CreditsObj::SCROLL).wrapping_sub(0x4000);
    x.set32(CreditsObj::SCROLL, v);
    let scroll = x.u16(CreditsObj::SCROLL) & 0x1ff;
    x.set16(CreditsObj::SCROLL, scroll);
    x.set_a5_u16(vars::SCROLL_Y_PLANE1, scroll);
    if x.u8(SetObj::SUBSTATE) != 0 {
        return;
    }
    let phase = (x.a5_u16(vars::SCROLL_Y_PLANE1) & 8) as u8;
    if x.u8(CreditsObj::PHASE) == phase {
        return;
    }
    let toggled = x.u8(CreditsObj::PHASE) ^ 8;
    x.set8(CreditsObj::PHASE, toggled);
    let mut text = x.u32(CreditsObj::TEXT);
    let mut dest = 0x910000u32.wrapping_add(adr16(x.u16(CreditsObj::DEST)));
    x.add16(CreditsObj::DEST, 4);
    x.add8(CreditsObj::COLUMN, 1);
    if x.u8(CreditsObj::COLUMN) == 0x20 {
        let v = x.u16(CreditsObj::DEST).wrapping_sub(0x80);
        x.set16(CreditsObj::DEST, v);
        x.set8(CreditsObj::COLUMN, 0);
        let v = x.u16(CreditsObj::DEST).wrapping_add(0x2000) & 0x3fff;
        x.set16(CreditsObj::DEST, v);
    }
    let mut attr = x.u8(CreditsObj::ATTR) as u16;
    // cell cleared at the same time: the one on the opposite page (0x2000 further), set once
    let erase = 0x910000u32 + ((dest as u16).wrapping_add(0x2000) & 0x3fff) as u32;
    loop {
        let c = set_tables::byte(x.b, text);
        text += 1;
        match c {
            0xfe => return next_substate(x),
            0xff => {
                let a = set_tables::byte(x.b, text);
                text += 1;
                x.set8(CreditsObj::ATTR, a);
                attr = a as u16;
            }
            0x2f => {
                x.set32(CreditsObj::TEXT, text);
                for i in 0..17 {
                    wr32(x.b, erase + 0x80 * i, 0x20_0000);
                }
                return;
            }
            _ => {
                wr16(x.b, dest, c as u16);
                wr16(x.b, dest + 2, attr);
                dest += 0x80;
            }
        }
    }
}

// --- variant 21: shimmer
// ---------------------------------------------------------------------------------------------

/// Variant 21 (0xd06e6): makes a palette color twinkle (0x91cfb2 takes 0xf00 from time
/// to time) and launches a SOBJ object of variant 0x14; withdraws when the conductor has `+8 < 0`.
pub fn flicker<R: Bench>(x: &mut Cx<'_, R>) {
    const COLOR: u32 = 0x91cfb2;
    const REFERENCE: u32 = 0x91cc00;
    match x.u8(Obj::STATE) {
        0 => {
            next_state(x);
            x.set8(FlickerObj::LIT, 0);
            let v = rd16(x.b, COLOR);
            x.set16(FlickerObj::COLOR, v);
        }
        2 => {}
        _ => return,
    }
    if (conductor(x).u8(x.b, Conductor::SIGNAL) as i8) < 0 {
        return x.set8(Obj::STATE, 6);
    }
    if x.u8(FlickerObj::LIT) != 0 {
        x.set8(FlickerObj::LIT, 0);
        let v = x.u16(FlickerObj::COLOR) & 0xfff | rd16(x.b, REFERENCE) & 0xf000;
        return wr16(x.b, COLOR, v);
    }
    if rng(x) & 7 != 0 {
        return;
    }
    x.set8(FlickerObj::LIT, 1);
    let v = 0xf00 | rd16(x.b, REFERENCE) & 0xf000;
    wr16(x.b, COLOR, v);
    let Some(o) = spawn_sobj(x, 0x14) else { return };
    let owner = x.u32(ScreenObj::CONDUCTOR);
    o.set_u32(x.b, ScreenObj::CONDUCTOR, owner);
}

// --- variant 22: entering initials (h) -----------------------------------------------------------

/// Variant 22 (0xd077c): goes through the players present, one after the other (`+0x20` = player,
/// `+0x22` = remaining players); for each one creates a SOBJ of variant 0x15 that displays their name
/// (list of tiles at `+0x118` of the player, written by 0xb230e on a background of 0x912000), waits 0x78
/// frames, removes the object and moves on to the next. Ends with 0xca8 and 0xcc8.
pub fn name_entry<R: Bench>(x: &mut Cx<'_, R>) {
    if x.u8(Obj::STATE) == 0 {
        next_state(x);
        x.set16(NameShowObj::PLAYER, PLAYERS as u16);
        x.set8(NameShowObj::LEFT, 4);
    } else if x.u8(Obj::STATE) != 2 {
        return;
    }
    match x.u8(SetObj::SUBSTATE) {
        0 => {
            let mut a1 = adr16(x.u16(NameShowObj::PLAYER));
            loop {
                if x.u8(NameShowObj::LEFT) == 0 {
                    next_state(x);
                    let v = x.a5_u8(vars::STEP).wrapping_add(2);
                    x.set_a5_u8(vars::STEP, v);
                    crate::services::video::clear_layer(x.b, crate::services::video::Layer::One);
                    return crate::services::video::clear_layer(x.b, crate::services::video::Layer::Two);
                }
                x.add8(NameShowObj::LEFT, 0xff);
                if rd8(x.b, a1) != 0 && (rd8(x.b, a1 + 4) as i8) < 4 {
                    next_substate(x);
                    x.set16(NameShowObj::PLAYER, a1 as u16);
                    mask_tasks(x, 0xfff1);
                    return;
                }
                a1 += PLAYER_STRIDE;
            }
        }
        2 => {
            let player = adr16(x.u16(NameShowObj::PLAYER));
            let Some(o) = spawn_sobj(x, 0x15) else { return };
            crate::services::video::clear_layer(x.b, crate::services::video::Layer::One);
            next_substate(x);
            x.set16(NameShowObj::CURSOR, o.0 as u16);
            let character = rd8(x.b, player + 3);
            o.set_u8(x.b, SetObj::PARAM, character);
            let dest = 0x912000u32.wrapping_add(set_tables::pc_word(x.b, 0xd0858 + 2 * character as u32) as i16 as i32 as u32);
            let kind = rd8(x.b, player + 2) as u32;
            let attr = set_tables::pc_byte(x.b, 0xd0854 + kind) as u16;
            write_name(x, player + 0x118, dest, attr);
            block_loader_for(x, character as u32);
        }
        4 => {
            next_substate(x);
            let v = x.a5_u16(TASK_FLAGS) | 0xe;
            x.set_a5_u16(TASK_FLAGS, v);
            x.set16(NameShowObj::DELAY, 0x78);
        }
        6 | 0xa => {}
        8 => {
            x.add16(NameShowObj::DELAY, 0xffff);
            if x.u16(NameShowObj::DELAY) == 0 {
                next_substate(x);
            }
        }
        0xc => {
            next_substate(x);
            let o = Obj(adr16(x.u16(NameShowObj::CURSOR)));
            o.set_u32(x.b, Obj::STATE, 0x0600_0000);
        }
        0xe => {
            next_substate(x);
            let next = adr16(x.u16(NameShowObj::PLAYER)) + PLAYER_STRIDE;
            x.set16(NameShowObj::PLAYER, next as u16);
            mask_tasks(x, 0xfff1);
            x.set16(NameShowObj::DELAY, 4);
        }
        0x10 => {
            x.add16(NameShowObj::DELAY, 0xffff);
            if x.u16(NameShowObj::DELAY) == 0 {
                x.set8(SetObj::SUBSTATE, 0);
            }
        }
        _ => {}
    }
}

/// Writes the name (subroutine 0xd0870): list of tile indices at `list` (end 0xff; beyond
/// 0xf8, a pattern of six consecutive tiles from a table), each tile drawn by 0xb230e at `at` (which moves on) with
/// attribute `attr`.
fn write_name<R: Bench>(x: &mut Cx<'_, R>, list: u32, mut at: u32, attr: u16) {
    const TILES: u32 = 0xb2920;
    const PATTERNS: u32 = 0xb2a24;
    let mut p = list;
    if set_tables::byte(x.b, p) >= 0xf9 {
        let code = (set_tables::byte(x.b, p) as i8 as i16).wrapping_neg().wrapping_sub(2) as u16;
        let mut tile = set_tables::word(x.b, PATTERNS.wrapping_add(code.wrapping_mul(2) as i16 as i32 as u32));
        for _ in 0..6 {
            // 0xb230e leaves D0 = tile + 0x11; the pattern advances by two (D0 - 0xf)
            at = crate::actors::families::effects_creators::tile_block(x.b, at, tile, attr).0;
            tile = tile.wrapping_add(2);
        }
        return;
    }
    loop {
        let c = set_tables::byte(x.b, p);
        p += 1;
        if c == 0xff {
            return;
        }
        let tile = set_tables::word(x.b, TILES + 2 * c as u32);
        at = crate::actors::families::effects_creators::tile_block(x.b, at, tile, attr).0;
    }
}

// --- variant 24: results table -----------------------------------------------------------

/// Variant 24 (0xd0a0c): prepares the results background (4x4 blocks at 0x91ba00, copy of palette
/// 0x91cc00 into the object, background at 0x200), then blinks the palette and from time to time launches a
/// SOBJ (variant 0x16) tied to the object. State 4 turns the blinking off.
pub fn results_panel<R: Bench>(x: &mut Cx<'_, R>) {
    match x.u8(Obj::STATE) {
        0 => {
            next_state(x);
            x.set_a5_u16(vars::TITLE_CARD, 4);
            let end = load_blocks(x, rom::LOAD_SMALL, 0x91ba00, 8);
            load_blocks(x, rom::LOAD_SMALL, end, 8);
            for i in 0..8 {
                let v = rd32(x.b, 0x91cc00 + 4 * i);
                x.set32(ResultsObj::PALETTE + 4 * i, v);
            }
            x.set8(ResultsObj::FADE, 0);
            x.set_a5_u16(a5::BG_X, 0x200);
            x.set_a5_u16(a5::BG_Y, 0);
            let (px, py) = (x.a5_u16(a5::BG_X), x.a5_u16(a5::BG_Y));
            x.set_a5_u16(a5::BG_X_PREV, px);
            x.set_a5_u16(a5::BG_Y_PREV, py);
            x.set8(ResultsObj::FADE, 1);
            let low = rd32(x.b, 0x91cc00) & 0xf000_f000;
            for i in 0..8 {
                wr32(x.b, 0x91cc00 + 4 * i, 0x0f00_0f00 | low);
            }
            results_tick(x)
        }
        2 => results_tick(x),
        4 => {
            next_state(x);
            fade_flag(x)
        }
        _ => {}
    }
}

fn fade_flag<R: Bench>(x: &mut Cx<'_, R>) {
    x.add8(ResultsObj::FADE, 0xff);
    mask_tasks(x, 0xfff7);
}

fn results_tick<R: Bench>(x: &mut Cx<'_, R>) {
    if x.u8(ResultsObj::FADE) != 0 {
        return fade_flag(x);
    }
    x.set8(ResultsObj::FADE, 2);
    if rng(x) & 7 != 0 {
        return;
    }
    let v = x.a5_u16(TASK_FLAGS) | 8;
    x.set_a5_u16(TASK_FLAGS, v);
    let Some(o) = spawn_sobj(x, 0x16) else { return };
    o.set_u16(x.b, SetObj::OWNER, x.o.0 as u16);
}

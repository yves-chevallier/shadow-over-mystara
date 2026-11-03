//! Character presentation of the attract demonstration (script operations other than 0 and 2): portrait,
//! frame and texts.

use super::*;
use crate::hw::video::{Cell, TEXT};
use crate::ram::Long;

/// Number of players of the cabinet, times two (`A5+0x72`), and the announcement banner (text address, x, y) that
/// the presentation sets for its SET objects.
const PLAYER_COUNT: Word = Word::a5(0x72);
const BANNER: Long = Long::a5(0x5c6);
const BANNER_X: Word = Word::a5(0x5ca);
const BANNER_Y: Word = Word::a5(0x5cc);
use crate::objects::Obj;
use crate::screens::obj;

// --- character presentation (operation other than 0 and 2) -----------------------------------

/// `0x1fbd6`: stores the three presentation parameters (`a1`: after the operation) and prepares the display.
pub(super) fn presentation(x: &mut Ctx, mut a1: u32) -> Flow {
    crate::tasks::prompt::stop(x.b);
    Obj(mode::BASE).set_u8(x.b, f::STEP, 6);
    Obj(mode::BASE).set_u16(x.b, f::TIMER, 0);
    let b = tables::byte(x.b, a1);
    Obj(mode::BASE).set_u8(x.b, f::PRESENT_9, b);
    let b = tables::byte(x.b, a1 + 1);
    Obj(mode::BASE).set_u8(x.b, f::PRESENT_KIND, b);
    let b = tables::byte(x.b, a1 + 2);
    Obj(mode::BASE).set_u8(x.b, f::PRESENT_CHARACTER, b);
    a1 += 3;
    Obj(mode::BASE).set_u32(x.b, f::SCRIPT, a1);
    sv::SCROLL_MODE.set(x.b, 0);
    crate::ram::CAMERA_ALT_ACTIVE.set(x.b, 0);
    sv::CAMERA_X.set(x.b, 0);
    sv::CAMERA_Y.set(x.b, 512);
    sv::CAMERA_X.set(x.b, 0);
    sv::CAMERA_2_Y.set(x.b, 512);
    Word(sv::SCROLL_1.0).set(x.b, 0);
    Word(sv::SCROLL_2.0).set(x.b, 0);
    let v = var::VIDEO_FLAGS_B.get(x.b) & !1;
    var::VIDEO_FLAGS_B.set(x.b, v);
    var::VIDEO.set(x.b, 0x12c0);
    crate::palette::load_palette_page(x.b, 2, 84);
    crate::palette::load_palette_page(x.b, 3, 84);
    fading::PAGES.set(x.b, 12);
    crate::palette::full_brightness(x.b);
    crate::tasks::wait_then_return(x, 1)
}

pub(super) fn presentation_step(x: &mut Ctx) -> Flow {
    let substep = rd_state(x, f::SUBSTEP);
    match substep {
        0 => p0_images(x),
        2 => p2_show(x),
        4 => p4_end(x),
        6 => p6_text(x),
        // table 0x1fc72 has four entries, all rewritten (code-mort.md, batch "CPU removal")
        _ => unreachable!("presentation: sub-step {substep:#x} outside table 0x1fc72"),
    }
}

/// `0x1fc68`: common end of each sub-state: terrain (SET, SOBJ) and sprites (`jmp 0x1de96`).
pub(super) fn p_render(x: &mut Ctx) -> Flow {
    crate::dispatch::SET.update_no_next(x.b);
    crate::dispatch::SOBJ.update_no_next(x.b);
    crate::sprites::build_in_game(x.b);
    Flow::Done
}

pub(super) fn next_substep(x: &mut Ctx) {
    let s = rd_state(x, f::SUBSTEP).wrapping_add(2);
    Obj(mode::BASE).set_u8(x.b, f::SUBSTEP, s);
}

/// Sub-state 0: loads the character's two tile blocks, creates the frame and portrait objects.
pub(super) fn p0_images(x: &mut Ctx) -> Flow {
    next_substep(x);
    let character = rd_state(x, f::PRESENT_CHARACTER) as u32 * 4;
    let (first, second) = (table::TILES.pc_word(x.b, character), table::TILES.pc_word(x.b, 2 + character));
    let end = crate::video::load_grid(x.b, crate::video::Grid::Layer2, first, crate::hw::video::SCROLL2.0 + 0x1000);
    crate::video::load_grid(x.b, crate::video::Grid::Layer2, second, end.destination);
    if rd_state(x, f::PRESENT_KIND) != 0 {
        crate::sound::service(x.b, crate::sound::Service::Ff07 { lock: 0, line: 0 });
        if let Ok(Obj(set)) = set_15_pure(x.b, 4) {
            let character = rd_state(x, f::PRESENT_CHARACTER);
            Obj(set).set_u8(x.b, obj::INDEX, character);
            Obj(mode::BASE).set_u16(x.b, f::TIMER, 30);
        }
        return p_render(x);
    }
    let Ok(Obj(set)) = set_15_pure(x.b, 4) else {
        return p_render(x);
    };
    let character = rd_state(x, f::PRESENT_CHARACTER);
    Obj(set).set_u8(x.b, obj::INDEX, character);
    portrait_object(x)
}

/// `0x1fce4`: portrait object; on failure or success the original *jumps* to `0x1fc68`.
pub(super) fn portrait_object(x: &mut Ctx) -> Flow {
    if let Ok(Obj(o)) = scenery_6(x.b, 15) {
        let character = rd_state(x, f::PRESENT_CHARACTER) >> 1;
        Obj(o).set_u8(x.b, obj::INDEX, character);
        Obj(mode::BASE).set_u16(x.b, f::LINK, o as u16);
    }
    p_render(x)
}

/// Sub-state 2: turns the planes on; next sub-state 6 (text) if the presentation has a text, else 4.
pub(super) fn p2_show(x: &mut Ctx) -> Flow {
    let v = var::VIDEO.get(x.b) | 0xe;
    var::VIDEO.set(x.b, v);
    let next = if rd_state(x, f::PRESENT_KIND) != 0 { 6 } else { 4 };
    Obj(mode::BASE).set_u8(x.b, f::SUBSTEP, next);
    p_render(x)
}

/// Sub-state 6: after the timer (word `+8`, counted down by the objects), displays the character's text.
pub(super) fn p6_text(x: &mut Ctx) -> Flow {
    let left = Obj(mode::BASE).u16(x.b, f::TIMER).wrapping_sub(1);
    Obj(mode::BASE).set_u16(x.b, f::TIMER, left);
    if left != 0 {
        return p_render(x);
    }
    Obj(mode::BASE).set_u8(x.b, f::SUBSTEP, 4);
    Obj(mode::BASE).set_u16(x.b, f::TIMER, 240);
    let entry = (rd_state(x, f::PRESENT_CHARACTER) as u32) << 3;
    // choice by number of players (A5+0x72: 2, 4, 6...) of a data address (plain)
    let players = PLAYER_COUNT.get(x.b).wrapping_sub(2).wrapping_mul(2) as u32;
    let mut a1 = table::PRESENT_DATA.long(x.b, entry + players);
    let (cx, cy) = (tables::word(x.b, a1), tables::word(x.b, a1 + 2));
    BANNER_X.set(x.b, cx);
    BANNER_Y.set(x.b, cy);
    a1 += 4;
    BANNER.set(x.b, a1);
    let offset = table::PRESENT_DATA.word(x.b, entry + 12) as i16 as i32 as u32;
    let src = table::PRESENT_PALETTES.addr().wrapping_add(offset);
    for i in 0..8 {
        let v = tables::long(x.b, src + 4 * i);
        palette(0, 0xf0 + 2 * i).set_long(x.b, v);
    }
    x.push_return(Step::PresentAgain);
    text_screen(x);
    portrait_object(x)
}

/// Sub-state 4: after the timer, closes the presentation and reads the next operation.
pub(super) fn p4_end(x: &mut Ctx) -> Flow {
    let left = Obj(mode::BASE).u16(x.b, f::TIMER).wrapping_sub(1);
    Obj(mode::BASE).set_u16(x.b, f::TIMER, left);
    if left != 0 {
        return p_render(x);
    }
    let link = Obj(mode::BASE).u16(x.b, f::LINK) as i16 as i32 as u32 & 0xff_ffff;
    Obj(link).set_u32(x.b, Obj::STATE, 0x0600_0000);
    Obj(mode::BASE).set_u8(x.b, f::STEP, 2);
    Obj(mode::BASE).set_u16(x.b, f::WORD_30, 300);
    crate::objects::lists::unlink_all(x.b);
    crate::sprites::clear_lists(x.b);
    for pool in pools::POOLS {
        pool.reset(x.b);
    }
    crate::video::clear_objects(x.b);
    x.wait(1, Step::P4Hide)
}

/// `0x1ffae`: frame and texts of the presentation, written into plane 1 (GFX RAM `0x912000`).
pub(super) fn text_screen(x: &mut Ctx) {
    let character = rd_state(x, f::PRESENT_CHARACTER) as u32;
    let frame = table::FRAME_ATTR.pc_word(x.b, character) as i16 as i32 as u32;
    let a4 = TEXT.at(frame).0;
    let attr = table::BOX_ATTR.pc_word(x.b, character);
    frame_box(x, a4, attr);
    text_lines(x, a4 + 128, table::TEXT_1.addr(), attr);
    let which = (character & 0xffff) * 2;
    let text = table::TEXT_2.pc_long(x.b, which);
    text_lines(x, a4 + 0x700, text, attr);
}

/// `0x1ffdc`: the frame (eight edge and corner tiles).
pub(super) fn frame_box(x: &mut Ctx, a4: u32, attr: u16) {
    let put = |x: &mut Ctx, a: &mut u32, tile: u16| {
        Cell(*a).put(x.b, tile, attr);
        *a += 4;
    };
    let mut a1 = a4;
    put(x, &mut a1, 0x145);
    for _ in 0..10 {
        put(x, &mut a1, 0x14b);
    }
    put(x, &mut a1, 0x147);
    a1 += 124;
    for _ in 0..16 {
        put(x, &mut a1, 0x14a);
        a1 += 124;
    }
    a1 = a4 + 128;
    for _ in 0..16 {
        put(x, &mut a1, 0x149);
        a1 += 124;
    }
    put(x, &mut a1, 0x146);
    for _ in 0..10 {
        put(x, &mut a1, 0x14c);
    }
    put(x, &mut a1, 0x148);
}

/// `0x20044`: writes a vertical text (one tile per line); `0x2f` moves to the next column,
/// `0x40` termine.
pub(super) fn text_lines(x: &mut Ctx, start: u32, text: u32, attr: u16) {
    let mut a3 = start;
    let mut a2 = text;
    let mut a1 = a3 + 4;
    a3 = a1;
    loop {
        let c = tables::byte(x.b, a2);
        a2 += 1;
        match c {
            0x2f => {
                a1 = a3 + 4;
                a3 = a1;
            }
            0x40 => return,
            _ => {
                Cell(a1).put(x.b, c as u16, attr);
                a1 += 4 + 124;
            }
        }
    }
}

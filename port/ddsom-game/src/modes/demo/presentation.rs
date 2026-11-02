//! Character presentation of the attract demonstration (script operations other than 0 and 2): portrait,
//! frame and texts.

use super::*;

// --- character presentation (operation other than 0 and 2) -----------------------------------

/// `0x1fbd6`: stores the three presentation parameters (`a1`: after the operation) and prepares the display.
pub(super) fn presentation(x: &mut Ctx, mut a1: u32) -> Flow {
    crate::tasks::prompt::stop(x.b);
    wr8(x.b, mode::BASE + f::STEP, 6);
    wr16(x.b, mode::BASE + f::TIMER, 0);
    let b = rd8(x.b, a1);
    wr8(x.b, mode::BASE + f::PRESENT_9, b);
    let b = rd8(x.b, a1 + 1);
    wr8(x.b, mode::BASE + f::PRESENT_KIND, b);
    let b = rd8(x.b, a1 + 2);
    wr8(x.b, mode::BASE + f::PRESENT_CHARACTER, b);
    a1 += 3;
    wr32(x.b, mode::BASE + f::SCRIPT, a1);
    wr8(x.b, A5 + 0x53c, 0);
    wr8(x.b, A5 + 0x36, 0);
    wr16(x.b, A5 + 0x506, 0);
    wr16(x.b, A5 + 0x50a, 512);
    wr16(x.b, A5 + 0x506, 0);
    wr16(x.b, A5 + 0x55a, 512);
    wr16(x.b, A5 + 0x4b6, 0);
    wr16(x.b, A5 + 0x4ba, 0);
    let v = rd8(x.b, var::VIDEO_FLAGS_B) & !1;
    wr8(x.b, var::VIDEO_FLAGS_B, v);
    wr16(x.b, var::VIDEO, 0x12c0);
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
    wr8(x.b, mode::BASE + f::SUBSTEP, s);
}

/// Sub-state 0: loads the character's two tile blocks, creates the frame and portrait objects.
pub(super) fn p0_images(x: &mut Ctx) -> Flow {
    next_substep(x);
    let character = rd_state(x, f::PRESENT_CHARACTER) as u32 * 4;
    let (first, second) = (x.prog16(table::TILES + character), x.prog16(table::TILES + 2 + character));
    let end = crate::video::load_grid(x.b, crate::video::Grid::Layer2, first, 0x915000);
    crate::video::load_grid(x.b, crate::video::Grid::Layer2, second, end.destination);
    if rd_state(x, f::PRESENT_KIND) != 0 {
        crate::sound::service(x.b, crate::sound::Service::Ff07 { lock: 0, line: 0 });
        if let Ok(Obj(set)) = set_15_pure(x.b, 4) {
            let character = rd_state(x, f::PRESENT_CHARACTER);
            wr8(x.b, set + 0x2f, character);
            wr16(x.b, mode::BASE + f::TIMER, 30);
        }
        return p_render(x);
    }
    let Ok(Obj(set)) = set_15_pure(x.b, 4) else {
        return p_render(x);
    };
    let character = rd_state(x, f::PRESENT_CHARACTER);
    wr8(x.b, set + 0x2f, character);
    portrait_object(x)
}

/// `0x1fce4`: portrait object; on failure or success the original *jumps* to `0x1fc68`.
pub(super) fn portrait_object(x: &mut Ctx) -> Flow {
    if let Ok(Obj(o)) = scenery_6(x.b, 15) {
        let character = rd_state(x, f::PRESENT_CHARACTER) >> 1;
        wr8(x.b, o + 0x2f, character);
        wr16(x.b, mode::BASE + f::LINK, o as u16);
    }
    p_render(x)
}

/// Sub-state 2: turns the planes on; next sub-state 6 (text) if the presentation has a text, else 4.
pub(super) fn p2_show(x: &mut Ctx) -> Flow {
    let v = rd16(x.b, var::VIDEO) | 0xe;
    wr16(x.b, var::VIDEO, v);
    let next = if rd_state(x, f::PRESENT_KIND) != 0 { 6 } else { 4 };
    wr8(x.b, mode::BASE + f::SUBSTEP, next);
    p_render(x)
}

/// Sub-state 6: after the timer (word `+8`, counted down by the objects), displays the character's text.
pub(super) fn p6_text(x: &mut Ctx) -> Flow {
    let left = rd16(x.b, mode::BASE + f::TIMER).wrapping_sub(1);
    wr16(x.b, mode::BASE + f::TIMER, left);
    if left != 0 {
        return p_render(x);
    }
    wr8(x.b, mode::BASE + f::SUBSTEP, 4);
    wr16(x.b, mode::BASE + f::TIMER, 240);
    let entry = table::PRESENT_DATA + ((rd_state(x, f::PRESENT_CHARACTER) as u32) << 3);
    // choice by number of players (A5+0x72: 2, 4, 6...) of a data address (plain)
    let players = rd16(x.b, A5 + 0x72).wrapping_sub(2).wrapping_mul(2) as u32;
    let mut a1 = rd32(x.b, entry + players);
    let a4 = entry + 12;
    let (cx, cy) = (rd16(x.b, a1), rd16(x.b, a1 + 2));
    wr16(x.b, A5 + 0x5ca, cx);
    wr16(x.b, A5 + 0x5cc, cy);
    a1 += 4;
    wr32(x.b, A5 + 0x5c6, a1);
    let offset = rd16(x.b, a4) as i16 as i32 as u32;
    let src = 0x1fe3e_u32.wrapping_add(offset);
    for i in 0..8 {
        let v = rd32(x.b, src + 4 * i);
        wr32(x.b, 0x91c1e0 + 4 * i, v);
    }
    x.push_return(Step::PresentAgain);
    text_screen(x);
    portrait_object(x)
}

/// Sub-state 4: after the timer, closes the presentation and reads the next operation.
pub(super) fn p4_end(x: &mut Ctx) -> Flow {
    let left = rd16(x.b, mode::BASE + f::TIMER).wrapping_sub(1);
    wr16(x.b, mode::BASE + f::TIMER, left);
    if left != 0 {
        return p_render(x);
    }
    let link = rd16(x.b, mode::BASE + f::LINK) as i16 as i32 as u32 & 0xff_ffff;
    wr32(x.b, link + 4, 0x0600_0000);
    wr8(x.b, mode::BASE + f::STEP, 2);
    wr16(x.b, mode::BASE + 0x30, 300);
    crate::objects::lists::unlink_all(x.b);
    crate::sprites::clear_lists(x.b);
    for pool in pools::POOLS {
        pool.reset(x.b);
    }
    crate::video::clear_objects(x.b);
    x.wait(1, Step::P4Hide)
}
pub(super) const P4_HIDE: u32 = 0x1ff5e;
pub(super) const P4_CLEAR: u32 = 0x1ff6c;

/// `0x1ffae`: frame and texts of the presentation, written into plane 1 (GFX RAM `0x912000`).
pub(super) fn text_screen(x: &mut Ctx) {
    let character = rd_state(x, f::PRESENT_CHARACTER) as u32;
    let frame = x.prog16(table::FRAME_ATTR + character) as i16 as i32 as u32;
    let a4 = 0x912000u32.wrapping_add(frame);
    let attr = x.prog16(table::BOX_ATTR + character);
    frame_box(x, a4, attr);
    text_lines(x, a4 + 128, table::TEXT_1, attr);
    let which = (character & 0xffff) * 2;
    let text = prog32(x, table::TEXT_2 + which);
    text_lines(x, a4 + 0x700, text, attr);
}

/// `0x1ffdc`: the frame (eight edge and corner tiles).
pub(super) fn frame_box(x: &mut Ctx, a4: u32, attr: u16) {
    let put = |x: &mut Ctx, a: &mut u32, tile: u16| {
        wr16(x.b, *a, tile);
        wr16(x.b, *a + 2, attr);
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
        let c = rd8(x.b, a2);
        a2 += 1;
        match c {
            0x2f => {
                a1 = a3 + 4;
                a3 = a1;
            }
            0x40 => return,
            _ => {
                wr16(x.b, a1, c as u16);
                wr16(x.b, a1 + 2, attr);
                a1 += 4 + 124;
            }
        }
    }
}

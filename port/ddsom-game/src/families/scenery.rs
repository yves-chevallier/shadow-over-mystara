//! Scenery routines still shared by several handlers (zone gates, scene passages, purses, shops...), written for
//! generic handlers (`Cx<'_, R: Bench>`): cameras, scene palettes, players placed again. Each one leaves in D2-D5
//! what the original routine leaves there, since the next handlers of the dispatch loop may read it back
//! (`dispatch::Leftovers`).

use crate::families::cx::{Bench, Cx};
use crate::mem::{rd32, rd8, wr16, wr32, wr8, A5};
use crate::objects::pools;
use crate::objects::Obj;

/// Sets the four leftovers D2-D5.
fn set_leftovers<R: Bench>(x: &mut Cx<'_, R>, [d2, d3, d4, d5]: [u32; 4]) {
    (*x.c.d2(), *x.c.d3(), *x.c.d4(), *x.c.d5()) = (d2, d3, d4, d5);
}

/// Copies `blocks` blocks of eight longs (`movem.l` through D0-D5, A3, A4) from `from` (step 0x20) to `to` (step
/// `step`); returns the last block, whose longs 2 to 5 stay in D2-D5.
fn movem_blocks(b: &mut cps2::Cps2Bus, from: u32, to: u32, blocks: u32, step: u32) -> [u32; 8] {
    let mut last = [0; 8];
    for k in 0..blocks {
        last = std::array::from_fn(|i| rd32(b, from + 0x20 * k + 4 * i as u32));
        for (i, v) in last.iter().enumerate() {
            wr32(b, to + step * k + 4 * i as u32, *v);
        }
    }
    last
}

/// 0x1a608: palette page 1 from entry 0 of its table.
pub(crate) fn palette_page_1<R: Bench>(x: &mut Cx<'_, R>) {
    crate::palette::load_palette_page(x.b, 1, 0);
    let leftovers = crate::palette::page_leftovers(x.b, 1);
    set_leftovers(x, leftovers);
}

/// 0x1ad2a: palette page 1 (0x91c400, 1 KB) saved at FF1190.
pub(crate) fn save_palette_page_1<R: Bench>(x: &mut Cx<'_, R>) {
    let last = movem_blocks(x.b, 0x91c400, 0xff1190, 32, 0x20);
    set_leftovers(x, [last[2], last[3], last[4], last[5]]);
}

/// 0x1ad6a: 32 blocks of 32 bytes read from 0x285280 are stored in turn at the same place, FF1190 (the original
/// does not move its destination: only the last block stays there), then eight colours of 0x1ad4a go to FF1390.
pub(crate) fn restore_palette_page<R: Bench>(x: &mut Cx<'_, R>) {
    movem_blocks(x.b, 0x285280, 0xff1190, 32, 0);
    let last = movem_blocks(x.b, 0x1ad4a, 0xff1390, 1, 0);
    set_leftovers(x, [last[2], last[3], last[4], last[5]]);
}

/// The cameras of a scenery resumption.
#[derive(Clone, Copy, PartialEq, Eq)]
pub(crate) enum Cameras {
    /// 0x33aa6: the scene's start positions.
    Start,
    /// 0x33b24: back from an annex zone.
    Return,
}

/// The scenery of the scene is resumed: cameras, scene script reset (0xf2e56), palette pages 0, 2 and 3 of the scene
/// (0x1a360, 0x1a678, 0x1a8ac). The last page loader leaves D2-D5.
pub(crate) fn resume_scenery<R: Bench>(x: &mut Cx<'_, R>, cameras: Cameras) {
    match cameras {
        Cameras::Start => crate::scroll::cameras_start(x.b),
        Cameras::Return => crate::scroll::cameras_return(x.b),
    }
    crate::scroll::update_cameras(x.b);
    crate::scene::script::reset(x.b);
    crate::palette::load_scene_palettes(x.b);
    crate::palette::load_scene_page(x.b, 2);
    crate::palette::load_scene_page(x.b, 3);
    let leftovers = crate::palette::page_leftovers(x.b, 3);
    set_leftovers(x, leftovers);
}

/// `resume_scenery`, then full brightness (0x1686): the common start of 0xcc0ca, of the annex zone gates (SET 7) and
/// of the passage (EFFCT 14).
pub(crate) fn resume_scenery_lit<R: Bench>(x: &mut Cx<'_, R>, cameras: Cameras) {
    resume_scenery(x, cameras);
    wr8(x.b, A5 + 0x48e, 0xf);
    crate::palette::full_brightness(x.b);
}

/// 0x20de6: every player in game placed in front of the camera. Original glitch: the rank of the last one stays in
/// D2.
pub(crate) fn place_players<R: Bench>(x: &mut Cx<'_, R>) {
    let r = crate::scene::entry::place_players_leftovers(x.b);
    *x.c.d2() = r.rank as u32;
}

/// 0xcc0ca: the scene starts again (scenery resumed); each living player goes back to normal play (hidden and in a
/// scripted entry if it was down or dead), is placed again (0x20de6) and sorted into the actors' list.
pub(crate) fn restart_scene<R: Bench>(x: &mut Cx<'_, R>) {
    resume_scenery_lit(x, Cameras::Start);
    wr16(x.b, A5 + 0x52, 0);
    let players = || (0..4).map(|k| Obj(A5 + 0x5de + 0x200 * k));
    let alive = |b: &mut cps2::Cps2Bus, p: Obj| p.u8(b, 0) != 0 && p.u16(b, 0x62) != 0;
    for p in players() {
        if !alive(x.b, p) {
            continue;
        }
        p.set_u8(x.b, 0, 1);
        let state = rd8(x.b, p.0 + 4);
        p.set_u32(x.b, 4, 0);
        let flags = p.u8(x.b, 1) & !2;
        p.set_u8(x.b, 1, flags);
        for off in [0x66, 0x15c, 0x8a, 0x10f] {
            p.set_u8(x.b, off, 0);
        }
        if state as i8 > 2 {
            p.set_u8(x.b, 1, flags | 2);
            p.set_u16(x.b, 4, 0x402);
        }
    }
    place_players(x);
    for p in players() {
        if alive(x.b, p) {
            // Original glitch: the reinsertion leaves the left link in D2 (`dispatch::Leftovers`)
            crate::objects::lists::ACTORS.reinsert_leftovers(x.b, p, x.c.d2());
        }
    }
}

/// 0xa66c0: `count + 1` ESHL shots of type 0x3e whose variant goes down from `count` (byte of table 0xa66e2 at row
/// `row`, the D2 the caller leaves) to 0. Original glitch: the `dbf` counter stays in D5 (0xffff, or the variant the
/// full pool refused).
pub(crate) fn eshl_volley<R: Bench>(x: &mut Cx<'_, R>, row: u32) {
    let at = 0xa66e2u32.wrapping_add(row as i16 as i32 as u32);
    let mut d5 = crate::mem::prog8(x.b, at) as u32;
    loop {
        let Some(o) = pools::ESHL.alloc(x.b) else { break };
        o.set_u8(x.b, 0x0, 1);
        o.set_u8(x.b, 0x2, 0x3e);
        o.set_u8(x.b, 0x3, d5 as u8);
        let n = (d5 as u16).wrapping_sub(1);
        d5 = d5 & 0xffff_0000 | n as u32;
        if n == 0xffff {
            break;
        }
    }
    *x.c.d5() = d5;
}

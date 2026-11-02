//! Game-over screen (sub-mode 20, `0xfc576`): the ranking scrolls, then the players'
//! scores are displayed. The mode machine arrives here after phase 20 (`0x1c80c`), with the
//! startup flag raised (`0x133f6`); it lowers it on leaving (`0x133f0`).
//!
//! The screen has its own state block, at `FF2B10`:
//!
//! | Field | Role |
//! |---|---|
//! | `+0` | step (of 2): 0 preparation, 2 ranking scroll, 4 scores, 6 and 8 terrain, 10 exit |
//! | `+4` | timer (word) |
//! | `+6` | pointer to the ranking text (long, in ROM) |
//! | `+10` | camera y at the last added line (word) |
//!
//! The vertical scroll is the long `FF2B1C` (16.16); a pressed button slows it down.

use super::{Flow, Sequence};
use crate::families::effects_creators::{end_screen, or_rest};
use crate::mem::{rd16, rd32, rd8, wr16, wr32, wr8, A5};
use crate::objects::pools;
use crate::scene::Ctx;
use crate::video::{clear_layer, Layer};
use crate::Native;

const STATE: u32 = 0xff_2b10;
mod f {
    pub const STEP: u32 = 0;
    pub const TIMER: u32 = 4;
    pub const TEXT: u32 = 6;
    pub const MARK: u32 = 0xa;
}
/// Background scroll (long, 16.16) and second plane.
const SCROLL: u32 = 0xff_2b1c;
const SCROLL_2: u32 = 0xff_2b20;
/// Table of the four players (16 bytes each: score and state).
const SCORES: u32 = A5 - 0x5b18;
const FRAME_COUNT: u32 = A5 + 0x7e68;
const VIDEO: u32 = A5 + 0x14;
const CPS_REG_4: u32 = 0x400004;


/// Return from the step at `0xfc584`: A0 goes back to the mode machine block.
const RETURN: u32 = 0xfc584;

/// Screen of sub-mode `sub`, if it is rewritten here.
pub fn screen(x: &mut Ctx, sub: u8) -> Option<Flow> {
    (sub == 0x14).then(|| post_game(x))
}

fn post_game(x: &mut Ctx) -> Flow {
    let step = rd8(x.b, STATE + f::STEP);
    x.push_return(Step::Return);
    match step {
        0 => s0_prepare(x),
        2 => s2_scroll(x),
        4 => s4_scores(x),
        6 => s6_scenery(x),
        8 => s8_scenery(x),
        10 => sa_leave(x),
        _ => Flow::Done,
    }
}

crate::steps! {
    /// Steps (resume points) of the state machine.
    pub enum Step {
        Return = RETURN => |_| Flow::Done,
        S0Video = S0_VIDEO => |x| {
            wr16(x.b, CPS_REG_4, 0x1355);
            wr16(x.b, VIDEO, 0x24c0);
            x.wait(1, Step::S0Palettes)
        },
        S0Palettes = S0_PALETTES => |x| {
            clear_layers(x);
            crate::palette::load_scene_page(x.b, 1);
            crate::palette::load_palette_page(x.b, 2, 83);
            crate::palette::load_palette_page(x.b, 2, 83);
            wr8(x.b, A5 + 0x34, 0);
            crate::palette::load_scene_palettes_of(x.b, 89);
            // four palette blocks, from ROM to the palette table
            for (dst, src) in [(0x91c3e0u32, 0x291f00u32), (0x91c2e0, 0xfc6d8), (0x91c000, 0x292260), (0x91c3c0, 0x2537c0)] {
                for i in 0..8 {
                    let w = rd32(x.b, src + 4 * i);
                    wr32(x.b, dst + 4 * i, w);
                }
            }
            wr8(x.b, A5 + 0x48e, 0xf);
            crate::palette::full_brightness(x.b);
            for off in [0x506u32, 0x50a, 0x556, 0x4b6, 0x4ba] {
                wr16(x.b, A5 + off, 0);
            }
            wr16(x.b, A5 + 0x55a, 0x701);
            wr32(x.b, SCROLL, 0x0701_0000);
            wr8(x.b, A5 + 0x36, 1);
            x.wait(4, Step::S0Tiles)
        },
        S0Tiles = S0_TILES => |x| {
            let end = crate::video::load_grid(x.b, crate::video::Grid::Layer2, 197, 0x917000);
            crate::video::load_grid(x.b, crate::video::Grid::Layer2, 198, end.destination);
            wr16(x.b, VIDEO, 0x24ce);
            wr16(x.b, STATE + f::MARK, 0x728);
            wr32(x.b, STATE + f::TEXT, 0xfc938);
            Flow::Done
        },
        SaFaded = SA_FADED => |x| {
            crate::sound::service(x.b, crate::sound::Service::GeneralStop);
            reset_objects(x);
            x.wait(1, Step::SaVideo)
        },
        SaVideo = SA_VIDEO => |x| {
            wr16(x.b, VIDEO, 0x12c0);
            x.wait(4, Step::SaCleared)
        },
        SaCleared = SA_CLEARED => |x| {
            clear_layers(x);
            wr8(x.b, super::mode::SUB, 0);
            wr16(x.b, super::mode::STEP, 0);
            wr8(x.b, A5 + 0x36, 0);
            for off in [0x4bau32, 0x50a, 0x55a] {
                wr16(x.b, A5 + off, 0);
            }
            crate::families::small::startup_done(x.b, false);
            crate::families::items::clear_picked_up(x.b);
            wr8(x.b, A5 + 0x30, 0);
            Flow::Done
        },
    }
}

fn next_step(x: &mut Ctx) {
    let s = rd8(x.b, STATE).wrapping_add(2);
    wr8(x.b, STATE, s);
}

/// The three cleared layers.
fn clear_layers(x: &mut Ctx) {
    for layer in [Layer::One, Layer::Two, Layer::Three] {
        clear_layer(x.b, layer);
    }
}

/// Lists undone and emptied, objects cleared, pools reset (0x1f184, 0x1de7c, 0xc58, 0x2062).
fn reset_objects(x: &mut Ctx) {
    crate::objects::lists::unlink_all(x.b);
    crate::sprites::clear_lists(x.b);
    crate::video::clear_objects(x.b);
    for pool in pools::POOLS {
        pool.reset(x.b);
    }
}

/// One render: PEFF, EFFCT, sprites (`0xfc91c`).
fn render(x: &mut Ctx) {
    for class in [&crate::dispatch::PEFF, &crate::dispatch::EFFCT] {
        class.update_no_next(x.b);
    }
    crate::sprites::build_in_game(x.b);
}

/// Scroll speed and pressed buttons (D0 and D1.b of the original).
struct Speed {
    speed: u32,
    top_buttons: u8,
}

/// `0xfc6f8`: scroll speed (1 pixel per frame, 1/4 if a button is pressed).
fn speed(x: &mut Ctx) -> Speed {
    let mut buttons = 0;
    for k in 0..4 {
        buttons |= rd8(x.b, A5 + k);
    }
    let top_buttons = buttons & 0xf0;
    Speed { speed: if top_buttons == 0 { 0x10000 } else { 0x4000 }, top_buttons }
}

/// Advances the background scroll by one frame; camera 2 follows it.
fn scroll(x: &mut Ctx) -> Speed {
    let v = speed(x);
    let s = rd32(x.b, SCROLL).wrapping_sub(v.speed);
    wr32(x.b, SCROLL, s);
    wr16(x.b, A5 + 0x55a, (s >> 16) as u16);
    v
}

fn s0_prepare(x: &mut Ctx) -> Flow {
    next_step(x);
    wr16(x.b, STATE + f::TIMER, 300);
    crate::sound::play_global(x.b, crate::sound::CMD_27_B);
    reset_objects(x);
    x.wait(1, Step::S0Video)
}
const S0_VIDEO: u32 = 0xfc5cc;
const S0_PALETTES: u32 = 0xfc5e2;
const S0_TILES: u32 = 0xfc6a8;

/// Step 2: the ranking scrolls; every 40 pixel lines, the next text lines
/// are added. When the text ends (negative byte), next step.
fn s2_scroll(x: &mut Ctx) -> Flow {
    render(x);
    scroll(x);
    let d0 = rd16(x.b, STATE + f::MARK).wrapping_sub(rd16(x.b, A5 + 0x55a));
    if d0 as u8 != 40 {
        return Flow::Done;
    }
    let camera = rd16(x.b, A5 + 0x55a);
    wr16(x.b, STATE + f::MARK, camera);
    let mut a4 = rd32(x.b, STATE + f::TEXT);
    for counter in (0..6u8).rev() {
        if rd8(x.b, a4) & 0x80 != 0 {
            next_step(x);
            return Flow::Done;
        }
        // 0x517ba: a ranking line (variant = counter), its setting is the next byte of the text, which
        // only advances if the effect is created
        let setting = rd8(x.b, a4);
        if crate::effects_aids::effect_44(x.b, counter, setting).is_some() {
            a4 += 1;
        }
    }
    if rd8(x.b, a4) & 0x80 != 0 {
        next_step(x);
        return Flow::Done;
    }
    wr32(x.b, STATE + f::TEXT, a4);
    Flow::Done
}

/// Step 4: the score cartridges (one object per player who played), after ten game frames.
fn s4_scores(x: &mut Ctx) -> Flow {
    if (rd16(x.b, FRAME_COUNT) as i16) < 10 {
        return Flow::Done;
    }
    render(x);
    let v = scroll(x);
    next_step(x);
    // the first two cartridges receive the speed's D1.b as setting (the pressed buttons)
    for kind in [4u8, 6] {
        end_screen(x.b, kind, v.top_buttons);
    }
    // Original glitch: the carry is not tested, a failure writes into the A1 left by the allocation
    let mut rank = 3u8;
    let mut a4 = SCORES;
    for counter in (0..4u8).rev() {
        if rd32(x.b, a4) != 0 || rd32(x.b, a4 + 4) != 0 {
            let o = end_screen(x.b, 5, counter);
            let o = or_rest(x.b, o);
            wr8(x.b, o.0 + 0x42, rank);
            rank = rank.wrapping_sub(1);
            wr32(x.b, o.0 + 0x4c, a4);
        }
        a4 += 0x10;
    }
    let mut rank = 3u8;
    a4 = SCORES;
    for counter in (0..4u8).rev() {
        if rd8(x.b, a4) != 0 {
            if rd8(x.b, a4 + 14) != 0 {
                let o = end_screen(x.b, 9, counter);
                let o = or_rest(x.b, o);
                wr8(x.b, o.0 + 0x42, rank);
                wr32(x.b, o.0 + 0x4c, a4);
            }
            rank = rank.wrapping_sub(1);
        }
        a4 += 0x10;
    }
    Flow::Done
}

/// Step 6: the background descends to y 1064, then a second plane starts scrolling.
fn s6_scenery(x: &mut Ctx) -> Flow {
    render(x);
    scroll(x);
    if rd16(x.b, A5 + 0x55a) != 1064 {
        return Flow::Done;
    }
    next_step(x);
    wr32(x.b, SCROLL_2, 0x0108_0000);
    wr16(x.b, A5 + 0x4ba, 264);
    Flow::Done
}

/// Step 8: the two planes scroll together until y 832.
fn s8_scenery(x: &mut Ctx) -> Flow {
    render(x);
    let speed = scroll(x).speed;
    let v = rd32(x.b, SCROLL_2).wrapping_sub(speed);
    wr32(x.b, SCROLL_2, v);
    wr16(x.b, A5 + 0x4ba, (v >> 16) as u16);
    if rd16(x.b, A5 + 0x55a) != 832 {
        return Flow::Done;
    }
    next_step(x);
    wr16(x.b, STATE + f::TIMER, 300);
    Flow::Done
}

/// Step 10: after 300 frames (or a button), fade to black and back to attract.
fn sa_leave(x: &mut Ctx) -> Flow {
    let mut buttons = 0;
    for k in 0..4 {
        buttons |= rd8(x.b, A5 + 0xc + k);
    }
    if buttons & 0xf0 != 0 {
        wr16(x.b, STATE + f::TIMER, 1);
    }
    let left = rd16(x.b, STATE + f::TIMER).wrapping_sub(1);
    wr16(x.b, STATE + f::TIMER, left);
    if left != 0 {
        render(x);
        return Flow::Done;
    }
    crate::sound::service(x.b, crate::sound::Service::Ff07 { lock: 0, line: 4 });
    wr8(x.b, A5 + 0x48e, 0xf);
    wr16(x.b, A5 + 0x490, 3);
    x.subroutine(crate::tasks::fading::descent, Step::SaFaded)
}
const SA_FADED: u32 = 0xfc8a8;
const SA_VIDEO: u32 = 0xfc8ce;
const SA_CLEARED: u32 = 0xfc8dc;

pub const NATIVES: &[Native] = &[];

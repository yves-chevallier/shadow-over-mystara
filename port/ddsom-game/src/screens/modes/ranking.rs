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
use crate::actors::families::effects_creators::{end_screen, or_rest};
use crate::hw::video::{palette, regs, Vram, SCROLL2};
use crate::screens::scene::fading;
use crate::screens::tables::{self, Rom};
use crate::screens::vars as sv;
use crate::world::objects::pools;
use crate::screens::scene::Ctx;
use crate::services::video::{clear_layer, Layer};
use crate::Native;
use crate::world::ram::{Long, Word};
use crate::world::objects::Obj;
use crate::screens::obj;

pub(super) const STATE: u32 = 0xff_2b10;
mod f {
    pub const STEP: u32 = 0;
    pub const TIMER: u32 = 4;
    pub const TEXT: u32 = 6;
    pub const MARK: u32 = 0xa;
}
/// Background scroll (long, 16.16) and second plane.
const SCROLL: Long = Long(0xff_2b1c);
const SCROLL_2: Long = Long(0xff_2b20);
/// Table of the four players (16 bytes each: score and state; byte `+14`: the player named an entry, h).
const SCORES: u32 = 0xff_24e8;
const NAMED: u32 = 14;
/// Entry of a player of the score table: played (byte `+0`), score (two longs, `+0`).
const PLAYED: u32 = 0;
const SCORE: u32 = 0;
const FRAME_COUNT: Word = Word::a5(0x7e68);
const VIDEO: Word = Word::a5(0x14);
/// Rank of a score cartridge (`+0x42`).
const RANK: u32 = 0x42;
/// Ranking text: one setting byte per line, ended by a negative byte.
const RANKING_TEXT: Rom = Rom::at(0xfc938);
/// Palettes of the screen: library palettes and the ranking palette (`0xfc6d8`), and where they go.
const PALETTES: [(Vram, Rom); 4] = [
    (palette(0, 0x1f0), Rom::at(0x291f00)),
    (palette(0, 0x170), Rom::at(0xfc6d8)),
    (palette(0, 0), Rom::at(0x292260)),
    (palette(0, 0x1e0), Rom::at(0x2537c0)),
];

/// Screen of sub-mode `sub`, if it is rewritten here.
pub fn screen(x: &mut Ctx, sub: u8) -> Option<Flow> {
    (sub == 0x14).then(|| post_game(x))
}

fn post_game(x: &mut Ctx) -> Flow {
    let step = Obj(STATE).u8(x.b, f::STEP);
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
        Return = 0xfc584 => |_| Flow::Done,
        S0Video = 0xfc5cc => |x| {
            regs::OBJECT_OUTPUT.set(x.b, 0x1355);
            VIDEO.set(x.b, 0x24c0);
            x.wait(1, Step::S0Palettes)
        },
        S0Palettes = 0xfc5e2 => |x| {
            clear_layers(x);
            crate::services::palette::load_scene_page(x.b, 1);
            crate::services::palette::load_palette_page(x.b, 2, 83);
            crate::services::palette::load_palette_page(x.b, 2, 83);
            crate::world::ram::SCENE.set(x.b, 0);
            crate::services::palette::load_scene_palettes_of(x.b, 89);
            // four palette blocks, from ROM to the palette table
            for (dst, src) in PALETTES {
                for i in 0..8 {
                    let w = src.long(x.b, 4 * i);
                    dst.offset(4 * i).set_long(x.b, w);
                }
            }
            fading::PAGES.set(x.b, 0xf);
            crate::services::palette::full_brightness(x.b);
            for v in [sv::CAMERA_X, sv::CAMERA_Y, sv::CAMERA_2_X, Word(sv::SCROLL_1.0), Word(sv::SCROLL_2.0)] {
                v.set(x.b, 0);
            }
            sv::CAMERA_2_Y.set(x.b, 0x701);
            SCROLL.set(x.b, 0x0701_0000);
            crate::world::ram::CAMERA_ALT_ACTIVE.set(x.b, 1);
            x.wait(4, Step::S0Tiles)
        },
        S0Tiles = 0xfc6a8 => |x| {
            let end = crate::services::video::load_grid(x.b, crate::services::video::Grid::Layer2, 197, SCROLL2.0 + 0x3000);
            crate::services::video::load_grid(x.b, crate::services::video::Grid::Layer2, 198, end.destination);
            VIDEO.set(x.b, 0x24ce);
            Obj(STATE).set_u16(x.b, f::MARK, 0x728);
            Obj(STATE).set_u32(x.b, f::TEXT, RANKING_TEXT.addr());
            Flow::Done
        },
        SaFaded = 0xfc8a8 => |x| {
            crate::hw::sound::service(x.b, crate::hw::sound::Service::GeneralStop);
            reset_objects(x);
            x.wait(1, Step::SaVideo)
        },
        SaVideo = 0xfc8ce => |x| {
            VIDEO.set(x.b, 0x12c0);
            x.wait(4, Step::SaCleared)
        },
        SaCleared = 0xfc8dc => |x| {
            clear_layers(x);
            super::mode::SUB.set(x.b, 0);
            super::mode::STEP_WORD.set(x.b, 0);
            crate::world::ram::CAMERA_ALT_ACTIVE.set(x.b, 0);
            for v in [Word(sv::SCROLL_2.0), sv::CAMERA_Y, sv::CAMERA_2_Y] {
                v.set(x.b, 0);
            }
            crate::actors::families::small::startup_done(x.b, false);
            crate::actors::families::items::clear_picked_up(x.b);
            crate::world::ram::GAME.set(x.b, 0);
            Flow::Done
        },
    }
}

fn next_step(x: &mut Ctx) {
    let s = Obj(STATE).u8(x.b, f::STEP).wrapping_add(2);
    Obj(STATE).set_u8(x.b, f::STEP, s);
}

/// The three cleared layers.
fn clear_layers(x: &mut Ctx) {
    for layer in [Layer::One, Layer::Two, Layer::Three] {
        clear_layer(x.b, layer);
    }
}

/// Lists undone and emptied, objects cleared, pools reset (0x1f184, 0x1de7c, 0xc58, 0x2062).
fn reset_objects(x: &mut Ctx) {
    crate::world::objects::lists::unlink_all(x.b);
    crate::services::sprites::clear_lists(x.b);
    crate::services::video::clear_objects(x.b);
    for pool in pools::POOLS {
        pool.reset(x.b);
    }
}

/// One render: PEFF, EFFCT, sprites (`0xfc91c`).
fn render(x: &mut Ctx) {
    for class in [&crate::core::dispatch::PEFF, &crate::core::dispatch::EFFCT] {
        class.update_no_next(x.b);
    }
    crate::services::sprites::build_in_game(x.b);
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
        buttons |= sv::buttons(k).get(x.b);
    }
    let top_buttons = buttons & 0xf0;
    Speed { speed: if top_buttons == 0 { 0x10000 } else { 0x4000 }, top_buttons }
}

/// Advances the background scroll by one frame; camera 2 follows it.
fn scroll(x: &mut Ctx) -> Speed {
    let v = speed(x);
    let s = SCROLL.get(x.b).wrapping_sub(v.speed);
    SCROLL.set(x.b, s);
    sv::CAMERA_2_Y.set(x.b, (s >> 16) as u16);
    v
}

fn s0_prepare(x: &mut Ctx) -> Flow {
    next_step(x);
    Obj(STATE).set_u16(x.b, f::TIMER, 300);
    crate::hw::sound::play_global(x.b, crate::hw::sound::CMD_27_B);
    reset_objects(x);
    x.wait(1, Step::S0Video)
}

/// Step 2: the ranking scrolls; every 40 pixel lines, the next text lines
/// are added. When the text ends (negative byte), next step.
fn s2_scroll(x: &mut Ctx) -> Flow {
    render(x);
    scroll(x);
    let d0 = Obj(STATE).u16(x.b, f::MARK).wrapping_sub(sv::CAMERA_2_Y.get(x.b));
    if d0 as u8 != 40 {
        return Flow::Done;
    }
    let camera = sv::CAMERA_2_Y.get(x.b);
    Obj(STATE).set_u16(x.b, f::MARK, camera);
    let mut a4 = Obj(STATE).u32(x.b, f::TEXT);
    for counter in (0..6u8).rev() {
        if tables::byte(x.b, a4) & 0x80 != 0 {
            next_step(x);
            return Flow::Done;
        }
        // 0x517ba: a ranking line (variant = counter), its setting is the next byte of the text, which
        // only advances if the effect is created
        let setting = tables::byte(x.b, a4);
        if crate::services::effects_aids::effect_44(x.b, counter, setting).is_some() {
            a4 += 1;
        }
    }
    if tables::byte(x.b, a4) & 0x80 != 0 {
        next_step(x);
        return Flow::Done;
    }
    Obj(STATE).set_u32(x.b, f::TEXT, a4);
    Flow::Done
}

/// Step 4: the score cartridges (one object per player who played), after ten game frames.
fn s4_scores(x: &mut Ctx) -> Flow {
    if (FRAME_COUNT.get(x.b) as i16) < 10 {
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
        if Obj(a4).u32(x.b, SCORE) != 0 || Obj(a4).u32(x.b, SCORE + 4) != 0 {
            let o = end_screen(x.b, 5, counter);
            let o = or_rest(x.b, o);
            o.set_u8(x.b, RANK, rank);
            rank = rank.wrapping_sub(1);
            o.set_u32(x.b, obj::SIGNAL, a4);
        }
        a4 += 0x10;
    }
    let mut rank = 3u8;
    a4 = SCORES;
    for counter in (0..4u8).rev() {
        if Obj(a4).u8(x.b, PLAYED) != 0 {
            if Obj(a4).u8(x.b, NAMED) != 0 {
                let o = end_screen(x.b, 9, counter);
                let o = or_rest(x.b, o);
                o.set_u8(x.b, RANK, rank);
                o.set_u32(x.b, obj::SIGNAL, a4);
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
    if sv::CAMERA_2_Y.get(x.b) != 1064 {
        return Flow::Done;
    }
    next_step(x);
    SCROLL_2.set(x.b, 0x0108_0000);
    Word(sv::SCROLL_2.0).set(x.b, 264);
    Flow::Done
}

/// Step 8: the two planes scroll together until y 832.
fn s8_scenery(x: &mut Ctx) -> Flow {
    render(x);
    let speed = scroll(x).speed;
    let v = SCROLL_2.get(x.b).wrapping_sub(speed);
    SCROLL_2.set(x.b, v);
    Word(sv::SCROLL_2.0).set(x.b, (v >> 16) as u16);
    if sv::CAMERA_2_Y.get(x.b) != 832 {
        return Flow::Done;
    }
    next_step(x);
    Obj(STATE).set_u16(x.b, f::TIMER, 300);
    Flow::Done
}

/// Step 10: after 300 frames (or a button), fade to black and back to attract.
fn sa_leave(x: &mut Ctx) -> Flow {
    let mut buttons = 0;
    for k in 0..4 {
        buttons |= sv::pressed(k).get(x.b);
    }
    if buttons & 0xf0 != 0 {
        Obj(STATE).set_u16(x.b, f::TIMER, 1);
    }
    let left = Obj(STATE).u16(x.b, f::TIMER).wrapping_sub(1);
    Obj(STATE).set_u16(x.b, f::TIMER, left);
    if left != 0 {
        render(x);
        return Flow::Done;
    }
    crate::hw::sound::service(x.b, crate::hw::sound::Service::Ff07 { lock: 0, line: 4 });
    fading::PAGES.set(x.b, 0xf);
    fading::DELAY.set(x.b, 3);
    x.subroutine(crate::core::tasks::fading::descent, Step::SaFaded)
}

pub const NATIVES: &[Native] = &[];

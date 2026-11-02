//! Screens of the mode machine: startup, warnings, fixed attract frames.
//!
//! Each sub-mode (`FF0E71`) is a screen; it is entered at step 0 (preparation) and stays there,
//! one step at a time, until its timer (`FF0E78`) reaches zero.

use super::{mode, Flow, Sequence};
use crate::scene::fading;
use crate::video::{load_grid, clear_layer, Layer, Grid};
use crate::mem::{rd16, rd8, wr16, wr8, A5};
use crate::scene::Ctx;
use crate::Native;

/// Work RAM variables (offsets from A5).
mod var {
    /// Bits 1 to 3: background planes turned on (video command word).
    pub const LAYERS: u32 = 0x14;
    /// Fade: pages, delay.
    /// Vertical scroll of plane 2; camera x positions (longs).
    pub const SCROLL_2: u32 = 0x4ba;
    pub const CAMERA: u32 = 0x506;
    pub const CAMERA_2: u32 = 0x50a;
}

/// Background planes (bits 1 to 3 of the video command word).
const LAYER_BITS: u16 = 0xe;
/// Byte at FF0F53, set by the fixed-screen task when the screen is ready.
const SCREEN_READY: u32 = 0xff_0f53;
/// Fixed-screen task (object executor) and its slot.
const FIXED_SCREEN_TASK: u32 = 0x1835e;
const FIXED_SCREEN_SLOT: u32 = 0xf0;
/// Palette of the text screens and its two tile blocks ("copyright" screen).
const TEXT_PALETTE: u16 = 0x53;
const TILE_PLANE_3: u32 = 0x917000;
const SOUND_SCREEN_A: crate::sound::Sound = crate::sound::CMD_3A;
const SOUND_SCREEN_B: crate::sound::Sound = crate::sound::CMD_3B;

/// Screen of sub-mode `sub`, if it is rewritten.
pub fn screen(x: &mut Ctx, sub: u8) -> Option<Flow> {
    Some(match sub {
        0 | 6 => fixed_screen(x),
        8 => legal_screen(x),
        16 => second_legal_screen(x),
        18 => notice_screen(x),
        _ => return None,
    })
}

/// Sub-modes 0 and 6 (`0x1371e`): creates the fixed-screen task, waits for it to finish, then fades.
fn fixed_screen(x: &mut Ctx) -> Flow {
    if rd8(x.b, mode::STEP) == 0 {
        wr8(x.b, mode::STEP, 2);
        wr8(x.b, mode::BOOTED, 1);
        crate::families::small::reset_fixed_screen(x.b);
        crate::kernel::create_task(x.b, FIXED_SCREEN_SLOT, FIXED_SCREEN_TASK);
        return Flow::Done;
    }
    if rd8(x.b, SCREEN_READY) == 0 {
        return Flow::Done;
    }
    let s = rd8(x.b, mode::SUB).wrapping_add(2);
    wr8(x.b, mode::SUB, s);
    wr8(x.b, mode::STEP, 0);
    wr16(x.b, mode::TIMER, 1);
    fading::DELAY.set(x.b, 3);
    fading::PAGES.set(x.b, 0xf);
    x.subroutine(crate::tasks::fading::descent, Step::FixedScreenAfterFade)
}

const FIXED_SCREEN_FADED: u32 = 0x1376e;
crate::steps! {
    /// Steps (resume points) of the state machine.
    pub enum Step {
        FixedScreenAfterFade = FIXED_SCREEN_FADED => |x| {
            x.set_v32(var::SCROLL_2, 0);
            crate::video::clear_objects(x.b);
            clear_layer(x.b, Layer::One);
            Flow::Done
        },
        LegalScreenSetup = LEGAL_SETUP => |x| {
            wr16(x.b, mode::TIMER, 300);
            let step = rd8(x.b, mode::STEP).wrapping_add(2);
            wr8(x.b, mode::STEP, step);
            crate::palette::load_palette_page(x.b, 2, TEXT_PALETTE);
            clear_layer(x.b, Layer::Three);
            x.set_v32(var::CAMERA, 0);
            x.set_v32(var::CAMERA_2, 0);
            x.wait(1, Step::LegalScreenTiles)
        },
        LegalScreenTiles = LEGAL_TILES => |x| {
            two_tile_blocks(x, 0x1c0);
            x.or16(var::LAYERS, LAYER_BITS);
            x.wait(1, Step::LegalScreenSound)
        },
        LegalScreenSound = LEGAL_SOUND => |x| {
            crate::sound::play_global(x.b, SOUND_SCREEN_A);
            crate::sound::play_global(x.b, SOUND_SCREEN_B);
            fading::PAGES.set(x.b, 0xf);
            crate::tasks::fading::rise(x)
        },
        LegalScreenFaded = LEGAL_FADED => |x| {
            x.and16(var::LAYERS, !LAYER_BITS);
            x.wait(1, Step::LegalScreenCleared)
        },
        LegalScreenCleared = LEGAL_CLEARED => |x| {
            clear_layer(x.b, Layer::Two);
            x.or16(var::LAYERS, LAYER_BITS);
            x.wait(1, Step::LegalScreenPalette)
        },
        LegalScreenPalette = LEGAL_PALETTE => |x| {
            wr16(x.b, mode::TIMER, 300);
            crate::palette::load_palette_page(x.b, 2, 0);
            Flow::Done
        },
        SecondSetup = SECOND_SETUP => |x| {
            wr16(x.b, mode::TIMER, 300);
            let step = rd8(x.b, mode::STEP).wrapping_add(2);
            wr8(x.b, mode::STEP, step);
            crate::palette::load_palette_page(x.b, 2, TEXT_PALETTE);
            clear_layer(x.b, Layer::Three);
            x.set_v32(var::CAMERA, 0);
            x.set_v32(var::CAMERA_2, 0);
            crate::tasks::prompt::stop(x.b);
            x.wait(1, Step::SecondTiles)
        },
        SecondTiles = SECOND_TILES => |x| {
            two_tile_blocks(x, 0x1c2);
            x.or16(var::LAYERS, LAYER_BITS);
            x.wait(1, Step::SecondFadeIn)
        },
        SecondFadeIn = SECOND_FADE_IN => |x| {
            fading::PAGES.set(x.b, 0xf);
            crate::tasks::fading::rise(x)
        },
        SecondFaded = SECOND_FADED => |x| {
            x.and16(var::LAYERS, !LAYER_BITS);
            x.wait(1, Step::SecondCleared)
        },
        SecondCleared = SECOND_CLEARED => |x| {
            clear_layer(x.b, Layer::Two);
            x.or16(var::LAYERS, LAYER_BITS);
            x.wait(1, Step::SecondPalette)
        },
        SecondPalette = SECOND_PALETTE => |x| {
            // `jmp 0x1a67e`
            crate::palette::load_palette_page(x.b, 2, 0);
            Flow::Done
        },
        NoticeTextShown = NOTICE_TEXT_SHOWN => |x| {
            crate::text::print(x.b, NOTICE_TEXT);
            x.or16(var::LAYERS, LAYER_BITS);
            x.wait(1, Step::NoticeFadeIn)
        },
        NoticeFadeIn = NOTICE_FADE_IN => |x| {
            fading::PAGES.set(x.b, 0xf);
            // `jsr 0x144c`: what follows is the countdown, just after
            x.subroutine(crate::tasks::fading::rise, Step::NoticeCountdown)
        },
        NoticeCountdown = NOTICE_COUNTDOWN => notice_countdown,
        NoticeFaded = NOTICE_FADED => |x| {
            x.and16(var::LAYERS, !LAYER_BITS);
            x.wait(1, Step::NoticeCleared)
        },
        NoticeCleared = NOTICE_CLEARED => |x| {
            clear_layer(x.b, Layer::One);
            x.or16(var::LAYERS, LAYER_BITS);
            crate::tasks::wait_then_return(x, 1)
        },
    }
}

/// Two tile blocks of layer 2 (`n`, `n + 1`) in a row, starting from `TILE_PLANE_3`.
fn two_tile_blocks(x: &mut Ctx, n: u16) {
    let end = load_grid(x.b, Grid::Layer2, n, TILE_PLANE_3);
    load_grid(x.b, Grid::Layer2, n + 1, end.destination);
}

/// Sub-mode 8 (`0x1377e`): picture made of two tile blocks for 300 frames, between two fades.
fn legal_screen(x: &mut Ctx) -> Flow {
    if rd8(x.b, mode::STEP) != 0 {
        return legal_screen_wait(x);
    }
    x.and16(var::LAYERS, !LAYER_BITS);
    x.wait(4, Step::LegalScreenSetup)
}

const LEGAL_SETUP: u32 = 0x13794;

const LEGAL_TILES: u32 = 0x137bc;

const LEGAL_SOUND: u32 = 0x137e8;

/// Wait step of sub-mode 8: when the timer ends, fade to black and next screen.
fn legal_screen_wait(x: &mut Ctx) -> Flow {
    let left = rd16(x.b, mode::TIMER).wrapping_sub(1);
    wr16(x.b, mode::TIMER, left);
    if left != 0 {
        return Flow::Done;
    }
    let s = rd8(x.b, mode::SUB).wrapping_add(2);
    wr8(x.b, mode::SUB, s);
    wr8(x.b, mode::STEP, 0);
    fading::PAGES.set(x.b, 0xf);
    x.subroutine(crate::tasks::fading::descent, Step::LegalScreenFaded)
}

const LEGAL_FADED: u32 = 0x1381c;

const LEGAL_CLEARED: u32 = 0x1382a;

const LEGAL_PALETTE: u32 = 0x1383e;

// ---------------------------------------------------------------------------------------------
// Sub-mode 16 (`0x1384c`): second rights picture, only if `A5+0x70` equals 2

fn second_legal_screen(x: &mut Ctx) -> Flow {
    if rd16(x.b, A5 + 0x70) != 2 {
        wr8(x.b, mode::SUB, 0);
        wr8(x.b, mode::STEP, 0);
        return Flow::Done;
    }
    if rd8(x.b, mode::STEP) != 0 {
        return second_legal_wait(x);
    }
    x.and16(var::LAYERS, !LAYER_BITS);
    x.wait(4, Step::SecondSetup)
}
const SECOND_SETUP: u32 = 0x13876;
const SECOND_TILES: u32 = 0x1389e;
const SECOND_FADE_IN: u32 = 0x138d0;
fn second_legal_wait(x: &mut Ctx) -> Flow {
    let left = rd16(x.b, mode::TIMER).wrapping_sub(1);
    wr16(x.b, mode::TIMER, left);
    if left != 0 {
        return Flow::Done;
    }
    let s = rd8(x.b, mode::SUB).wrapping_add(2);
    wr8(x.b, mode::SUB, s);
    wr8(x.b, mode::STEP, 0);
    fading::PAGES.set(x.b, 0xf);
    x.subroutine(crate::tasks::fading::descent, Step::SecondFaded)
}
const SECOND_FADED: u32 = 0x138f8;
const SECOND_CLEARED: u32 = 0x13906;
const SECOND_PALETTE: u32 = 0x1391a;

// ---------------------------------------------------------------------------------------------
// Sub-mode 18 (`0x13922`): "animated violence" notice, 120 frames

/// Text of the notice (in the original code).
const NOTICE_TEXT: u32 = 0x13b52;

fn notice_screen(x: &mut Ctx) -> Flow {
    if rd8(x.b, mode::STEP) != 0 {
        return notice_countdown(x);
    }
    wr16(x.b, mode::TIMER, 120);
    let step = rd8(x.b, mode::STEP).wrapping_add(2);
    wr8(x.b, mode::STEP, step);
    x.and16(var::LAYERS, !LAYER_BITS);
    x.wait(4, Step::NoticeTextShown)
}
const NOTICE_TEXT_SHOWN: u32 = 0x13942;
const NOTICE_FADE_IN: u32 = 0x1395a;
const NOTICE_COUNTDOWN: u32 = 0x13966;
fn notice_countdown(x: &mut Ctx) -> Flow {
    let left = rd16(x.b, mode::TIMER).wrapping_sub(1);
    wr16(x.b, mode::TIMER, left);
    if left != 0 {
        return Flow::Done;
    }
    wr16(x.b, mode::TIMER, 300);
    wr8(x.b, mode::SUB, 2);
    wr8(x.b, mode::STEP, 0);
    fading::PAGES.set(x.b, 0xf);
    x.subroutine(crate::tasks::fading::descent, Step::NoticeFaded)
}
const NOTICE_FADED: u32 = 0x1398a;
const NOTICE_CLEARED: u32 = 0x13998;

pub const NATIVES: &[Native] = &[];

//! Screens of the mode machine: startup, warnings, fixed attract frames.
//!
//! Each sub-mode (`FF0E71`) is a screen; it is entered at step 0 (preparation) and stays there,
//! one step at a time, until its timer (`FF0E78`) reaches zero.

use super::{mode, Flow, Sequence};
use crate::screens::scene::fading;
use crate::services::video::{load_grid, clear_layer, Layer, Grid};
use crate::world::screen_tables::Rom;
use crate::world::tasks::Ctx;
use crate::Native;
use crate::world::ram::Byte;
pub use crate::world::tasks::stages::Attract as Step;

/// Work RAM variables (offsets from A5).
mod var {
    use crate::world::ram::Long;
    /// Bits 1 to 3: background planes turned on (video command word).
    pub const LAYERS: u32 = 0x14;
    /// Fade: pages, delay.
    /// Vertical scroll of plane 2; camera x positions (longs).
    pub const SCROLL_2: Long = Long::a5(0x4ba);
    pub const CAMERA: Long = Long::a5(0x506);
    pub const CAMERA_2: Long = Long::a5(0x50a);
}

/// Background planes (bits 1 to 3 of the video command word).
const LAYER_BITS: u16 = 0xe;
/// Byte at FF0F53, set by the fixed-screen task when the screen is ready.
const SCREEN_READY: Byte = Byte(0xff_0f53);
/// Fixed-screen task (object executor) and its slot.
const FIXED_SCREEN_TASK: crate::world::tasks::Task = crate::world::tasks::Task::Scores;
const FIXED_SCREEN_SLOT: u32 = 0xf0;
/// Palette of the text screens and its two tile blocks ("copyright" screen).
const TEXT_PALETTE: u16 = 0x53;
const TILE_PLANE_3: u32 = crate::hw::video::SCROLL2.0 + 0x3000;
const SOUND_SCREEN_A: crate::services::sound::Sound = crate::services::sound::CMD_3A;
const SOUND_SCREEN_B: crate::services::sound::Sound = crate::services::sound::CMD_3B;

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
    if mode::STEP.get(x.b) == 0 {
        mode::STEP.set(x.b, 2);
        mode::BOOTED.set(x.b, 1);
        crate::actors::families::small::reset_fixed_screen(x.b);
        crate::world::tasks::traps::create_task(x.b, FIXED_SCREEN_SLOT, FIXED_SCREEN_TASK);
        return Flow::Done;
    }
    if SCREEN_READY.get(x.b) == 0 {
        return Flow::Done;
    }
    let s = mode::SUB.get(x.b).wrapping_add(2);
    mode::SUB.set(x.b, s);
    mode::STEP.set(x.b, 0);
    mode::TIMER.set(x.b, 1);
    fading::DELAY.set(x.b, 3);
    fading::PAGES.set(x.b, 0xf);
    x.subroutine(crate::services::fading::descent, Step::FixedScreenAfterFade)
}

crate::steps! {
    impl Step {
        FixedScreenAfterFade = 0x1376e => |x| {
            var::SCROLL_2.set(x.b, 0);
            crate::services::video::clear_objects(x.b);
            clear_layer(x.b, Layer::One);
            Flow::Done
        },
        LegalScreenSetup = 0x13794 => |x| {
            mode::TIMER.set(x.b, 300);
            let step = mode::STEP.get(x.b).wrapping_add(2);
            mode::STEP.set(x.b, step);
            crate::services::palette::load_palette_page(x.b, 2, TEXT_PALETTE);
            clear_layer(x.b, Layer::Three);
            var::CAMERA.set(x.b, 0);
            var::CAMERA_2.set(x.b, 0);
            x.wait(1, Step::LegalScreenTiles)
        },
        LegalScreenTiles = 0x137bc => |x| {
            two_tile_blocks(x, 0x1c0);
            x.or16(var::LAYERS, LAYER_BITS);
            x.wait(1, Step::LegalScreenSound)
        },
        LegalScreenSound = 0x137e8 => |x| {
            crate::services::sound::play_global(x.b, SOUND_SCREEN_A);
            crate::services::sound::play_global(x.b, SOUND_SCREEN_B);
            fading::PAGES.set(x.b, 0xf);
            crate::services::fading::rise(x)
        },
        LegalScreenFaded = 0x1381c => |x| {
            x.and16(var::LAYERS, !LAYER_BITS);
            x.wait(1, Step::LegalScreenCleared)
        },
        LegalScreenCleared = 0x1382a => |x| {
            clear_layer(x.b, Layer::Two);
            x.or16(var::LAYERS, LAYER_BITS);
            x.wait(1, Step::LegalScreenPalette)
        },
        LegalScreenPalette = 0x1383e => |x| {
            mode::TIMER.set(x.b, 300);
            crate::services::palette::load_palette_page(x.b, 2, 0);
            Flow::Done
        },
        SecondSetup = 0x13876 => |x| {
            mode::TIMER.set(x.b, 300);
            let step = mode::STEP.get(x.b).wrapping_add(2);
            mode::STEP.set(x.b, step);
            crate::services::palette::load_palette_page(x.b, 2, TEXT_PALETTE);
            clear_layer(x.b, Layer::Three);
            var::CAMERA.set(x.b, 0);
            var::CAMERA_2.set(x.b, 0);
            crate::screens::prompt::stop(x.b);
            x.wait(1, Step::SecondTiles)
        },
        SecondTiles = 0x1389e => |x| {
            two_tile_blocks(x, 0x1c2);
            x.or16(var::LAYERS, LAYER_BITS);
            x.wait(1, Step::SecondFadeIn)
        },
        SecondFadeIn = 0x138d0 => |x| {
            fading::PAGES.set(x.b, 0xf);
            crate::services::fading::rise(x)
        },
        SecondFaded = 0x138f8 => |x| {
            x.and16(var::LAYERS, !LAYER_BITS);
            x.wait(1, Step::SecondCleared)
        },
        SecondCleared = 0x13906 => |x| {
            clear_layer(x.b, Layer::Two);
            x.or16(var::LAYERS, LAYER_BITS);
            x.wait(1, Step::SecondPalette)
        },
        SecondPalette = 0x1391a => |x| {
            // `jmp 0x1a67e`
            crate::services::palette::load_palette_page(x.b, 2, 0);
            Flow::Done
        },
        NoticeTextShown = 0x13942 => |x| {
            crate::services::text::print(x.b, NOTICE_TEXT.addr());
            x.or16(var::LAYERS, LAYER_BITS);
            x.wait(1, Step::NoticeFadeIn)
        },
        NoticeFadeIn = 0x1395a => |x| {
            fading::PAGES.set(x.b, 0xf);
            // `jsr 0x144c`: what follows is the countdown, just after
            x.subroutine(crate::services::fading::rise, Step::NoticeCountdown)
        },
        NoticeCountdown = 0x13966 => notice_countdown,
        NoticeFaded = 0x1398a => |x| {
            x.and16(var::LAYERS, !LAYER_BITS);
            x.wait(1, Step::NoticeCleared)
        },
        NoticeCleared = 0x13998 => |x| {
            clear_layer(x.b, Layer::One);
            x.or16(var::LAYERS, LAYER_BITS);
            crate::world::tasks::wait_then_return(x, 1)
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
    if mode::STEP.get(x.b) != 0 {
        return legal_screen_wait(x);
    }
    x.and16(var::LAYERS, !LAYER_BITS);
    x.wait(4, Step::LegalScreenSetup)
}




/// Wait step of sub-mode 8: when the timer ends, fade to black and next screen.
fn legal_screen_wait(x: &mut Ctx) -> Flow {
    let left = mode::TIMER.get(x.b).wrapping_sub(1);
    mode::TIMER.set(x.b, left);
    if left != 0 {
        return Flow::Done;
    }
    let s = mode::SUB.get(x.b).wrapping_add(2);
    mode::SUB.set(x.b, s);
    mode::STEP.set(x.b, 0);
    fading::PAGES.set(x.b, 0xf);
    x.subroutine(crate::services::fading::descent, Step::LegalScreenFaded)
}




// ---------------------------------------------------------------------------------------------
// Sub-mode 16 (`0x1384c`): second rights picture, only if `A5+0x70` equals 2

fn second_legal_screen(x: &mut Ctx) -> Flow {
    if crate::screens::hud::var::LANGUAGE.get(x.b) != 2 {
        mode::SUB.set(x.b, 0);
        mode::STEP.set(x.b, 0);
        return Flow::Done;
    }
    if mode::STEP.get(x.b) != 0 {
        return second_legal_wait(x);
    }
    x.and16(var::LAYERS, !LAYER_BITS);
    x.wait(4, Step::SecondSetup)
}
fn second_legal_wait(x: &mut Ctx) -> Flow {
    let left = mode::TIMER.get(x.b).wrapping_sub(1);
    mode::TIMER.set(x.b, left);
    if left != 0 {
        return Flow::Done;
    }
    let s = mode::SUB.get(x.b).wrapping_add(2);
    mode::SUB.set(x.b, s);
    mode::STEP.set(x.b, 0);
    fading::PAGES.set(x.b, 0xf);
    x.subroutine(crate::services::fading::descent, Step::SecondFaded)
}

// ---------------------------------------------------------------------------------------------
// Sub-mode 18 (`0x13922`): "animated violence" notice, 120 frames

/// Text of the notice (in the original code).
const NOTICE_TEXT: Rom = Rom::at(0x13b52);

fn notice_screen(x: &mut Ctx) -> Flow {
    if mode::STEP.get(x.b) != 0 {
        return notice_countdown(x);
    }
    mode::TIMER.set(x.b, 120);
    let step = mode::STEP.get(x.b).wrapping_add(2);
    mode::STEP.set(x.b, step);
    x.and16(var::LAYERS, !LAYER_BITS);
    x.wait(4, Step::NoticeTextShown)
}
fn notice_countdown(x: &mut Ctx) -> Flow {
    let left = mode::TIMER.get(x.b).wrapping_sub(1);
    mode::TIMER.set(x.b, left);
    if left != 0 {
        return Flow::Done;
    }
    mode::TIMER.set(x.b, 300);
    mode::SUB.set(x.b, 2);
    mode::STEP.set(x.b, 0);
    fading::PAGES.set(x.b, 0xf);
    x.subroutine(crate::services::fading::descent, Step::NoticeFaded)
}

pub const NATIVES: &[Native] = &[];

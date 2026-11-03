//! Shared state `0xf7ecc` of the scene exits: fade to black and preparation of the next scene.
//!
//! Three sub-states (byte `+3` of the block, step of 2): (0) fade to black, players and objects reset, layers
//! cleared, cameras at the origin (scene 105, end of the game, keeps a camera in place); (1) palettes of the next
//! scene (table `0xf80c4`, four bytes per scene); (2) transition script specific to the scene (table of
//! jumps `0xf81fa`, variables `A5-0x5520`, module [`super::transition`]).

use super::screen::blocks::EXIT_SUB_STEP;
use crate::hw::video::regs;
use super::bonus::dispatch;
use super::screen;
use super::Ctx;
use crate::video::{clear_layer, Layer};
use crate::modes::{Flow, Sequence};
use crate::Native;

const DISPATCH: crate::screens::tables::Rom = crate::screens::tables::Rom::at(0xf7eda);
/// Palettes of the next scene: set for page 0 (negative: none), then D0 of 0x1a60a, 0x1a67e, 0x1a8b2.
const PALETTES: crate::screens::tables::Rom = crate::screens::tables::Rom::at(0xf80c4);
/// Scene 105 (end of the game) takes the palettes and script of scene 69.
const FINALE: u8 = 105;
const FINALE_AS: u32 = 69;

mod at {
    pub const FADED: crate::data::RomAddr = crate::data::RomAddr(0xf7f12);
    pub const CLEARED: crate::data::RomAddr = crate::data::RomAddr(0xf7f60);
}

fn next_sub_step(x: &mut Ctx) {
    let s = EXIT_SUB_STEP.get(x.b).wrapping_add(2);
    EXIT_SUB_STEP.set(x.b, s);
}

/// Scene whose tables are read.
fn table_scene(x: &mut Ctx) -> u32 {
    let scene = x.v8(0x34);
    if scene == FINALE { FINALE_AS } else { scene as u32 }
}

pub fn run(x: &mut Ctx) -> Flow {
    match dispatch(x, DISPATCH.addr()) {
        0 => {
            next_sub_step(x);
            x.set_v8(0x42, 2);
            x.set_v8(0x5f, 0);
            x.set_v16(0x490, 3);
            x.set_v8(0x48e, 15);
            x.subroutine(crate::tasks::fading::descent, Step::Faded)
        }
        2 => {
            palettes(x);
            Flow::Done
        }
        4 => script(x),
        _ => Flow::Done,
    }
}

crate::steps! {
    /// Steps (resume points) of the state machine.
    pub enum Step {
        Faded = at::FADED.0 => |x| {
            x.set_v8(0x48e, 15);
            crate::palette::no_brightness(x.b);
            if x.v8(0x34) == FINALE {
                // end of the game: cameras placed on the final terrain
                for (off, v) in [(0x506, 0), (0x50a, 0x200), (0x51e, 4), (0x556, 0), (0x55a, 0x500), (0x56e, 4)] {
                    x.set_v16(off, v);
                }
                crate::sprites::build_in_game(x.b);
                return x.wait(1, Step::Finale1);
            }
            for off in [0x602, 0x802, 0xa02, 0xc02] {
                x.set_v32(off, 0);
            }
            // Original glitch: the A0 left by the list unlinking (the last unlinked object, `movea.w`) survives the
            // yield, and the scene exit dispatch rereads byte `+2` of it at the end of the state
            if let Some(last) = crate::objects::lists::unlink_all(x.b) {
                super::exit::set_dispatch_a0(x.b, last.0 | 0xff00_0000);
            }
            crate::sprites::clear_lists(x.b);
            crate::video::clear_objects(x.b);
            x.set_v16(0x14, 0x12c0 & 0xfff1);
            x.wait(1, Step::Cleared)
        },
        /// Layers cleared, scrolls and cameras reset.
        Cleared = at::CLEARED.0 => |x| {
            regs::OBJECT_OUTPUT.set(x.b, 0x1355);
            clear_layer(x.b, Layer::One);
            x.set_v8(0x3c, 1);
            clear_layer(x.b, Layer::Two);
            clear_layer(x.b, Layer::Three);
            let f = x.v8(0x13) & !1;
            x.set_v8(0x13, f);
            screen::clear_920000(x.b);
            for off in [0x4b6, 0x506, 0x556, 0x4ba, 0x50a, 0x55a] {
                x.set_v32(off, 0);
            }
            x.set_v16(0x52c, 0);
            x.set_v16(0x57c, 0);
            x.set_v8(0x36, 0);
            x.set_v8(0x53c, 0);
            x.set_v32(0x534, 0);
            super::bonus::reset_camera_copies(x);
            for pool in crate::objects::pools::POOLS {
                pool.reset(x.b);
            }
            Flow::Done
        },
        Finale1 = 0xf802e => |x| {
            x.set_v16(0x14, 0x12c0 & 0xfff1);
            x.wait(1, Step::Finale2)
        },
        Finale2 = 0xf8042 => |x| {
            x.set_v8(0x3c, 1);
            Flow::Done
        },
    }
}

/// Sub-state 1: the palettes of the next scene, then the player screen and the fade.
fn palettes(x: &mut Ctx) {
    next_sub_step(x);
    let at = 4 * table_scene(x);
    let set = PALETTES.byte(x.b, at);
    if (set as i8) >= 0 {
        crate::palette::load_palette_set(x.b, set as u16);
    }
    // pages 1, 2 and 3 (0x1a60a, 0x1a67e, 0x1a8b2)
    for page in 1..=3 {
        let n = PALETTES.byte(x.b, at + page as u32);
        crate::palette::load_palette_page(x.b, page, n as u16);
    }
    crate::hud::forget_displayed(x.b);
    x.set_v8(0x48e, 12);
    crate::palette::no_brightness(x.b);
}

/// Sub-state 2: the scene's transition script (`transition`, table 0xf81fa).
fn script(x: &mut Ctx) -> Flow {
    let scene = table_scene(x);
    super::transition::run(x, scene)
}

pub const NATIVES: &[Native] = &[];

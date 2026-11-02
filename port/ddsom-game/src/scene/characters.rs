//! Shared state `0xf6492` of the scene exits: an interlude specific to certain characters.
//!
//! Certain scenes (byte table `0xf672e`: one bit per character, from the player's `+3` byte divided by
//! two) make each concerned player play a short scene: screen cleared, palettes loaded, then for each
//! player three SET objects (types 5, 6, 7) created as soon as there is room, 240 frames, and the next player.
//! Six sub-states (byte `+3` of the block, step of 2); several cede control to the scheduler.

use super::bonus::{dispatch, fade};
use crate::video::{clear_layer, Layer};
use super::Ctx;
use crate::mem::{prog8, rd16, rd8, wr16, wr32, wr8, A5};
use crate::modes::{Flow, Sequence};
use crate::Native;

mod v {
    use crate::mem::A5;
    /// Characters concerned (mask), current player (word), players left to examine.
    pub const MASK: u32 = A5 - 0x5576;
    pub const PLAYER: u32 = A5 - 0x5578;
    pub const LEFT: u32 = A5 - 0x5575;
    /// Timer (byte) of a player's interlude.
    pub const TIMER: u32 = A5 - 0x55a8;
}

const DISPATCH: u32 = 0xf64a0;
/// Mask of the characters concerned, per scene (bytes relative to the PC).
const MASKS: u32 = 0xf672e;

mod at {
    pub const FADED_OUT: u32 = 0xf64d4;
    /// After a wait, new attempt to create objects 5, 6 and 7.
    pub const RETRY_5: u32 = 0xf6600;
    pub const RETRY_6: u32 = 0xf6626;
    pub const RETRY_7: u32 = 0xf6644;
    pub const NEXT_WAITED: u32 = 0xf66aa;
}

/// Scene exit block: the original's A0, set by the dispatch and kept across the yields.
const BLOCK: u32 = super::screen::blocks::EXIT;

fn sub_step(x: &mut Ctx) -> u8 {
    rd8(x.b, BLOCK + 3)
}
fn set_sub_step(x: &mut Ctx, s: u8) {
    wr8(x.b, BLOCK + 3, s)
}
fn next_sub_step(x: &mut Ctx) {
    let s = sub_step(x).wrapping_add(2);
    set_sub_step(x, s);
}

/// End of the state without an interlude: next state of the sequence.
fn skip(x: &mut Ctx) {
    let s = rd8(x.b, A5 - 0x55c8).wrapping_add(4);
    wr8(x.b, A5 - 0x55c8, s);
    wr8(x.b, A5 - 0x55c7, 0);
}

/// Is player `p` in the game and concerned by the mask (`btst` on a register: bit modulo 32)?
fn concerned(x: &mut Ctx, p: u32, mask: u8) -> bool {
    let bit = (rd8(x.b, p + 3) >> 1) as u32 % 32;
    rd8(x.b, p) != 0 && (rd8(x.b, p + 4) as i8) < 4 && (mask as u32) >> bit & 1 != 0
}

pub fn run(x: &mut Ctx) -> Flow {
    match dispatch(x, DISPATCH) {
        0 => start(x),
        2 => next_player(x),
        4 => player_scene(x),
        6 => {
            next_sub_step(x);
            x.set_v16(0x14, 0x12ce);
            Flow::Done
        }
        8 => player_done(x),
        10 => end(x),
        _ => Flow::Done,
    }
}

/// Sub-state 0 (0xf66d6): the scene mask; with no player concerned, the state is skipped, otherwise fade to black.
fn start(x: &mut Ctx) -> Flow {
    wr8(x.b, v::MASK, 0);
    let scene = x.v8(0x34) as u32;
    let mask = prog8(x.b, MASKS + scene);
    let any = mask != 0 && (0..4).any(|k| concerned(x, super::PLAYERS + k * super::PLAYER_STRIDE, mask));
    if !any {
        skip(x);
        return Flow::Done;
    }
    wr8(x.b, v::MASK, mask);
    x.set_v8(0x5f, 0);
    fade(x, crate::tasks::fading::descent, Step::FadedOut)
}

/// The player of the current interlude: its record (A6 of the original) and its `+3` byte (D1), which the original
/// keeps in registers during the yields.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Player {
    record: u32,
    byte: u8,
}

/// Steps (resume points) of the state machine.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Step {
    /// Screen cleared, cameras at the origin, interlude palettes (84); then the first player.
    FadedOut,
    /// After a wait, new attempt to create objects 5, 6 and 7.
    Retry5(Player),
    Retry6(Player),
    Retry7(Player),
    NextWaited,
}

impl Step {
    /// Address the original resumes at in this step.
    pub const fn address(self) -> u32 {
        match self {
            Step::FadedOut => at::FADED_OUT,
            Step::Retry5(_) => at::RETRY_5,
            Step::Retry6(_) => at::RETRY_6,
            Step::Retry7(_) => at::RETRY_7,
            Step::NextWaited => at::NEXT_WAITED,
        }
    }

    /// The step's body.
    pub fn step(self, x: &mut Ctx) -> Flow {
        match self {
            Step::FadedOut => faded_out(x),
            Step::Retry5(j) => objects(x, j),
            Step::Retry6(j) => objects_6_7(x, j),
            Step::Retry7(j) => object_7(x, j),
            Step::NextWaited => {
                for layer in [Layer::One, Layer::Two, Layer::Three] {
                    clear_layer(x.b, layer);
                }
                Flow::Done
            }
        }
    }
}

/// Screen cleared, cameras at the origin, interlude palettes (84); then the first player.
fn faded_out(x: &mut Ctx) -> Flow {
    for off in [0x50a, 0x55a, 0x506, 0x556] {
        x.set_v16(off, 0);
    }
    x.set_v8(0x53c, 0);
    super::bonus::reset_camera_copies(x);
    x.set_v8(0x42, 2);
    next_sub_step(x);
    wr16(x.b, v::PLAYER, super::PLAYERS as u16);
    wr8(x.b, v::LEFT, 4);
    reset_objects(x);
    clear_layer(x.b, Layer::One);
    x.set_v8(0x3c, 0xff);
    crate::palette::load_palette_page(x.b, 2, 84);
    crate::palette::load_palette_page(x.b, 3, 84);
    crate::palette::load_palette_page(x.b, 1, 0);
    x.set_v8(0x48e, 15);
    crate::palette::full_brightness(x.b);
    next_player(x)
}

/// Sub-state 2: the next concerned player, or the end of the state.
fn next_player(x: &mut Ctx) -> Flow {
    if rd8(x.b, v::LEFT) == 0 {
        return end(x);
    }
    let mut p = rd16(x.b, v::PLAYER) as i16 as i32 as u32;
    x.set_v8(0x33, 1);
    let mask = rd8(x.b, v::MASK);
    loop {
        if concerned(x, p & 0xff_ffff, mask) {
            next_sub_step(x);
            wr16(x.b, v::PLAYER, p as u16);
            return Flow::Done;
        }
        p = p.wrapping_add(0x200);
        let left = rd8(x.b, v::LEFT).wrapping_sub(1);
        wr8(x.b, v::LEFT, left);
        if left == 0 {
            return end(x);
        }
    }
}

/// Sub-state 4: the player's interlude, after four frames (the player follows in the resume step).
fn player_scene(x: &mut Ctx) -> Flow {
    next_sub_step(x);
    wr8(x.b, v::TIMER, 0xf0);
    x.set_v16(0x14, 0x12c0);
    let p = rd16(x.b, v::PLAYER) as i16 as i32 as u32;
    let player = Player { record: p, byte: rd8(x.b, p + 3) };
    x.wait(4, Step::Retry5(player))
}

/// Lists undone and emptied (0x1f184, 0x1de7c), pools reset.
fn reset_objects(x: &mut Ctx) {
    crate::objects::lists::unlink_all(x.b);
    crate::sprites::clear_lists(x.b);
    for pool in crate::objects::pools::POOLS {
        pool.reset(x.b);
    }
}

/// Creates the SET object of type 15 and variant `kind` (0xce00a); if there is no room, waits `frames` frames and
/// resumes at `retry`.
fn object(x: &mut Ctx, kind: u8, frames: u32, retry: Step) -> Option<u32> {
    match crate::families::creators::set_15_pure(x.b, kind) {
        Ok(o) => Some(o.0),
        Err(_) => {
            x.wait(frames, retry);
            None
        }
    }
}

/// The object is linked to the player (byte `+0x2f`) and to the scene exit block (long `+0x4c`, A0 of the original).
fn bind(x: &mut Ctx, o: u32, j: Player) {
    wr8(x.b, o + 0x2f, j.byte);
    wr32(x.b, o + 0x4c, BLOCK);
}

/// Objects 5, 6 and 7 of the player.
fn objects(x: &mut Ctx, j: Player) -> Flow {
    let Some(o) = object(x, 5, 4, Step::Retry5(j)) else { return Flow::Yield };
    bind(x, o, j);
    objects_6_7(x, j)
}

fn objects_6_7(x: &mut Ctx, j: Player) -> Flow {
    let Some(o) = object(x, 6, 1, Step::Retry6(j)) else { return Flow::Yield };
    bind(x, o, j);
    object_7(x, j)
}

fn object_7(x: &mut Ctx, j: Player) -> Flow {
    let Some(o) = object(x, 7, 1, Step::Retry7(j)) else { return Flow::Yield };
    wr16(x.b, o + 0x34, j.record as u16);
    Flow::Done
}

/// Sub-state 8: after 240 frames, the screen is cleared and the next player goes (sub-state 2).
fn player_done(x: &mut Ctx) -> Flow {
    let t = rd8(x.b, v::TIMER).wrapping_sub(1);
    wr8(x.b, v::TIMER, t);
    if t != 0 {
        return Flow::Done;
    }
    set_sub_step(x, 2);
    let p = (rd16(x.b, v::PLAYER) as i16 as i32 as u32).wrapping_add(0x200);
    wr16(x.b, v::PLAYER, p as u16);
    let left = rd8(x.b, v::LEFT).wrapping_sub(1);
    wr8(x.b, v::LEFT, left);
    reset_objects(x);
    crate::sprites::build_in_game(x.b);
    crate::video::clear_objects(x.b);
    x.set_v16(0x14, 0x12c0);
    x.wait(1, Step::NextWaited)
}

/// End of the state (0xf66bc): next state of the sequence, one frame yielded (direct return to the dispatch).
fn end(x: &mut Ctx) -> Flow {
    skip(x);
    x.set_v8(0x33, 0);
    x.set_v8(0x3c, 1);
    crate::tasks::wait_then_return(x, 1)
}

pub const NATIVES: &[Native] = &[
    
];

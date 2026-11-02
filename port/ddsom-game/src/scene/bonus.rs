//! Shared state `0xf5938` of the scene exits: the bonus screen.
//!
//! It only takes place if a player in the game has both longs `+0x118` and `+0x11c` at -1. Fade to black, cameras set
//! aside, then (seven sub-states, byte `+3` of the block, step of 2): bonus terrain and objects (four numbered effects,
//! five SET objects sorted by address, the last of which drives the game), 90-second countdown displayed by an
//! effect (`+0x42`: seconds left), fade, cameras returned. The game itself is in the objects.

use super::screen::{self, copy_page};
use super::fading;
use super::{Ctx, PLAYERS, PLAYER_STRIDE};
use crate::mem::{prog16, rd16, rd32, rd8, wr16, wr32, wr8, A5};
use crate::modes::{Flow, Sequence};
use crate::Native;
use crate::families::effects_creators::{end_screen, or_rest};
use crate::objects::{pools, Obj};
use crate::video::{load_grid, clear_layer, Layer, Grid};

/// Bonus screen variables (below A5).
mod v {
    use crate::mem::A5;
    /// Cameras set aside: X and Y of the main one, X and Y of the alternate one.
    pub const SAVED_CAMERAS: u32 = A5 - 0x5594;
    /// Seconds left (byte; word during the final fade), frames of the second, total duration (word).
    pub const SECONDS: u32 = A5 - 0x558c;
    pub const FRAMES: u32 = A5 - 0x558b;
    pub const DURATION: u32 = A5 - 0x558a;
    /// The object that drives the game (word), its five sorted objects (words), the effect that displays the countdown.
    pub const LEADER: u32 = A5 - 0x5588;
    pub const OBJECTS: u32 = A5 - 0x5586;
    pub const COUNTER: u32 = A5 - 0x5564;
    /// Last value seen of the leader's `+0x17` byte.
    pub const LAST_SCORE: u32 = A5 - 0x55a7;
}

/// Work RAM variables (offsets from A5) touched by the bonus.
mod a5 {
    pub const SCENE: u32 = 0x34;
    /// Main camera: X, Y, copies of X and Y, and other fields reset to zero; same for the alternate one.
    pub const CAM_X: u32 = 0x506;
    pub const CAM_Y: u32 = 0x50a;
    pub const ALT_X: u32 = 0x556;
    pub const ALT_Y: u32 = 0x55a;
}

/// ROM tables and subroutines.
mod rom {
    pub const DISPATCH: u32 = 0xf5946;
    /// Bonus terrain (rectangles of cells), according to `A5+0x70`.
    pub const BACKDROP: u32 = 0x3f7e0e;
    pub const BACKDROP_ALT: u32 = 0x3f6c08;
}
mod sub {
    pub const MUSIC: u32 = 0xe1114;
}

/// Resumes (original addresses following a yield).
mod at {
    pub const FADED_OUT: u32 = 0xf5fcc;
    pub const SET_UP: u32 = 0xf60ca;
    pub const STARTED: u32 = 0xf62a8;
    pub const ENDED: u32 = 0xf6380;
    pub const SCENE_1_WAITED: u32 = 0xf63ee;
    pub const LAST_FRAME: u32 = 0xf6470;
    pub const FADED_IN: u32 = 0xf6488;
}

/// The scene-exit block (A0 of the original throughout the state).
const BLOCK: u32 = super::screen::blocks::EXIT;

fn sub_step(x: &mut Ctx) -> u8 {
    rd8(x.b, BLOCK + 3)
}
fn next_sub_step(x: &mut Ctx) {
    let s = sub_step(x).wrapping_add(2);
    wr8(x.b, BLOCK + 3, s);
}
/// End of the state: next state of the sequence (`+2` += 4), sub-state 0.
fn leave(x: &mut Ctx) {
    let a0 = BLOCK;
    let s = rd8(x.b, a0 + 2).wrapping_add(4);
    wr8(x.b, a0 + 2, s);
    wr8(x.b, a0 + 3, 0);
}

/// Original dispatch (`moveq #0,D0 ; move.b 3(A0),D0 ; move.w table(PC,D0),D1`): the sub-step.
pub fn dispatch(x: &mut Ctx, _table: u32) -> u8 {
    sub_step(x)
}

/// Fade to black or from black (delay 4, all pages, 15 steps); the routine cedes control and returns to
/// `resume`.
pub fn fade(x: &mut Ctx, routine: fn(&mut Ctx) -> Flow, resume: impl Into<crate::tasks::table::Next>) -> Flow {
    fading::STEP.set(x.b, 15);
    fading::DELAY.set(x.b, 4);
    fading::PAGES.set(x.b, 15);
    x.subroutine(routine, resume)
}

/// `movem.l A0,-(SP) ; jsr 0x1de96 ; movem.l (SP)+,A0`: the sprite list, A0 preserved.
pub fn sprites_keep_a0(x: &mut Ctx) {
    crate::sprites::build_in_game(x.b);
}

/// A state of 0xf5938.
pub fn run(x: &mut Ctx) -> Flow {
    match dispatch(x, rom::DISPATCH) {
        0 => start(x),
        2 => set_up(x),
        4 => objects(x),
        6 => {
            countdown(x);
            Flow::Done
        }
        8 => end(x),
        10 => {
            if tick16(x, v::SECONDS) {
                next_sub_step(x);
                for k in 0..4 {
                    let w = rd16(x.b, v::SAVED_CAMERAS + 2 * k);
                    let at = [a5::CAM_X, a5::CAM_Y, a5::ALT_X, a5::ALT_Y][k as usize];
                    x.set_v16(at, w);
                }
            }
            Flow::Done
        }
        12 => {
            x.or16(0x14, 12);
            sprites_keep_a0(x);
            x.wait(1, Step::LastFrame)
        }
        _ => Flow::Done,
    }
}

/// Word decremented; true when it reaches zero.
fn tick16(x: &mut Ctx, at: u32) -> bool {
    let t = rd16(x.b, at).wrapping_sub(1);
    wr16(x.b, at, t);
    t == 0
}

/// The players in the game (present, state < 4) who won the bonus (`+0x118` and `+0x11c` at -1).
fn qualified(x: &mut Ctx) -> u8 {
    let mut n = 0u8;
    for k in 0..4 {
        let p = PLAYERS + k * PLAYER_STRIDE;
        if rd8(x.b, p) != 0 && (rd8(x.b, p + 4) as i8) < 4 && rd32(x.b, p + 0x118) == u32::MAX && rd32(x.b, p + 0x11c) == u32::MAX {
            n += 1;
        }
    }
    n
}

/// Sub-state 0: with no qualified player, the state is skipped; otherwise fade to black.
fn start(x: &mut Ctx) -> Flow {
    if qualified(x) == 0 {
        let s = rd8(x.b, A5 - 0x55c8).wrapping_add(4);
        wr8(x.b, A5 - 0x55c8, s);
        return Flow::Done;
    }
    fade(x, crate::tasks::fading::descent, Step::FadedOut)
}

crate::steps! {
    /// Steps (resume points) of the state machine.
    pub enum Step {
        /// The cameras are set aside and placed on the bonus terrain.
        FadedOut = at::FADED_OUT => |x| {
            next_sub_step(x);
            let main = (x.v16(a5::CAM_X), x.v16(a5::CAM_Y));
            wr16(x.b, v::SAVED_CAMERAS, main.0);
            wr16(x.b, v::SAVED_CAMERAS + 2, main.1);
            x.set_v8(0x53c, 0);
            let alt = (x.v16(a5::ALT_X), x.v16(a5::ALT_Y));
            wr16(x.b, v::SAVED_CAMERAS + 4, alt.0);
            wr16(x.b, v::SAVED_CAMERAS + 6, alt.1);
            x.set_v16(a5::CAM_Y, 0x408);
            x.set_v16(a5::ALT_Y, 0x400);
            x.set_v16(a5::CAM_X, 0);
            x.set_v16(a5::ALT_X, 0);
            x.set_v16(0x4ba, 0);
            if x.v8(a5::SCENE) != 1 {
                reset_camera_copies(x);
            }
            Flow::Done
        },
        SetUp = at::SET_UP => |x| {
            next_sub_step(x);
            for (from, to) in PAGES {
                copy_page(x.b, from, to);
            }
            crate::palette::load_palette_page(x.b, 2, 85);
            crate::palette::load_palette_page(x.b, 3, 85);
            crate::palette::load_palette_set(x.b, 2);
            Flow::Done
        },
        /// The bonus title (SET object type 0, variant 69).
        Started = at::STARTED => |x| {
            if let Some(Obj(o)) = pools::SET.alloc(x.b) {
                for (off, v) in [(0, 1), (2, 0), (3, 69), (0x2f, 3)] {
                    wr8(x.b, o + off, v);
                }
            }
            Flow::Done
        },
        /// The bonus screen is cleared; outside scene 1 the state is finished, in scene 1 the palette pages come back
        /// after four frames.
        Ended = at::ENDED => |x| {
            next_sub_step(x);
            screen::clear_bonus_frame(x.b);
            let counter = rd16(x.b, v::COUNTER) as i16 as i32 as u32 & 0xff_ffff;
            wr32(x.b, counter + 4, 0x0600_0000);
            crate::dispatch::EFFCT.update_no_next(x.b);
            crate::sprites::build_in_game(x.b);
            if x.v8(a5::SCENE) != 1 {
                x.set_v8(0x3c, 0xff);
                clear_layer(x.b, Layer::Two);
                clear_layer(x.b, Layer::Three);
                crate::video::clear_objects(x.b);
                leave(x);
                return Flow::Done;
            }
            x.set_v8(0x3c, 1);
            wr16(x.b, v::SECONDS, 4);
            x.and16(0x14, 0xfff3);
            x.wait(4, Step::Scene1Waited)
        },
        Scene1Waited = at::SCENE_1_WAITED => |x| {
            for (to, from) in PAGES {
                copy_page(x.b, from, to);
            }
            Flow::Done
        },
        /// Last frame: fade from black, then the next state of the sequence.
        LastFrame = at::LAST_FRAME => |x| fade(x, crate::tasks::fading::rise, Step::FadedIn),
        FadedIn = at::FADED_IN => |x| {
            leave(x);
            Flow::Done
        },
    }
}

/// Copies of the camera positions and scroll fields reset to their starting values.
pub fn reset_camera_copies(x: &mut Ctx) {
    let (cx, cy, ax, ay) = (x.v16(a5::CAM_X), x.v16(a5::CAM_Y), x.v16(a5::ALT_X), x.v16(a5::ALT_Y));
    x.set_v16(0x50e, cx);
    x.set_v16(0x512, cy);
    x.set_v16(0x55e, ax);
    x.set_v16(0x562, ay);
    x.set_v16(0x52c, 0);
    x.set_v16(0x57c, 0);
    x.set_v16(0x51e, 4);
    x.set_v16(0x56e, 4);
    let f = x.v8(0x13) & !1;
    x.set_v8(0x13, f);
    x.set_v8(0x36, 0);
}

/// Sub-state 2: when the terrain is loaded (`A5+0x7e68` >= 5), the screen is cleared (except scene 1) and the bonus
/// effects created; four frames of waiting.
fn set_up(x: &mut Ctx) -> Flow {
    if (x.v16(0x7e68) as i16) < 5 {
        return Flow::Done;
    }
    // Original glitch: the countdown effect receives in `+0x2f` the low byte of D1 as left by
    // the clearing of the text area (D1.w = 0xffff), or, in scene 1, the dispatch (the offset of the
    // sub-step 2 in its table)
    let setting = if x.v8(a5::SCENE) != 1 {
        x.set_v8(0x42, 2);
        crate::objects::lists::unlink_all(x.b);
        crate::sprites::clear_lists(x.b);
        for pool in crate::objects::pools::POOLS {
            pool.reset(x.b);
        }
        clear_layer(x.b, Layer::Two);
        clear_layer(x.b, Layer::Three);
        screen::clear_text(x.b);
        0xff
    } else {
        prog16(x.b, rom::DISPATCH + 2) as u8
    };
    // Original glitch: allocation failures are not tested (the A1 left over receives the writes)
    let created = end_screen(x.b, 3, setting);
    let counter = or_rest(x.b, created).0;
    wr16(x.b, v::COUNTER, counter as u16);
    wr8(x.b, counter + 0x42, 20);
    for k in 0..4u8 {
        let created = end_screen(x.b, 8, 3 - k);
        let o = or_rest(x.b, created);
        wr8(x.b, o.0 + 0x2f, k);
    }
    crate::sound::play_global(x.b, sub::MUSIC);
    x.wait(4, Step::SetUp)
}

/// Bonus palette pages: page 0x91c400 goes to 0x910000, and so on.
const PAGES: [(u32, u32); 4] = [(0x91c400, 0x910000), (0x91c800, 0x910400), (0x91cc00, 0x910800), (0x91c000, 0x910c00)];

/// Sub-state 4: when the terrain is ready (`A5+0x7e66` >= 5), five SET objects, sorted by address: the first four
/// are linked to the players, the last (the leader) to the other four. 90-second countdown,
/// terrain, tile blocks, fade from black.
fn objects(x: &mut Ctx) -> Flow {
    if x.v16(0x7e66) < 5 {
        return Flow::Done;
    }
    // Original glitch: allocation failures are not tested (the A1 left over is stored)
    let mut objs = [0u16; 5];
    for o in objs.iter_mut() {
        *o = crate::families::creators::set_5_pure(x.b).unwrap_or_else(|a1| a1).0 as u16;
    }
    // original bubble sort on sign-extended addresses (cmpa.w)
    objs.sort_by_key(|&w| w as i16 as i32 as u32);
    for (k, &o) in objs.iter().enumerate() {
        wr16(x.b, v::OBJECTS + 2 * k as u32, o);
    }
    let addr = |w: u16| w as i16 as i32 as u32 & 0xff_ffff;
    let leader = addr(objs[4]);
    for k in 0..4 {
        let o = addr(objs[k]);
        wr8(x.b, o + 0x2f, k as u8);
        wr16(x.b, o + 0x26, objs[4]);
        wr16(x.b, o + 0x22, (PLAYERS + k as u32 * PLAYER_STRIDE) as u16);
    }
    wr8(x.b, leader + 3, 0);
    let (first, second) = (rd32(x.b, v::OBJECTS), rd32(x.b, v::OBJECTS + 4));
    wr32(x.b, leader + 0x18, first);
    wr32(x.b, leader + 0x1c, second);
    wr16(x.b, v::LEADER, objs[4]);
    let score = rd8(x.b, leader + 0x17);
    wr8(x.b, v::LAST_SCORE, score);
    next_sub_step(x);
    wr8(x.b, v::SECONDS, 20);
    wr8(x.b, v::FRAMES, 60);
    wr16(x.b, v::DURATION, 5400);
    x.set_v8(0x5f, 0);
    backdrop(x);
    show_seconds(x);
    x.set_v16(0x14, 0x12ce);
    let f = x.v8(0x13) & !1;
    x.set_v8(0x13, f);
    for (grid, n, destination) in [(Grid::Layer2, 0x205, 0x917000), (Grid::Layer3, 0x172, 0x919800)] {
        let end = load_grid(x.b, grid, n, destination);
        load_grid(x.b, grid, n + 1, end.destination);
    }
    x.set_v8(0x3c, 2);
    crate::palette::load_palette_page(x.b, 1, 1);
    fade(x, crate::tasks::fading::rise, Step::Started)
}

/// 0xf67fa: the bonus terrain, rectangle of cells read from ROM (width, height, then the cells by columns).
fn backdrop(x: &mut Ctx) {
    let mut src = if x.v16(0x70) != 0 { rom::BACKDROP_ALT } else { rom::BACKDROP };
    let (rows, columns) = (rd16(x.b, src) as u32, rd16(x.b, src + 2) as u32);
    src += 4;
    for col in 0..columns {
        for row in 0..rows {
            let v = rd32(x.b, src);
            wr32(x.b, 0x912014 + col * 0x80 + 4 * row, v);
            src += 4;
        }
    }
}

/// 0xf6772: the countdown effect displays the seconds left.
fn show_seconds(x: &mut Ctx) {
    let counter = rd16(x.b, v::COUNTER) as i16 as i32 as u32 & 0xff_ffff;
    let s = rd8(x.b, v::SECONDS);
    wr8(x.b, counter + 0x42, s);
}

/// Sub-state 6: the countdown. It restarts from 20 seconds when the leader's score (`+0x17`) changes, drops to
/// 10 when the leader asks for it (`+0x14`), and the game ends after the 90 seconds, at zero, or when the score
/// reaches the goal (`+0x16`).
fn countdown(x: &mut Ctx) {
    let leader = rd16(x.b, v::LEADER) as i16 as i32 as u32 & 0xff_ffff;
    if (rd8(x.b, v::SECONDS) as i8) < 10 && rd8(x.b, leader + 0x14) != 0 {
        wr8(x.b, v::SECONDS, 10);
        wr8(x.b, v::FRAMES, 60);
        return show_seconds(x);
    }
    let score = rd8(x.b, leader + 0x17);
    if score != rd8(x.b, leader + 0x16) {
        if score != rd8(x.b, v::LAST_SCORE) {
            wr8(x.b, v::LAST_SCORE, score);
            wr8(x.b, v::SECONDS, 20);
            wr8(x.b, v::FRAMES, 60);
            return show_seconds(x);
        }
        let left = rd16(x.b, v::DURATION).wrapping_sub(1);
        wr16(x.b, v::DURATION, left);
        if left as i16 > 0 {
            let f = rd8(x.b, v::FRAMES).wrapping_sub(1);
            wr8(x.b, v::FRAMES, f);
            if f != 0 {
                return;
            }
            let s = rd8(x.b, v::SECONDS).wrapping_sub(1);
            wr8(x.b, v::SECONDS, s);
            if (s as i8) >= 0 {
                wr8(x.b, v::FRAMES, 60);
                return show_seconds(x);
            }
        } else {
            wr16(x.b, v::SECONDS, 1);
            show_seconds(x);
        }
    }
    wr32(x.b, leader + 4, 0x0600_0000);
    next_sub_step(x);
    wr16(x.b, v::SECONDS, 30);
}

/// Sub-state 8: 30 frames, then fade to black.
fn end(x: &mut Ctx) -> Flow {
    if !tick16(x, v::SECONDS) {
        return Flow::Done;
    }
    fade(x, crate::tasks::fading::descent, Step::Ended)
}

pub const NATIVES: &[Native] = &[
    
];

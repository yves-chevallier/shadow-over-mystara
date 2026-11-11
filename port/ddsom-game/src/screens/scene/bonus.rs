//! Shared state `0xf5938` of the scene exits: the bonus screen.
//!
//! It only takes place if a player in the game has both longs `+0x118` and `+0x11c` at -1. Fade to black, cameras set
//! aside, then (seven sub-states, byte `+3` of the block, step of 2): bonus terrain and objects (four numbered effects,
//! five SET objects sorted by address, the last of which drives the game), 90-second countdown displayed by an
//! effect (`+0x42`: seconds left), fade, cameras returned. The game itself is in the objects.

use super::screen::{self, copy_page};
use super::fading;
use super::{Ctx, PLAYERS, PLAYER_STRIDE};
use super::screen::blocks::{EXIT_STEP, EXIT_SUB_STEP};
use crate::hw::video::{PALETTES, SCROLL1, SCROLL2, SCROLL3, TEXT};
use crate::world::ram::{Long, Word};
use crate::world::screen_tables as tables;
use crate::screens::modes::{Flow, Sequence};
use crate::Native;
use crate::actors::families::effects_creators::{end_screen, or_rest};
use crate::world::objects::{pools, Obj};
use crate::services::video::{load_grid, clear_layer, Layer, Grid};
use crate::screens::obj;

/// Bonus screen variables (below A5).
mod v {
    use crate::world::ram::{Byte, Word};
    
    /// Seconds left (byte; word during the final fade), frames of the second, total duration (word).
    pub const SECONDS: Byte = Byte(0xff_2a74);
    pub const SECONDS_WORD: Word = Word(0xff_2a74);
    /// The cameras set aside during the bonus (four words).
    pub const SAVED: u32 = 0xff_2a6c;
    pub const FRAMES: Byte = Byte(0xff_2a75);
    pub const DURATION: Word = Word(0xff_2a76);
    /// The object that drives the game (word), its five sorted objects (words), the effect that displays the countdown.
    pub const LEADER: Word = Word(0xff_2a78);
    pub const OBJECTS: u32 = 0xff_2a7a;
    pub const COUNTER: Word = Word(0xff_2a9c);
    /// Last value seen of the leader's `+0x17` byte.
    pub const LAST_SCORE: Byte = Byte(0xff_2a59);
}

/// Work RAM variables (offsets from A5) touched by the bonus.
mod a5 {
    pub const SCENE: u32 = 0x34;
    /// Main camera: X, Y, copies of X and Y, and other fields reset to zero; same for the alternate one.
    use crate::hw::mem::A5;
    use crate::screens::vars as sv;
    pub const CAM_X: u32 = sv::CAMERA_X.0 - A5;
    pub const CAM_Y: u32 = sv::CAMERA_Y.0 - A5;
    pub const ALT_X: u32 = sv::CAMERA_2_X.0 - A5;
    pub const ALT_Y: u32 = sv::CAMERA_2_Y.0 - A5;
}

/// ROM tables and subroutines.
mod rom {
    pub const DISPATCH: crate::world::screen_tables::Rom = crate::world::screen_tables::Rom::at(0xf5946);
    /// Bonus terrain (rectangles of cells), according to `A5+0x70`.
    pub const BACKDROP: crate::world::screen_tables::Rom = crate::world::screen_tables::Rom::at(0x3f7e0e);
    pub const BACKDROP_ALT: crate::world::screen_tables::Rom = crate::world::screen_tables::Rom::at(0x3f6c08);
}
mod sub {
    pub const MUSIC: crate::services::sound::Sound = crate::services::sound::CMD_03;
}

/// Resumes (original addresses following a yield).
mod at {
    pub const FADED_OUT: crate::world::data::RomAddr = crate::world::data::RomAddr(0xf5fcc);
    pub const STARTED: crate::world::data::RomAddr = crate::world::data::RomAddr(0xf62a8);
}


fn sub_step(x: &mut Ctx) -> u8 {
    EXIT_SUB_STEP.get(x.b)
}
fn next_sub_step(x: &mut Ctx) {
    let s = sub_step(x).wrapping_add(2);
    EXIT_SUB_STEP.set(x.b, s);
}
/// End of the state: next state of the sequence (`+2` += 4), sub-state 0.
fn leave(x: &mut Ctx) {
    let s = EXIT_STEP.get(x.b).wrapping_add(4);
    EXIT_STEP.set(x.b, s);
    EXIT_SUB_STEP.set(x.b, 0);
}

/// Original dispatch (`moveq #0,D0 ; move.b 3(A0),D0 ; move.w table(PC,D0),D1`): the sub-step.
pub fn dispatch(x: &mut Ctx, _table: u32) -> u8 {
    sub_step(x)
}

/// Fade to black or from black (delay 4, all pages, 15 steps); the routine cedes control and returns to
/// `resume`.
pub fn fade(x: &mut Ctx, routine: fn(&mut Ctx) -> Flow, resume: impl Into<crate::core::tasks::table::Next>) -> Flow {
    fading::STEP.set(x.b, 15);
    fading::DELAY.set(x.b, 4);
    fading::PAGES.set(x.b, 15);
    x.subroutine(routine, resume)
}

/// `movem.l A0,-(SP) ; jsr 0x1de96 ; movem.l (SP)+,A0`: the sprite list, A0 preserved.
pub fn sprites_keep_a0(x: &mut Ctx) {
    crate::services::sprites::build_in_game(x.b);
}

/// A state of 0xf5938.
pub fn run(x: &mut Ctx) -> Flow {
    match dispatch(x, rom::DISPATCH.addr()) {
        0 => start(x),
        2 => set_up(x),
        4 => objects(x),
        6 => {
            countdown(x);
            Flow::Done
        }
        8 => end(x),
        10 => {
            if tick16(x, v::SECONDS_WORD) {
                next_sub_step(x);
                for k in 0..4 {
                    let w = Word(v::SAVED + 2 * k).get(x.b);
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
fn tick16(x: &mut Ctx, at: Word) -> bool {
    let t = at.get(x.b).wrapping_sub(1);
    at.set(x.b, t);
    t == 0
}

/// The players in the game (present, state < 4) who won the bonus (`+0x118` and `+0x11c` at -1).
fn qualified(x: &mut Ctx) -> u8 {
    let mut n = 0u8;
    for k in 0..4 {
        let p = Obj(PLAYERS + k * PLAYER_STRIDE);
        let items = crate::screens::hud::player::ITEMS;
        if p.u8(x.b, Obj::ACTIVE) != 0
            && (p.u8(x.b, Obj::STATE) as i8) < 4
            && p.u32(x.b, items) == u32::MAX
            && p.u32(x.b, items + 4) == u32::MAX
        {
            n += 1;
        }
    }
    n
}

/// Sub-state 0: with no qualified player, the state is skipped; otherwise fade to black.
fn start(x: &mut Ctx) -> Flow {
    if qualified(x) == 0 {
        let s = EXIT_STEP.get(x.b).wrapping_add(4);
        EXIT_STEP.set(x.b, s);
        return Flow::Done;
    }
    fade(x, crate::core::tasks::fading::descent, Step::FadedOut)
}

crate::steps! {
    /// Steps (resume points) of the state machine.
    pub enum Step {
        /// The cameras are set aside and placed on the bonus terrain.
        FadedOut = at::FADED_OUT.0 => |x| {
            next_sub_step(x);
            let main = (x.v16(a5::CAM_X), x.v16(a5::CAM_Y));
            Word(v::SAVED).set(x.b, main.0);
            Word(v::SAVED + 2).set(x.b, main.1);
            x.set_v8(0x53c, 0);
            let alt = (x.v16(a5::ALT_X), x.v16(a5::ALT_Y));
            Word(v::SAVED + 4).set(x.b, alt.0);
            Word(v::SAVED + 6).set(x.b, alt.1);
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
        SetUp = 0xf60ca => |x| {
            next_sub_step(x);
            for (from, to) in PAGES {
                copy_page(x.b, from, to);
            }
            crate::services::palette::load_palette_page(x.b, 2, 85);
            crate::services::palette::load_palette_page(x.b, 3, 85);
            crate::services::palette::load_palette_set(x.b, 2);
            Flow::Done
        },
        /// The bonus title (SET object type 0, variant 69).
        Started = at::STARTED.0 => |x| {
            if let Some(o) = pools::SET.alloc(x.b) {
                for (off, v) in [(Obj::ACTIVE, 1), (Obj::KIND, 0), (Obj::VARIANT, 69), (obj::INDEX, 3)] {
                    o.set_u8(x.b, off, v);
                }
            }
            Flow::Done
        },
        /// The bonus screen is cleared; outside scene 1 the state is finished, in scene 1 the palette pages come back
        /// after four frames.
        Ended = 0xf6380 => |x| {
            next_sub_step(x);
            screen::clear_bonus_frame(x.b);
            let counter = v::COUNTER.get(x.b) as i16 as i32 as u32 & 0xff_ffff;
            Obj(counter).set_u32(x.b, Obj::STATE, 0x0600_0000);
            crate::core::dispatch::EFFCT.update_no_next(x.b);
            crate::services::sprites::build_in_game(x.b);
            if x.v8(a5::SCENE) != 1 {
                x.set_v8(0x3c, 0xff);
                clear_layer(x.b, Layer::Two);
                clear_layer(x.b, Layer::Three);
                crate::services::video::clear_objects(x.b);
                leave(x);
                return Flow::Done;
            }
            x.set_v8(0x3c, 1);
            v::SECONDS_WORD.set(x.b, 4);
            x.and16(0x14, 0xfff3);
            x.wait(4, Step::Scene1Waited)
        },
        Scene1Waited = 0xf63ee => |x| {
            for (to, from) in PAGES {
                copy_page(x.b, from, to);
            }
            Flow::Done
        },
        /// Last frame: fade from black, then the next state of the sequence.
        LastFrame = 0xf6470 => |x| fade(x, crate::core::tasks::fading::rise, Step::FadedIn),
        FadedIn = 0xf6488 => |x| {
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
        crate::world::objects::lists::unlink_all(x.b);
        crate::services::sprites::clear_lists(x.b);
        for pool in crate::world::objects::pools::POOLS {
            pool.reset(x.b);
        }
        clear_layer(x.b, Layer::Two);
        clear_layer(x.b, Layer::Three);
        screen::clear_text(x.b);
        0xff
    } else {
        rom::DISPATCH.pc_word(x.b, 2) as u8
    };
    // Original glitch: allocation failures are not tested (the A1 left over receives the writes)
    let created = end_screen(x.b, 3, setting);
    let counter = or_rest(x.b, created).0;
    v::COUNTER.set(x.b, counter as u16);
    Obj(counter).set_u8(x.b, SHOWN, 20);
    for k in 0..4u8 {
        let created = end_screen(x.b, 8, 3 - k);
        let o = or_rest(x.b, created);
        o.set_u8(x.b, obj::INDEX, k);
    }
    crate::services::sound::play_global(x.b, sub::MUSIC);
    x.wait(4, Step::SetUp)
}

/// Bonus palette pages: page 0x91c400 goes to 0x910000, and so on.
const PAGES: [(u32, u32); 4] = [
    (PALETTES.0 + 0x400, SCROLL1.base),
    (PALETTES.0 + 0x800, SCROLL1.base + 0x400),
    (PALETTES.0 + 0xc00, SCROLL1.base + 0x800),
    (PALETTES.0, SCROLL1.base + 0xc00),
];
/// Fields of the bonus objects: value shown by the countdown effect, link of the four player objects to the leader,
/// their player, the leader's goal, score and request (h), its two lists of objects.
const SHOWN: u32 = 0x42;
const LEADER_LINK: u32 = 0x26;
const PLAYER_LINK: u32 = 0x22;
const REQUEST: u32 = 0x14;
const GOAL: u32 = 0x16;
const SCORE: u32 = 0x17;
const LISTS: u32 = 0x18;

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
        *o = crate::actors::families::creators::set_5_pure(x.b).unwrap_or_else(|a1| a1).0 as u16;
    }
    // original bubble sort on sign-extended addresses (cmpa.w)
    objs.sort_by_key(|&w| w as i16 as i32 as u32);
    for (k, &o) in objs.iter().enumerate() {
        Word(v::OBJECTS + 2 * k as u32).set(x.b, o);
    }
    let addr = |w: u16| w as i16 as i32 as u32 & 0xff_ffff;
    let leader = addr(objs[4]);
    for k in 0..4 {
        let o = addr(objs[k]);
        Obj(o).set_u8(x.b, obj::INDEX, k as u8);
        Obj(o).set_u16(x.b, LEADER_LINK, objs[4]);
        Obj(o).set_u16(x.b, PLAYER_LINK, (PLAYERS + k as u32 * PLAYER_STRIDE) as u16);
    }
    let leader = Obj(leader);
    leader.set_u8(x.b, Obj::VARIANT, 0);
    let (first, second) = (Long(v::OBJECTS).get(x.b), Long(v::OBJECTS + 4).get(x.b));
    leader.set_u32(x.b, LISTS, first);
    leader.set_u32(x.b, LISTS + 4, second);
    v::LEADER.set(x.b, objs[4]);
    let score = leader.u8(x.b, SCORE);
    v::LAST_SCORE.set(x.b, score);
    next_sub_step(x);
    v::SECONDS.set(x.b, 20);
    v::FRAMES.set(x.b, 60);
    v::DURATION.set(x.b, 5400);
    x.set_v8(0x5f, 0);
    backdrop(x);
    show_seconds(x);
    x.set_v16(0x14, 0x12ce);
    let f = x.v8(0x13) & !1;
    x.set_v8(0x13, f);
    for (grid, n, destination) in [(Grid::Layer2, 0x205, SCROLL2.0 + 0x3000), (Grid::Layer3, 0x172, SCROLL3.0 + 0x1800)] {
        let end = load_grid(x.b, grid, n, destination);
        load_grid(x.b, grid, n + 1, end.destination);
    }
    x.set_v8(0x3c, 2);
    crate::services::palette::load_palette_page(x.b, 1, 1);
    fade(x, crate::core::tasks::fading::rise, Step::Started)
}

/// 0xf67fa: the bonus terrain, rectangle of cells read from ROM (width, height, then the cells by columns).
fn backdrop(x: &mut Ctx) {
    let mut src = if x.v16(0x70) != 0 { rom::BACKDROP_ALT.addr() } else { rom::BACKDROP.addr() };
    let (rows, columns) = (tables::word(x.b, src) as u32, tables::word(x.b, src + 2) as u32);
    src += 4;
    for col in 0..columns {
        for row in 0..rows {
            let v = tables::long(x.b, src);
            TEXT.at(0x14).offset(col * TEXT.column + TEXT.row * row).set_long(x.b, v);
            src += 4;
        }
    }
}

/// 0xf6772: the countdown effect displays the seconds left.
fn show_seconds(x: &mut Ctx) {
    let counter = v::COUNTER.get(x.b) as i16 as i32 as u32 & 0xff_ffff;
    let s = v::SECONDS.get(x.b);
    Obj(counter).set_u8(x.b, SHOWN, s);
}

/// Sub-state 6: the countdown. It restarts from 20 seconds when the leader's score (`+0x17`) changes, drops to
/// 10 when the leader asks for it (`+0x14`), and the game ends after the 90 seconds, at zero, or when the score
/// reaches the goal (`+0x16`).
fn countdown(x: &mut Ctx) {
    let leader = Obj(v::LEADER.get(x.b) as i16 as i32 as u32 & 0xff_ffff);
    if (v::SECONDS.get(x.b) as i8) < 10 && leader.u8(x.b, REQUEST) != 0 {
        v::SECONDS.set(x.b, 10);
        v::FRAMES.set(x.b, 60);
        return show_seconds(x);
    }
    let score = leader.u8(x.b, SCORE);
    if score != leader.u8(x.b, GOAL) {
        if score != v::LAST_SCORE.get(x.b) {
            v::LAST_SCORE.set(x.b, score);
            v::SECONDS.set(x.b, 20);
            v::FRAMES.set(x.b, 60);
            return show_seconds(x);
        }
        let left = v::DURATION.get(x.b).wrapping_sub(1);
        v::DURATION.set(x.b, left);
        if left as i16 > 0 {
            let f = v::FRAMES.get(x.b).wrapping_sub(1);
            v::FRAMES.set(x.b, f);
            if f != 0 {
                return;
            }
            let s = v::SECONDS.get(x.b).wrapping_sub(1);
            v::SECONDS.set(x.b, s);
            if (s as i8) >= 0 {
                v::FRAMES.set(x.b, 60);
                return show_seconds(x);
            }
        } else {
            v::SECONDS_WORD.set(x.b, 1);
            show_seconds(x);
        }
    }
    leader.set_u32(x.b, Obj::STATE, 0x0600_0000);
    next_sub_step(x);
    v::SECONDS_WORD.set(x.b, 30);
}

/// Sub-state 8: 30 frames, then fade to black.
fn end(x: &mut Ctx) -> Flow {
    if !tick16(x, v::SECONDS_WORD) {
        return Flow::Done;
    }
    fade(x, crate::core::tasks::fading::descent, Step::Ended)
}

pub const NATIVES: &[Native] = &[
    
];

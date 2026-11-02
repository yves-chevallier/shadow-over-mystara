//! Transition scripts between two scenes (table `0xf81fa`, sub-state 2 of the shared state `0xf7ecc`).
//!
//! After the fade to black of the scene exit, the script specific to the scene that ends (scene 105
//! takes that of 69) does, one frame after another, the interlude between two levels: fixed scenery,
//! dialogues (SET objects `0xc8312`, whose byte `+0x23` gives the chosen answer), animated system
//! objects, scrolls, fades; then it sets the next scene and hands control back to the scene exit.
//!
//! Variables (`A5-0x5520`, A6 in the original): `+0` state (step of 2), `+1` sub-state, `+2` timer (word),
//! `+4` open message (word) or chosen scene (byte), `+6` flag or choice, `+8` signal exchanged with the
//! system objects of the script (they receive the address of the variables at `+0x4c`).
//!
//! The original code yields control to the scheduler at almost every step (`jsr 0xa40`, fades `0x144c`/`0x1460`,
//! `0x1ccc2`...); as with the scene exit, each resume is a native placed at the address where the
//! original resumes (the address following the `jsr`/`bsr`), and the subroutines that yield are called as
//! in the original: return address pushed (`bsr`, [`Ret::At`]) or tail call (`bra`, [`Ret::Caller`],
//! return to the scene exit dispatch).

pub(crate) mod early;
pub(crate) mod late;

use super::Ctx;
use crate::scene::fading;
use crate::video::{load_grid, clear_layer, Layer, Grid};
use crate::mem::{rd16, rd32, rd8, wr16, wr32, wr8, A5};
use crate::modes::{Flow, Sequence};
use crate::objects::Obj;
use crate::tasks::table::Next;
use crate::Native;

/// Script variables (A6).
pub const VARS: u32 = A5 - 0x5520;
mod var {
    pub const STATE: u32 = 0;
    pub const SUB: u32 = 1;
    pub const TIMER: u32 = 2;
    pub const MESSAGE: u32 = 4;
    pub const FLAG: u32 = 6;
    pub const SIGNAL: u32 = 8;
}

/// Work RAM variables (offsets from A5).
mod a5 {
    /// Displayed layers (word: bits 2-3 planes 2 and 3, bit 1 plane 1).
    pub const LAYERS: u32 = 0x14;
    pub const SCENE: u32 = 0x34;
    /// Scene clock (BCD word, minutes and seconds); non-zero: time remaining.
    pub const CLOCK: u32 = 0x38;
    /// Fade: pages, steps left, delay, target palettes (longs).
    /// Cameras: position (X, Y) and their copies (`+8`).
    pub const CAM1_X: u32 = 0x506;
    pub const CAM1_Y: u32 = 0x50a;
    pub const CAM2_X: u32 = 0x556;
    pub const CAM2_Y: u32 = 0x55a;
    pub const COPY: u32 = 8;
    /// Scrolling text (h): pointer to the text, then two header words.
    pub const SCROLL_TEXT: u32 = 0x5c6;
    /// Region (word, 2 and above), which chooses the language of the texts.
    pub const REGION: u32 = 0x72;
    pub const PLAYERS: u32 = 0x5de;
}

/// Palette pages: all of them, or those of the scene (0, 2 and 3).
const ALL_PAGES: u8 = 15;
const SCENE_PAGES: u8 = 13;

/// Tile block grids: layer 2 (0x26d4) and layer 3 (0x2766).
const TB: Grid = Grid::Layer2;
const TB2: Grid = Grid::Layer3;

/// Scenery planes: plane 1 (characters), plane 2 and plane 3, in GFX RAM.
const SCROLL_2: u32 = 0x917000;
const SCROLL_3: u32 = 0x91b800;

/// Continuation of a subroutine that yields: return to an address of the script (`bsr`), or to the caller of the script
/// (`bra`, the scene exit dispatch).
#[derive(Clone, Copy)]
enum Ret {
    At(Next),
    Caller,
}
use Ret::{At, Caller};

/// Hands control back (the task yielded) if the wait is not over.
macro_rules! or_yield {
    ($e:expr) => {
        if !$e {
            return Flow::Yield;
        }
    };
}
use or_yield;

// ---------------------------------------------------------------------------------------------
// Variables

fn get8(x: &mut Ctx, off: u32) -> u8 {
    rd8(x.b, VARS + off)
}
fn set8(x: &mut Ctx, off: u32, v: u8) {
    wr8(x.b, VARS + off, v)
}
fn get16(x: &mut Ctx, off: u32) -> u16 {
    rd16(x.b, VARS + off)
}
fn set16(x: &mut Ctx, off: u32, v: u16) {
    wr16(x.b, VARS + off, v)
}
/// Next state.
fn next(x: &mut Ctx) {
    let s = get8(x, var::STATE).wrapping_add(2);
    set8(x, var::STATE, s);
}
fn next_sub(x: &mut Ctx) {
    let s = get8(x, var::SUB).wrapping_add(2);
    set8(x, var::SUB, s);
}
/// Subtracts one frame from the timer; true when it reaches zero.
fn tick(x: &mut Ctx) -> bool {
    let t = get16(x, var::TIMER).wrapping_sub(1);
    set16(x, var::TIMER, t);
    t == 0
}

/// Dispatch `moveq #0,D0 ; move.b (off,A6),D0 ; move.w (table,PC,D0.w),D1`: returns the state (the table is only
/// that of the original).
fn dispatch(x: &mut Ctx, off: u32, _table: u32) -> u8 {
    get8(x, off)
}

fn layers_or(x: &mut Ctx, bits: u16) {
    let v = x.v16(a5::LAYERS) | bits;
    x.set_v16(a5::LAYERS, v);
}
fn layers_and(x: &mut Ctx, mask: u16) {
    let v = x.v16(a5::LAYERS) & mask;
    x.set_v16(a5::LAYERS, v);
}

/// Tile block `n` of the grid at `destination`; returns the following cell (A1 of the original: the next block
/// of a series is written there).
fn tiles(x: &mut Ctx, grid: Grid, n: u16, destination: u32) -> u32 {
    load_grid(x.b, grid, n, destination).destination
}

/// Sound command 0xff07 of line `line` (0xe0a82, without lock).
fn sound(x: &mut Ctx, line: u16) {
    crate::sound::service(x.b, crate::sound::Service::Ff07 { lock: 0, line });
}

/// Camera: position and copy.
fn set_camera(x: &mut Ctx, off: u32, v: u16) {
    x.set_v16(off, v);
    x.set_v16(off + a5::COPY, v);
}

// ---------------------------------------------------------------------------------------------
// Script subroutines (0xfa0e6-0xfa294)

/// 0xfa0e6: end of the script, next scene `next`; the scene exit moves to its next state, one frame yielded.
fn finish(x: &mut Ctx, next_scene: u8) -> Flow {
    x.set_v8(a5::SCENE, next_scene);
    end(x)
}

/// The scene exit moves to its next state (`+2` of the block designated by A0), script reset, one frame yielded.
fn end(x: &mut Ctx) -> Flow {
    let a0 = x.c.a[0];
    let step = rd8(x.b, a0 + 2).wrapping_add(4);
    wr8(x.b, a0 + 2, step);
    set8(x, var::STATE, 0);
    wr8(x.b, a0 + 3, 0);
    layers_or(x, 0xe);
    yield_frames(x, 1, Caller)
}

/// Cedes `n` frames (`moveq #n,D0 ; jsr/jmp 0xa40`).
fn yield_frames(x: &mut Ctx, n: u32, ret: Ret) -> Flow {
    x.c.d[0] = n;
    goto(x, crate::tasks::wait_then_return, ret)
}

/// Jumps to a subroutine that yields (`bsr`, or `bra` which returns to the caller of the script), storing the
/// continuation if applicable.
fn goto(x: &mut Ctx, routine: fn(&mut Ctx) -> Flow, ret: Ret) -> Flow {
    if let At(r) = ret {
        x.push_return(r);
    }
    routine(x)
}

/// 0xfa102: displayed layers, one frame, then opening fade of all the pages (resume 0xfa110).
fn show_and_fade_in(x: &mut Ctx, ret: Ret) -> Flow {
    if let At(r) = ret {
        x.push_return(r);
    }
    layers_or(x, 0xe);
    x.wait(1, Step::FadeInAfterFrame)
}
crate::steps! {
    /// Steps (resume points) of the state machine.
    pub enum Step {
        FadeInAfterFrame = 0xfa110 => |x| fade(x, ALL_PAGES, crate::tasks::fading::rise, Caller),
        SpritesAfterFrame = 0xfa1ca => |x| {
            crate::sprites::build_68k(x.c, x.b);
            Flow::Done
        },
    }
}

/// Fade (delay 3) of the `pages` to black (0x1460) or from black (0x144c): 0xfa122, 0xfa134, 0xfa146.
fn fade(x: &mut Ctx, pages: u8, routine: fn(&mut Ctx) -> Flow, ret: Ret) -> Flow {
    fading::DELAY.set(x.b, 3);
    fading::PAGES.set(x.b, pages);
    goto(x, routine, ret)
}

/// Fade with given delay and pages, without waiting for its end: fade task `task` (0xfa1fa, 0xfa214).
fn fade_task(x: &mut Ctx, delay: u16, pages: u8, task: u32) {
    fading::DELAY.set(x.b, delay);
    fading::PAGES.set(x.b, pages);
    start_task(x, task);
}
fn start_task(x: &mut Ctx, task: u32) {
    crate::kernel::create_task(x.b, 0x10, task);
}

/// 0xfa158: planes 2 and 3 hidden, one frame.
fn hide_layers(x: &mut Ctx, ret: Ret) -> Flow {
    layers_and(x, !0xc);
    yield_frames(x, 1, ret)
}
/// 0xfa166: planes 2 and 3 shown, one frame.
fn show_layers(x: &mut Ctx, ret: Ret) -> Flow {
    layers_or(x, 0xc);
    yield_frames(x, 1, ret)
}
/// 0xfa174: camera 1 at the origin, one frame.
fn home_camera_1(x: &mut Ctx, ret: Ret) -> Flow {
    set_camera(x, a5::CAM1_X, 0);
    set_camera(x, a5::CAM1_Y, 0);
    yield_frames(x, 1, ret)
}
/// 0xfa1ac: both cameras at the origin, one frame.
fn home_cameras(x: &mut Ctx, ret: Ret) -> Flow {
    for off in [a5::CAM1_X, a5::CAM1_Y, a5::CAM2_X, a5::CAM2_Y] {
        set_camera(x, off, 0);
    }
    yield_frames(x, 1, ret)
}

/// 0xfa1c2: one frame, then the sprite list (resume 0xfa1ca), return to `ret`.
fn sprites_next_frame(x: &mut Ctx, ret: impl Into<Next>) -> Flow {
    x.push_return(ret);
    x.wait(1, Step::SpritesAfterFrame)
}

/// 0xfa1e2: world update (A0 and A6 kept).
fn world(x: &mut Ctx) {
    let (a0, a6) = (x.c.a[0], x.c.a[6]);
    // sonde `rendering` (voir `dispatch::Class::update_no_next`)
    crate::diag::dead_regs(x.c, "handback", crate::diag::TASK_REGS);
    let flow = crate::game::update_world(x.c, x.b);
    assert!(flow == Flow::Done, "transition script: a handler yielded control during the world update");
    (x.c.a[0], x.c.a[6]) = (a0, a6);
}

/// 0xf7fdc: object pools cleared (A0 kept).
fn reset_pools(x: &mut Ctx) {
    for pool in crate::objects::pools::POOLS {
        pool.reset(x.b);
    }
}

/// 0xfa22e: palette `n` of the background pages (0x1a67e), then black on page 2.
fn dark_palette(x: &mut Ctx, n: u16) {
    crate::palette::load_palette_page(x.b, 2, n);
    fading::PAGES.set(x.b, 4);
    crate::palette::no_brightness(x.b);
}

/// 0xfa240: 0x800 bytes cleared at 0x920000 (registers kept).
fn clear_920000(x: &mut Ctx) {
    super::screen::clear_920000(x.b);
}

/// 0xfa270: does a player in the game press a button (`A5+0xc + k`, four high bits)?
fn any_button(x: &mut Ctx) -> bool {
    let mut pressed = 0u8;
    for k in (0..4u32).rev() {
        let player = A5 + a5::PLAYERS + k * 0x200;
        if rd8(x.b, player) != 0 {
            pressed |= rd8(x.b, A5 + 0xc + k);
        }
    }
    pressed & 0xf0 != 0
}

// ---------------------------------------------------------------------------------------------
// Objects: messages, system objects, SET objects

/// Le message ouvert (`movea.w 4(A6),A1`).
fn message_obj(x: &mut Ctx) -> Obj {
    Obj(get16(x, var::MESSAGE) as i16 as i32 as u32)
}
/// Is the open message finished (byte `+0x22`)?
fn message_done(x: &mut Ctx) -> bool {
    let o = message_obj(x);
    rd8(x.b, o.0 + 0x22) != 0
}
/// Answer chosen in the open message (byte `+0x23`), which indexes a ROM byte table.
fn answer(x: &mut Ctx, table: u32) -> u8 {
    let o = message_obj(x);
    let k = rd8(x.b, o.0 + 0x23);
    x.prog8(table + k as u32)
}

/// Opens message `n` (0xc8312, stored at `+4`); true if the object was created.
fn try_message(x: &mut Ctx, n: u8) -> bool {
    let Some(o) = crate::families::creators::scene_set(x.b, n) else { return false };
    store_message(x, o);
    true
}
fn store_message(x: &mut Ctx, o: Obj) {
    set16(x, var::MESSAGE, o.0 as u16);
}
/// Message `n` whose failure is not tested. Original glitch: the A1 left by the allocation is stored.
fn message_untested(x: &mut Ctx, n: u8) -> Obj {
    crate::families::creators::scene_set_or_rest(x.b, n)
}
/// Loop `moveq #n,D0 ; jsr 0xc8312 ; bcc ; moveq #1,D0 ; jsr 0xa40 ; bra`: true if the message is open;
/// otherwise the task yields one frame and will resume at `retry`.
fn message(x: &mut Ctx, n: u8, retry: impl Into<Next>) -> bool {
    try_message(x, n) || retry_next_frame(x, retry)
}
fn retry_next_frame(x: &mut Ctx, retry: impl Into<Next>) -> bool {
    yield_frames(x, 1, At(retry.into()));
    false
}

/// System object of variant `n` (0xddeaa) that receives the address of the variables; otherwise yields and resumes
/// at `retry`.
fn system_object(x: &mut Ctx, n: u8, retry: impl Into<Next>) -> Option<Obj> {
    let Ok(o) = crate::effects_aids::scenery_6(x.b, n) else {
        retry_next_frame(x, retry);
        return None;
    };
    Some(attach(x, o))
}
/// System object of variant `n` whose failure is not tested. Original glitch: the A1 left by the allocation receives
/// the address of the variables.
fn system_object_untested(x: &mut Ctx, n: u8) -> Obj {
    let o = crate::effects_aids::scenery_6(x.b, n).unwrap_or_else(|a1| a1);
    attach(x, o)
}
/// SET object of type `n` (0xce00a) attached to the variables; `None` if the pool is full.
fn try_set_object(x: &mut Ctx, n: u8) -> Option<Obj> {
    let o = crate::families::creators::set_15_pure(x.b, n).ok()?;
    Some(attach(x, o))
}
fn set_object(x: &mut Ctx, n: u8, retry: impl Into<Next>) -> bool {
    try_set_object(x, n).is_some() || retry_next_frame(x, retry)
}
/// `move.l A6,0x4c(A1)`: the created object knows the script variables (A6 of the original).
fn attach(x: &mut Ctx, o: Obj) -> Obj {
    wr32(x.b, o.0 + 0x4c, VARS);
    o
}

/// Scrolling text of the region (pointer table `table` PC-relative, indexed by `A5+0x72`): header of two
/// words, then the text.
fn scroll_text(x: &mut Ctx, table: u32) {
    let k = x.v16(a5::REGION).wrapping_sub(2).wrapping_mul(2);
    let mut a4 = crate::mem::prog32(x.b, table + (k as i16 as i32 as u32));
    for off in [a5::SCROLL_TEXT + 4, a5::SCROLL_TEXT + 6] {
        let w = rd16(x.b, a4);
        x.set_v16(off, w);
        a4 += 2;
    }
    x.set_v32(a5::SCROLL_TEXT, a4);
}

/// Copy of 0x400 bytes of palettes (0xf9e38, `movem.l` by eight longs).
fn copy_palettes(x: &mut Ctx, from: u32, to: u32) {
    for i in (0..0x400).step_by(4) {
        let v = rd32(x.b, from + i);
        wr32(x.b, to + i, v);
    }
}

// ---------------------------------------------------------------------------------------------
// Dispatch by scene

/// Script of the scene; hands control back to the caller (`Flow::Done`) or yields.
pub fn run(x: &mut Ctx, scene: u32) -> Flow {
    match scene {
        1 => early::scene_1(x),
        2 => early::scene_2(x),
        4 | 10 => early::scene_4(x),
        5 | 14 | 20 => early::scene_5(x),
        11 => early::scene_11(x),
        13 => early::scene_13(x),
        27 => late::scene_27(x),
        31 => late::scene_31(x),
        33 => late::scene_33(x),
        32 | 34 | 35 | 67 => late::scene_32(x),
        38 | 40 | 42 | 44 => late::scene_38(x),
        46 => late::scene_46(x),
        69 => late::finale(x),
        // no script: the next scene is the low byte of D0 from the dispatch (twice the scene)
        _ => finish(x, (2 * scene) as u8),
    }
}

const COMMON: &[Native] = &[];

pub fn natives() -> impl Iterator<Item = &'static Native> {
    COMMON.iter().chain(early::NATIVES).chain(late::NATIVES)
}

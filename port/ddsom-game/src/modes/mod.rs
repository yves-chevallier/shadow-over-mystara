//! Mode machine (task 0 outside a game): startup, attract screens, title. See `docs/modes.md`.
//!
//! Outside a game, task 0 runs the loop at `0x133fe`: on each turn it calls the *mode* handler
//! (`FF0E70`: 0 initialisation, 2 execution), which dispatches on the *sub-mode* (`FF0E71`,
//! step of 2); the turn ends with a one-frame yield to the scheduler. A sub-mode handler
//! is itself a state machine (`FF0E72`) that may cede control in the middle of its work
//! (waiting for a fade, a frame, a load): each handler is split at the yield points,
//! and the resume is a Rust step (`steps!`, the address the original resumes at being no more
//! than the name of the step; see [`Flow`]).

pub mod attract;
pub mod boot;
pub mod demo;
pub mod ending;
pub mod gameover;
pub mod ranking;
pub mod select;
pub mod title;

use cps2::Cps2Bus;

use crate::mem::{rd16, rd8, wr16, wr8, A5};
use crate::scene::Ctx;
use crate::tasks::table::Next;
use crate::Native;

/// Mode machine state block (`A0` in the original) and its fields.
pub mod mode {
    pub const BASE: u32 = 0xff_0e70;
    /// Mode: 0 initialisation, 2 execution.
    pub const MODE: u32 = BASE;
    /// Sub-mode (step of 2): the current screen.
    pub const SUB: u32 = BASE + 1;
    /// Sub-mode step.
    pub const STEP: u32 = BASE + 2;
    pub const STEP_2: u32 = BASE + 3;
    /// Screen timer (word, in frames).
    pub const TIMER: u32 = BASE + 8;
    /// Byte != 0: the startup screen has already been shown.
    pub const BOOTED: u32 = BASE + 0x32;
}

/// Addresses of the loop at `0x133fe`.
mod at {
    pub const LOOP: u32 = 0x133fe;
    /// Return of the mode handler: yield one frame.
    pub const AFTER_HANDLER: u32 = 0x1341a;
    /// Resume after the yield: next turn.
    pub const AFTER_WAIT: u32 = 0x13422;
}

/// Engine subroutines called by the screens.
pub mod sub {
    /// VRAM clears: sprites, plane 1, plane 2, plane 3.
    pub const CLEAR_SPRITES: u32 = 0xc58;
    pub const CLEAR_SCROLL_1: u32 = 0xca8;
    pub const CLEAR_SCROLL_2: u32 = 0xcc8;
    pub const CLEAR_SCROLL_3: u32 = 0xce8;
    /// Loads palette n (D0) of the background pages.
    pub const LOAD_PALETTE: u32 = 0x1a67e;
    /// Copies a tile block (D0 = number, A1 = destination) into a plane.
    pub const TILE_BLOCK: u32 = 0x26d4;
}

/// Continuation of a handler: finished (it returns to what follows the current call) or not (it yielded to
/// the scheduler, jumped to another native, or gave up: the task will continue elsewhere).
#[derive(Clone, Copy, PartialEq, Eq)]
pub enum Flow {
    Done,
    Yield,
}

/// Yields and calls that may yield, seen from a screen sequence.
pub trait Sequence {
    /// Yields `frames` frames to the scheduler; the task will resume at `resume` (Rust step, or the address
    /// following the original's `jsr 0xa40`). The sequence must return `Flow::Yield` immediately.
    fn wait(&mut self, frames: u32, resume: impl Into<Next>) -> Flow;
    /// Calls a Rust subroutine that may cede control (fade, black screen: `tasks::fading::descent`,
    /// `video::black_screen::start`...): `resume` is stored in the current calls; if it returns without yielding,
    /// it runs at once.
    fn subroutine(&mut self, sp: impl FnOnce(&mut Ctx) -> Flow, resume: impl Into<Next>) -> Flow;
    /// `bsr`/`jsr` call whose continuation is a resume native: stored in the task's current calls
    /// (`tasks::table::Frame`), it will resume when the callee returns.
    fn push_return(&mut self, next: impl Into<Next>);
    /// The callee returned without going through its `rts` (Rust function that returned `Flow::Done`): the
    /// current call opened by `push_return` is abandoned.
    fn drop_return(&mut self);
    /// Work RAM words and bytes, by offset from A5.
    fn or16(&mut self, off: u32, v: u16);
    fn and16(&mut self, off: u32, v: u16);
}

/// Call of a native subroutine from a Rust step: its continuation is a current call of the task, not a pushed
/// address.
fn push(b: &mut Cps2Bus, next: impl Into<Next>) {
    crate::tasks::table::call(b, next);
}

impl Sequence for Ctx<'_> {
    fn wait(&mut self, frames: u32, resume: impl Into<Next>) -> Flow {
        crate::tasks::cede(self, crate::tasks::Wait::Images(frames), resume)
    }
    fn push_return(&mut self, next: impl Into<Next>) {
        push(self.b, next);
    }
    fn subroutine(&mut self, sp: impl FnOnce(&mut Ctx) -> Flow, resume: impl Into<Next>) -> Flow {
        push(self.b, resume);
        if sp(self) == Flow::Done {
            crate::tasks::return_to_caller(self.b);
        }
        Flow::Yield
    }
    fn drop_return(&mut self) {
        crate::tasks::table::pop_return(self.b);
    }
    fn or16(&mut self, off: u32, v: u16) {
        let w = self.v16(off) | v;
        self.set_v16(off, w);
    }
    fn and16(&mut self, off: u32, v: u16) {
        let w = self.v16(off) & v;
        self.set_v16(off, w);
    }
}

/// Access to the screens' state block (`mode::BASE`) and usual calls.
pub trait Block {
    fn m8(&mut self, off: u32) -> u8;
    fn m16(&mut self, off: u32) -> u16;
    fn set_m8(&mut self, off: u32, v: u8);
    fn set_m16(&mut self, off: u32, v: u16);
    /// Moves to the next screen step (`FF0E72` += 2).
    fn next_step(&mut self);
    /// One round of objects and sprites (`0xfaca2`): SET, EFFCT, SOBJ then sprite list.
    fn render(&mut self);
}

impl Block for Ctx<'_> {
    fn m8(&mut self, off: u32) -> u8 {
        rd8(self.b, mode::BASE + off)
    }
    fn m16(&mut self, off: u32) -> u16 {
        rd16(self.b, mode::BASE + off)
    }
    fn set_m8(&mut self, off: u32, v: u8) {
        wr8(self.b, mode::BASE + off, v)
    }
    fn set_m16(&mut self, off: u32, v: u16) {
        wr16(self.b, mode::BASE + off, v)
    }
    fn next_step(&mut self) {
        let s = self.m8(2).wrapping_add(2);
        self.set_m8(2, s);
    }
    fn render(&mut self) {
        for class in [&crate::dispatch::SET, &crate::dispatch::EFFCT, &crate::dispatch::SOBJ] {
            class.update_no_next(self.b);
        }
        crate::sprites::build_in_game(self.b);
    }
}

// ---------------------------------------------------------------------------------------------
// The loop

crate::steps! {
    /// Steps of the mode machine loop (`0x133fe`).
    pub enum Step {
        /// Loop entry (task 0 restarted after a game, or end of startup).
        Entry = at::LOOP => start,
        /// The handler yielded: one yield frame, then a new turn.
        AfterHandler = at::AFTER_HANDLER => |x| x.wait(1, Step::AfterWait),
        /// After the yield: a new turn.
        AfterWait = at::AFTER_WAIT => turn,
    }
}

/// One turn: calls the mode handler, which returns to [`Step::AfterHandler`].
fn turn(x: &mut Ctx) -> Flow {
    let m = rd8(x.b, mode::MODE);
    push(x.b, Step::AfterHandler);
    match m {
        0 => init(x),
        2 => dispatch(x),
        // two table entries only; `init` alone writes the mode (code-mort.md, batch "CPU removal")
        _ => unreachable!("mode {m:#x}: outside table 0x13424 (modes 0 and 2)"),
    }
}

/// Mode 0 (`0x13428`): moves to mode 2 and chooses the first sub-mode.
fn init(x: &mut Ctx) -> Flow {
    let m = rd8(x.b, mode::MODE).wrapping_add(2);
    wr8(x.b, mode::MODE, m);
    wr8(x.b, mode::SUB, 2);
    if rd8(x.b, mode::BOOTED) != 0 {
        wr8(x.b, mode::SUB, 0x14);
    }
    wr8(x.b, mode::STEP, 0);
    wr8(x.b, mode::STEP_2, 0);
    wr16(x.b, mode::TIMER, 0x12c);
    Flow::Done
}

/// Mode 2 (`0x13450`): dispatches on the sub-mode.
fn dispatch(x: &mut Ctx) -> Flow {
    let sub = rd8(x.b, mode::SUB);
    match attract::screen(x, sub).or_else(|| title::screen(x, sub)).or_else(|| ranking::screen(x, sub)).or_else(|| demo::screen(x, sub)) {
        Some(flow) => flow,
        // the eleven entries of the table (0 to 0x14) are rewritten (code-mort.md, batch "CPU removal")
        None => unreachable!("sub-mode {sub:#x}: outside table 0x1345c"),
    }
}

/// `0x133fe`: loop entry.
pub fn start(x: &mut Ctx) -> Flow {
    wr8(x.b, A5.wrapping_sub(0x7151), 0xff);
    wr8(x.b, A5.wrapping_sub(0x7152), 0);
    turn(x)
}

/// Native routines of the mode machine.
pub fn natives() -> impl Iterator<Item = &'static Native> {
    attract::NATIVES.iter().chain(title::NATIVES).chain(select::NATIVES).chain(ranking::NATIVES).chain(gameover::NATIVES).chain(boot::NATIVES).chain(demo::NATIVES).chain(ending::NATIVES)
}

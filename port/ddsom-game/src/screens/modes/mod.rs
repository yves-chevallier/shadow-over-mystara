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

use crate::world::objects::Obj;
use crate::world::ram::Byte;

/// Bytes set by the entry of the mode loop (`A5-0x7151`, `A5-0x7152`, h).
const MODE_LOOP_FLAGS: Byte = Byte(0xff_0eaf);
const MODE_LOOP_FLAGS_B: Byte = Byte(0xff_0eae);
use crate::world::tasks::{Ctx, Flow, Sequence};
pub use crate::world::tasks::stages::Modes as Step;

/// Mode machine state block (`A0` in the original) and its fields.
pub mod mode {
    use crate::world::ram::{Byte, Word};
    pub const BASE: u32 = 0xff_0e70;
    /// Mode: 0 initialisation, 2 execution.
    pub const MODE: Byte = Byte(BASE);
    /// Sub-mode (step of 2): the current screen.
    pub const SUB: Byte = Byte(BASE + 1);
    /// Sub-mode step (byte; cleared with the next byte as a word).
    pub const STEP: Byte = Byte(BASE + 2);
    pub const STEP_WORD: Word = Word(BASE + 2);
    pub const STEP_2: Byte = Byte(BASE + 3);
    /// Screen timer (word, in frames).
    pub const TIMER: Word = Word(BASE + 8);
    /// Byte != 0: the startup screen has already been shown.
    pub const BOOTED: Byte = Byte(BASE + 0x32);
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
        Obj(mode::BASE).u8(self.b, off)
    }
    fn m16(&mut self, off: u32) -> u16 {
        Obj(mode::BASE).u16(self.b, off)
    }
    fn set_m8(&mut self, off: u32, v: u8) {
        Obj(mode::BASE).set_u8(self.b, off, v)
    }
    fn set_m16(&mut self, off: u32, v: u16) {
        Obj(mode::BASE).set_u16(self.b, off, v)
    }
    fn next_step(&mut self) {
        let s = self.m8(2).wrapping_add(2);
        self.set_m8(2, s);
    }
    fn render(&mut self) {
        for class in [&crate::actors::dispatch::SET, &crate::actors::dispatch::EFFCT, &crate::actors::dispatch::SOBJ] {
            class.update_no_next(self.b);
        }
        crate::services::sprites::build_in_game(self.b);
    }
}

// ---------------------------------------------------------------------------------------------
// The loop

crate::steps! {
    impl Step {
        Entry = 0x133fe => start,
        AfterHandler = 0x1341a => |x| x.wait(1, Step::AfterWait),
        AfterWait = 0x13422 => turn,
    }
}

/// One turn: calls the mode handler, which returns to [`Step::AfterHandler`].
fn turn(x: &mut Ctx) -> Flow {
    let m = mode::MODE.get(x.b);
    crate::world::tasks::table::call(x.b, Step::AfterHandler);
    match m {
        0 => init(x),
        2 => dispatch(x),
        // two table entries only; `init` alone writes the mode (code-mort.md, batch "CPU removal")
        _ => unreachable!("mode {m:#x}: outside table 0x13424 (modes 0 and 2)"),
    }
}

/// Mode 0 (`0x13428`): moves to mode 2 and chooses the first sub-mode.
fn init(x: &mut Ctx) -> Flow {
    let m = mode::MODE.get(x.b).wrapping_add(2);
    mode::MODE.set(x.b, m);
    mode::SUB.set(x.b, 2);
    if mode::BOOTED.get(x.b) != 0 {
        mode::SUB.set(x.b, 0x14);
    }
    mode::STEP.set(x.b, 0);
    mode::STEP_2.set(x.b, 0);
    mode::TIMER.set(x.b, 0x12c);
    Flow::Done
}

/// Mode 2 (`0x13450`): dispatches on the sub-mode.
fn dispatch(x: &mut Ctx) -> Flow {
    let sub = mode::SUB.get(x.b);
    match attract::screen(x, sub).or_else(|| title::screen(x, sub)).or_else(|| ranking::screen(x, sub)).or_else(|| demo::screen(x, sub)) {
        Some(flow) => flow,
        // the eleven entries of the table (0 to 0x14) are rewritten (code-mort.md, batch "CPU removal")
        None => unreachable!("sub-mode {sub:#x}: outside table 0x1345c"),
    }
}

/// `0x133fe`: loop entry.
pub fn start(x: &mut Ctx) -> Flow {
    MODE_LOOP_FLAGS.set(x.b, 0xff);
    MODE_LOOP_FLAGS_B.set(x.b, 0);
    turn(x)
}


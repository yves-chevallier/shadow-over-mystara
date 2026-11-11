//! Game task (`0x1c45a`): the game loop, one step per frame (see `docs/boucle.md` §1).
//!
//! Created at the first start (`credits_task.rs`) in slot 0, it loops endlessly:
//! frame counter, service every 60 frames (`0x2e540`), cession of one frame, then a game step
//! (the phase `A5+0x2c`, table `0x1c502`, followed by the frame end, [`Step::FrameEnd`]). The phases never cede: all
//! the
//! work of a step fits in a native.
//!
//! | Address | Stage |
//! |---|---|
//! | `0x1c45a` | entry ([`Step::Entry`]): phase variables reset to zero, then loop |
//! | `0x1c468` | start of turn ([`Step::Tour`], return from the frame end `0x1c4c4`, [`Step::FrameEnd`]): counters, service, then the cession `0x1c48c`: one wait frame, except for a skipped turn (see `wait`) |
//! | `0x1c4b0` | game step ([`Step::Frame`]): call of the phase |
//!
//! The step starts at `0x1c4b0` = [`crate::GAME_STEP`], where the reference machine caps the number of steps per frame
//! (`cps2::StepCap`, tested on the PC); the game counts its steps itself (`frame::game_step`): a step beyond the
//! cap is held until the start of the next frame, just after VBLANK (`kernel::Point::StepKept`).

use super::{cede, Wait};
use crate::hw::mem::{rd8, wr32, wr8, A5};
use crate::screens::modes::Flow;
use crate::screens::scene::Ctx;

/// Stages of the game task (the game step, `0x1c4b0`, remains a native: see above).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Step {
    Entry,
    /// Start of turn: return from the frame end, or from a phase that pops its own return.
    Tour,
    /// The game step: the current phase. `d4`: the D4 that the previous step left and that the phase re-reads
    /// (leftover of the
    /// dispatch loop, `dispatch::Leftovers::d4`: the pilotless raft, 0xad7b0, adds it to the players'
    /// position); the original keeps it in its registers, which the cession saves and restores.
    Frame { d4: u32 },
    /// Return from the phase: frame end (`game::frame_end`), then the next turn.
    FrameEnd,
}

impl Step {
    /// Address at which the original resumes at this stage.
    pub const fn address(self) -> u32 {
        match self {
            Step::Entry => at::ENTRY,
            Step::Tour => at::TURN,
            Step::Frame { .. } => at::STEP,
            Step::FrameEnd => at::FRAME_END,
        }
    }

    /// The step of the stage.
    pub fn step(self, x: &mut Ctx) -> Flow {
        match self {
            Step::Entry => entry(x),
            Step::Tour => turn(x),
            Step::Frame { d4 } => {
                x.b.leftovers.d4 = d4;
                step(x)
            }
            Step::FrameEnd => {
                crate::core::game::frame_end(x.b);
                Flow::Done
            }
        }
    }
}

/// Work RAM variables (offsets from A5).
mod var {
    /// Phase (step of 4), stage, and following words: reset to zero at entry.
    pub const PHASE: u32 = 0x2c;
    /// Game frame counter.
    pub const FRAMES: u32 = 0x6e;
    /// Development board: play time is counted every second.
    pub const SERVICE: u32 = 0x82;
}

/// Bytes outside the A5 page: service frame counter (A5 - 0x6610), turns skipped without cession
/// (A5 - 0x660f) and "a step took place since VBLANK" (A5 - 0x660e).
const SERVICE_COUNT: u32 = 0xff_19f0;
const FREE_STEPS: u32 = 0xff_19f1;
const STEPPED: u32 = 0xff_19f2;

/// Frames between two services, steps chained without cession after a cession.
const SERVICE_PERIOD: u8 = 60;
const MAX_FREE_STEPS: i8 = 3;
/// The handler of phase 8 in the phase table (`0x1c502 + 8`): the original jumps through A0, which the scene entry
/// rereads.
const PHASE_8_HANDLER: u32 = 0x20c38;

/// Addresses of the original.
mod at {
    pub const ENTRY: u32 = 0x1c45a;
    pub const TURN: u32 = 0x1c468;
    pub const STEP: u32 = crate::GAME_STEP;
    /// Return from a phase: the frame end.
    pub const FRAME_END: u32 = 0x1c4c4;
}

/// Current call of the task: `next` will resume when the callee returns (nothing is pushed on the 68000 stack).
fn push(x: &mut Ctx, next: impl Into<super::table::Next>) {
    super::table::call(x.b, next);
}

/// `0x1c44a`: the game variables `A5+0x2c..0x6b` (phase, stage...) to zero (the registers of the original loop,
/// D0 = 0, D6.w = -1, A0 = end of the area, are reread by nobody).
pub fn clear_variables(b: &mut crate::world::World) {
    for i in 0..16 {
        wr32(b, A5 + var::PHASE + 4 * i, 0);
    }
}

/// `0x1c45a`: phase and stage to zero.
fn entry(x: &mut Ctx) -> Flow {
    wr32(x.b, A5 + var::PHASE, 0);
    wr32(x.b, A5 + var::PHASE + 4, 0);
    turn(x)
}

/// `0x1c468`: one turn. The four words `A5+0x2c..0x33` are those that `0x1c44a` resets to zero.
fn turn(x: &mut Ctx) -> Flow {
    let frames = x.v16(var::FRAMES).wrapping_add(1);
    x.set_v16(var::FRAMES, frames);
    if x.v8(var::SERVICE) != 0 {
        let count = rd8(x.b, SERVICE_COUNT).wrapping_add(1);
        wr8(x.b, SERVICE_COUNT, if count < SERVICE_PERIOD { count } else { 0 });
        if count >= SERVICE_PERIOD {
            crate::services::eeprom::dev_card::play_time(x.b);
        }
    }
    wait(x)
}

/// `0x1c48c`: cedes one frame, unless no step took place since VBLANK (which clears `STEPPED`) and fewer
/// than four turns have been skipped in a row.
fn wait(x: &mut Ctx) -> Flow {
    let stepped = rd8(x.b, STEPPED) != 0;
    let free = rd8(x.b, FREE_STEPS) as i8;
    if !stepped && free <= MAX_FREE_STEPS {
        wr8(x.b, FREE_STEPS, free.wrapping_add(1) as u8);
        return step(x);
    }
    wr8(x.b, FREE_STEPS, 0);
    let d4 = x.b.leftovers.d4;
    cede(x, Wait::Images(1), Step::Frame { d4 })
}

/// `0x1c4b0`: the game step, if it fits in the frame (otherwise it is held until the next one). The phase returns to
/// the frame end, which returns to the start of the turn.
fn step(x: &mut Ctx) -> Flow {
    if !crate::core::frame::game_step() {
        let d4 = x.b.leftovers.d4;
        crate::core::kernel::keep_step(x.b, d4);
        return Flow::Yield;
    }
    wr8(x.b, STEPPED, 0xff);
    push(x, Step::Tour);
    // the original jumps through its table `0x1c502` (handler in A0, phase in D0: reread by nobody)
    let phase = x.v8(var::PHASE);
    push(x, Step::FrameEnd);
    run_handler(x, phase)
}

/// The phase handler (table `0x1c502` of the original, one address per phase, left in A0), called
/// directly: it returns (`Flow::Done`, at the frame end) or has ceded control.
fn run_handler(x: &mut Ctx, phase: u8) -> Flow {
    match phase {
        0 => {
            crate::core::game::phase0(x.b);
            Flow::Done
        }
        4 => crate::screens::modes::select::phase_4(x),
        8 => {
            // A0 = the handler (table 0x1c502), reread by the scene entry (original glitch)
            crate::screens::scene::entry::phase_8(x.b, PHASE_8_HANDLER);
            Flow::Done
        }
        12 => crate::core::game::phase12(x.b),
        16 => match crate::screens::scene::exit::phase_16(x.b) {
            true => Flow::Yield,
            false => Flow::Done,
        },
        20 => crate::screens::modes::gameover::phase_20(x),
        24 => {
            crate::screens::scene::intermission::phase_24(x.b);
            Flow::Done
        }
        28 => crate::screens::modes::ending::phase_28(x),
        // the table has eight entries (code-mort.md, lot "CPU removal")
        _ => unreachable!("phase {phase:#x}: outside the table 0x1c502"),
    }
}


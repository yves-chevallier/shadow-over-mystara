//! Kernel (K1): cooperative task scheduler (0x928) and traps 0 to 7 and 12.
//!
//! The game runs 16 tasks in turn. A task yields through a *cession wrapper* (`jsr 0xa40`,
//! `0xa86`...); the scheduler then picks the first ready task of the table. The task state is the Rust table
//! of [`crate::world::tasks::table`]. The scheduler is a Rust state machine ([`Point`], stored in the table) that the
//! frame loop (`frame`) advances one point at a time, taking the VBLANK interrupt between two
//! points when the interrupt mask allows it (`traps::interrupt_mask`, the level of the original's SR), like the processor
//! between two instructions; it keeps the interrupt window between two slots, but no longer any register, record in
//! RAM (FF0064), stack (neither A7 nor USP: FF0166-FF0E65 is no longer read, probe `DDSOM_POISON=stack`), stack heap
//! (FF005A) or saved registers. It no longer sets a PC: a task is launched by calling the step of its stage.
//!
//! ## Scheduler
//!
//! It walks the slots in order (`Table::exam`; the original's A0 = address of the record, D0.w = remaining slots,
//! from 15 to 0). Each slot is examined by a separate point ([`Point::Exam`]), interrupts
//! masked up to level 6; between two, the window of 0x952 enables them. A VBLANK sets the flag
//! `NEW_FRAME` (FF0164): the walk then restarts from the beginning. A ready task resumes at the step that follows its
//! call of the wrapper, unmasked, without registers: what the step rereads is in its fields (`tasks::resume_step`); a
//! new task starts at its entry, unmasked, without registers: it does not read those that the original
//! gives it (A0 = record, A1 = entry, A4 = stack, D1.w = state; probe `DDSOM_POISON=new`). When it yields, the
//! scheduler resumes at the next slot ([`Point::Return`] 0x984, then [`Point::NextTask`] 0x952).
//!
//! ## Traps
//!
//! The traps that the tasks call (creation, death, cessions) only change the task table: they are in
//! `world::tasks::traps`.

use crate::world::World;

use crate::hw::mem::{rd8, wr8};
use crate::world::tasks::table::{record, table, State, Resume, Table, SLOTS};
use crate::world::tasks::traps::{set_interrupt_mask, Point};

/// Flag set by VBLANK: the scheduler's walk restarts.
const NEW_FRAME: u32 = 0xff0164;

/// Interrupt mask (level of the SR's mask) during the examination of a slot (`move #0x2600,SR`).
const MASK_EXAM: u8 = 6;

/// Address of the original code of point `point` (diagnostics; a task: its current stage).
pub fn address(point: Point, b: &World) -> u32 {
    match point {
        Point::Reset => 0x350,
        Point::ReleaseZ80 => 0x18cc,
        Point::Handshake => 0x18e2,
        Point::BootInit => 0x492,
        Point::Launch => 0x50a,
        Point::Start => 0x928,
        Point::Walk => 0x932,
        Point::Exam => 0x93e,
        Point::NextTask => 0x952,
        Point::Return => 0x984,
        Point::Task => crate::world::tasks::table::read(b).map_or(0, |t| match t.slots[t.current].resume {
            Resume::Entry(task) => task.address(),
            Resume::Step(step) => step.address(),
            #[cfg(feature = "tools")]
            Resume::Address(address) => address,
        }),
        Point::StepKept { .. } => crate::GAME_STEP,
        Point::Blocked(address) => address,
        Point::Interruption => 0x574,
    }
}

/// Executes scheduler point `point` and returns the next one (the startup and task points are
/// executed by `frame`).
pub fn execute(b: &mut World, point: Point) -> Point {
    match point {
        Point::Start => Point::Walk,
        Point::Walk => walk(b),
        Point::Exam => exam(b),
        Point::NextTask => after_cession(b),
        Point::Return => scheduler_return(b),
        _ => unreachable!("point {point:?}: not a scheduler point"),
    }
}

// ---------------------------------------------------------------------------------------------
// Scheduler

/// 0x932: new walk of the table, from slot 0.
fn walk(b: &mut World) -> Point {
    wr8(b, NEW_FRAME, 0);
    table(b).exam = 0;
    Point::Exam
}

/// Moves to the next slot (`dbf`), or restarts when the table is exhausted.
fn next(b: &mut World) -> Point {
    let t = table(b);
    t.exam += 1;
    if t.exam == SLOTS { Point::Walk } else { Point::Exam }
}

/// 0x93e: examination of a slot, with interrupts masked (up to level 6).
fn exam(b: &mut World) -> Point {
    set_interrupt_mask(b, MASK_EXAM);
    if rd8(b, NEW_FRAME) != 0 {
        return Point::Walk;
    }
    let k = table(b).exam;
    if table(b).slots[k].state.castable() {
        launch(b, k)
    } else {
        set_interrupt_mask(b, 0);
        next(b)
    }
}

/// 0x952: the task ceded, the interrupt window opens and the walk continues.
fn after_cession(b: &mut World) -> Point {
    set_interrupt_mask(b, 0);
    next(b)
}

/// 0x960: launches the task of slot `k`: the step of its stage runs. Returns the point where the task left the
/// processor (the scheduler if it ceded). A task starts or resumes without registers, unmasked (user mode): a new
/// task does not read those that the original gives it (A1 = entry, A4 = stack, D1.w = state; probe
/// `DDSOM_POISON=new` before the removal of the registers), and a task that resumes rereads none of the registers
/// saved at its cession: what it rereads is in the fields of its stage (`tasks::resume_step`).
fn launch(b: &mut World, k: usize) -> Point {
    let t = table(b);
    t.current = k;
    t.point = Point::Task;
    t.interrupt_mask = 0;
    let e = &mut t.slots[k];
    let step = match (e.state, e.resume) {
        (State::Fresh, Resume::Entry(task)) => {
            e.state = State::InProgress;
            task.first_step()
        }
        (State::Ready | State::InProgress, Resume::Step(step)) => {
            e.state = State::InProgress;
            crate::core::tasks::resume_step(step, b);
            return table(b).point;
        }
        (state, resume) => unreachable!("task {k} launched in state {state:?} with resumption {resume:?}"),
    };
    crate::core::tasks::execute(step, b);
    table(b).point
}

/// 0x984: a task ceded (or ended); the scheduler resumes at the slot following its own.
fn scheduler_return(b: &mut World) -> Point {
    let t = table(b);
    t.exam = t.current;
    Point::NextTask
}

/// The VBLANK interrupt is taken at point `point` (between two points, the mask allowing it): the kernel remembers
/// where to return. The original pushes PC and SR on the current stack (the task's, or the supervisor's); nothing
/// rereads them there apart from the `rte`.
pub fn interrupt(b: &mut World, point: Point) {
    let t = table(b);
    assert!(t.interrupted.is_none(), "interrupt taken at point {point:?} during an interrupt");
    t.interrupted = Some(point);
    t.point = Point::Interruption;
}

/// `rte`: end of the interrupt, return to the interrupted point.
pub fn interrupt_end(b: &mut World) -> Point {
    table(b).interrupted.take().expect("end of interrupt without an interrupted point")
}

// ---------------------------------------------------------------------------------------------
// Differential test: Rust table against the FF0064 table of an original machine

/// Fields of a record of the original.
mod field {
    pub const STATE: u32 = 0;
    pub const COUNT: u32 = 1;
    pub const PC: u32 = 4;
    pub const USP: u32 = 8;
    pub const STACK: u32 = 0xc;
}

/// Resumes of the original's cession wrappers (`0xa40`/`0xb64`, `0xa86`): they restore the 15 registers
/// saved on the task's stack then return (`rts`) to the resume address.
const ORIGINAL_RESUMES: [u32; 2] = [0xa4c, 0xa8c];
/// Stack heap of the original: top (FF005A) and bottom.
const HEAP: u32 = 0xff_005a;
const HEAP_BOTTOM: u32 = 0xff_01a6;

/// Slot of the main task (game, mode machine), which the freeze task counts down itself.
const MAIN: usize = 0;

/// The two states of the main task differ only by one frame counted down by the freeze task (fast forward).
/// When the game works to the end of the frame (fast forward of a loaded scene), the frame end falls at a
/// moment that depends on the duration of the routines, native or original: shortly before or after a pass of the
/// freeze, or even
/// in the middle (count decremented, state not yet "ready").
fn one_gap_countdown(rust: (State, u8), original: (u8, u8)) -> bool {
    let waits = State::Waiting.byte();
    let next = |(state, count): (u8, u8)| match (state == waits, count.wrapping_sub(1)) {
        (true, 0) => Some((State::Ready.byte(), 0)),
        (true, remainder) => Some((waits, remainder)),
        _ => None,
    };
    let rust = (rust.0.byte(), rust.1);
    // "running" equals "ready"; "waiting" at zero is a freeze countdown interrupted between the count and the state
    let normal = |(state, count): (u8, u8)| {
        let ready = state == State::InProgress.byte() || state == waits && count == 0;
        (if ready { State::Ready.byte() } else { state }, count)
    };
    let (rust, original) = (normal(rust), normal(original));
    rust == original || next(rust) == Some(original) || next(original) == Some(rust)
}

/// Differences between the Rust table and the FF0064 table of the RAM `ram` (FF0000-FFFFFF) of an original machine,
/// at the
/// same frame: state ("ready" and "running" merged: the frame end may fall while a task runs
/// in the original; for the main task, a one-frame gap counted down by the freeze is allowed, see
/// [`one_gap_countdown`]), count, stack, entry of a new task, and stack heap. With `all`, the resume address
/// of the tasks that ceded too (diagnostics): it may differ with no effect (a native sometimes resumes at an identical
/// copy of the original resume code, `0xf32cc` for `0xf5902`).
pub fn gaps(t: &Table, ram: &[u16], all: bool) -> Vec<String> {
    let word = |a: u32| ram[(a as usize & 0xffff) / 2];
    let byte = |a: u32| (word(a & !1) >> if a & 1 == 0 { 8 } else { 0 }) as u8;
    let long = |a: u32| (word(a) as u32) << 16 | word(a + 2) as u32;
    let mut gaps = Vec::new();
    for (k, e) in t.slots.iter().enumerate() {
        let r = record(k);
        let state = byte(r + field::STATE);
        let normal = |o: u8| if o == State::InProgress.byte() { State::Ready.byte() } else { o };
        let mut gap = |what: &str, rust: u32, original: u32| {
            if rust != original {
                gaps.push(format!("task {k}: {what} {rust:x} instead of {original:x}"));
            }
        };
        let count = byte(r + field::COUNT);
        if !(k == MAIN && one_gap_countdown((e.state, e.count), (state, count))) {
            gap("state", normal(e.state.byte()) as u32, normal(state) as u32);
            gap("count", e.count as u32, count as u32);
        }
        if e.state == State::Free || state == State::Free.byte() {
            continue;
        }
        gap("stack", e.stack, long(r + field::STACK));
        let in_progress = e.state == State::InProgress || state == State::InProgress.byte();
        match e.resume {
            Resume::Entry(task) => gap("entry", task.address(), long(r + field::PC)),
            #[cfg(feature = "tools")]
            Resume::Address(address) => gap("entry", address, long(r + field::PC)),
            // the stack of a running task has changed since its cession
            Resume::Step(step) if all && !in_progress => {
                let usp = long(r + field::USP);
                if !ORIGINAL_RESUMES.contains(&long(r + field::PC)) {
                    gap("resume (PC of the record)", step.address(), long(r + field::PC));
                    continue;
                }
                gap("resume", step.address(), long(usp + 60));
            }
            Resume::Step(_) => {}
        }
    }
    let top = if long(HEAP) == 0 { HEAP_BOTTOM } else { long(HEAP) };
    let heap: Vec<u32> = (top..HEAP_BOTTOM).step_by(4).rev().map(long).collect();
    if heap != t.stacks {
        gaps.push(format!("stack heap {:x?} instead of {:x?}", t.stacks, heap));
    }
    gaps
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_frame_counted_down_by_freeze_is_allowed() {
        assert!(one_gap_countdown((State::Waiting, 1), (4, 0)));
        assert!(!one_gap_countdown((State::Waiting, 2), (8, 0)));
        assert!(one_gap_countdown((State::Waiting, 2), (1, 3)));
        assert!(one_gap_countdown((State::Ready, 0), (1, 1)));
        assert!(!one_gap_countdown((State::Waiting, 3), (1, 1)));
        assert!(one_gap_countdown((State::Waiting, 1), (1, 0)));
        assert!(one_gap_countdown((State::Ready, 0), (1, 0)));
    }
}

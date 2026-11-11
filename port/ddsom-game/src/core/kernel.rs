//! Kernel (K1): cooperative task scheduler (0x928) and traps 0 to 7 and 12.
//!
//! The game runs 16 tasks in turn. A task yields through a *cession wrapper* (`jsr 0xa40`,
//! `0xa86`...); the scheduler then picks the first ready task of the table. The task state is the Rust table
//! of [`crate::core::tasks::table`]. The scheduler is a Rust state machine ([`Point`], stored in the table) that the
//! frame loop (`frame`) advances one point at a time, taking the VBLANK interrupt between two
//! points when the interrupt mask allows it ([`interrupt_mask`], the level of the original's SR), like the processor
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
//! ## Wrappers (and traps of the original)
//!
//! | Wrapper | Trap | Effect |
//! |---|---|---|
//! | 0x9b6 | 0 | creates task D0 (offset in the table) with entry A0 |
//! | 0x9f8 | 1 | the current task ends |
//! | 0xa14 | 2 | kills task D0 |
//! | 0xa40, 0xb64 | 3 | cedes D0 frames (0: suspended until trap 6); 0xb64: one frame |
//! | 0xa86 | 4 | cedes, stays ready |
//! | 0xaac | 5 | suspends task D0 |
//! | 0xac8 | 6 | wakes task D0 with count D1 |
//! | 0xae4 | 7 | the current task restarts at A0 |
//! | 0xaf6 | 12 | masks (D0 ≠ 0) or unmasks the interrupts |
//!
//! In the original, a wrapper saves the 15 registers on the task's stack, takes the trap, and its resumption
//! (0x9bc, 0xa4c...) restores them: the wrappers that do not cede thus leave registers and flags intact, and
//! those that cede save the context here in the table. The trap handlers are only reached through their
//! wrappers (`trap #7` is also reached by the original debug menu, 0x2c3e0, which the game does not run): they
//! have no native. The wrappers remain natives for the calls by address and the jumps that remain.

use crate::world::World;

use crate::hw::mem::{rd16, rd8, wr8};
use crate::core::tasks::table::{slot, table, State, Resume, Next, SLOTS};
use crate::core::tasks::Task;

/// RAM addresses.
mod ram {
    /// Flag set by VBLANK: the scheduler's walk restarts.
    pub const NEW_FRAME: u32 = 0xff0164;
    /// Settings word read at VBLANK (DSW): bits 0 to 2, game speed.
    pub const DSW: u32 = 0xffffd0;
}

/// Interrupt masks (level of the SR's mask): during the examination of a slot (`move #0x2600,SR`), and at power-on.
const MASK_EXAM: u8 = 6;
pub const MASK_ALL: u8 = 7;

/// Where the processor is: a startup or scheduler point, or a task. The frame loop
/// (`frame::line`) executes one point at a time; the VBLANK interrupt can be taken between two, if the interrupt
/// mask allows it ([`interrupt_mask`]). Each point keeps the address of the original code it replaces (diagnostics).
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum Point {
    /// Power-on (`0x350`), then the startup steps at the original's time (`startup.rs`).
    #[default]
    Reset,
    ReleaseZ80,
    Handshake,
    BootInit,
    Launch,
    /// `0x928`: the scheduler takes control.
    Start,
    /// `0x932`: new walk of the table.
    Walk,
    /// `0x93e`: examination of a slot (`Table::exam`; the original's A0 = its record, D0.w = slots left).
    Exam,
    /// `0x952`: move to the next slot, after a cession (interrupt window).
    NextTask,
    /// `0x984`: return to the scheduler of a task that ceded (or ended).
    Return,
    /// A task runs (the step of its stage, `tasks::execute`); it leaves by ceding control, ending
    /// or blocking.
    Task,
    /// The game step waits for the next frame (cap on steps per frame, `tasks::game`), with the D4 it will reread
    /// (`tasks::game::Step::Step`).
    StepKept { d4: u32 },
    /// Endless loop of the original (`bra.b *` at this address: blocks of the TEST menu and of the EEPROM, [`block`]):
    /// nothing runs any more, apart from interrupts.
    Blocked(u32),
    /// `0x574`: the VBLANK interrupt is taken ([`interrupt`]); after its processing, the processor returns
    /// (`rte`) to the interrupted point, which the table keeps (`Table::interrupted`) instead of the pushed PC and SR.
    Interruption,
}

impl Point {
    /// Address of the original code (diagnostics; a task: its current stage).
    pub fn address(self, b: &World) -> u32 {
        match self {
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
            Point::Task => crate::core::tasks::table::read(b).map_or(0, |t| match t.slots[t.current].resume {
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

    /// A startup point (before the scheduler takes control).
    pub fn startup(self) -> bool {
        matches!(self, Point::Reset | Point::ReleaseZ80 | Point::Handshake | Point::BootInit | Point::Launch)
    }

    /// The scheduler between two tasks (where the original idles waiting for a ready task).
    pub fn between_two_tasks(self) -> bool {
        matches!(self, Point::Walk | Point::Exam | Point::NextTask)
    }
}

/// The point the processor is at (before the table is created: power-on).
pub fn point(b: &World) -> Point {
    crate::core::tasks::table::read(b).map_or(Point::Reset, |t| t.point)
}

/// Interrupt mask of the processor (level 0 to 7 of the original's SR): power-on masks everything, `boot_init`
/// opens, the scheduler masks up to level 6 while it examines a slot, a task runs unmasked (except the TEST menu's
/// memory test, `trap #12`).
pub fn interrupt_mask(b: &World) -> u8 {
    crate::core::tasks::table::read(b).map_or(MASK_ALL, |t| t.interrupt_mask)
}

/// Sets the interrupt mask (`move #…,SR`, `trap #12`).
pub fn set_interrupt_mask(b: &mut World, mask: u8) {
    table(b).interrupt_mask = mask;
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
    wr8(b, ram::NEW_FRAME, 0);
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
    if rd8(b, ram::NEW_FRAME) != 0 {
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

/// Hands control back to the scheduler after an operation on the current task: the Rust code still running under
/// the cession returns without doing anything (`tasks::has_ceded`).
fn to_scheduler(b: &mut World) {
    table(b).point = Point::Return;
    crate::core::tasks::note_cession();
}

/// The current task enters an endless loop of the original (`bra.b *` at `address`): the processor will only
/// leave it for interrupts. The Rust code in progress returns without doing anything (`tasks::has_ceded`).
pub fn block(b: &mut World, address: u32) {
    table(b).point = Point::Blocked(address);
    crate::core::tasks::note_cession();
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

/// The game step is held until the next frame (cap on steps per frame): the game task stays in progress.
pub fn keep_step(b: &mut World, d4: u32) {
    table(b).point = Point::StepKept { d4 };
}

// ---------------------------------------------------------------------------------------------
// Cessions: the current task hands control back; it will resume at the step `resume`, which carries what it rereads
// (`tasks::resume_step`). The Rust tasks cede directly (`tasks::cede`); the original's wrappers first pop
// their return address (the continuation of a call in progress).

/// Stores the resumption of the current task in the table (its step, without registers: what it will reread is in
/// the step), then hands control back to the scheduler.
fn cede(b: &mut World, state: State, count: Option<u8>, resume: Next) {
    let Next::Step(step) = resume else {
        unreachable!("cession to address {:x}: all resumptions are Rust steps", resume.address())
    };
    // the only masking in the game (TEST menu memory test) does not cede: the resumption returns unmasked
    assert!(interrupt_mask(b) == 0, "cession to {step:?} with interrupts masked");
    table(b).cede(state, count, Resume::Step(step));
    to_scheduler(b);
}

/// Trap 3 (`jsr 0xa40`, D0.b = `frames`): cedes `frames` frames plus the DSW settings delay; 0: suspended until
/// trap 6. The wrapper 0xa40 first calls 0xb40, which according to DSW fills a GFX RAM block of the charge gauge
/// (never, in this game: bit 0x400 of DSW). The registers that the trap leaves (D0.w = state and count, D1.w) and
/// its flags are reread by nobody (probe `DDSOM_POISON=resume`): the task resumes at a Rust stage.
pub fn wait(b: &mut World, frames: u8, resume: impl Into<Next>) {
    let resume = resume.into();
    let setting = !rd16(b, ram::DSW) & 7;
    let count = frames.wrapping_add(setting as u8);
    let state = if count == 0 { State::Suspended } else { State::Waiting };
    cede(b, state, Some(count), resume);
}

/// Trap 4: cedes while staying ready.
pub fn cede_ready(b: &mut World, resume: impl Into<Next>) {
    cede(b, State::Ready, None, resume.into());
}

/// Trap 1: the current task ends and gives back its stack.
pub fn finish(b: &mut World) {
    let t = table(b);
    let k = t.current;
    t.free(k);
    to_scheduler(b);
}

/// 0xae4 (and `trap #7`, 0xae6): the current task restarts from scratch as `task`, same stack.
pub fn restart(b: &mut World, task: Task) {
    table(b).restart(task);
    to_scheduler(b);
}

/// Creates the task of offset `offset` in the table (`0x10 k`), which will start at the entry of `task` (wrapper
/// `0x9b6`: no register or flag changes). A task already there gives back its stack.
pub fn create_task(b: &mut World, offset: u32, task: Task) {
    table(b).create(slot(offset), task);
}

/// Kills the task of offset `offset` (wrapper `0xa14`: no register or flag changes).
pub fn kill_task(b: &mut World, offset: u32) {
    table(b).kill(slot(offset));
}

//! The kernel's traps as the tasks call them (K1, see `core::kernel`): task creation and death, cessions,
//! interrupt mask, blocking. They only change the task table: the scheduler that launches the tasks is in
//! `core::kernel`.
//!
//! | Wrapper | Trap | Effect |
//! |---|---|---|
//! | 0x9b6 | 0 | creates task D0 (offset in the table) with entry A0 ([`create_task`]) |
//! | 0x9f8 | 1 | the current task ends ([`finish`]) |
//! | 0xa14 | 2 | kills task D0 ([`kill_task`]) |
//! | 0xa40, 0xb64 | 3 | cedes D0 frames (0: suspended until trap 6); 0xb64: one frame ([`wait`]) |
//! | 0xa86 | 4 | cedes, stays ready ([`cede_ready`]) |
//! | 0xaac | 5 | suspends task D0 |
//! | 0xac8 | 6 | wakes task D0 with count D1 |
//! | 0xae4 | 7 | the current task restarts at A0 ([`restart`]) |
//! | 0xaf6 | 12 | masks (D0 ≠ 0) or unmasks the interrupts ([`set_interrupt_mask`]) |
//!
//! In the original, a wrapper saves the 15 registers on the task's stack, takes the trap, and its resumption
//! (0x9bc, 0xa4c...) restores them: the wrappers that do not cede thus leave registers and flags intact, and
//! those that cede save the context here in the table. The trap handlers are only reached through their
//! wrappers (`trap #7` is also reached by the original debug menu, 0x2c3e0, which the game does not run): they
//! have no native. The wrappers remain natives for the calls by address and the jumps that remain.

use crate::world::World;

use super::table::{slot, table, Next, Resume, State};
use super::Task;
use crate::hw::mem::rd16;

/// Settings word read at VBLANK (DSW): bits 0 to 2, game speed.
const DSW: u32 = 0xffffd0;

/// Interrupt mask at power-on (level of the SR's mask).
pub const MASK_ALL: u8 = 7;

/// Where the processor is: a startup or scheduler point, or a task. The frame loop
/// (`frame::line`) executes one point at a time; the VBLANK interrupt can be taken between two, if the interrupt
/// mask allows it ([`interrupt_mask`]). Each point keeps the address of the original code it replaces (diagnostics,
/// `kernel::address`).
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
    /// (`stages::Game::Frame`).
    StepKept { d4: u32 },
    /// Endless loop of the original (`bra.b *` at this address: blocks of the TEST menu and of the EEPROM, [`block`]):
    /// nothing runs any more, apart from interrupts.
    Blocked(u32),
    /// `0x574`: the VBLANK interrupt is taken (`kernel::interrupt`); after its processing, the processor returns
    /// (`rte`) to the interrupted point, which the table keeps (`Table::interrupted`) instead of the pushed PC and SR.
    Interruption,
}

impl Point {
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
    super::table::read(b).map_or(Point::Reset, |t| t.point)
}

/// Interrupt mask of the processor (level 0 to 7 of the original's SR): power-on masks everything, `boot_init`
/// opens, the scheduler masks up to level 6 while it examines a slot, a task runs unmasked (except the TEST menu's
/// memory test, `trap #12`).
pub fn interrupt_mask(b: &World) -> u8 {
    super::table::read(b).map_or(MASK_ALL, |t| t.interrupt_mask)
}

/// Sets the interrupt mask (`move #…,SR`, `trap #12`).
pub fn set_interrupt_mask(b: &mut World, mask: u8) {
    table(b).interrupt_mask = mask;
}

/// Hands control back to the scheduler after an operation on the current task: the Rust code still running under
/// the cession returns without doing anything (`tasks::has_ceded`).
fn to_scheduler(b: &mut World) {
    table(b).point = Point::Return;
    super::note_cession();
}

/// The current task enters an endless loop of the original (`bra.b *` at `address`): the processor will only
/// leave it for interrupts. The Rust code in progress returns without doing anything (`tasks::has_ceded`).
pub fn block(b: &mut World, address: u32) {
    table(b).point = Point::Blocked(address);
    super::note_cession();
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
        unreachable!("cession to {resume:x?}: all resumptions are Rust steps")
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
    let setting = !rd16(b, DSW) & 7;
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

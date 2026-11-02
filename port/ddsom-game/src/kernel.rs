//! Kernel (K1): cooperative task scheduler (0x928) and traps 0 to 7 and 12.
//!
//! The game runs 16 tasks in turn. A task yields through a *cession wrapper* (`jsr 0xa40`,
//! `0xa86`...); the scheduler then picks the first ready task of the table. The task state is the Rust table
//! of [`crate::tasks::table`]. The scheduler is a Rust state machine ([`Point`], stored in the table) that the
//! frame loop (`no68k`) advances one point at a time, taking the VBLANK interrupt between two
//! points when the SR allows it, like the processor between two instructions; it keeps the registers that the original
//! leaves and the interrupt window between two slots, but no longer any record in RAM (FF0064), nor
//! stack (neither A7 nor USP: FF0166-FF0E65 is no longer read, probe `DDSOM_POISON=stack`), nor stack heap (FF005A),
//! nor saved registers. It no longer sets a PC: a task is launched by calling the step of its stage.
//!
//! ## Scheduler
//!
//! It walks the slots in order (A0 = address of the original record, D0.w = remaining slots, from 15 to 0).
//! Each slot is examined by a separate point ([`Point::Exam`]), interrupts
//! masked up to level 6; between two, the window of 0x952 enables them. A VBLANK sets the flag
//! `NEW_FRAME` (FF0164): the walk then restarts from the beginning. A ready task resumes at the step that follows its
//! call of the wrapper, SR = 0, without registers: what the step rereads is in its fields (`tasks::resume_step`); a
//! new task starts at its entry, SR = 0 (user mode), without registers: it does not read those that the original
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

use cps2::Cps2Bus;
use m68k::Cpu;

use crate::mem::{rd16, rd8, rts, set_low_byte, set_low_word, wr8, A5};
use crate::tasks::table::{slot, record, table, State, Resume, Next, RECORDS};
use crate::Native;

/// RAM addresses.
mod ram {
    /// Flag set by VBLANK: the scheduler's walk restarts.
    pub const NEW_FRAME: u32 = 0xff0164;
    /// Settings word read at VBLANK (DSW): bits 0 to 2, game speed.
    pub const DSW: u32 = 0xffffd0;
}

const SR_SUPERVISOR: u16 = 0x2000;
const SR_WALK: u16 = 0x2600;
/// Slots examined by a walk, minus one (D0.w at the start).
const LAST: u16 = 15;

/// Where the processor is: a startup or scheduler point, or a task. The frame loop
/// (`no68k::line`) executes one point at a time; the VBLANK interrupt can be taken between two, if the SR
/// allows it. Each point keeps the address of the original code it replaces (diagnostics).
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
    /// `0x93e`: examination of a slot (D0 = remainder to see, A0 = record).
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
    /// Address of the original code (diagnostics).
    pub fn address(self, c: &Cpu) -> u32 {
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
            Point::Task => c.pc,
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
pub fn point(b: &Cps2Bus) -> Point {
    crate::tasks::table::read(b).map_or(Point::Reset, |t| t.point)
}

/// Executes scheduler point `point` and returns the next one (the startup and task points are
/// executed by `no68k`).
pub fn execute(c: &mut Cpu, b: &mut Cps2Bus, point: Point) -> Point {
    match point {
        Point::Start => start(c),
        Point::Walk => walk(c, b),
        Point::Exam => exam(c, b),
        Point::NextTask => after_cession(c),
        Point::Return => scheduler_return(c, b),
        _ => unreachable!("point {point:?}: not a scheduler point"),
    }
}

/// Switch to supervisor, as when a trap is taken.
fn supervisor(c: &mut Cpu) {
    c.set_sr((c.sr | SR_SUPERVISOR) & !m68k::SR_T);
}

// ---------------------------------------------------------------------------------------------
// Scheduler

/// 0x928: the scheduler takes control (once, at the end of startup).
fn start(c: &mut Cpu) -> Point {
    c.a[5] = A5;
    Point::Walk
}

/// 0x932: new walk of the table.
fn walk(c: &mut Cpu, b: &mut Cps2Bus) -> Point {
    wr8(b, ram::NEW_FRAME, 0);
    c.a[0] = RECORDS;
    set_low_word(&mut c.d[0], LAST);
    Point::Exam
}

/// Moves to the next slot (`dbf`), or restarts when the table is exhausted.
fn next(c: &mut Cpu) -> Point {
    c.a[0] += 0x10;
    let remainder = (c.d[0] as u16).wrapping_sub(1);
    set_low_word(&mut c.d[0], remainder);
    if remainder == 0xffff { Point::Walk } else { Point::Exam }
}

/// 0x93e: examination of a slot, with interrupts masked (up to level 6).
fn exam(c: &mut Cpu, b: &mut Cps2Bus) -> Point {
    c.set_sr(SR_WALK);
    if rd8(b, ram::NEW_FRAME) != 0 {
        return Point::Walk;
    }
    let k = slot(c.a[0] - RECORDS);
    let state = table(b).slots[k].state;
    set_low_byte(&mut c.d[1], state.byte());
    if state.castable() {
        launch(c, b, k)
    } else {
        c.set_sr(SR_SUPERVISOR);
        next(c)
    }
}

/// 0x952: the task ceded, the interrupt window opens and the walk continues.
fn after_cession(c: &mut Cpu) -> Point {
    c.set_sr(SR_SUPERVISOR);
    next(c)
}

/// 0x960: launches the task of slot `k` (A0 = its record): the step of its stage runs. Returns the
/// point where the task left the processor (the scheduler if it ceded).
fn launch(c: &mut Cpu, b: &mut Cps2Bus, k: usize) -> Point {
    let t = table(b);
    t.current = k;
    t.point = Point::Task;
    let e = &mut t.slots[k];
    let step = match (e.state, e.resume) {
        (State::Fresh, Resume::Entry(entry)) => {
            e.state = State::InProgress;
            // the original gives it A1 = entry, A4 = stack and D1.w = state: no task reads them (probe `new`)
            c.set_sr(0);
            let step = crate::tasks::entry(entry).unwrap_or_else(|| panic!("task {k}: entry {entry:x} without a Rust step"));
            crate::diag::probe_launch(c, "new", &step, crate::regs::a(5));
            step
        }
        (State::Ready | State::InProgress, Resume::Step(step)) => {
            e.state = State::InProgress;
            // user mode, interrupts allowed (no task cedes while masking them); the registers that
            // the step rereads are its fields (`tasks::resume_step`)
            c.set_sr(0);
            crate::diag::probe_launch(c, "resume", &step, crate::regs::a(5));
            crate::tasks::resume_step(step, c, b);
            return table(b).point;
        }
        (state, resume) => unreachable!("task {k} launched in state {state:?} with resumption {resume:?}"),
    };
    crate::tasks::execute(step, c, b);
    table(b).point
}

/// 0x984: a task ceded (or ended); the scheduler resumes at the slot following its own.
fn scheduler_return(c: &mut Cpu, b: &mut Cps2Bus) -> Point {
    let k = table(b).current;
    c.a[0] = record(k);
    set_low_word(&mut c.d[0], LAST - k as u16);
    Point::NextTask
}

/// Hands control back to the scheduler after an operation on the current task (registers left by the trap): the
/// Rust code still running under the cession returns without doing anything (`tasks::has_ceded`).
fn to_scheduler(c: &mut Cpu, b: &mut Cps2Bus) {
    let t = table(b);
    c.a[0] = record(t.current);
    t.point = Point::Return;
    supervisor(c);
    crate::tasks::note_cession();
}

/// The current task enters an endless loop of the original (`bra.b *` at `address`): the processor will only
/// leave it for interrupts. The Rust code in progress returns without doing anything (`tasks::has_ceded`).
pub fn block(b: &mut Cps2Bus, address: u32) {
    table(b).point = Point::Blocked(address);
    crate::tasks::note_cession();
}

/// The VBLANK interrupt is taken at point `point` (between two points, the SR allowing it): the kernel remembers
/// where to return. The original pushes PC and SR on the current stack (the task's, or the supervisor's); nothing
/// rereads them there apart from the `rte`.
pub fn interrupt(b: &mut Cps2Bus, point: Point) {
    let t = table(b);
    assert!(t.interrupted.is_none(), "interrupt taken at point {point:?} during an interrupt");
    t.interrupted = Some(point);
    t.point = Point::Interruption;
}

/// `rte`: end of the interrupt, return to the interrupted point.
pub fn interrupt_end(b: &mut Cps2Bus) -> Point {
    table(b).interrupted.take().expect("end of interrupt without an interrupted point")
}

/// The game step is held until the next frame (cap on steps per frame): the game task stays in progress.
pub fn keep_step(b: &mut Cps2Bus, d4: u32) {
    table(b).point = Point::StepKept { d4 };
}

// ---------------------------------------------------------------------------------------------
// Cessions: the current task hands control back; it will resume at the step `resume`, which carries what it rereads
// (`tasks::resume_step`). The Rust tasks cede directly (`tasks::cede`); the original's wrappers first pop
// their return address (the continuation of a call in progress).

/// Stores the resumption of the current task in the table (its step, without registers: what it will reread is in
/// the step), then hands control back to the scheduler.
fn cede(c: &mut Cpu, b: &mut Cps2Bus, state: State, count: Option<u8>, resume: Next) {
    let Next::Step(step) = resume else {
        unreachable!("cession to address {:x}: all resumptions are Rust steps", resume.address())
    };
    // the only masking in the game (TEST menu memory test) does not cede: the resumption returns an unmasked SR
    assert!(c.sr & 0x0700 == 0, "cession to {step:?} with interrupts masked");
    table(b).cede(state, count, Resume::Step(step));
    to_scheduler(c, b);
}

/// Trap 3: cedes D0.b frames (plus the DSW settings delay), 0: suspended. The wrapper 0xa40 first calls
/// 0xb40, which according to DSW fills a GFX RAM block of the charge gauge (never, in this game: bit 0x400 of DSW);
/// the flags saved are those of its DSW test. The trap leaves D0.w = state and count, D1.w = 0x100 or 0x200.
pub fn wait(c: &mut Cpu, b: &mut Cps2Bus, resume: impl Into<Next>) {
    let resume = resume.into();
    c.sr &= !0x0f;
    let setting = !rd16(b, ram::DSW) & 7;
    let count = (c.d[0] as u8).wrapping_add(setting as u8);
    let (mut d0, mut d1) = (c.d[0], c.d[1]);
    set_low_word(&mut d1, if count == 0 { 0x200 } else { 0x100 });
    let word = (d0 as u16 & 0xff00) | count as u16 | d1 as u16;
    set_low_word(&mut d0, word);
    let state = State::from_byte((word >> 8) as u8).unwrap_or_else(|| panic!("cession to {resume:x?}: state {word:#06x}"));
    cede(c, b, state, Some(count), resume);
    (c.d[0], c.d[1]) = (d0, d1);
}

/// Trap 4: cedes while staying ready.
pub fn cede_ready(c: &mut Cpu, b: &mut Cps2Bus, resume: impl Into<Next>) {
    cede(c, b, State::Ready, None, resume.into());
}

/// Trap 1: the current task ends and gives back its stack.
pub fn finish(c: &mut Cpu, b: &mut Cps2Bus) {
    let t = table(b);
    let k = t.current;
    t.free(k);
    to_scheduler(c, b);
}

/// Cession wrapper called by `jsr`: its return address is the resumption (or the continuation of the call in progress,
/// when a Rust step called it).
fn envelope(c: &mut Cpu, b: &mut Cps2Bus, cession: fn(&mut Cpu, &mut Cps2Bus, Next)) {
    let resume = crate::tasks::table::pop_return(c, b);
    cession(c, b, resume);
}

/// 0xae4 (and `trap #7`, 0xae6): the current task restarts from scratch at A0, same stack.
pub fn restart(c: &mut Cpu, b: &mut Cps2Bus) {
    let t = table(b);
    t.restart(c.a[0]);
    t.point = Point::Return;
    supervisor(c);
    crate::tasks::note_cession();
}

// ---------------------------------------------------------------------------------------------
// Operations on another task: registers and flags intact

/// 0x9b6: creates task D0 (offset), entry A0. A task already there gives back its stack.
fn create(c: &mut Cpu, b: &mut Cps2Bus) {
    create_task(b, c.d[0], c.a[0]);
    rts(c, b);
}

/// Creates the task of offset `offset` in the table (`0x10 k`), which will start at `entry` (wrapper `0x9b6`: no
/// register or flag changes).
pub fn create_task(b: &mut Cps2Bus, offset: u32, entry: u32) {
    table(b).create(slot(offset), entry);
}

/// 0xa14: kills task D0.
fn kill(c: &mut Cpu, b: &mut Cps2Bus) {
    kill_task(b, c.d[0]);
    rts(c, b);
}

/// Kills the task of offset `offset` (wrapper `0xa14`: no register or flag changes).
pub fn kill_task(b: &mut Cps2Bus, offset: u32) {
    table(b).kill(slot(offset));
}

/// 0xaac: suspends task D0.
fn suspend(c: &mut Cpu, b: &mut Cps2Bus) {
    table(b).suspend(slot(c.d[0]));
    rts(c, b);
}

/// 0xac8: wakes task D0 with count D1.
fn wake(c: &mut Cpu, b: &mut Cps2Bus) {
    table(b).wake(slot(c.d[0]), c.d[1] as u8);
    rts(c, b);
}

/// `0xaf6` (`trap #12`): D0.b ≠ 0 masks all the task's interrupts, D0.b = 0 enables them; the SR returned
/// by the trap is the caller's, mask changed.
fn mask(c: &mut Cpu, b: &mut Cps2Bus) {
    let sr = if c.d[0] as u8 == 0 { c.sr & 0xf8ff } else { c.sr | 0x0700 };
    c.set_sr(sr);
    rts(c, b);
}

macro_rules! native {
    ($ident:ident, $entry:expr, $body:expr) => {
        pub const $ident: Native = Native {
            entry: $entry,
            name: stringify!($ident),
            run: crate::Body::Processeur($body),
            touches_sound: false,
            outputs: 0,
        };
    };
}

// wrappers
native!(CREATE_TASK, 0x9b6, create);
native!(FINISH_TASK, 0x9f8, finish);
native!(KILL_TASK, 0xa14, kill);
native!(WAIT, 0xa40, |c, b| envelope(c, b, wait));
native!(CEDE, 0xa86, |c, b| envelope(c, b, cede_ready));
native!(SUSPEND_TASK, 0xaac, suspend);
native!(WAKE_TASK, 0xac8, wake);
native!(RESTART_TASK, 0xae4, restart);
native!(MASK, 0xaf6, mask);
native!(WAIT_ONE_FRAME, 0xb64, |c, b| {
    set_low_word(&mut c.d[0], 1);
    envelope(c, b, wait)
});

pub const NATIVES: &[Native] = &[
    CREATE_TASK,
    FINISH_TASK,
    KILL_TASK,
    WAIT,
    CEDE,
    SUSPEND_TASK,
    WAKE_TASK,
    RESTART_TASK,
    MASK,
    WAIT_ONE_FRAME,
];

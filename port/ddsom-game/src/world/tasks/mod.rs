//! The tasks as the world holds them (see `docs/noyau.md`, "Rust tasks"): their table, their stages and the way a
//! task cedes control.
//!
//! A Rust task is a state machine whose each *step* runs between two cessions of control. Each resume point is a
//! stage ([`Step`]: one `enum` per module, declared in [`stages`]) whose original address (the one following the
//! original's `jsr 0xa40` or `0xa86`) is now just a name; the step cedes through [`cede`] and its relatives, which
//! store the task's wait and resume stage in the table ([`table`], through the kernel's traps, [`traps`]). Its 68000
//! stack is empty at each cession: the calls in progress are continuations in the table ([`table::Frame`]).
//!
//! The table belongs to the world: it only holds data (stages, counts, continuations), never code. The code of each
//! stage stays in its module (`steps!`), and the core runs it: the scheduler launches a task by calling the step of
//! its stage (`core::tasks::execute`), and a subroutine that returns without ceding runs the continuation of its
//! caller at once ([`Sequence::subroutine`], which the core implements).

pub mod stages;
pub mod table;
pub mod traps;

use crate::world::World;
use table::Next;

/// Continuation of a step or handler: finished (it returns to what follows the current call) or not (it yielded to
/// the scheduler, jumped to another native, or gave up: the task will continue elsewhere).
#[derive(Clone, Copy, PartialEq, Eq)]
pub enum Flow {
    Done,
    Yield,
}

/// Context of a step (scene sequence, screen, task): the machine, and named accesses to work RAM (`screens::scene`).
pub struct Ctx<'a> {
    pub b: &'a mut World,
    /// Scene entry (phase 8): the address of the phase handler, which the original keeps in A0 (`jsr (A0)` of the
    /// game loop and of the demo) and which the opening sequences reread as an object (original glitch: see
    /// `entry::phase_8`); 0 elsewhere.
    pub phase_handler: u32,
}

impl<'a> Ctx<'a> {
    pub fn new(b: &'a mut World) -> Self {
        Ctx { b, phase_handler: 0 }
    }
}

/// Yields and calls that may yield, seen from a step. The core implements it (`core::tasks`): a subroutine that
/// returns without ceding runs the continuation of its caller, which may be any stage.
pub trait Sequence {
    /// Yields `frames` frames to the scheduler; the task will resume at `resume` (Rust step, or the address
    /// following the original's `jsr 0xa40`). The sequence must return `Flow::Yield` immediately.
    fn wait(&mut self, frames: u32, resume: impl Into<Next>) -> Flow;
    /// Calls a Rust subroutine that may cede control (fade, black screen: `fading::descent`,
    /// `scene::black_screen::start`...): `resume` is stored in the current calls; if it returns without yielding,
    /// it runs at once.
    fn subroutine(&mut self, sp: impl FnOnce(&mut Ctx) -> Flow, resume: impl Into<Next>) -> Flow;
    /// `bsr`/`jsr` call whose continuation is a resume native: stored in the task's current calls
    /// (`table::Frame`), it will resume when the callee returns.
    fn push_return(&mut self, next: impl Into<Next>);
    /// The callee returned without going through its `rts` (Rust function that returned `Flow::Done`): the
    /// current call opened by `push_return` is abandoned.
    fn drop_return(&mut self);
    /// `rts`: the task continues at once at the continuation of the call in progress (a stage of any module), unless a
    /// Rust function waits for this return or the task has ceded control.
    fn return_to_caller(&mut self);
    /// Work RAM words and bytes, by offset from A5.
    fn or16(&mut self, off: u32, v: u16);
    fn and16(&mut self, off: u32, v: u16);
}

/// What a task asks of the scheduler at the end of a step.
#[derive(Clone, Copy)]
pub enum Wait {
    /// Sleeps this number of frames (trap 3: `jsr 0xa40`, D0 = number of frames).
    Images(u32),
    /// Stays ready: the scheduler relaunches it as soon as it comes back to it (trap 4: `jsr 0xa86`).
    Ready,
}

/// Cedes control: the kernel stores the task's wait and resume in the table; it will resume at `resume`
/// (a Rust stage, which carries what it will re-read).
pub fn cede(x: &mut Ctx, wait_kind: Wait, resume: impl Into<Next>) -> Flow {
    let resume = resume.into();
    match wait_kind {
        Wait::Images(n) => traps::wait(x.b, n as u8, resume),
        Wait::Ready => traps::cede_ready(x.b, resume),
    }
    Flow::Yield
}

/// `jsr 0xb64`: cedes one frame; the task will resume at `resume`.
pub fn one_frame(x: &mut Ctx, resume: impl Into<Next>) -> Flow {
    wait(x, 1, resume)
}

/// `jsr 0xa40`: cedes `frames` frames (D0.b of the original); the task will resume at `resume`.
pub fn wait(x: &mut Ctx, frames: u8, resume: impl Into<Next>) -> Flow {
    traps::wait(x.b, frames, resume);
    Flow::Yield
}

/// `jmp 0xa40`: cedes `frames` frames; the task will resume at the continuation of the caller (the call in progress).
pub fn wait_then_return(x: &mut Ctx, frames: u8) -> Flow {
    let next = table::pop_return(x.b);
    wait(x, frames, next)
}

/// `jmp 0xae4`: the current task restarts from zero as `task` (its entry in A0 of the original).
pub fn restart(x: &mut Ctx, task: Task) -> Flow {
    traps::restart(x.b, task);
    Flow::Yield
}

/// `jsr 0x9f8`: the current task ends.
pub fn finish(x: &mut Ctx) -> Flow {
    traps::finish(x.b);
    Flow::Yield
}

/// Maps the stages of a module (an `enum` of [`stages`], which the module names `Step`) to the address the original
/// resumes at (`address`, kept for diagnostics) and to the step that runs there (`step`).
#[macro_export]
macro_rules! steps {
    (impl $name:ident { $($var:ident = $adr:expr => $step:expr,)* }) => {
        impl $name {
            /// Address at which the original resumes at this stage.
            pub const fn address(self) -> u32 {
                match self { $(Self::$var => $adr,)* }
            }
            /// The step of the stage.
            pub fn step(self, x: &mut $crate::world::tasks::Ctx) -> $crate::world::tasks::Flow {
                match self {
                    $(Self::$var => {
                        let step: fn(&mut $crate::world::tasks::Ctx) -> $crate::world::tasks::Flow = $step;
                        step(x)
                    })*
                }
            }
        }
    };
}

/// Declares the stage of each kind of task.
macro_rules! task_steps {
    ($($(#[$vm:meta])* $var:ident,)*) => {
        /// Resume point of a Rust task: the stage of its state machine.
        #[derive(Clone, Copy, Debug, PartialEq, Eq)]
        pub enum Step { $($(#[$vm])* $var(stages::$var),)* }
        $(
            impl From<stages::$var> for Step {
                fn from(e: stages::$var) -> Self { Step::$var(e) }
            }
            impl From<stages::$var> for Next {
                fn from(e: stages::$var) -> Self { Next::Step(Step::$var(e)) }
            }
        )*
    };
}

task_steps! {
    Game,
    Freeze,
    Prompt,
    Scores,
    Service,
    Interface,
    Credits,
    Fading,
    BlackScreen,
    // mode machine (task 0 outside the game): its loop, then its screens
    Modes,
    Boot,
    Attract,
    Title,
    Select,
    Ranking,
    Demo,
    Ending,
    Gameover,
    // scene sequences (game task)
    Exit,
    Blackout,
    Transition,
    TransitionEarly,
    TransitionLate,
    Bonus,
    Shop,
    Characters,
    Tally,
    // game loop when an object handler makes the task cede control: world update, dispatch
    World,
    Dispatch,
    // object handlers that call a subroutine that cedes (SET doors: fade to black)
    DoorSet,
    // object handlers that make the game task wait
    Firework,
}

thread_local! {
    /// The current task has ceded control (or ended) during the native in progress: the Rust functions still
    /// in progress below it return without doing anything (see [`has_ceded`]).
    static CEDED: std::cell::Cell<bool> = const { std::cell::Cell::new(false) };
}

/// The current task ceded control during the native in progress (`traps::wait`, `finish`, `restart`...): its resume
/// is stored in the table (stage and calls in progress), and the Rust code still in progress below the cession (object
/// dispatch loop, world update, phase...) must return without doing anything. This is what the abandonment by panic
/// (`Unwound`) of the outdated Rust functions used to do.
#[inline]
pub fn has_ceded() -> bool {
    CEDED.with(|c| c.get())
}

/// The kernel hands control back to the scheduler for the current task.
#[inline]
pub(crate) fn note_cession() {
    CEDED.with(|c| c.set(true));
}

/// A top-level native begins (scheduler, VBLANK, relaunched task): no cession in progress.
#[inline]
pub(crate) fn forget_cession() {
    CEDED.with(|c| c.set(false));
}

/// A task of the game, as created by `0x9b6` (`traps::create_task`) or restarted by `0xae4` (`traps::restart`): the
/// original designates it by its entry address, which the reference's task table (FF0064) keeps until the task runs
/// (`Task::address`, compared by `kernel::gaps`).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Task {
    /// `0x11b16` (slot 0): initialization, warnings, creation of the credits and the freeze (`modes/boot.rs`),
    /// then the mode machine (`modes/`), in which it continues without changing entry.
    Startup,
    /// `0x133fe` (slot 0): the mode machine relaunched after a game (`modes/gameover.rs`).
    Modes,
    /// `0x1c45a` (slot 0): the game (`core::tasks::game`).
    Game,
    /// `0x29162` (slot 0): TEST menu (`screens::service::task`).
    Service,
    /// `0x14ba`... `0x1cf12` (slot 1): a palette fade (`services::fading`).
    Fading(stages::Fade),
    /// `0x13c0a` (slot 2): credits and starts (`credits_task.rs`).
    Credits,
    /// `0x150ac` (slot 3): player interface (`hud/`).
    Interface,
    /// `0xf7dfe` (slot 12): shop portraits (`scene/shop.rs`).
    Portraits,
    /// `0xe03fe` (slot 13): blinking "INSERT COIN" (`screens::prompt`).
    Prompt,
    /// `0xee080` (slot 14): busy wait and fast forward (`core::tasks::freeze`).
    Freeze,
    /// `0x1835e` (slot 15): attract score table (`screens::fixed_screen`).
    Scores,
}

//! Scheduler tasks written in Rust (see `docs/noyau.md`, "Rust tasks").
//!
//! A Rust task is a state machine whose each *step* runs between two cessions of control. Each resume
//! point is a stage ([`Step`], one `enum` per module: `steps!`) whose original address (the one following
//! the original's `jsr 0xa40` or `0xa86`) is now just a name; the step returns a [`Wait`], which [`cede`] hands to the
//! kernel. The task table ([`table`]) stores the task's state, count, resume stage, registers and calls in
//! progress ([`table::Frame`]): its 68000 stack is empty at each cession. The scheduler (`kernel`) launches a
//! task by calling the step of its stage ([`execute`]).
//!
//! Tasks: [`game`] (the game, `0x1c45a`), [`freeze`] (fast forward, `0xee080`), [`fading`] (palette fades,
//! tasks and subroutines that cede control), [`fixed_screen`] (attract score table, `0x1835e`),
//! [`service`] (TEST menu, `0x29162`) and [`prompt`] (blinking "INSERT COIN" text, `0xe03fe`).

pub mod fixed_screen;
pub mod fading;
pub mod freeze;
pub mod prompt;
pub mod game;
pub mod service;
pub mod table;

use cps2::Cps2Bus;
use m68k::Cpu;

use crate::modes::Flow;
use crate::scene::Ctx;
use crate::Native;
use table::Next;

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
        Wait::Images(n) => crate::kernel::wait(x.b, n as u8, resume),
        Wait::Ready => crate::kernel::cede_ready(x.b, resume),
    }
    Flow::Yield
}

/// `jsr 0xb64`: cedes one frame; the task will resume at `resume`.
pub fn one_frame(x: &mut Ctx, resume: impl Into<Next>) -> Flow {
    wait(x, 1, resume)
}

/// `jsr 0xa40`: cedes `frames` frames (D0.b of the original); the task will resume at `resume`.
pub fn wait(x: &mut Ctx, frames: u8, resume: impl Into<Next>) -> Flow {
    crate::kernel::wait(x.b, frames, resume);
    Flow::Yield
}

/// `jmp 0xa40`: cedes `frames` frames; the task will resume at the continuation of the caller (the call in progress).
pub fn wait_then_return(x: &mut Ctx, frames: u8) -> Flow {
    let next = table::pop_return(x.b);
    wait(x, frames, next)
}

/// `jmp 0xae4`: the current task restarts from zero at `entry` (A0 of the original).
pub fn restart(x: &mut Ctx, entry: u32) -> Flow {
    crate::kernel::restart(x.b, entry);
    Flow::Yield
}

/// `jsr 0x9f8`: the current task ends.
pub fn finish(x: &mut Ctx) -> Flow {
    crate::kernel::finish(x.b);
    Flow::Yield
}

/// Declares the stages of a task: one variant per resume point, with the address the original resumes at
/// (`address`, kept for diagnostics) and the step that runs there (`step`).
#[macro_export]
macro_rules! steps {
    ($(#[$m:meta])* $vis:vis enum $name:ident { $($(#[$vm:meta])* $var:ident = $adr:expr => $step:expr,)* }) => {
        $(#[$m])*
        #[derive(Clone, Copy, Debug, PartialEq, Eq)]
        $vis enum $name { $($(#[$vm])* $var,)* }
        impl $name {
            /// Address at which the original resumes at this stage.
            pub const fn address(self) -> u32 {
                match self { $(Self::$var => $adr,)* }
            }
            /// The step of the stage.
            pub fn step(self, x: &mut $crate::scene::Ctx) -> $crate::modes::Flow {
                match self {
                    $(Self::$var => {
                        let step: fn(&mut $crate::scene::Ctx) -> $crate::modes::Flow = $step;
                        step(x)
                    })*
                }
            }
        }
    };
}

/// Declares the stage of each kind of task.
macro_rules! task_steps {
    ($($(#[$vm:meta])* $var:ident($t:ty),)*) => {
        /// Resume point of a Rust task: the stage of its state machine.
        #[derive(Clone, Copy, Debug, PartialEq, Eq)]
        pub enum Step { $($(#[$vm])* $var($t),)* }
        impl Step {
            /// Address at which the original resumes at this stage (diagnostics, comparison with the FF0064 table).
            pub fn address(self) -> u32 {
                match self { $(Step::$var(e) => e.address(),)* }
            }
            fn step(self, x: &mut Ctx) -> Flow {
                match self { $(Step::$var(e) => e.step(x),)* }
            }
        }
        $(
            impl From<$t> for Step {
                fn from(e: $t) -> Self { Step::$var(e) }
            }
            impl From<$t> for Next {
                fn from(e: $t) -> Self { Next::Step(Step::$var(e)) }
            }
        )*
    };
}

task_steps! {
    Game(game::Step),
    Freeze(freeze::Step),
    Prompt(prompt::Step),
    Scores(fixed_screen::Step),
    Service(service::Step),
    Interface(crate::hud::Step),
    Credits(crate::credits_task::Step),
    Fading(fading::Step),
    BlackScreen(crate::video::black_screen::Step),
    // mode machine (task 0 outside the game): its loop, then its screens
    Modes(crate::modes::Step),
    Boot(crate::modes::boot::Step),
    Attract(crate::modes::attract::Step),
    Title(crate::modes::title::Step),
    Select(crate::modes::select::Step),
    Ranking(crate::modes::ranking::Step),
    Demo(crate::modes::demo::Step),
    Ending(crate::modes::ending::Step),
    Gameover(crate::modes::gameover::Step),
    // scene sequences (game task)
    Exit(crate::scene::exit::Step),
    Blackout(crate::scene::blackout::Step),
    Transition(crate::scene::transition::Step),
    TransitionEarly(crate::scene::transition::early::Step),
    TransitionLate(crate::scene::transition::late::Step),
    Bonus(crate::scene::bonus::Step),
    Shop(crate::scene::shop::Step),
    Characters(crate::scene::characters::Step),
    Tally(crate::scene::tally::Step),
    // game loop when an object handler makes the task cede control: world update, dispatch
    World(crate::game::Step),
    Dispatch(crate::dispatch::Step),
    // object handlers that call a subroutine that cedes (SET doors: fade to black)
    DoorSet(crate::families::set::exit_fading::Step),
    // object handlers that make the game task wait
    Firework(crate::families::effects_g::Step),
}

/// Executes stage `step` of the current task; if it finishes (`Flow::Done`, subroutine return), the continuation of
/// the call in progress ([`return_to_caller`]).
pub fn execute(step: Step, b: &mut Cps2Bus) {
    let mut x = Ctx::new(b);
    if step.step(&mut x) == Flow::Done {
        return_to_caller(x.b);
    }
}

/// Resume of a task that ceded control: stage `step`. The original gives the task back the registers saved at
/// the cession; none is re-read any more (probe `DDSOM_POISON=resume`, before their removal): what was re-read is
/// named, in the fields of the stages (object and counter of the dispatch loop, D4 of the game step, A0 of the caller
/// of a fade...).
pub fn resume_step(step: Step, b: &mut Cps2Bus) {
    execute(step, b);
}

/// Return from a subroutine (`rts`): the continuation of the call in progress, Rust stage executed immediately;
/// nothing if a Rust function was waiting for this return (`table::call_expected`, `crate::call`), or if the task has
/// already ceded control (the Rust code following a cession only returns: see [`has_ceded`]). In the tools (natives
/// called by the original code), a return to the original code: noted, the processor jumps there at the end of the
/// native ([`return_to_caller_68k`], `Native::execute`).
pub fn return_to_caller(b: &mut Cps2Bus) {
    if has_ceded() {
        return;
    }
    match table::ret(b) {
        table::Return::Render => {}
        table::Return::Next(Next::Step(e)) => execute(e, b),
        // a frame whose continuation is an address: tools (natives called by a diagnostic tool)
        table::Return::Next(Next::Address(a)) => TOOLS_RETURN.with(|r| r.set(Some(ToolsReturn::Address(a)))),
        // game: each return is that of a call in progress (stage, expected call, call by address)
        table::Return::Stack68000 if crate::no68k::active() => panic!("return without a call in progress"),
        table::Return::Stack68000 => TOOLS_RETURN.with(|r| r.set(Some(ToolsReturn::Stack))),
    }
}

/// Tools: where the original code resumes after a native that returned to it.
#[derive(Clone, Copy)]
enum ToolsReturn {
    Address(u32),
    /// The return address popped from the 68000 stack.
    Stack,
}

thread_local! {
    static TOOLS_RETURN: std::cell::Cell<Option<ToolsReturn>> = const { std::cell::Cell::new(None) };
}

/// `rts` of a native on the processor: [`return_to_caller`], then, in the tools, the PC of the original code that
/// resumes.
pub fn return_to_caller_68k(c: &mut Cpu, b: &mut Cps2Bus) {
    return_to_caller(b);
    apply_tools_return(c, b);
}

/// Tools: a native returned to the original code ([`return_to_caller`]): its PC.
pub fn apply_tools_return(c: &mut Cpu, b: &mut Cps2Bus) {
    match TOOLS_RETURN.with(|r| r.take()) {
        None => {}
        Some(ToolsReturn::Address(a)) => c.pc = a,
        Some(ToolsReturn::Stack) => c.pc = table::pop_68000(c, b),
    }
}

thread_local! {
    /// The current task has ceded control (or ended) during the native in progress: the Rust functions still
    /// in progress below it return without doing anything (see [`has_ceded`]).
    static CEDED: std::cell::Cell<bool> = const { std::cell::Cell::new(false) };
}

/// The current task ceded control during the native in progress (`kernel::cede`, `finish`, `restart`): its resume
/// is stored in the table (stage and calls in progress), and the Rust code still in progress below the cession (object
/// dispatch loop, world update, phase...) must return without doing anything. This is what the abandonment by panic
/// (`Unwound`) of the outdated Rust functions used to do.
pub fn has_ceded() -> bool {
    CEDED.with(|c| c.get())
}

/// The kernel hands control back to the scheduler for the current task.
pub(crate) fn note_cession() {
    CEDED.with(|c| c.set(true));
}

/// A top-level native begins (scheduler, VBLANK, relaunched task): no cession in progress.
pub(crate) fn forget_cession() {
    CEDED.with(|c| c.set(false));
}

/// First stage of a new Rust task, from its entry (otherwise the native placed at the entry).
pub fn entry(entry: u32) -> Option<Step> {
    [
        Step::from(freeze::Step::Entry),
        game::Step::Entry.into(),
        prompt::Step::Entry.into(),
        fixed_screen::Step::Entry.into(),
        service::Step::Entry.into(),
        crate::hud::Step::Entry.into(),
        crate::credits_task::Step::Entry.into(),
        crate::scene::shop::Step::Portraits.into(),
        crate::modes::boot::Step::Entry.into(),
        crate::modes::Step::Entry.into(),
    ]
    .into_iter()
    .find(|e| e.address() == entry)
    .or_else(|| fading::task_from_entry(entry).map(Step::Fading))
}

/// Natives of the Rust tasks.
pub fn natives() -> impl Iterator<Item = &'static Native> {
    fading::NATIVES
        .iter()
        .chain(prompt::NATIVES)
}

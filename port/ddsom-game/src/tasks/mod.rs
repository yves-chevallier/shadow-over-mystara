//! Scheduler tasks written in Rust (see `docs/noyau.md`, « Tâches Rust »).
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
        Wait::Images(n) => {
            x.c.d[0] = n;
            crate::kernel::wait(x.c, x.b, resume);
        }
        Wait::Ready => crate::kernel::cede_ready(x.c, x.b, resume),
    }
    Flow::Yield
}

/// `jsr 0xb64`: cedes one frame (D0.w = 1, the high word of D0 is kept); the task will resume at `resume`.
pub fn one_frame(x: &mut Ctx, resume: impl Into<Next>) -> Flow {
    crate::mem::set_low_word(&mut x.c.d[0], 1);
    crate::kernel::wait(x.c, x.b, resume);
    Flow::Yield
}

/// `jsr 0xa40`: cedes D0.b frames (D0 as is); the task will resume at `resume`.
pub fn wait(x: &mut Ctx, resume: impl Into<Next>) -> Flow {
    crate::kernel::wait(x.c, x.b, resume);
    Flow::Yield
}

/// `jmp 0xa40`: cedes D0.b frames; the task will resume at the continuation of the caller (the call in progress).
pub fn wait_then_return(x: &mut Ctx) -> Flow {
    let next = table::pop_return(x.c, x.b);
    wait(x, next)
}

/// `jmp 0x9b6`: creates task D0 (offset) at entry A0, then returns to the caller.
pub fn create_then_return(x: &mut Ctx) -> Flow {
    crate::kernel::create_task(x.b, x.c.d[0], x.c.a[0]);
    Flow::Done
}

/// `jmp 0xae4`: the current task restarts from zero at entry A0.
pub fn restart(x: &mut Ctx) -> Flow {
    crate::kernel::restart(x.c, x.b);
    Flow::Yield
}

/// `jsr 0x9f8`: the current task ends.
pub fn finish(x: &mut Ctx) -> Flow {
    crate::kernel::finish(x.c, x.b);
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
pub fn execute(step: Step, c: &mut Cpu, b: &mut Cps2Bus) {
    let mut x = Ctx::new(c, b);
    crate::modes::c_a5(&mut x);
    if step.step(&mut x) == Flow::Done {
        crate::diag::probe_return(x.c, &step);
        return_to_caller(x.c, x.b);
    }
}

/// Resume of a task that ceded control: stage `step`. The original gives the task back the registers saved at
/// the cession; only a few are re-read (probe `DDSOM_POISON=resume`), and they are named: fields of the stages that
/// re-read them (object and counter of the dispatch loop, D4 of the game step, A0 of the caller of a fade), which their
/// step puts back in the bench, or A0 that the stages of certain tasks always find again ([`resume_a0`]).
pub fn resume_step(step: Step, c: &mut Cpu, b: &mut Cps2Bus) {
    if let Some(a0) = resume_a0(step, b) {
        c.a[0] = a0;
    }
    execute(step, c, b);
}

/// The A0 that a resumed stage finds and re-reads, when it depends only on it:
/// * the scene-exit sequences (dispatch, shared states, transitions) work on the scene-exit block,
///   which the dispatch puts in A0 and which the cessions keep;
/// * the handler that makes the game wait (fireworks) finds its object: that of the dispatch loop
///   in progress (`dispatch::current_object`).
fn resume_a0(step: Step, b: &mut Cps2Bus) -> Option<u32> {
    match step {
        Step::Exit(_)
        | Step::Bonus(_)
        | Step::Tally(_)
        | Step::Characters(_)
        | Step::Transition(_)
        | Step::TransitionEarly(_)
        | Step::TransitionLate(_) => Some(crate::scene::screen::blocks::EXIT),
        Step::Firework(_) => {
            let object = crate::dispatch::current_object(b).expect("fireworks resume outside a dispatch loop");
            Some(object.0)
        }
        _ => None,
    }
}

/// Return from a subroutine (`rts`): the continuation of the call in progress, Rust stage executed immediately;
/// nothing if a
/// Rust function was waiting for this return (`table::call_expected`, `crate::call`), or if the task has already
/// ceded control (the
/// Rust code following a cession only returns: see [`has_ceded`]).
pub fn return_to_caller(c: &mut Cpu, b: &mut Cps2Bus) {
    if has_ceded() {
        return;
    }
    match table::ret(b) {
        table::Return::Render => {}
        table::Return::Next(Next::Step(e)) => execute(e, c, b),
        // a frame whose continuation is an address: tools (natives called by a diagnostic tool)
        table::Return::Next(Next::Address(a)) => c.pc = a,
        // game: each return is that of a call in progress (stage, expected call, call by address)
        table::Return::Stack68000 if crate::no68k::active() => panic!("return without a call in progress"),
        // tools (natives called by the original code): the popped return address
        table::Return::Stack68000 => c.pc = table::pop_68000(c, b),
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

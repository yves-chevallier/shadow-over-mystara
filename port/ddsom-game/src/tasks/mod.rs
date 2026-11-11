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

use crate::world::World;

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

/// `jmp 0xae4`: the current task restarts from zero as `task` (its entry in A0 of the original).
pub fn restart(x: &mut Ctx, task: Task) -> Flow {
    crate::kernel::restart(x.b, task);
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
    Firework(crate::families::effects::g::Step),
}

/// Executes stage `step` of the current task; if it finishes (`Flow::Done`, subroutine return), the continuation of
/// the call in progress ([`return_to_caller`]).
pub fn execute(step: Step, b: &mut World) {
    let mut x = Ctx::new(b);
    if step.step(&mut x) == Flow::Done {
        return_to_caller(x.b);
    }
}

/// Resume of a task that ceded control: stage `step`. The original gives the task back the registers saved at
/// the cession; none is re-read any more (probe `DDSOM_POISON=resume`, before their removal): what was re-read is
/// named, in the fields of the stages (object and counter of the dispatch loop, D4 of the game step, A0 of the caller
/// of a fade...).
pub fn resume_step(step: Step, b: &mut World) {
    execute(step, b);
}

/// Return from a subroutine (`rts`): the continuation of the call in progress, Rust stage executed immediately;
/// nothing if a Rust function was waiting for this return (`table::call_expected`), or if the task has already ceded
/// control (the Rust code following a cession only returns: see [`has_ceded`]). In the tools (natives called by the
/// original code), a return to the original code: noted, the processor jumps there at the end of the native
/// (`tools::frames`).
pub fn return_to_caller(b: &mut World) {
    if has_ceded() {
        return;
    }
    match table::ret(b) {
        table::Return::Render => {}
        table::Return::Next(Next::Step(e)) => execute(e, b),
        // a frame whose continuation is an address: tools (natives called by a diagnostic tool)
        table::Return::Next(Next::Address(a)) => original_code_resumes(Some(a)),
        // game: each return is that of a call in progress (stage, expected call)
        table::Return::Stack68000 if crate::no68k::active() => panic!("return without a call in progress"),
        table::Return::Stack68000 => original_code_resumes(None),
    }
}

/// Tools: a native called by the original code returns to it, at `address` or at the address on the 68000 stack.
fn original_code_resumes(address: Option<u32>) {
    #[cfg(feature = "tools")]
    crate::tools::frames::original_code_resumes(address);
    #[cfg(not(feature = "tools"))]
    unreachable!("return to the original code ({address:x?}): the game has no 68000");
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

/// A task of the game, as created by `0x9b6` (`kernel::create_task`) or restarted by `0xae4` (`kernel::restart`): the
/// original designates it by its entry address, which the reference's task table (FF0064) keeps until the task runs
/// ([`Task::address`], compared by `table::gaps`).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Task {
    /// `0x11b16` (slot 0): initialization, warnings, creation of the credits and the freeze (`modes/boot.rs`),
    /// then the mode machine (`modes/`), in which it continues without changing entry.
    Startup,
    /// `0x133fe` (slot 0): the mode machine relaunched after a game (`modes/gameover.rs`).
    Modes,
    /// `0x1c45a` (slot 0): the game ([`game`]).
    Game,
    /// `0x29162` (slot 0): TEST menu ([`service`]).
    Service,
    /// `0x14ba`... `0x1cf12` (slot 1): a palette fade ([`fading`]).
    Fading(fading::Fade),
    /// `0x13c0a` (slot 2): credits and starts (`credits_task.rs`).
    Credits,
    /// `0x150ac` (slot 3): player interface (`hud/`).
    Interface,
    /// `0xf7dfe` (slot 12): shop portraits (`scene/shop.rs`).
    Portraits,
    /// `0xe03fe` (slot 13): blinking "INSERT COIN" ([`prompt`]).
    Prompt,
    /// `0xee080` (slot 14): busy wait and fast forward ([`freeze`]).
    Freeze,
    /// `0x1835e` (slot 15): attract score table ([`fixed_screen`]).
    Scores,
}

impl Task {
    /// The first stage of the task.
    pub fn first_step(self) -> Step {
        match self {
            Task::Startup => crate::modes::boot::Step::Entry.into(),
            Task::Modes => crate::modes::Step::Entry.into(),
            Task::Game => game::Step::Entry.into(),
            Task::Service => service::Step::Entry.into(),
            Task::Fading(fade) => fade.first_step().into(),
            Task::Credits => crate::credits_task::Step::Entry.into(),
            Task::Interface => crate::hud::Step::Entry.into(),
            Task::Portraits => crate::scene::shop::Step::Portraits.into(),
            Task::Prompt => prompt::Step::Entry.into(),
            Task::Freeze => freeze::Step::Entry.into(),
            Task::Scores => fixed_screen::Step::Entry.into(),
        }
    }

    /// The entry address of the original (that of its first stage).
    pub fn address(self) -> u32 {
        self.first_step().address()
    }
}

/// Natives of the Rust tasks.
pub fn natives() -> impl Iterator<Item = &'static Native> {
    fading::NATIVES
        .iter()
        .chain(prompt::NATIVES)
}

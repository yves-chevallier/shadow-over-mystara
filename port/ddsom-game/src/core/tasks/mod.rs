//! Scheduler tasks written in Rust (see `docs/noyau.md`, "Rust tasks"): how the core runs their stages.
//!
//! The table, the stages and the cessions belong to the world (`world::tasks`); each module maps its stages to their
//! steps (`steps!`). The core runs them: the scheduler (`kernel`) launches a task by calling the step of its stage
//! ([`execute`]), and the return of a subroutine runs the continuation of its caller ([`return_to_caller`]).
//!
//! Tasks of the core: [`game`] (the game, `0x1c45a`) and [`freeze`] (fast forward, `0xee080`). The other tasks
//! belong to their layer: fades (`services::fading`), attract score table (`screens::fixed_screen`), TEST menu
//! (`screens::service::task`), blinking "INSERT COIN" text (`screens::prompt`), credits (`screens::credits_task`),
//! player interface (`screens::hud`).

pub mod freeze;
pub mod game;

use crate::world::tasks::table::{self, Next};
use crate::world::tasks::{has_ceded, stages, Ctx, Flow, Sequence, Step, Task};
use crate::world::World;
use crate::Native;

/// The step of each stage, and the address the original resumes at.
macro_rules! run_steps {
    ($($var:ident,)*) => {
        impl Step {
            /// Address at which the original resumes at this stage (diagnostics, comparison with the FF0064 table).
            pub fn address(self) -> u32 {
                match self { $(Step::$var(e) => e.address(),)* }
            }
            fn step(self, x: &mut Ctx) -> Flow {
                match self { $(Step::$var(e) => e.step(x),)* }
            }
        }
    };
}

run_steps! {
    Game, Freeze, Prompt, Scores, Service, Interface, Credits, Fading, BlackScreen, Modes, Boot, Attract, Title, Select,
    Ranking, Demo, Ending, Gameover, Exit, Blackout, Transition, TransitionEarly, TransitionLate, Bonus, Shop,
    Characters, Tally, World, Dispatch, DoorSet, Firework,
}

impl Next {
    /// Original address (the one the original pushes).
    pub fn address(self) -> u32 {
        match self {
            Next::Address(a) => a,
            Next::Step(e) => e.address(),
        }
    }
}

impl Task {
    /// The first stage of the task.
    pub fn first_step(self) -> Step {
        match self {
            Task::Startup => stages::Boot::Entry.into(),
            Task::Modes => stages::Modes::Entry.into(),
            Task::Game => stages::Game::Entry.into(),
            Task::Service => stages::Service::Entry.into(),
            Task::Fading(fade) => fade.first_step().into(),
            Task::Credits => stages::Credits::Entry.into(),
            Task::Interface => stages::Interface::Entry.into(),
            Task::Portraits => stages::Shop::Portraits.into(),
            Task::Prompt => stages::Prompt::Entry.into(),
            Task::Freeze => stages::Freeze::Entry.into(),
            Task::Scores => stages::Scores::Entry.into(),
        }
    }

    /// The entry address of the original (that of its first stage).
    pub fn address(self) -> u32 {
        self.first_step().address()
    }
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
        table::Return::Stack68000 if crate::core::frame::active() => panic!("return without a call in progress"),
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

impl Sequence for Ctx<'_> {
    fn wait(&mut self, frames: u32, resume: impl Into<Next>) -> Flow {
        crate::world::tasks::cede(self, crate::world::tasks::Wait::Images(frames), resume)
    }
    fn push_return(&mut self, next: impl Into<Next>) {
        table::call(self.b, next);
    }
    fn subroutine(&mut self, sp: impl FnOnce(&mut Ctx) -> Flow, resume: impl Into<Next>) -> Flow {
        table::call(self.b, resume);
        if sp(self) == Flow::Done {
            return_to_caller(self.b);
        }
        Flow::Yield
    }
    fn drop_return(&mut self) {
        table::pop_return(self.b);
    }
    fn return_to_caller(&mut self) {
        return_to_caller(self.b);
    }
    fn or16(&mut self, off: u32, v: u16) {
        let w = self.v16(off) | v;
        self.set_v16(off, w);
    }
    fn and16(&mut self, off: u32, v: u16) {
        let w = self.v16(off) & v;
        self.set_v16(off, w);
    }
}

/// Natives of the Rust tasks.
pub fn natives() -> impl Iterator<Item = &'static Native> {
    crate::services::fading::NATIVES.iter().chain(crate::screens::prompt::NATIVES)
}

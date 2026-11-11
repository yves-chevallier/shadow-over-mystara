//! Palette fades (V3): one step per cession, until the end of the count (see `docs/video.md`, "fades").
//!
//! Two families, each in two forms: a *task* (created by `0x9b6`, it ends with `0x9f8` at the
//! end of the fade) and a *subroutine* (called by `jsr` from a sequence, it returns when the fade is finished;
//! the caller therefore waits for the end). The steps cede control through `jsr 0xa40` (D0.w = delay `A5+0x490`); the
//! next step is a stage of the state machine ([`Step`]), which carries the fade in progress.
//!
//! * **Brightness**: each step adds ±0x1000 to the colors of the checked pages (`palette::brightness_step`).
//!   The state is at `A5+0x48e` (pages, remaining steps, delay, addresses of the four pages); the direction is a field
//!   of the stage (the original keeps it in D1, which the cession saves and restores, and which the context also
//!   keeps).
//! * **Per component**: each step brings each component one notch closer (`palette::component_step`);
//!   16 steps, `A5-0x65ec` raised during the fade, registers saved on entry (`movem`, kept in the stage) and
//!   restored at the end.
//!
//! The subroutines are functions ([`rise`], [`descent`], [`descent_steps`], [`to_white`],
//! [`to_colours`], [`to_palettes`]) that the steps call directly (the SET doors too:
//! `families::set::exit_fading`).

use crate::core::dispatch::Leftovers;
use crate::world::mem::{rd16, rd8, wr32, wr8, A5};
use crate::screens::modes::Flow;
use crate::services::palette::{brightness_step, component_step, white_out, Target};
use crate::screens::scene::Ctx;
use crate::Native;

/// State of the brightness fade (`A4` in the original).
mod state {
    use crate::world::mem::A5;
    pub const BASE: u32 = A5 + 0x48e;
    /// Pages concerned (bit k: page k), reset to zero at the end of the fade.
    pub const PAGES: u32 = BASE;
    /// Remaining steps.
    pub const STEP: u32 = BASE + 1;
    /// Delay between two steps (word, frames).
    pub const DELAY: u32 = BASE + 2;
    /// Addresses of the four pages (longs).
    pub const ADDRESSES: u32 = BASE + 4;
}

/// "Per-component fade in progress" (byte, A5 - 0x65ec).
const CURRENT_COMPONENT: u32 = A5 - 0x65ec;

/// Brightness step: rise (toward full brightness) or descend.
const RISES: u32 = 0x1000;
const DESCEND: u32 = 0xffff_f000;
/// Number of steps of a complete fade.
const STEP_FULL: u8 = 0x10;

/// Pages of the fades: the four pages, or the end of page 0 for the partial fade.
const PAGES: [u32; 4] = [0x91c000, 0x91c400, 0x91c800, 0x91cc00];
const PARTIAL_START: u32 = 0x91c3a0;

/// Stages of a fade: the start of a fade task (from its entry), then the next step after each cession.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Step {
    /// Start of a brightness fade task; `step`: imposed number of steps (otherwise that of the state).
    BrightnessTask { fading: Brightness, step: Option<u8> },
    /// Start of a per-component fade task (`whiten`: task `0x1ced4` first whitens the pages).
    ComponentTask { fading: Component, whiten: bool },
    /// Next step of a brightness fade.
    Brightness { fading: Brightness },
    /// Next step of a per-component fade; `saved`: the leftovers D2-D5 saved on entry.
    Component { fading: Component, saved: Leftovers },
}

/// The fade tasks (created by `0x9b6` in slot 1), by entry.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Fade {
    /// `0x14ba`: full rise of the brightness (16 steps).
    Rise,
    /// `0x14c8`: rise by the number of steps of the state.
    RiseSteps,
    /// `0x14d0`: full descent of the brightness (16 steps).
    Descent,
    /// `0x14de`: descent by the number of steps of the state.
    DescentSteps,
    /// `0x1536`: rise of the end of page 0 only.
    PartialRise,
    /// `0x153e`: descent of the end of page 0 only.
    PartialDescent,
    /// `0x1ccf8`: per component, toward white.
    ToWhite,
    /// `0x1ce64`: per component, toward the colours.
    ToColours,
    /// `0x1ced4`: whitens the pages, then per component toward the palettes.
    FromWhite,
    /// `0x1cf12`: per component toward the palettes, without whitening first.
    ToPalettes,
}

/// The fade tasks.
const FADES: [Fade; 10] = [
    Fade::Rise,
    Fade::RiseSteps,
    Fade::Descent,
    Fade::DescentSteps,
    Fade::PartialRise,
    Fade::PartialDescent,
    Fade::ToWhite,
    Fade::ToColours,
    Fade::FromWhite,
    Fade::ToPalettes,
];

impl Fade {
    /// The first stage of the task.
    pub const fn first_step(self) -> Step {
        match self {
            Fade::Rise => Step::BrightnessTask { fading: task(RISES, false), step: Some(STEP_FULL) },
            Fade::RiseSteps => Step::BrightnessTask { fading: task(RISES, false), step: None },
            Fade::Descent => Step::BrightnessTask { fading: task(DESCEND, false), step: Some(STEP_FULL) },
            Fade::DescentSteps => Step::BrightnessTask { fading: task(DESCEND, false), step: None },
            Fade::PartialRise => Step::BrightnessTask { fading: task(RISES, true), step: None },
            Fade::PartialDescent => Step::BrightnessTask { fading: task(DESCEND, true), step: None },
            Fade::ToWhite => Step::ComponentTask { fading: COMPONENTS[3].1, whiten: false },
            Fade::ToColours => Step::ComponentTask { fading: COMPONENTS[4].1, whiten: false },
            Fade::FromWhite => Step::ComponentTask { fading: COMPONENTS[5].1, whiten: true },
            Fade::ToPalettes => Step::ComponentTask { fading: COMPONENTS[5].1, whiten: false },
        }
    }

    /// The entry address of the original.
    pub const fn address(self) -> u32 {
        match self {
            Fade::Rise => 0x14ba,
            Fade::RiseSteps => 0x14c8,
            Fade::Descent => 0x14d0,
            Fade::DescentSteps => 0x14de,
            Fade::PartialRise => 0x1536,
            Fade::PartialDescent => 0x153e,
            Fade::ToWhite => 0x1ccf8,
            Fade::ToColours => 0x1ce64,
            Fade::FromWhite => 0x1ced4,
            Fade::ToPalettes => 0x1cf12,
        }
    }
}

impl Step {
    /// Address that the original resumes at this stage (task entry, or return from the step's cession).
    pub fn address(self) -> u32 {
        match self {
            Step::BrightnessTask { .. } | Step::ComponentTask { .. } => {
                FADES.into_iter().find(|f| f.first_step() == self).expect("fade task").address()
            }
            Step::Brightness { fading, .. } => fading.resume(),
            Step::Component { fading, .. } => fading.resume,
        }
    }

    pub fn step(self, x: &mut Ctx) -> Flow {
        match self {
            Step::BrightnessTask { fading, step } => fading.task_start(x, step),
            Step::ComponentTask { fading, whiten } => fading.start(x, whiten),
            Step::Brightness { fading } => fading.step(x),
            Step::Component { fading, saved } => fading.step(x, saved),
        }
    }
}

/// Cedes `delay` frames (D0.w of the original; the trap takes its low byte); what follows is stage `resume`.
fn wait(x: &mut Ctx, delay: u16, resume: Step) -> Flow {
    super::wait(x, delay as u8, resume)
}

// ---------------------------------------------------------------------------------------------
// Brightness

/// A brightness fade: direction (D1 of the original), form (subroutine or task), pages (complete or partial).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Brightness {
    delta: u32,
    task: bool,
    partial: bool,
}

impl Brightness {
    /// Address following the step's cession.
    fn resume(self) -> u32 {
        match (self.task, self.partial) {
            (false, _) => 0x14b4,
            (true, false) => 0x152c,
            (true, true) => 0x158c,
        }
    }

    /// Start of a task: `step` = imposed number of steps (otherwise that of the state). It only starts if the
    /// pages, the delay and the number of steps are non-zero.
    fn task_start(self, x: &mut Ctx, step: Option<u8>) -> Flow {
        if let Some(n) = step {
            wr8(x.b, state::STEP, n);
        }
        if rd8(x.b, state::PAGES) == 0 || rd16(x.b, state::DELAY) == 0 || rd8(x.b, state::STEP) == 0 {
            return self.end(x);
        }
        self.first_step(x)
    }

    /// Start of a subroutine of `steps` steps (D0.b of the original); it only starts if some pages are checked.
    fn subroutine_start(self, x: &mut Ctx, steps: u8) -> Flow {
        if rd8(x.b, state::PAGES) == 0 {
            return self.end(x);
        }
        wr8(x.b, state::STEP, steps);
        self.first_step(x)
    }

    fn first_step(self, x: &mut Ctx) -> Flow {
        for (k, page) in PAGES.into_iter().enumerate() {
            let page = if self.partial && k == 0 { PARTIAL_START } else { page };
            wr32(x.b, state::ADDRESSES + 4 * k as u32, page);
        }
        self.step(x)
    }

    /// One step, then the cession until the next one, or the end.
    fn step(self, x: &mut Ctx) -> Flow {
        brightness_step(x.b, self.delta, self.partial);
        let remainder = rd8(x.b, state::STEP).wrapping_sub(1);
        wr8(x.b, state::STEP, remainder);
        if remainder == 0 {
            return self.end(x);
        }
        let delay = rd16(x.b, state::DELAY);
        wait(x, delay, Step::Brightness { fading: self })
    }

    /// End: no more pages; the subroutine returns to its caller, the task ends.
    fn end(self, x: &mut Ctx) -> Flow {
        wr8(x.b, state::PAGES, 0);
        if self.task {
            super::finish(x)
        } else {
            Flow::Done
        }
    }
}

const fn subroutine(delta: u32) -> Brightness {
    Brightness { delta, task: false, partial: false }
}
const fn task(delta: u32, partial: bool) -> Brightness {
    Brightness { delta, task: true, partial }
}

/// Complete fade of a subroutine: 16 steps.
fn full(x: &mut Ctx, delta: u32) -> Flow {
    subroutine(delta).subroutine_start(x, STEP_FULL)
}

/// `0x144c`: complete fade from black (subroutine: the caller's continuation waits for the end).
pub fn rise(x: &mut Ctx) -> Flow {
    full(x, RISES)
}

/// `0x1460`: complete fade to black.
pub fn descent(x: &mut Ctx) -> Flow {
    full(x, DESCEND)
}

/// `0x146c`: fade to black of `steps` steps (D0.b of the original).
pub fn descent_steps(x: &mut Ctx, steps: u8) -> Flow {
    subroutine(DESCEND).subroutine_start(x, steps)
}

// tasks: 0x14ba / 0x14d0 complete fade, 0x14c8 / 0x14de number of steps of the state, 0x1536 / 0x153e partial (see
// `TASKS`)

// ---------------------------------------------------------------------------------------------
// Per component

/// A per-component fade: target, form and address following the step's cession.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Component {
    target: Target,
    task: bool,
    resume: u32,
}

/// The six per-component fades, by entry address.
const COMPONENTS: [(u32, Component); 6] = [
    (0x1ccc2, Component { target: Target::White, task: false, resume: 0x1cce8 }),
    (0x1ce30, Component { target: Target::Colours, task: false, resume: 0x1ce54 }),
    (0x1ce9e, Component { target: Target::Palettes, task: false, resume: 0x1cec4 }),
    (0x1ccf8, Component { target: Target::White, task: true, resume: 0x1cd1e }),
    (0x1ce64, Component { target: Target::Colours, task: true, resume: 0x1ce8a }),
    (0x1ced4, Component { target: Target::Palettes, task: true, resume: 0x1cefe }),
];

fn component(entry: u32) -> Component {
    COMPONENTS.into_iter().find(|&(e, _)| e == entry).unwrap().1
}

impl Component {
    /// Entry: registers saved, 16 steps (task `0x1ced4` first whitens the pages, its other entry
    /// `0x1cf12` does not: `whiten`). Of the registers that the original saves (`movem.l D0-D7/A0-A6,-(SP)`) and
    /// restores at the end, only the leftovers D2-D5 are reread after.
    fn start(self, x: &mut Ctx, whiten: bool) -> Flow {
        let saved = super::table::table(x.b).leftovers;
        if whiten {
            white_out(x.b);
        }
        wr8(x.b, state::STEP, STEP_FULL);
        wr8(x.b, CURRENT_COMPONENT, 1);
        self.step(x, saved)
    }

    fn step(self, x: &mut Ctx, saved: Leftovers) -> Flow {
        component_step(x.b, self.target);
        let remainder = rd8(x.b, state::STEP).wrapping_sub(1);
        wr8(x.b, state::STEP, remainder);
        if remainder != 0 {
            let delay = rd16(x.b, state::DELAY);
            return wait(x, delay, Step::Component { fading: self, saved });
        }
        wr8(x.b, state::PAGES, 0);
        wr8(x.b, CURRENT_COMPONENT, 0);
        super::table::table(x.b).leftovers = saved;
        if self.task {
            super::finish(x)
        } else {
            Flow::Done
        }
    }
}

/// `0x1ccc2`: per-component fade to white (subroutine).
pub fn to_white(x: &mut Ctx) -> Flow {
    component(0x1ccc2).start(x, false)
}

/// `0x1ce30`: per-component fade to the colors.
pub fn to_colours(x: &mut Ctx) -> Flow {
    component(0x1ce30).start(x, false)
}

/// `0x1ce9e`: per-component fade to the palettes.
pub fn to_palettes(x: &mut Ctx) -> Flow {
    component(0x1ce9e).start(x, false)
}

pub const NATIVES: &[Native] = &[];

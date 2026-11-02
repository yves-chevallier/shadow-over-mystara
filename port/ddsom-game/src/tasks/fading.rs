//! Palette fades (V3): one step per cession, until the end of the count (see `docs/video.md`, "fades").
//!
//! Two families, each in two forms: a *task* (created by `0x9b6`, it ends with `0x9f8` at the
//! end of the fade) and a *subroutine* (called by `jsr` from a sequence, it returns when the fade is finished;
//! the caller therefore waits for the end). The steps cede control through `jsr 0xa40` (D0.w = delay `A5+0x490`); the
//! next
//! step is a stage of the state machine ([`Step`]), which carries the fade in progress.
//!
//! * **Brightness**: each step adds ±0x1000 to the colors of the checked pages (`palette::brightness_step`).
//! The state is at `A5+0x48e` (pages, remaining steps, delay, addresses of the four pages); the direction is a field
//! of
//!   the stage (the original keeps it in D1, which the cession saves and restores, and which the context also keeps).
//! * **Per component**: each step brings each component one notch closer (`palette::component_step`);
//!   16 steps, `A5-0x65ec` raised during the fade, registers saved on entry (`movem`, kept in the stage) and
//!   restored at the end.
//!
//! The subroutines are functions ([`rise`], [`descent`], [`descent_d0`], [`to_white`],
//! [`to_colours`], [`to_palettes`]) that the steps call directly (the SET doors too:
//! `families::set::exit_fading`).

use crate::mem::{rd16, rd8, set_low_word, wr32, wr8, A5};
use crate::modes::Flow;
use crate::palette::{brightness_step, component_step, white_out, Target};
use crate::scene::Ctx;
use crate::Native;

/// State of the brightness fade (`A4` in the original).
mod state {
    use crate::mem::A5;
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
    /// Next step of a brightness fade; `a0`: A0 of the subroutine's caller, which the fade does not touch
    /// (the original keeps it in its registers, which the cession saves and restores) and which the caller re-reads
    /// on return (block
    /// of the shared state of the scene-exit sequences, object of a SET door...).
    Brightness { fading: Brightness, a0: u32 },
    /// Next step of a per-component fade; `regs`: D0-D7/A0-A6 saved on entry.
    Component { fading: Component, regs: Regs },
}

/// Registers saved by `movem.l D0-D7/A0-A6,-(SP)`.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Regs {
    d: [u32; 8],
    a: [u32; 7],
}

/// The fade tasks, by entry.
const TASKS: [(u32, Step); 10] = [
    (0x14ba, Step::BrightnessTask { fading: task(RISES, false), step: Some(STEP_FULL) }),
    (0x14c8, Step::BrightnessTask { fading: task(RISES, false), step: None }),
    (0x14d0, Step::BrightnessTask { fading: task(DESCEND, false), step: Some(STEP_FULL) }),
    (0x14de, Step::BrightnessTask { fading: task(DESCEND, false), step: None }),
    (0x1536, Step::BrightnessTask { fading: task(RISES, true), step: None }),
    (0x153e, Step::BrightnessTask { fading: task(DESCEND, true), step: None }),
    (0x1ccf8, Step::ComponentTask { fading: COMPONENTS[3].1, whiten: false }),
    (0x1ce64, Step::ComponentTask { fading: COMPONENTS[4].1, whiten: false }),
    (0x1ced4, Step::ComponentTask { fading: COMPONENTS[5].1, whiten: true }),
    (0x1cf12, Step::ComponentTask { fading: COMPONENTS[5].1, whiten: false }),
];

/// First stage of the fade task that starts at `entry`.
pub fn task_from_entry(entry: u32) -> Option<Step> {
    TASKS.into_iter().find(|&(e, _)| e == entry).map(|(_, step)| step)
}

impl Step {
    /// Address that the original resumes at this stage (task entry, or return from the step's cession).
    pub fn address(self) -> u32 {
        match self {
            Step::BrightnessTask { .. } | Step::ComponentTask { .. } => {
                TASKS.into_iter().find(|&(_, e)| e == self).expect("fade task").0
            }
            Step::Brightness { fading, .. } => fading.resume(),
            Step::Component { fading, .. } => fading.resume,
        }
    }

    pub fn step(self, x: &mut Ctx) -> Flow {
        match self {
            Step::BrightnessTask { fading, step } => fading.task_start(x, step),
            Step::ComponentTask { fading, whiten } => fading.start(x, whiten),
            Step::Brightness { fading, a0 } => {
                x.c.a[0] = a0;
                fading.step(x)
            }
            Step::Component { fading, regs } => fading.step(x, regs),
        }
    }
}

/// Cedes `delay` frames (D0.w, the high word of D0 is kept); what follows is stage `resume`.
fn wait(x: &mut Ctx, delay: u16, resume: Step) -> Flow {
    set_low_word(&mut x.c.d[0], delay);
    crate::kernel::wait(x.c, x.b, resume);
    Flow::Yield
}

// ---------------------------------------------------------------------------------------------
// Brightness

/// A brightness fade: direction (D1), form (subroutine or task), pages (complete or partial).
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
        x.c.d[1] = self.delta;
        if let Some(n) = step {
            wr8(x.b, state::STEP, n);
        }
        x.c.a[4] = state::BASE;
        if rd8(x.b, state::PAGES) == 0 || rd16(x.b, state::DELAY) == 0 || rd8(x.b, state::STEP) == 0 {
            return self.end(x);
        }
        self.first_step(x)
    }

    /// Start of a subroutine: D0.b = number of steps; it only starts if some pages are checked.
    fn subroutine_start(self, x: &mut Ctx) -> Flow {
        x.c.d[1] = self.delta;
        x.c.a[4] = state::BASE;
        if rd8(x.b, state::PAGES) == 0 {
            return self.end(x);
        }
        wr8(x.b, state::STEP, x.c.d[0] as u8);
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
        let a0 = x.c.a[0];
        wait(x, delay, Step::Brightness { fading: self, a0 })
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

/// Complete fade of a subroutine: D0.b = 16 steps.
fn full(x: &mut Ctx, delta: u32) -> Flow {
    crate::mem::set_low_byte(&mut x.c.d[0], STEP_FULL);
    subroutine(delta).subroutine_start(x)
}

/// `0x144c`: complete fade from black (subroutine: the caller's continuation waits for the end).
pub fn rise(x: &mut Ctx) -> Flow {
    full(x, RISES)
}

/// `0x1460`: complete fade to black.
pub fn descent(x: &mut Ctx) -> Flow {
    full(x, DESCEND)
}

/// `0x146c`: fade to black of D0.b steps.
pub fn descent_d0(x: &mut Ctx) -> Flow {
    subroutine(DESCEND).subroutine_start(x)
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
    /// `0x1cf12` does not: `whiten`).
    fn start(self, x: &mut Ctx, whiten: bool) -> Flow {
        let mut regs = Regs { d: x.c.d, a: [0; 7] };
        regs.a.copy_from_slice(&x.c.a[..7]);
        if whiten {
            white_out(x.b);
        }
        wr8(x.b, state::STEP, STEP_FULL);
        wr8(x.b, CURRENT_COMPONENT, 1);
        self.step(x, regs)
    }

    fn step(self, x: &mut Ctx, regs: Regs) -> Flow {
        component_step(x.b, self.target);
        let remainder = rd8(x.b, state::STEP).wrapping_sub(1);
        wr8(x.b, state::STEP, remainder);
        if remainder != 0 {
            let delay = rd16(x.b, state::DELAY);
            return wait(x, delay, Step::Component { fading: self, regs });
        }
        wr8(x.b, state::PAGES, 0);
        wr8(x.b, CURRENT_COMPONENT, 0);
        x.c.d = regs.d;
        x.c.a[..7].copy_from_slice(&regs.a);
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

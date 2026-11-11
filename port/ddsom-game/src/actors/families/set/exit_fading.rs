use super::super::set_b;
use super::{Bench, Cx};
use crate::screens::modes::{Flow, Sequence};
use crate::world::objects::Obj;

/// Brightness fade parameters (`tasks::fading`): pages (bit k: page k) and delay between two steps.
pub(super) const PAGES: u32 = 0x48e;
const DELAY: u32 = 0x490;

/// Where the task resumes after the fade (return address of the original's call to 0x1460).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Step {
    /// Porte de sortie (SET 7), changement de zone (0xcb896) : `exit_gate::after_fade`.
    Zone(Obj),
    /// Exit door (SET 7), next scene (0xcb9c0): `exit_gate::next_scene`.
    Scene(Obj),
    /// Porte de passage (SET 29, 0xd8f06) : `set_b::door_b::after_fade`.
    Passage(Obj),
}

impl Step {
    pub const fn address(self) -> u32 {
        match self {
            Step::Zone(_) => 0xcb896,
            Step::Scene(_) => 0xcb9c0,
            Step::Passage(_) => 0xd8f06,
        }
    }

    /// The end of the door's handler, then its return (`rts`) into the dispatch loop.
    pub fn step(self, s: &mut crate::screens::scene::Ctx) -> Flow {
        let (Step::Zone(door) | Step::Scene(door) | Step::Passage(door)) = self;
        crate::core::dispatch::resume_handler(s.b, Some(door), |b, a| {
            let mut x = Cx { c: a, b, o: door };
            match self {
                Step::Zone(_) => super::exit_gate::after_fade(&mut x),
                Step::Scene(_) => super::exit_gate::next_scene(&mut x),
                Step::Passage(_) => set_b::door_b::after_fade(&mut x),
            }
        });
        Flow::Done
    }
}

/// Full fade to black (`jsr 0x1460`, delay 2, four pages), followed by `next`. The fade yields: the handler must
/// return at once (tail call), and the dispatch loop that called it gives up; the task will resume through the
/// fade's steps, then `next`, then the loop (`dispatch::Step::Dispatch`).
pub fn fade_to_black<R: Bench>(x: &mut Cx<'_, R>, next: fn(Obj) -> Step) {
    x.set_a5_u16(DELAY, 2);
    x.set_a5_u8(PAGES, 0xf);
    // The fade runs as a subroutine of the task (`Sequence::subroutine`); the leftovers (`dispatch::Leftovers`)
    // go to the game's state for it, and come back (an empty fade returns at once and runs `next`, which resumes
    // the door through `dispatch::resume_handler`).
    let door = x.o;
    let leftovers = crate::core::dispatch::Leftovers { d2: *x.c.d2(), d3: *x.c.d3(), d4: *x.c.d4(), d5: *x.c.d5() };
    crate::core::tasks::table::table(x.b).leftovers = leftovers;
    let mut s = crate::screens::scene::Ctx::new(&mut *x.b);
    s.subroutine(crate::core::tasks::fading::descent, next(door));
    let l = crate::core::tasks::table::table(x.b).leftovers;
    (*x.c.d2(), *x.c.d3(), *x.c.d4(), *x.c.d5()) = (l.d2, l.d3, l.d4, l.d5);
}

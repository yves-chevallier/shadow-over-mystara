//! Tools: the natives of `crate::core::kernel` for the original code that calls them (arguments and results in the registers
//! of the original).

use crate::world::World;
use m68k::Cpu;

use crate::core::kernel::*;
use crate::tools::frames::*;
use crate::Native;
use crate::core::tasks::table::{slot, table};
use crate::core::tasks::Task;

macro_rules! native {
    ($ident:ident, $entry:expr, $body:expr) => {
        pub(crate) const $ident: Native = Native {
            entry: $entry,
            name: stringify!($ident),
            run: crate::Body::WithCpu($body),
            touches_sound: false,
            outputs: 0,
        };
    };
}

// ---------------------------------------------------------------------------------------------
// The wrappers, natives for the calls by address that remain (arguments in registers: D0 = offset of a task in the
// table, A0 = entry, D1 = count). They leave registers and flags intact, except the cessions: the cession
// wrappers called by `jsr` take their return address as resumption (the continuation of the call in progress).

/// Cession wrapper called by `jsr`: its return address is the resumption.
fn envelope(c: &mut Cpu, b: &mut World, frames: Option<u8>) {
    let resume = crate::tools::frames::pop_return_68k(c, b);
    match frames {
        Some(frames) => wait(b, frames, resume),
        None => cede_ready(b, resume),
    }
}

/// The task of the game whose entry is `entry` (A0 of the original code that calls the wrappers), if any.
pub fn task_at(entry: u32) -> Option<Task> {
    use crate::core::tasks::fading::Fade;
    let fades = [
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
    [
        Task::Startup,
        Task::Modes,
        Task::Game,
        Task::Service,
        Task::Credits,
        Task::Interface,
        Task::Portraits,
        Task::Prompt,
        Task::Freeze,
        Task::Scores,
    ]
    .into_iter()
    .chain(fades.map(Task::Fading))
    .find(|t| t.address() == entry)
}

/// [`task_at`], for a task that the original code creates or restarts: it must be one of the game.
fn task_of_game(entry: u32) -> Task {
    task_at(entry).unwrap_or_else(|| panic!("entry {entry:06x}: not a task of the game"))
}

/// 0x9b6: creates task D0 (offset), entry A0.
fn create(c: &mut Cpu, b: &mut World) {
    create_task(b, c.d[0], task_of_game(c.a[0]));
    rts(c, b);
}

/// 0xa14: kills task D0.
fn kill(c: &mut Cpu, b: &mut World) {
    kill_task(b, c.d[0]);
    rts(c, b);
}

/// 0xaac: suspends task D0.
fn suspend(c: &mut Cpu, b: &mut World) {
    table(b).suspend(slot(c.d[0]));
    rts(c, b);
}

/// 0xac8: wakes task D0 with count D1.
fn wake(c: &mut Cpu, b: &mut World) {
    table(b).wake(slot(c.d[0]), c.d[1] as u8);
    rts(c, b);
}

/// `0xaf6` (`trap #12`): D0.b ≠ 0 masks all the task's interrupts, D0.b = 0 enables them.
fn mask(c: &mut Cpu, b: &mut World) {
    set_interrupt_mask(b, if c.d[0] as u8 == 0 { 0 } else { MASK_ALL });
    rts(c, b);
}

// wrappers
native!(CREATE_TASK, 0x9b6, create);
native!(FINISH_TASK, 0x9f8, |_, b| finish(b));
native!(KILL_TASK, 0xa14, kill);
native!(WAIT, 0xa40, |c, b| envelope(c, b, Some(c.d[0] as u8)));
native!(CEDE, 0xa86, |c, b| envelope(c, b, None));
native!(SUSPEND_TASK, 0xaac, suspend);
native!(WAKE_TASK, 0xac8, wake);
native!(RESTART_TASK, 0xae4, |c, b| restart(b, task_of_game(c.a[0])));
native!(MASK, 0xaf6, mask);
native!(WAIT_ONE_FRAME, 0xb64, |c, b| envelope(c, b, Some(1)));

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

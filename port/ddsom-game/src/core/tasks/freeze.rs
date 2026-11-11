//! Freeze task (`0xee080`, slot 14 of the table): busy wait and "fast forward".
//!
//! On each pass of the scheduler it does what VBLANK does for sleeping tasks, but
//! only for the main task (slot 0), and only when a present player presses a button
//! during a game: the game then chains its steps without waiting for the frame (see `docs/boucle.md` §1). In all
//! other cases it cedes control doing nothing (`Wait::Ready`): this is the busy wait that occupies the
//! overclocked processor ([`idle_wait`] recognizes it).
//!
//! The step is the same at entry (`0xee080`, new task) and after each of the two cessions (`0xee0fa`,
//! `0xee16a`: the original loops back with a `bra 0xee080`): three stages ([`Step`]), a single step.

use super::table::{record, table, State, Resume, Table, SLOTS};
use super::{cede, Wait};
use crate::world::mem::A5;
use crate::screens::modes::Flow;
use crate::screens::scene::Ctx;

/// Work RAM variables (offsets from A5).
mod var {
    /// Settings word (DSW): bit 7 is 1 in normal operation.
    pub const DSW: u32 = 0x7fd0;
    /// Game in progress, and active game screen.
    pub const GAME: u32 = 0x30;
    pub const SCREEN: u32 = 0x33;
    /// Buttons of the four players (`A5+0..3`) and their presence flags.
    pub const BUTTONS: u32 = 0;
    pub const PRESENT: [u32; 4] = [0x5de, 0x7de, 0x9de, 0xbde];
}

/// Slot of the freeze task, and that of the main task.
pub const SLOT: usize = 14;
const MAIN: usize = 0;
/// Button bits that count.
const BUTTON_MASK: u8 = 0x16;
/// Bit of the settings word designating normal operation.
const NORMAL: u16 = 0x80;

crate::steps! {
    /// Stages of the task: its entry, and the returns from its two cessions.
    pub enum Step {
        Entry = 0xee080 => step,
        /// After a cession with no effect.
        AfterCession = 0xee0fa => step,
        /// After a cession that counted down the main task.
        AfterCountdown = 0xee16a => step,
    }
}

/// One step: the effect of the pass, then the cession.
fn step(x: &mut Ctx) -> Flow {
    let normal = x.v16(var::DSW) & NORMAL;
    let resume = if normal == 0 {
        wake_all(x);
        Step::AfterCession
    } else if x.v8(var::SCREEN) != 0 && x.v8(var::GAME) != 0 {
        fast_forward(x)
    } else {
        Step::AfterCession
    };
    cede(x, Wait::Ready, resume)
}

/// `0xee0ce` (settings without bit 7, never seen in this game): countdown like VBLANK, for all tasks.
fn wake_all(x: &mut Ctx) {
    let tasks = table(x.b);
    for slot in 0..SLOTS {
        tasks.count_down(slot);
    }
}

/// `0xee0fc`: if a present player presses a button, the main task loses one wait frame. Returns
/// the stage that follows the cession.
fn fast_forward(x: &mut Ctx) -> Step {
    let mut seen = 0;
    for (player, present) in var::PRESENT.into_iter().enumerate() {
        if x.v8(present) != 0 {
            seen |= x.v8(var::BUTTONS + player as u32) & BUTTON_MASK;
        }
    }
    if seen == 0 {
        return Step::AfterCession;
    }
    table(x.b).count_down(MAIN);
    Step::AfterCountdown
}

/// The scheduler is idling: the freeze task is the only one ready, it has just ceded control, and its next
/// step would do nothing. The game state cannot change before the next interrupt: `idle` (`lib.rs`) can
/// advance up to it. `ram` is the work RAM (FF0000-FFFFFF).
pub fn idle_wait(tasks: &Table, ram: &[u16]) -> bool {
    let ready = |slot: usize| tasks.slots[slot].state.castable();
    if !ready(SLOT) || (0..SLOTS).any(|slot| slot != SLOT && ready(slot)) {
        return false;
    }
    // it ceded through `jsr 0xa86` from one of its two cessions, and it is the last task launched
    // (the scheduler state is then that of each following pass)
    let ceded = matches!(tasks.slots[SLOT].resume, Resume::Step(super::Step::Freeze(Step::AfterCession | Step::AfterCountdown)));
    let main_waiting = tasks.slots[MAIN].state == State::Waiting;
    ceded && tasks.current == SLOT && !step_would_act(ram, main_waiting)
}

/// Same test on the RAM table of a reference machine (records FF0064: state, resume PC `0xa8c`
/// of the wrapper, stack where the return address follows the 15 saved registers; current task at FF0056).
pub fn idle_wait_original(ram: &[u16]) -> bool {
    let byte = |a: u32| (ram[(a as usize & 0xffff) / 2] >> if a & 1 == 0 { 8 } else { 0 }) as u8;
    let long = |a: u32| (ram[(a as usize & 0xffff) / 2] as u32) << 16 | ram[(a as usize & 0xffff) / 2 + 1] as u32;
    let ready = |slot: usize| byte(record(slot)) >= State::Ready.byte();
    let freeze = record(SLOT);
    if !ready(SLOT) || (0..SLOTS).any(|slot| slot != SLOT && ready(slot)) {
        return false;
    }
    let after_yield = [Step::AfterCession.address(), Step::AfterCountdown.address()];
    if long(freeze + 4) != 0xa8c || !after_yield.contains(&long(long(freeze + 8) + 60)) || long(0xff_0056) != freeze {
        return false;
    }
    let main_waiting = byte(record(MAIN)) == State::Waiting.byte();
    !step_would_act(ram, main_waiting)
}

/// The next step would do something: settings without bit 7 (count everything down), or a present player presses a
/// button in game while the main task is waiting.
fn step_would_act(ram: &[u16], main_waiting: bool) -> bool {
    let byte = |a: u32| (ram[(a as usize & 0xffff) / 2] >> if a & 1 == 0 { 8 } else { 0 }) as u8;
    let var = |offset: u32| A5.wrapping_add(offset);
    if ram[(var(var::DSW) as usize & 0xffff) / 2] & NORMAL == 0 {
        return true;
    }
    byte(var(var::SCREEN)) != 0
        && byte(var(var::GAME)) != 0
        && main_waiting
        && (0..4).any(|player| byte(var(var::PRESENT[player])) != 0 && byte(var(var::BUTTONS + player as u32)) & BUTTON_MASK != 0)
}

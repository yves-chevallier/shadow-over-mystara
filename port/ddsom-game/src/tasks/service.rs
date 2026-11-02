//! TEST menu task (`0x29162`), created by the credits task when the TEST switch is activated.
//!
//! At creation: menu record reset to zero, task in slot 15 (`0xf0`) killed. Then, on
//! each frame, the menu state handler (`service::handler`), followed by a wait frame. The
//! exit (9. EXIT) replaces the task with the boot task.

use crate::modes::{Flow, Sequence};
use crate::scene::Ctx;
use crate::service::{self, MENU, MENU_LONGS};

crate::steps! {
    /// Stages of the task.
    pub enum Step {
        Entry = 0x29162 => entry,
        /// After the wait frame: one turn.
        Turn = 0x29190 => turn,
        /// Return of the handler when it ceded control on the way: the wait frame.
        HandlerReturn = 0x2918a => wait,
        /// Resume of the menu after the wait frame that follows the clearing of the screen (`service::show_menu`).
        MenuAfterClear = 0x291ec => service::menu_after_clear,
    }
}

/// Slot killed at entry.
const KILLED_SLOT: u16 = 0xf0;

fn entry(x: &mut Ctx) -> Flow {
    for k in 0..MENU_LONGS {
        crate::mem::wr32(x.b, MENU + 4 * k, 0);
    }
    crate::kernel::kill_task(x.b, KILLED_SLOT as u32);
    turn(x)
}

/// `0x2917a`: the handler, called as by `jsr` (its continuation is a call in progress); if it returns without
/// ceding, the wait frame.
fn turn(x: &mut Ctx) -> Flow {
    x.push_return(Step::HandlerReturn);
    match service::handler(x) {
        Flow::Done => {
            x.drop_return();
            wait(x)
        }
        Flow::Yield => Flow::Yield,
    }
}

/// `0x2918a`: `jsr 0xb64`, then a new turn.
fn wait(x: &mut Ctx) -> Flow {
    crate::tasks::one_frame(x, Step::Turn)
}

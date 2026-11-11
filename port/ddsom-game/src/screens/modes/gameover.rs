//! Phase 20 (`0x1c80c`): game over.
//!
//! The game is over (no player or credit left): the player interface task is killed, the
//! terrain fades out, everything is cleared, then the main task is *replaced* by the mode machine
//! (`jmp 0xae4` with A0 = `0x133fe`). The startup flag is raised: the machine begins with
//! the game-over screen (sub-mode 20, `ranking.rs`) instead of the title.
//!
//! Replacing the task does not touch its stack: the mode machine restarts with the stack pointer
//! the task had at its last yield (the video clear one, `0xc32`).

use crate::world::ram::Long;
use super::{mode, Flow, Sequence};
use crate::hw::video::regs;
use crate::screens::scene::fading;
use crate::screens::vars as sv;
use crate::world::tasks::Ctx;
use crate::Native;
pub use crate::world::tasks::stages::Gameover as Step;

/// Mode machine loop, which the task becomes.
const MODE_LOOP: crate::world::tasks::Task = crate::world::tasks::Task::Modes;

/// Player interface task (slot 0x30).
const PLAYER_UI_SLOT: u16 = 0x30;
const VIDEO_ON: u16 = 0x12ce;

/// Phase 20 of the game (`0x1c80c`): game over.
pub fn phase_20(x: &mut Ctx) -> Flow {
    crate::world::tasks::traps::kill_task(x.b, PLAYER_UI_SLOT as u32);
    fading::DELAY.set(x.b, 1);
    fading::PAGES.set(x.b, 0xf);
    x.subroutine(crate::services::fading::descent, Step::Faded)
}

crate::steps! {
    impl Step {
        Faded = 0x1c828 => |x| {
            regs::LAYER_CONTROL.set(x.b, VIDEO_ON);
            sv::LAYERS.set(x.b, VIDEO_ON);
            sv::SCROLL_MODE.set(x.b, 0);
            let flags = sv::VIDEO_FLAGS.get(x.b) & !1;
            sv::VIDEO_FLAGS.set(x.b, flags);
            x.subroutine(crate::screens::scene::black_screen::start, Step::Cleared)
        },
        Cleared = 0x1c846 => |x| {
            crate::actors::families::small::zero_game_variables(x.b);
            crate::world::ram::GAME.set(x.b, 1);
            reset_modes(x);
            mode::BOOTED.set(x.b, 1);
            crate::services::sound::service(x.b, crate::services::sound::Service::GeneralStop);
            crate::world::tasks::restart(x, MODE_LOOP)
        },
    }
}

/// `0x133dc`: clears the mode machine state block (64 bytes) and that of the ranking
/// screen (four longs at `FF2B10`, `0xfc562`).
pub fn reset_modes(x: &mut Ctx) {
    for i in 0..16 {
        Long(mode::BASE + 4 * i).set(x.b, 0);
    }
    for i in 0..4 {
        Long(super::ranking::STATE + 4 * i).set(x.b, 0);
    }
}

pub const NATIVES: &[Native] = &[];

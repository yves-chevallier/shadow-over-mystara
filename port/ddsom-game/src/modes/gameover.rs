//! Phase 20 (`0x1c80c`): game over.
//!
//! The game is over (no player or credit left): the player interface task is killed, the
//! terrain fades out, everything is cleared, then the main task is *replaced* by the mode machine
//! (`jmp 0xae4` with A0 = `0x133fe`). The startup flag is raised: the machine begins with
//! the game-over screen (sub-mode 20, `ranking.rs`) instead of the title.
//!
//! Replacing the task does not touch its stack: the mode machine restarts with the stack pointer
//! the task had at its last yield (the video clear one, `0xc32`).

use super::{mode, Flow, Sequence};
use crate::mem::{rd16, wr16, wr32, wr8, A5};
use crate::scene::Ctx;
use crate::Native;

/// Mode machine loop, which the task becomes.
const MODE_LOOP: u32 = 0x133fe;

/// Player interface task (slot 0x30).
const PLAYER_UI_SLOT: u16 = 0x30;
const VIDEO_ON: u16 = 0x12ce;
const CPS_REG_VIDEO_B: u32 = 0x804166;
/// Original resumes after each yield.
const FADED: u32 = 0x1c828;
const CLEARED: u32 = 0x1c846;

/// Phase 20 of the game (`0x1c80c`): game over.
pub fn phase_20(x: &mut Ctx) -> Flow {
    crate::kernel::kill_task(x.b, PLAYER_UI_SLOT as u32);
    wr16(x.b, A5 + 0x490, 1);
    wr8(x.b, A5 + 0x48e, 0xf);
    x.subroutine(crate::tasks::fading::descent, Step::Faded)
}

crate::steps! {
    /// Steps (resume points) of the state machine.
    pub enum Step {
        Faded = FADED => |x| {
            wr16(x.b, CPS_REG_VIDEO_B, VIDEO_ON);
            wr16(x.b, A5 + 0x14, VIDEO_ON);
            wr8(x.b, A5 + 0x53c, 0);
            let flags = rd16(x.b, A5 + 0x12) & !1;
            wr16(x.b, A5 + 0x12, flags);
            x.subroutine(crate::video::black_screen::start, Step::Cleared)
        },
        Cleared = CLEARED => |x| {
            crate::families::small::zero_game_variables(x.b);
            wr8(x.b, A5 + 0x30, 1);
            reset_modes(x);
            wr8(x.b, mode::BOOTED, 1);
            crate::sound::service(x.b, crate::sound::Service::GeneralStop);
            crate::tasks::restart(x, MODE_LOOP)
        },
    }
}

/// `0x133dc`: clears the mode machine state block (64 bytes) and that of the ranking
/// screen (four longs at `FF2B10`, `0xfc562`).
pub fn reset_modes(x: &mut Ctx) {
    for i in 0..16 {
        wr32(x.b, mode::BASE + 4 * i, 0);
    }
    for i in 0..4 {
        wr32(x.b, 0xff_2b10 + 4 * i, 0);
    }
}

pub const NATIVES: &[Native] = &[];

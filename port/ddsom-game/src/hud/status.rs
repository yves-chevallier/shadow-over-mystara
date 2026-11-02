//! Common parts of the interface: the status line (blinking counter, `0x17814`) and the end of the game
//! when nobody is playing any more (`0x175a8`), with its on-screen countdown.

use cps2::Cps2Bus;

use super::layer::{Brush, COLUMN, EMPTY};
use super::{panel, slot, v8, var, Slot};
use crate::mem::{rd16, rd8, wr16, wr32, wr8};
use crate::modes::Flow;
use crate::scene::Ctx;

/// Status line variables (A5 - 0x70f5 and following) and of A5.
mod line {
    /// Display flag (`A5+0x5f`) and displayed value (`A5+0x45`).
    pub const ON: u32 = 0x5f;
    pub const VALUE: u32 = 0x45;
    /// Last flag, last value, true during blinking, remaining time (the latter also counts
    /// the blink phase: bit 3).
    pub const SEEN_ON: u32 = 0xff_0f0b;
    pub const SEEN_VALUE: u32 = 0xff_0f08;
    pub const BLINKING: u32 = 0xff_0f09;
    pub const BLINK_TIMER: u32 = 0xff_0f0a;
    /// Position of the status line in the character layer, and blink duration (frames).
    pub const AT: u32 = 0x91_2014;
    pub const BLINK_FRAMES: u8 = 0x60;
    /// Tiles: icon (two columns) and digits of the value.
    pub const ICON_TILE: u16 = 0x9f3;
    pub const DIGIT_TILE: u16 = 0x9f6;
    pub const ATTR: u16 = 0x14;
}

/// 0x178d4: clears the status line (2x2 cells).
fn clear_status_line(b: &mut Cps2Bus) {
    for column in 0..2 {
        for row in 0..2 {
            wr32(b, line::AT + column * COLUMN + row * 4, EMPTY);
        }
    }
}

/// 0x17814: the status line, two columns of two cells (icon and digit): the value `A5+0x45` in BCD,
/// which blinks for 96 frames at each change. Returns without drawing anything when the flag is off.
pub fn status_line(b: &mut Cps2Bus) {
    let on = v8(b, line::ON);
    let mut shown = on; // the original draws "D0": the flag, or the value depending on the path
    let seen_on = rd8(b, line::SEEN_ON);
    wr8(b, line::SEEN_ON, on);
    if on == seen_on {
        if on == 0 {
            return;
        }
        let value = v8(b, line::VALUE);
        shown = value;
        if value != rd8(b, line::SEEN_VALUE) {
            wr8(b, line::SEEN_VALUE, value);
            wr8(b, line::BLINK_TIMER, line::BLINK_FRAMES);
            wr8(b, line::BLINKING, 1);
        }
    } else if on == 0 {
        return clear_status_line(b);
    }
    let timer = rd8(b, line::BLINK_TIMER);
    if timer != 0 {
        let timer = timer - 1;
        wr8(b, line::BLINK_TIMER, timer);
        if timer == 0 {
            wr8(b, line::BLINKING, 0);
            if v8(b, line::VALUE) == 0 {
                return clear_status_line(b);
            }
        }
    }
    if v8(b, line::VALUE) == 0 || rd8(b, line::BLINK_TIMER) & 8 != 0 {
        return clear_status_line(b);
    }
    let mut brush = Brush::new(b, line::AT);
    wr32(brush.b, line::AT, (line::ICON_TILE as u32) << 16 | line::ATTR as u32);
    wr32(brush.b, line::AT + COLUMN, (line::ICON_TILE as u32 + 1) << 16 | line::ATTR as u32);
    brush.at += 4;
    // the value in packed decimal: tens and units, like the original (remainder and tens added together)
    let mut rest = shown as u16;
    let mut tens = 0u16;
    while rest >= 10 {
        rest -= 10;
        tens += 0x10;
    }
    let bcd = rest + tens;
    for digit in [((bcd as u8) >> 4) & 0xf, (bcd as u8) & 0xf] {
        brush.cell(line::DIGIT_TILE + digit as u16, line::ATTR);
        brush.skip_column_after_cell();
    }
}

// ---------------------------------------------------------------------------------------------
// End of the game: nobody is playing any more
// ---------------------------------------------------------------------------------------------

/// Record of the end countdown (A5 - 0x713a) and its fields.
mod over {
    pub const RECORD: u32 = 0xff_0ec6;
    pub const ACTIVE: u32 = 0x00;
    pub const STATE: u32 = 0x04;
    pub const FLAG: u32 = 0x27;
    pub const TIMER: u32 = 0x28;
    pub const PLAYERS_FREE: u32 = 0x30;
    /// States: start, countdown display, wait for the end.
    pub const START: u8 = 0;
    pub const DISPLAY: u8 = 2;
    pub const WAIT: u8 = 4;
    /// Frame of the message in the character layer, and digits of the countdown.
    pub const FRAME_AT: u32 = 0x91_253c;
    pub const DIGITS_AT: u32 = 0x91_313c;
    pub const ATTR: u16 = 0x1f;
    /// Duration of the "GAME OVER" message before returning to the attract screens (frames).
    pub const GAME_OVER_FRAMES: u16 = 0x3c;
    pub const PHASE_GAME_OVER: u8 = 0x14;
    /// Original routines: countdown start music, end music, exit to the modes.
    pub const MUSIC_START: crate::sound::Sound = crate::sound::CMD_05;
    pub const MUSIC_STOP: crate::sound::Sound = crate::sound::MEMO_REPLAY;
}

/// All the panels (even those that do not exist in the current layout).
fn all_panels() -> impl Iterator<Item = Slot> {
    (0..4).map(Slot::new)
}

/// 0x177ce: clears the message frame (30 columns of 4 cells).
fn clear_message(b: &mut Cps2Bus) {
    for column in 0..30 {
        for row in 0..4 {
            wr32(b, over::FRAME_AT + column * COLUMN + row * 4, EMPTY);
        }
    }
}

/// 0x176fe: `columns` columns of four consecutive tiles (step 0x10), the next column starting
/// one tile further.
fn frame_columns(brush: &mut Brush, mut tile: u16, columns: u32) {
    for _ in 0..columns {
        for row in 0..4 {
            brush.cell(tile, over::ATTR);
            if row < 3 {
                tile = tile.wrapping_add(0x10);
            }
        }
        tile = tile.wrapping_sub(0x2f);
        brush.at += COLUMN - 16;
    }
}

/// 0x176d8: frame of the countdown message ("CONTINUE ?").
fn draw_countdown_frame(b: &mut Cps2Bus) {
    let mut brush = Brush::new(b, over::FRAME_AT);
    frame_columns(&mut brush, 0x700, 16);
    frame_columns(&mut brush, 0x740, 7);
}

/// 0x177f6: frame of the "GAME OVER" message.
fn draw_game_over_frame(b: &mut Cps2Bus) {
    clear_message(b);
    let mut brush = Brush::new(b, over::FRAME_AT);
    frame_columns(&mut brush, 0x680, 16);
    frame_columns(&mut brush, 0x6c0, 14);
}

/// Glyphs of the message digits (3 columns of 4 cells), read in the code as data.
const BIG_DIGITS: u32 = 0x17bb6;

/// 0x17782: the two digits of the countdown in large size.
fn draw_countdown_digits(b: &mut Cps2Bus) {
    let value = rd8(b, over::RECORD + over::TIMER);
    let mut brush = Brush::new(b, over::DIGITS_AT);
    for digit in [value >> 4, value & 0xf] {
        let offset = rd16(brush.b, BIG_DIGITS + 2 * digit as u32) as i16 as i32;
        let mut glyph = BIG_DIGITS.wrapping_add_signed(offset);
        for _ in 0..3 {
            for _ in 0..4 {
                let tile = rd16(brush.b, glyph);
                glyph += 2;
                brush.cell(tile, over::ATTR);
            }
            brush.at += COLUMN - 16;
        }
    }
}

/// 0x17728: the longest continue countdown among the players in state 8; when none is
/// running any more, and the one that finished was flagged (`+0x27`), the state advances.
fn longest_countdown(b: &mut Cps2Bus) {
    let record = over::RECORD;
    let mut longest = 0i16;
    let mut who = Slot::new(0);
    for s in all_panels() {
        if s.p8(b, panel::STATE) != 8 {
            continue;
        }
        let timer = s.p16(b, panel::COUNTDOWN) as i16;
        if longest > timer {
            continue;
        }
        longest = timer;
        who = s;
    }
    wr16(b, record + over::TIMER, longest as u16);
    if longest != 0 || who.p8(b, panel::FLAG_27) == 0 {
        return;
    }
    let state = rd8(b, record + over::STATE);
    wr8(b, record + over::STATE, state.wrapping_add(2));
    wr8(b, record + over::FLAG, 1);
}

/// 0x175a8: when no panel is in the game any more, end-of-game countdown: music, message with
/// the count, then "GAME OVER" and return to the mode machine. Always returns `Flow::Done`.
pub fn countdown(x: &mut Ctx) -> Flow {
    if x.v8(var::GAME_ON) == 0 || x.v8(var::DISPLAY) == 0 {
        return Flow::Done;
    }
    let record = over::RECORD;
    let busy = all_panels().fold(0u8, |sum, s| sum.wrapping_add(s.p8(x.b, panel::STEP)));
    if busy != 0 {
        // a player has just come back: the message disappears
        if rd8(x.b, record + over::ACTIVE) == 0 {
            return Flow::Done;
        }
        clear_message(x.b);
        for off in [over::ACTIVE, over::STATE, over::PLAYERS_FREE] {
            wr8(x.b, record + off, 0);
        }
        crate::sound::play_global(x.b, over::MUSIC_STOP);
        return Flow::Done;
    }
    match rd8(x.b, record + over::STATE) {
        over::START => {
            if x.v8(var::CONTINUE_ALLOWED) != 0 {
                crate::sound::play_global(x.b, over::MUSIC_START);
                wr8(x.b, record + over::ACTIVE, 1);
                let state = rd8(x.b, record + over::STATE);
                wr8(x.b, record + over::STATE, state + 2);
                let free = all_panels().fold(1u8, |free, s| free.wrapping_sub(s.p8(x.b, panel::SUBSTEP)));
                wr8(x.b, record + over::PLAYERS_FREE, free);
                draw_countdown_frame(x.b);
                wr8(x.b, record + over::FLAG, 0);
                longest_countdown(x.b);
                draw_countdown_digits(x.b);
                Flow::Done
            } else {
                wr8(x.b, record + over::STATE, over::WAIT);
                wr8(x.b, record + over::FLAG, 1);
                wait_game_over(x)
            }
        }
        over::DISPLAY => {
            longest_countdown(x.b);
            draw_countdown_digits(x.b);
            Flow::Done
        }
        over::WAIT => wait_game_over(x),
        _ => Flow::Done,
    }
}

/// 0x17670: the count is over: each panel goes to continue end, the "GAME OVER" message stays
/// displayed for `GAME_OVER_FRAMES` frames, then the end-of-game flag is raised.
fn wait_game_over(x: &mut Ctx) -> Flow {
    let record = over::RECORD;
    if rd8(x.b, record + over::FLAG) != 0 {
        wr8(x.b, record + over::FLAG, 0);
        let count = 3u8.wrapping_sub(v8(x.b, var::LAYOUT)) as u32 + 1;
        for k in 0..count {
            slot::resume_end_enter(x.b, Slot::new(k));
        }
        draw_game_over_frame(x.b);
        wr16(x.b, record + over::TIMER, over::GAME_OVER_FRAMES);
    }
    let left = rd16(x.b, record + over::TIMER).wrapping_sub(1);
    wr16(x.b, record + over::TIMER, left);
    if left != 0 {
        return Flow::Done;
    }
    x.set_v8(var::PHASE, over::PHASE_GAME_OVER);
    crate::sound::service(x.b, crate::sound::Service::GeneralStop);
    crate::families::small::startup_done(x.b, true);
    Flow::Done
}

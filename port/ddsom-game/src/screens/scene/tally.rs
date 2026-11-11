//! Shared state `0xf590e` of the scene exits: the end-of-level tally.
//!
//! A banner ("STAGE CLEAR" and the scene name, drawn off-screen then slid in from the right),
//! the remaining time, the time bonus (counted from 5:00 down to the remaining time, ten seconds per step), the
//! seconds bonus, then per player the total bonus, two optional bonuses (treasure and best hit count:
//! byte `+0x18a`, word `+0x190`), finally the payout of the bonuses to the players' score, digit by digit.
//! Fourteen sub-states (byte `+3` of the scene exit block, step of 2), one per frame; the first waits for the
//! end of a fade by ceding control to the scheduler.

use super::tiles::{self, Number, COLUMN, LAYER};
use super::fading;
use super::Ctx;
use super::screen::blocks::{EXIT_STEP, EXIT_SUB_STEP};
use crate::hw::video::{Cell, Vram, SCROLL1, TEXT};
use crate::world::ram::{Byte, Long};
use crate::screens::modes::{Flow, Sequence};
use crate::world::ops::abcd;
use crate::world::objects::Obj;
use crate::Native;
use crate::world::screen_tables as tables;
use crate::screens::obj;

/// Tally variables (work RAM, under A5).
mod v {
    use crate::world::ram::{Byte, Long, Word};
    /// Arrival column of the banner (decreases from 48 to 0) and number of columns already visible - 1.
    pub const BANNER_AT: Byte = Byte(0xff_2a56);
    pub const BANNER_WIDTH: Byte = Byte(0xff_2a57);
    /// Timer (byte) between two sub-states, and step counter of the time bonus.
    pub const TIMER: Byte = Byte(0xff_2a58);
    pub const STEPS: Byte = Byte(0xff_2a57);
    /// Counted-down time (BCD word m:ss).
    pub const TIME: Word = Word(0xff_2a5a);
    /// Total bonus (BCD long) and current bonus (BCD long; its byte `+2`: seconds bonus).
    pub const BONUS: Long = Long(0xff_2a5c);
    pub const PRIME: Long = Long(0xff_2a60);
    pub const PRIME_SECONDS: Byte = Byte(0xff_2a62);
    /// Index of the scene in the name and bonus tables, and its banner variant.
    pub const NAME: Byte = Byte(0xff_2a84);
    pub const NAME_FIRST: Byte = Byte(0xff_2a85);
    pub const PRIME_INDEX: Byte = Byte(0xff_2a86);
    /// Bonuses left to pay out, one BCD long per player.
    pub const PAYOUT: u32 = 0xff_2a8c;
    /// Player of the optional bonus in progress (word, RAM address).
    pub const WINNER: Word = Word(0xff_2a9e);
    /// Counter of open end screens (incremented at the start, decremented at the end).
    pub const SCREENS: Byte = Byte(0xff_2472);
}

/// Work RAM variables (offsets from A5).
mod a5 {
    /// Remaining time (BCD word m:ss of the clock long).
    pub const CLOCK: u32 = 0x38;
    /// Bit 0: two-column screen layout (positions of the player panels).
    pub const LAYOUT: u32 = 0x20;
    pub const HOLD: u32 = 0x33;
    pub const FLAG_5F: u32 = 0x5f;
}

/// Player fields read by the tally.
mod p {
    pub const CHARACTER: u32 = 2;
    pub const STATE: u32 = 4;
    /// Treasure bonus earned (byte); best hit count (word).
    pub const TREASURE: u32 = 0x18a;
    pub const HITS: u32 = 0x190;
}

/// ROM tables.
mod rom {
    /// Banner: empty columns before the text (bytes, PC-relative), by variant.
    pub const LEAD_NAME_FIRST: crate::world::screen_tables::Rom = crate::world::screen_tables::Rom::at(0xf6f48);
    pub const LEAD_CLEAR_FIRST: crate::world::screen_tables::Rom = crate::world::screen_tables::Rom::at(0xf6f6a);
    /// Scene names (pointers, PC-relative) and "STAGE CLEAR" text of the two variants.
    pub const NAMES: crate::world::screen_tables::Rom = crate::world::screen_tables::Rom::at(0xf6ed2);
    pub const CLEAR_AFTER: crate::world::data::RomAddr = crate::world::data::RomAddr(0xf7000);
    pub const CLEAR_BEFORE: crate::world::screen_tables::Rom = crate::world::screen_tables::Rom::at(0xf6fec);
    /// "TIME" and "TIME OVER" texts (descriptors of 0x1136).
    pub const TIME_TEXT: crate::world::screen_tables::Rom = crate::world::screen_tables::Rom::at(0xf7336);
    pub const NO_TIME_TEXT: crate::world::screen_tables::Rom = crate::world::screen_tables::Rom::at(0xf734e);
    /// BCD decrement of the time bonus: ten seconds, or fifty to cross the minute.
    pub const TEN_SECONDS: crate::world::screen_tables::Rom = crate::world::screen_tables::Rom::at(0xf5a62);
    pub const MINUTE: crate::world::screen_tables::Rom = crate::world::screen_tables::Rom::at(0xf5a64);
    /// Bonus of one step of the time bonus (BCD longs, 31 values).
    pub const TIME_BONUS: crate::world::screen_tables::Rom = crate::world::screen_tables::Rom::at(0xf6ba0);
    /// Bonus of one second (BCD byte, high nibble).
    pub const SECOND_BONUS: crate::world::screen_tables::Rom = crate::world::screen_tables::Rom::at(0xf5b06);
    /// Treasure and hit bonuses, by scene (BCD longs).
    pub const TREASURE_BONUS: crate::world::screen_tables::Rom = crate::world::screen_tables::Rom::at(0x19c0e);
    pub const HITS_BONUS: crate::world::screen_tables::Rom = crate::world::screen_tables::Rom::at(0xf6c1c);
    /// Positions (words, relative to the layer) per character and layout: bonus, label, total, payout.
    pub const TOTAL_AT: crate::world::screen_tables::Rom = crate::world::screen_tables::Rom::at(0xf5c08);
    pub const LABEL_AT: crate::world::screen_tables::Rom = crate::world::screen_tables::Rom::at(0xf5c18);
    pub const PANEL_AT: crate::world::screen_tables::Rom = crate::world::screen_tables::Rom::at(0xf5d82);
    pub const PANEL_VALUE_AT: crate::world::screen_tables::Rom = crate::world::screen_tables::Rom::at(0xf5d92);
    pub const PAYOUT_AT: crate::world::screen_tables::Rom = crate::world::screen_tables::Rom::at(0xf5f0a);
    /// Label of a player (pairs of tiles).
    pub const LABEL: crate::world::screen_tables::Rom = crate::world::screen_tables::Rom::at(0xf7320);
}

/// Game subroutines.
mod sub {
    pub const MUSIC: crate::services::sound::Sound = crate::services::sound::CMD_04;
}

/// The banner is drawn off-screen (rows 8 and 9 of the first page of the layer, 0x910000), then copied.
const BANNER: u32 = SCROLL1.base + 0x20;


/// Current sub-state (byte `+3` of the block designated by A0).
fn sub_step(x: &mut Ctx) -> u8 {
    EXIT_SUB_STEP.get(x.b)
}
fn set_sub_step(x: &mut Ctx, s: u8) {
    EXIT_SUB_STEP.set(x.b, s)
}
fn next_sub_step(x: &mut Ctx) {
    let s = sub_step(x).wrapping_add(2);
    set_sub_step(x, s);
}

/// Timer: subtracts one frame, true when it reaches zero.
fn tick(x: &mut Ctx) -> bool {
    let t = v::TIMER.get(x.b).wrapping_sub(1);
    v::TIMER.set(x.b, t);
    t == 0
}
fn set_timer(x: &mut Ctx, t: u8) {
    v::TIMER.set(x.b, t)
}

/// Players in the game (present, main state < 4): addresses of the records.
fn players_in_game(x: &mut Ctx) -> impl Iterator<Item = u32> {
    let alive: Vec<u32> = (0..4)
        .map(|k| super::PLAYERS + k * super::PLAYER_STRIDE)
        .filter(|&p| Obj(p).u8(x.b, Obj::ACTIVE) != 0 && (Obj(p).u8(x.b, p::STATE) as i8) < 4)
        .collect();
    alive.into_iter()
}

/// Position in the layer read from a table (signed PC-relative words) indexed by the player's character and
/// the screen layout.
fn panel_at(x: &mut Ctx, table: u32, player: u32, pc_relative: bool) -> u32 {
    let mut i = 2 * Obj(player).u8(x.b, p::CHARACTER) as u32;
    if x.v8(a5::LAYOUT) & 1 != 0 {
        i += 8;
    }
    let off = if pc_relative { tables::pc_word(x.b, table + i) } else { tables::word(x.b, table + i) };
    LAYER.wrapping_add(off as i16 as i32 as u32)
}

/// A state of 0xf590e.
pub fn run(x: &mut Ctx) -> Flow {
    match sub_step(x) {
        0 => start(x),
        2 => {
            banner_step(x);
            Flow::Done
        }
        4 => clock(x),
        6 => {
            if tick(x) {
                next_sub_step(x);
                v::TIME.set(x.b, 0x0500);
                set_timer(x, 1);
                v::STEPS.set(x.b, 0);
            }
            Flow::Done
        }
        8 => time_bonus(x),
        10 => second_bonus(x),
        12 => total(x),
        14 => player_totals(x),
        16 => treasure(x),
        18 => announce(x, 996),
        20 => best_hits(x),
        22 => announce(x, 1000),
        24 => payout(x),
        26 => {
            if tick(x) {
                x.set_v8(a5::HOLD, 0);
                let step = EXIT_STEP.get(x.b).wrapping_add(4);
                EXIT_STEP.set(x.b, step);
                EXIT_SUB_STEP.set(x.b, 0);
                let n = v::SCREENS.get(x.b).wrapping_sub(1);
                v::SCREENS.set(x.b, n);
            }
            Flow::Done
        }
        _ => Flow::Done,
    }
}

/// Sub-state 0: music, fade (task 0x14de, one level per frame, pages 13) whose end is awaited.
fn start(x: &mut Ctx) -> Flow {
    next_sub_step(x);
    x.set_v8(a5::HOLD, 1);
    crate::services::sound::play_global(x.b, sub::MUSIC);
    fading::STEP.set(x.b, 8);
    // 0xf68c8: the fade task (A0 kept)
    fading::DELAY.set(x.b, 1);
    fading::PAGES.set(x.b, 13);
    crate::core::kernel::create_task(x.b, 16, fading::DESCENT_STEP);
    x.push_return(Step::AfterFade);
    x.wait(1, Step::FadeWait)
}

crate::steps! {
    /// Steps (resume points) of the state machine.
    pub enum Step {
        FadeWait = 0xf6906 => |x| {
            if fading::PAGES.get(x.b) != 0 {
                return x.wait(1, Step::FadeWait);
            }
            Flow::Done
        },
        /// End of sub-state 0: the banner is drawn off-screen, then its first step.
        AfterFade = 0xf596e => |x| {
            banner(x);
            v::BANNER_AT.set(x.b, 48);
            v::BANNER_WIDTH.set(x.b, 0xff);
            x.set_v8(a5::FLAG_5F, 0);
            let n = v::SCREENS.get(x.b).wrapping_add(1);
            v::SCREENS.set(x.b, n);
            banner_step(x);
            Flow::Done
        },
    }
}

/// The banner, two rows from row 8 of column 0 (0x0f6e88): "STAGE CLEAR" text and scene name,
/// in the order of the variant, an empty column between the two; a word 0xffff marks the end.
fn banner(x: &mut Ctx) {
    let at = BANNER;
    let name = v::NAME.get(x.b) as u32;
    let name_text = rom::NAMES.pc_long(x.b, 4 * name);
    let at = if v::NAME_FIRST.get(x.b) != 0 {
        let lead = rom::LEAD_NAME_FIRST.pc_byte(x.b, name) as u32;
        let at = tiles::blank_columns(x.b, at, lead + 1);
        let at = tiles::glyphs(x.b, at, name_text);
        let at = tiles::blank_columns(x.b, at, 2);
        tiles::glyphs(x.b, at, rom::CLEAR_AFTER.0)
    } else {
        let lead = rom::LEAD_CLEAR_FIRST.pc_byte(x.b, name) as u32;
        let at = tiles::blank_columns(x.b, at, lead + 1);
        let at = tiles::glyphs(x.b, at, rom::CLEAR_BEFORE.addr());
        let at = tiles::blank_columns(x.b, at, 2);
        tiles::glyphs(x.b, at, name_text)
    };
    Vram(at).set(x.b, 0xffff);
}

/// Sub-state 2: the banner advances one column to the left (rows 10 and 11), up to column 0.
fn banner_step(x: &mut Ctx) {
    let col = v::BANNER_AT.get(x.b).wrapping_sub(1);
    v::BANNER_AT.set(x.b, col);
    let width = v::BANNER_WIDTH.get(x.b).wrapping_add(1);
    v::BANNER_WIDTH.set(x.b, width);
    // 0x0f71fa: copy of the visible columns, one empty cell after the last
    let mut dst = LAYER + ((col as u32) << 7) + 0x28;
    let mut src = BANNER;
    for _ in 0..=width as u32 {
        let (top, bottom) = (Cell(src).long(x.b), Cell(src + 4).long(x.b));
        Cell(dst).set_long(x.b, top);
        Cell(dst + 4).set_long(x.b, bottom);
        src += COLUMN;
        dst += COLUMN;
        if Cell(src).tile(x.b) == 0xffff {
            tiles::blank(x.b, dst);
            break;
        }
    }
    if col == 0 {
        next_sub_step(x);
        set_timer(x, 30);
    }
}

/// Sub-state 4: the remaining time, or "TIME OVER" and the treasure bonus if it has run out.
fn clock(x: &mut Ctx) -> Flow {
    if !tick(x) {
        return Flow::Done;
    }
    if x.v16(a5::CLOCK) == 0 {
        return time_over(x);
    }
    next_sub_step(x);
    set_timer(x, 30);
    crate::services::text::print_tall(x.b, rom::TIME_TEXT.addr());
    let t = x.v16(a5::CLOCK);
    tiles::duration(x.b, TEXT.at(0xdb4).0, t);
    Flow::Done
}

/// Time elapsed (0x0f6dca): the treasure bonus in large digits, then directly the players' bonuses.
fn time_over(x: &mut Ctx) -> Flow {
    let src = rom::TREASURE_BONUS.addr() + 4 * v::PRIME_INDEX.get(x.b) as u32 + 1;
    let mut n = Number::new(TEXT.at(0x8b4).0, 3);
    for i in 0..2 {
        let byte = tables::byte(x.b, src + i);
        tiles::big_digit(x.b, &mut n, byte >> 4);
        tiles::big_digit(x.b, &mut n, byte);
    }
    let byte = tables::byte(x.b, src + 2);
    tiles::big_digit(x.b, &mut n, byte >> 4);
    n.lead = true;
    tiles::big_digit(x.b, &mut n, byte);
    crate::services::text::print_tall(x.b, rom::NO_TIME_TEXT.addr());
    set_timer(x, 30);
    set_sub_step(x, 16);
    Flow::Done
}

/// Sub-state 8: the counted-down time descends by ten seconds per step down to the remaining time (tens); the bonus
/// of the step (table indexed by the number of steps, at most 30) is displayed next to it.
fn time_bonus(x: &mut Ctx) -> Flow {
    if !tick(x) {
        return Flow::Done;
    }
    set_timer(x, 10);
    let t = v::TIME.get(x.b);
    tiles::duration(x.b, TEXT.at(0x83c).0, t);
    // `cmpi.b #30 ; ble`: signed comparison of a byte then used unsigned
    let steps = v::STEPS.get(x.b);
    let steps = if (steps as i8) <= 30 { steps } else { 30 } as u32;
    let bonus = rom::TIME_BONUS.addr() + 4 * steps;
    let value = tables::long(x.b, bonus);
    v::BONUS.set(x.b, value);
    tiles::number6(x.b, TEXT.at(0xd3c).0, bonus, 0);
    if t == x.v16(a5::CLOCK) & 0xfff0 {
        set_timer(x, 30);
        next_sub_step(x);
        return Flow::Done;
    }
    // BCD subtraction on two bytes (sbcd), initial carry zero
    let dec = if t & 0xf0 != 0 { rom::TEN_SECONDS.addr() } else { rom::MINUTE.addr() };
    let (lo, borrow) = crate::world::ops::sbcd(t as u8, tables::byte(x.b, dec + 1), false);
    let (hi, _) = crate::world::ops::sbcd((t >> 8) as u8, tables::byte(x.b, dec), borrow);
    v::TIME.set(x.b, (hi as u16) << 8 | lo as u16);
    let n = v::STEPS.get(x.b).wrapping_add(1);
    v::STEPS.set(x.b, n);
    Flow::Done
}

/// Sub-state 10: the remaining seconds (units) make the seconds bonus, added to the total bonus.
fn second_bonus(x: &mut Ctx) -> Flow {
    if !tick(x) {
        return Flow::Done;
    }
    set_timer(x, 30);
    next_sub_step(x);
    let seconds = (x.v16(a5::CLOCK) & 15) as u8;
    tiles::digit(x.b, TEXT.at(0xa44).0, seconds, 0);
    tiles::tall(x.b, TEXT.at(0xa44).0 + COLUMN, tiles::SECONDS, 0);
    let rate = rom::SECOND_BONUS.byte(x.b, 0);
    v::PRIME.set(x.b, 0);
    if seconds != 0 {
        let (mut prime, mut carry) = (0u8, false);
        for _ in 0..seconds {
            (prime, carry) = abcd(rate >> 4, prime, false);
        }
        let _ = carry;
        v::PRIME_SECONDS.set(x.b, prime);
        add_bcd(x, v::BONUS, v::PRIME);
    }
    // the rate (two digits), and the seconds bonus followed by "00"
    let mut n = Number::new(TEXT.at(0xa44).0 + 2 * COLUMN, 0);
    n.byte(x.b, rate);
    n.digit(x.b, 0);
    let prime = v::PRIME_SECONDS.get(x.b);
    let mut n = Number::new(TEXT.at(0xe44).0, 0);
    n.byte(x.b, prime);
    n.digit(x.b, 0);
    n.last(x.b, 0);
    Flow::Done
}

/// `dst += src` (BCD longs), four `abcd` from the low byte to the high byte.
fn add_bcd(x: &mut Ctx, dst: Long, src: Long) {
    let mut carry = false;
    for i in (0..4).rev() {
        let (r, c) = abcd(Byte(src.0 + i).get(x.b), Byte(dst.0 + i).get(x.b), carry);
        Byte(dst.0 + i).set(x.b, r);
        carry = c;
    }
}

/// Sub-state 12: the total frame, the remaining time and the total bonus; each player will receive it.
fn total(x: &mut Ctx) -> Flow {
    if !tick(x) {
        return Flow::Done;
    }
    next_sub_step(x);
    set_timer(x, 30);
    let at = TEXT.at(0x7c4).0;
    for (k, tile) in [0x0460u16, 0x0462, 0x0464].into_iter().enumerate() {
        Cell(at + 4 * k as u32).put(x.b, tile, 0);
    }
    for (k, tile) in [0x0461u16, 0x0463, 0x0465].into_iter().enumerate() {
        Cell(at + COLUMN + 4 * k as u32).put(x.b, tile, 0);
    }
    for k in 0..8 {
        Cell(at + COLUMN + 8 + (k + 1) * COLUMN).put(x.b, 0x0466 + k as u16, 0);
    }
    let t = x.v16(a5::CLOCK);
    tiles::duration(x.b, TEXT.at(0x850).0, t);
    tiles::number6(x.b, TEXT.at(0xd50).0, v::BONUS.0, 0);
    let bonus = v::BONUS.get(x.b);
    for k in 0..4 {
        Long(v::PAYOUT + 4 * k).set(x.b, bonus);
    }
    Flow::Done
}

/// Sub-state 14: the total bonus under the label of each player in the game.
fn player_totals(x: &mut Ctx) -> Flow {
    if !tick(x) {
        return Flow::Done;
    }
    next_sub_step(x);
    set_timer(x, 30);
    for p in players_in_game(x) {
        let total_at = panel_at(x, rom::TOTAL_AT.addr(), p, true);
        let label_at = panel_at(x, rom::LABEL_AT.addr(), p, true);
        tiles::label(x.b, label_at, rom::LABEL.addr());
        tiles::number6(x.b, total_at, v::BONUS.0, 2);
    }
    Flow::Done
}

/// Sub-state 16: the first player who found the treasure receives their bonus (displayed 60 frames), otherwise we move
/// on to the hit bonus.
fn treasure(x: &mut Ctx) -> Flow {
    if !tick(x) {
        return Flow::Done;
    }
    let winner = players_in_game(x).find(|&p| Obj(p).u8(x.b, p::TREASURE) != 0);
    let Some(p) = winner else {
        set_sub_step(x, 20);
        set_timer(x, 1);
        return Flow::Done;
    };
    v::WINNER.set(x.b, p as u16);
    set_timer(x, 60);
    next_sub_step(x);
    sign(x, 996);
    let src = rom::TREASURE_BONUS.addr() + 4 * v::PRIME_INDEX.get(x.b) as u32;
    prime_value(x, src);
    Flow::Done
}

/// Sub-state 20: the player with the best hit count receives the hit bonus; the counters are reset to 0.
fn best_hits(x: &mut Ctx) -> Flow {
    if !tick(x) {
        return Flow::Done;
    }
    let (mut best, mut winner) = (0i16, 0u32);
    for p in players_in_game(x) {
        if Obj(p).u8(x.b, p::TREASURE) != 0 {
            continue;
        }
        let hits = Obj(p).u16(x.b, p::HITS) as i16;
        if best < hits {
            best = hits;
            winner = p;
        }
        Obj(p).set_u16(x.b, p::HITS, 0);
    }
    if best == 0 {
        set_timer(x, 1);
        set_sub_step(x, 24);
        return Flow::Done;
    }
    v::WINNER.set(x.b, winner as u16);
    next_sub_step(x);
    set_timer(x, 60);
    let src = rom::HITS_BONUS.addr() + 4 * v::PRIME_INDEX.get(x.b) as u32;
    prime_value(x, src);
    Flow::Done
}

/// The optional bonus `src` (BCD long) stored and displayed in the winner's panel.
fn prime_value(x: &mut Ctx, src: u32) {
    let w = winner(x);
    let at = panel_at(x, rom::PANEL_VALUE_AT.addr(), w, false);
    let value = tables::long(x.b, src);
    v::PRIME.set(x.b, value);
    tiles::number6(x.b, at, src, 2);
}

fn winner(x: &mut Ctx) -> u32 {
    v::WINNER.get(x.b) as i16 as i32 as u32 & 0xff_ffff
}

/// Sign of four columns (tiles `first`.., attributes 30) in the winner's panel (0x0f5cee, 0x0f5ce6).
fn sign(x: &mut Ctx, first: u16) {
    let w = winner(x);
    let at = panel_at(x, rom::PANEL_AT.addr(), w, false);
    for k in 0..4u16 {
        tiles::tall(x.b, at + k as u32 * COLUMN, first + k, 30);
    }
}

/// Sub-states 18 and 22: the sign blinks 30 frames, stays 30 frames, then a SET object (type 19) carries
/// the bonus to the winner.
fn announce(x: &mut Ctx, first: u16) -> Flow {
    if tick(x) {
        next_sub_step(x);
        set_timer(x, 20);
        sign(x, first);
        // 0xce00a: SET object of type 15, variant 19
        let Ok(Obj(o)) = crate::actors::families::creators::set_15_pure(x.b, 19) else {
            return Flow::Done;
        };
        let prime = v::PRIME.get(x.b);
        Obj(o).set_u32(x.b, obj::SIGNAL, prime);
        let w = winner(x);
        let character = Obj(w).u8(x.b, p::CHARACTER);
        Obj(o).set_u8(x.b, obj::INDEX, character);
        return Flow::Done;
    }
    let t = v::TIMER.get(x.b);
    if (t as i8) <= 30 {
        return Flow::Done;
    }
    if t & 2 != 0 {
        sign(x, first);
    } else {
        // 0x0f5ccc: four empty columns
        let w = winner(x);
        let at = panel_at(x, rom::PANEL_AT.addr(), w, false);
        tiles::blank_columns(x.b, at, 4);
    }
    Flow::Done
}

/// Sub-state 24: the bonuses are added to the players' score, one digit per step of two frames (hundreds, then
/// thousands, then ten-thousands); the remainder is displayed at each step.
fn payout(x: &mut Ctx) -> Flow {
    if x.v16(a5::CLOCK) == 0 {
        next_sub_step(x);
        set_timer(x, 30);
        return Flow::Done;
    }
    if v::TIMER.get(x.b) != 0 && !tick(x) {
        return Flow::Done;
    }
    set_timer(x, 2);
    let mut paying = false;
    for k in 0..4u32 {
        let p = super::PLAYERS + k * super::PLAYER_STRIDE;
        let left = Long(v::PAYOUT + 4 * k);
        if Obj(p).u8(x.b, Obj::ACTIVE) == 0 || (Obj(p).u8(x.b, p::STATE) as i8) >= 4 || left.get(x.b) == 0 {
            continue;
        }
        paying = true;
        let (rest, amount) = pay_digit(left.get(x.b));
        left.set(x.b, rest);
        let character = Obj(p).u8(x.b, p::CHARACTER);
        crate::world::events::enqueue_points(x.b, ((character as u32) << 8 | amount) as u16);
        let mut i = 2 * character as u32;
        if x.v8(a5::LAYOUT) & 1 != 0 {
            i += 8;
        }
        let at = LAYER.wrapping_add(rom::PAYOUT_AT.pc_word(x.b, i) as i16 as i32 as u32);
        tiles::number6(x.b, at, left.0, 2);
    }
    if !paying {
        next_sub_step(x);
    }
    Flow::Done
}

/// Subtracts one from the lowest non-zero digit among hundreds, thousands and ten-thousands (0x0f5f1a);
/// returns the remainder and the amount paid out (score code: 4, 14 or 33).
fn pay_digit(v: u32) -> (u32, u32) {
    if v & 0x0f00 != 0 {
        (v - 0x100, 4)
    } else if v & 0xf000 != 0 {
        (v - 0x1000, 14)
    } else {
        (v.wrapping_sub(0x1_0000), 33)
    }
}

pub const NATIVES: &[Native] = &[];

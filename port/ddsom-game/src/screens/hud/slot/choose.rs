//! States 6, 8, 0xa and 0xc of a player slot: character choice and continue.

use super::*;
use crate::world::ram::Byte;

/// Set when a player of character 5 starts (byte `+0x3f` of the record at `A5-0x5b88`, h).
const CHARACTER_5_FLAG: Byte = Byte(0xff_2478 + 0x3f);
/// Continues taken in the game (`A5+0x5a`).
const CONTINUES: Byte = Byte::a5(0x5a);
use crate::world::objects::Obj;

// ---------------------------------------------------------------------------------------------
// State 6: character choice
// ---------------------------------------------------------------------------------------------

/// Has a button (upper part of the rising edges) just been pressed?
pub(super) fn button_pressed(b: &mut World, s: Slot) -> bool {
    let index = s.index(b);
    select::inputs(b, index).1 & 0xf0 != 0
}

/// The panel's BCD countdown loses one unit and restarts for `frames` frames; true (without
/// changing anything) if it goes below zero.
pub(super) fn tick_countdown(b: &mut World, s: Slot, frames: u8) -> bool {
    let (value, _) = sbcd(s.p8(b, panel::COUNTDOWN), 1, false);
    if value & 0x80 != 0 {
        return true;
    }
    s.set_p8(b, panel::COUNTDOWN, value);
    s.set_p8(b, panel::COUNTDOWN_FRAMES, frames);
    false
}

/// Has the player's start button just been pressed?
pub(super) fn start_edge(b: &mut World, s: Slot) -> bool {
    start_pressed(b, s)
}

/// 0x157a8: redraws the choice screen (after a pause).
pub(super) fn redraw_choice(b: &mut World, s: Slot) {
    draw::portrait(b, s, false);
    let choice = s.p8(b, panel::CHOICE);
    choice::show_character(b, s, choice);
    draw::countdown_digits(b, s);
    s.set_p8(b, panel::PLAYER_DIRTY, 0);
}

/// 0x15788: the player chooses their character (d-pad: next or previous, limited time).
pub(super) fn choose(x: &mut Ctx, s: Slot) {
    let b = &mut *x.b;
    let pause = var::PAUSE.get(b);
    let seen = var::SEEN_PAUSE.get(b);
    if pause != seen {
        if !pause & seen != 0 {
            redraw_choice(b, s);
        } else {
            draw::clear_banner(b, s);
            return notice_banner(b, s);
        }
    }
    if s.p8(b, panel::PLAYER_DIRTY) != 0 {
        if var::PAUSE.get(b) == 0 {
            redraw_choice(b, s);
        } else {
            draw::clear_banner(b, s);
            return notice_banner(b, s);
        }
    }
    if var::PAUSE.get(b) != 0 {
        return notice_banner(b, s);
    }
    let current = s.p8(b, panel::CHOICE);
    let choice = select::free_choice(b, current, false);
    let choice = select::navigate(b, s, choice, &|b, c, back| select::free_choice(b, c, back));
    if choice != s.p8(b, panel::CHOICE) {
        s.set_p8(b, panel::CHOICE, choice);
        choice::show_character(b, s, choice);
        draw::countdown_digits(b, s);
    }
    let mut done = false;
    let frames = s.p8(b, panel::COUNTDOWN_FRAMES).wrapping_sub(1);
    s.set_p8(b, panel::COUNTDOWN_FRAMES, frames);
    if frames == 0 {
        if tick_countdown(b, s, 0x32) {
            done = true;
        } else {
            draw::countdown_digits(b, s);
        }
    }
    if !done && !button_pressed(b, s) {
        if !start_edge(b, s) {
            return;
        }
        let variant = s.p8(b, panel::CHOICE) ^ 1;
        s.set_p8(b, panel::CHOICE, variant);
    }
    // the choice is made (button, or time out)
    let current = s.p8(b, panel::CHOICE);
    let chosen = select::free_choice(b, current, false);
    s.set_p8(b, panel::CHOICE, chosen);
    s.set_g8(b, player::CHARACTER, chosen);
    crate::hw::eeprom::stat_chosen_character(x.b, chosen as u16);
    let b = &mut *x.b;
    s.set_p8(b, panel::LAST_MODE, 0x80);
    s.set_p8(b, panel::STATE, 0);
    let class = rom::CLASS_OF_CHARACTER.pc_byte(b, (chosen >> 1) as u32);
    s.set_g8(b, player::SELECTION_KIND, class);
    start_player(x, s, Join::New);
}

/// 0x1588c: the arrival of this panel's player, then the setting of character 5. The code tables (0x158b8:
/// 0x2f46... new player, 0x15de0: 0x3142... reload, 0x15e10: 0x3048... continue) give the entry of
/// `player::init::join_player` specific to the player.
pub(super) fn start_player(x: &mut Ctx, s: Slot, how: Join) {
    let index = s.index(x.b);
    crate::actors::player::init::join_player(x.b, index, how);
    let b = &mut *x.b;
    if (s.g8(b, player::CHARACTER) & 0x7f) >> 1 == 5 {
        s.set_g8(b, player::SPEED_CLASS, 1);
        s.set_g8(b, player::SETTING_16B, 5);
        CHARACTER_5_FLAG.set(b, 1);
    }
}

// ---------------------------------------------------------------------------------------------
// States 8, 0xa, 0xc: continue
// ---------------------------------------------------------------------------------------------

/// 0x15a26: the player has fallen (state 0x402): start of the continue.
pub(super) fn join_continue(x: &mut Ctx, s: Slot) {
    let frames = s.p32(x.b, panel::FRAMES);
    crate::hw::eeprom::stat_game_duration(x.b, frames);
    let b = &mut *x.b;
    s.set_p8(b, panel::SUBSTEP, 1);
    let character = s.g8(b, player::CHARACTER);
    s.set_p8(b, panel::CHOSEN, character);
    let class = s.g8(b, player::CLASS);
    s.set_p8(b, panel::CLASS, class);
    let flags = s.g8(b, player::FLAGS) | 2;
    s.set_g8(b, player::FLAGS, flags);
    s.set_p8(b, panel::CLAIM, 1);
    if crate::world::ram::PLAYERS_ALIVE.get(b) == 0 && others_free(b) {
        let flags = s.g8(b, player::FLAGS) & !2;
        s.set_g8(b, player::FLAGS, flags);
        s.set_p8(b, panel::CLAIM, 0);
    }
    // a player arrives: the rank drops (`objects::rank`)
    crate::world::objects::rank::lower(x.b);
    let b = &mut *x.b;
    s.set_p8(b, panel::STEP, 0);
    s.set_p8(b, panel::FLAG_27, 0);
    s.set_p8(b, panel::STATE, 8);
    if var::CONTINUE_ALLOWED.get(b) == 0 {
        return resume_declined(b, s);
    }
    draw::clear_frame(b, s);
    draw::start_banner(b, s, draw::START_BANNER_A);
    restart_countdown(b, s);
    resume(x, s);
}

/// No panel is in continue (sum of the sub-steps is zero): the player is the only one waiting.
pub(super) fn others_free(b: &mut World) -> bool {
    (0..4).fold(1u8, |free, k| free.wrapping_sub(Slot::new(k).p8(b, panel::SUBSTEP))) == 0
}

/// Coins counted for this player's continue: the partial coins of the price, or the credits.
pub(super) fn coins_inserted(b: &mut World, s: Slot) -> u8 {
    if var::PRICE_TWO.get(b) == 0 && var::PRICE.get(b) != 1 {
        s.coins.coins(b)
    } else {
        s.coins.credits(b)
    }
}

/// 0x15a9c: the countdown restarts from 19 seconds, with the current coins in memory.
pub(super) fn restart_countdown(b: &mut World, s: Slot) {
    let coins = coins_inserted(b, s);
    s.set_p8(b, panel::SHOWN_COINS, coins);
    s.set_p16(b, panel::COUNTDOWN, RESUME_COUNTDOWN);
    draw::continue_digits(b, s);
}

/// 0x15ac4: continue countdown: it restarts when a coin is inserted, rushes when a
/// button is pressed in the last seconds, and the continue takes place on the start key.
pub(super) fn resume(x: &mut Ctx, s: Slot) {
    let b = &mut *x.b;
    if s.p8(b, panel::FLAG_27) != 0 {
        return resume_end_enter(b, s);
    }
    if s.p8(b, panel::CLAIM) == 0 {
        let flags = s.g8(b, player::FLAGS) & !2;
        s.set_g8(b, player::FLAGS, flags);
        s.set_p8(b, panel::CLAIM, 1);
        s.set_g8(b, player::FLAGS, flags | 2);
        if crate::world::ram::PLAYERS_ALIVE.get(b) == 0 && others_free(b) {
            s.set_g8(b, player::FLAGS, flags);
            s.set_p8(b, panel::CLAIM, 0);
        }
    }
    if s.p8(b, panel::PLAYER_DIRTY) != 0 {
        s.set_p8(b, panel::PLAYER_DIRTY, 0);
        draw::start_banner(b, s, draw::START_BANNER_A);
        draw::continue_digits(b, s);
    }
    let hurry = s.p8(b, panel::COUNTDOWN) <= 0x16 && button_pressed(b, s);
    let tick = hurry || {
        let frames = s.p8(b, panel::COUNTDOWN_FRAMES).wrapping_sub(1);
        s.set_p8(b, panel::COUNTDOWN_FRAMES, frames);
        frames == 0
    };
    if tick {
        if tick_countdown(b, s, 0x3c) {
            return resume_declined(b, s);
        }
        draw::continue_digits(b, s);
    }
    let coins = coins_inserted(b, s);
    let shown = s.p8(b, panel::SHOWN_COINS);
    if coins as i8 > shown as i8 {
        restart_countdown(b, s);
        return resume(x, s);
    }
    if coins != shown {
        s.set_p8(b, panel::SHOWN_COINS, coins);
    }
    if var::CONTINUE_MODE.get(b) == 0 && s.coins.credits(b) == 0 {
        return;
    }
    if !start_pressed(b, s) {
        return;
    }
    crate::hw::eeprom::count_game(x.b);
    let b = &mut *x.b;
    let credits = s.coins.credits(b).wrapping_sub(1);
    s.coins.set_credits(b, credits);
    s.set_p8(b, panel::SUBSTEP, 0);
    s.set_p8(b, panel::STEP, 1);
    s.set_p8(b, panel::STATE, 0xc);
    s.set_p16(b, panel::COUNTDOWN, RESUME_COUNTDOWN);
    let character = s.g8(b, player::CHARACTER);
    s.set_p8(b, panel::CHOICE, character);
    if var::PAUSE.get(b) == 0 {
        draw::portrait(b, s, false);
        choice::show_character(b, s, character);
        let class = s.g8(b, player::CLASS);
        draw::face(b, s, class);
        draw::countdown_digits(b, s);
    } else {
        draw::clear_banner(b, s);
        notice_banner(b, s);
    }
}

/// 0x15c0c: the continue is refused or the time has run out: the player's game is over and their money
/// is added to their score (capped).
pub(super) fn resume_declined(b: &mut World, s: Slot) {
    s.set_p8(b, panel::SUBSTEP, 0);
    s.set_p8(b, panel::FLAG_27, 1);
    s.set_p16(b, panel::COUNTDOWN, 0);
    Obj(s.player).set_u32(b, player::STATE, 0x0600_0000);
    let money = Obj(s.player).u32(b, player::MONEY) & !0xf;
    Obj(s.player).set_u32(b, player::MONEY, money);
    // score += money, in packed decimal (byte by byte, from the end)
    let mut carry = false;
    let mut sum = [0u8; 4];
    for k in (0..4u32).rev() {
        let add = s.g8(b, player::MONEY + k);
        let base = s.g8(b, player::SCORE + k);
        let (value, out) = crate::world::ops::abcd(add, base, carry);
        sum[k as usize] = value;
        carry = out;
    }
    let total = u32::from_be_bytes(sum);
    Obj(s.player).set_u32(b, player::SCORE, if (total as i32) <= 0x999_9999 { total } else { 0x999_9999 });
}

/// 0x15c50: end of the continue: the end banner is displayed, then state 0xa waits for the end of the message.
pub fn resume_end_enter(b: &mut World, s: Slot) {
    s.set_p8(b, panel::FLAG_27, 0);
    draw::start_banner(b, s, draw::START_BANNER_B);
    s.set_p8(b, panel::STATE, 0xa);
    s.set_p16(b, panel::COUNTDOWN, RESUME_END_FRAMES);
    resume_end(b, s);
}

/// 0x15c64: state 0xa, the end message stays displayed for 120 frames, then the panel is released.
pub(super) fn resume_end(b: &mut World, s: Slot) {
    s.set_p8(b, panel::FLAG_27, 0);
    let left = s.p16(b, panel::COUNTDOWN).wrapping_sub(1);
    s.set_p16(b, panel::COUNTDOWN, left);
    if left != 0 {
        return;
    }
    s.set_p8(b, panel::STATE, 0);
    s.set_p8(b, panel::STEP, 0);
    s.set_p8(b, panel::SUBSTEP, 0);
    s.set_p8(b, 0x30, 0);
    s.set_p8(b, panel::LAST_MODE, 0xff);
    s.set_p8(b, panel::PLAYER_DIRTY, 0xff);
}

/// 0x15cae: redraws the continue choice screen (after a pause).
pub(super) fn redraw_resume_choice(b: &mut World, s: Slot) {
    draw::portrait(b, s, false);
    let choice = s.p8(b, panel::CHOICE);
    choice::show_character(b, s, choice);
    if choice >> 1 == s.p8(b, panel::CHOSEN) >> 1 {
        let class = s.g8(b, player::CLASS);
        draw::face(b, s, class);
    }
    draw::countdown_digits(b, s);
    s.set_p8(b, panel::PLAYER_DIRTY, 0);
}

/// 0x15c8e: state 0xc, the continuing player chooses their character; they may resume the one from their game
/// (same character: they keep their equipment) or another one.
pub(super) fn resume_choose(x: &mut Ctx, s: Slot) {
    let b = &mut *x.b;
    let pause = var::PAUSE.get(b);
    let seen = var::SEEN_PAUSE.get(b);
    if pause != seen {
        if !pause & seen != 0 {
            redraw_resume_choice(b, s);
        } else {
            draw::clear_banner(b, s);
            return notice_banner(b, s);
        }
    }
    if s.p8(b, panel::PLAYER_DIRTY) != 0 {
        if var::PAUSE.get(b) == 0 {
            redraw_resume_choice(b, s);
        } else {
            draw::clear_banner(b, s);
            return notice_banner(b, s);
        }
    }
    if var::PAUSE.get(b) != 0 {
        return notice_banner(b, s);
    }
    let own = s.p8(b, panel::CHOSEN) >> 1;
    let other = |b: &mut World, c: u8, back: bool| select::other_choice(b, s, c, back);
    let current = s.p8(b, panel::CHOICE);
    let choice = other(b, current, false);
    let choice = select::navigate(b, s, choice, &other);
    if choice != s.p8(b, panel::CHOICE) {
        s.set_p8(b, panel::CHOICE, choice);
        choice::show_character(b, s, choice);
        if s.p8(b, panel::CHOICE) >> 1 == own {
            let class = s.g8(b, player::CLASS);
            draw::face(b, s, class);
        }
        draw::countdown_digits(b, s);
    }
    let mut done = false;
    let frames = s.p8(b, panel::COUNTDOWN_FRAMES).wrapping_sub(1);
    s.set_p8(b, panel::COUNTDOWN_FRAMES, frames);
    if frames == 0 {
        if tick_countdown(b, s, 0x32) {
            done = true;
        } else {
            draw::countdown_digits(b, s);
        }
    }
    if !done && !button_pressed(b, s) {
        if !start_edge(b, s) {
            return;
        }
        let variant = s.p8(b, panel::CHOICE) ^ 1;
        s.set_p8(b, panel::CHOICE, variant);
    }
    // the choice is made: the last digit of the score advances (up to 9)
    let last = s.g8(b, player::SCORE_LOW);
    if last & 0xf != 9 {
        s.set_g8(b, player::SCORE_LOW, last.wrapping_add(1));
    }
    let current = s.p8(b, panel::CHOICE);
    let chosen = select::resume_choice(b, s, current, false);
    s.set_p8(b, panel::CHOICE, chosen);
    s.set_g8(b, player::CHARACTER, chosen);
    crate::hw::eeprom::stat_chosen_character(x.b, chosen as u16);
    let b = &mut *x.b;
    let counter = CONTINUES.get(b).wrapping_add(1);
    CONTINUES.set(b, counter);
    s.set_p8(b, panel::LAST_MODE, 0x80);
    s.set_p8(b, panel::STATE, 0);
    if chosen >> 1 == s.p8(b, panel::CHOSEN) >> 1 {
        s.set_p32(b, panel::FRAMES, 0);
        start_player(x, s, Join::Refill);
    } else {
        let class = rom::CLASS_OF_CHARACTER_ALT.pc_byte(b, (chosen >> 1) as u32);
        s.set_g8(b, player::SELECTION_KIND, class);
        start_player(x, s, Join::Continue);
    }
}

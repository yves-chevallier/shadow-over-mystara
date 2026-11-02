//! Character choice: characters already taken, search for the next free one, d-pad reading.
//!
//! A character is a number from 0 to 11: the number (step of 2) and, in bit 0, the color
//! variant. A player cannot take another player's character.

use cps2::Cps2Bus;

use super::{panel, player, v8, var, Slot};
use crate::mem::{rd8, A5};

/// Characters taken (one bit per character and variant): those of the present players, or the one they
/// have chosen if they are currently choosing (states 8 and 0xc). `except`: a player to ignore (their own).
fn taken(b: &mut Cps2Bus, except: Option<u32>) -> u32 {
    let count = 3u8.wrapping_sub(v8(b, var::LAYOUT)) as u32 + 1;
    let mut mask = 0u32;
    for k in 0..count {
        let who = A5 + var::PLAYERS + player::SIZE * k;
        let panel_k = A5 + var::PANELS + panel::SIZE * k;
        if rd8(b, who) == 0 || Some(who) == except {
            continue;
        }
        let state = rd8(b, panel_k + panel::STATE);
        let character = if state == 8 || state == 0xc { rd8(b, panel_k + panel::CHOSEN) } else { rd8(b, who + player::CHARACTER) };
        mask |= 1 << (character & 31);
    }
    mask
}

fn is_free(mask: u32, choice: u8) -> bool {
    mask >> (choice & 31) & 1 == 0
}

/// Searches from `choice` for a free character, forwards or backwards. If the neighboring variant is
/// free, it is taken; otherwise move on to the next (or previous) character. Also returns whether the search
/// stopped on the neighboring variant (`true`) or on the choice itself.
fn search(mask: u32, mut choice: u8, backward: bool) -> (u8, bool) {
    loop {
        if is_free(mask, choice) {
            return (choice, false);
        }
        choice ^= 1;
        if is_free(mask, choice) {
            return (choice, true);
        }
        choice &= !1;
        if backward {
            choice = choice.wrapping_sub(2);
            if (choice as i8) < 0 {
                choice = 0xa;
            }
        } else {
            choice += 2;
            if choice > 0xb {
                choice = 0;
            }
        }
    }
}

/// The player keeps their variant if the character found is their own (`panel::CHOSEN`).
fn keep_own_variant(b: &mut Cps2Bus, s: Slot, choice: u8) -> u8 {
    let own = s.p8(b, panel::CHOSEN);
    if own >> 1 == choice >> 1 {
        own
    } else {
        choice
    }
}

/// 0x153ba: first free character from `choice` (choice when entering the game).
pub fn free_choice(b: &mut Cps2Bus, choice: u8, backward: bool) -> u8 {
    let mask = taken(b, None);
    search(mask, choice, backward).0
}

/// 0x15444: like `free_choice`, not counting the player's own character, which keeps its variant.
pub fn other_choice(b: &mut Cps2Bus, s: Slot, choice: u8, backward: bool) -> u8 {
    let mask = taken(b, Some(s.player));
    let (found, _) = search(mask, choice, backward);
    keep_own_variant(b, s, found)
}

/// 0x154e8: like `other_choice`, but the character found is returned as is, except backwards when it is
/// the neighboring variant (the original then exits through the end of 0x15444).
pub fn resume_choice(b: &mut Cps2Bus, s: Slot, choice: u8, backward: bool) -> u8 {
    let mask = taken(b, Some(s.player));
    let (found, neighbour) = search(mask, choice, backward);
    if backward && neighbour {
        keep_own_variant(b, s, found)
    } else {
        found
    }
}

/// 0x173ee: inputs of player `index`: current state and rising edges.
pub fn inputs(b: &mut Cps2Bus, index: u8) -> (u8, u8) {
    (rd8(b, A5 + var::INPUT + index as u32), rd8(b, A5 + var::PRESSED + index as u32))
}

/// D-pad repeat: number of frames before repeating a held movement.
const REPEAT_FRAMES: i8 = 0x3c;

/// 0x158c6 / 0x15958: reads the d-pad and moves the choice `choice` by one step (to the right: 1 or 2 depending on
/// the variant;
/// to the left: 2 or 3) on each rising edge, then every 60 frames if the direction is held. The
/// new choice goes through the free-character search (`resolve`).
pub fn navigate(
    b: &mut Cps2Bus,
    s: Slot,
    choice: u8,
    resolve: &dyn Fn(&mut Cps2Bus, u8, bool) -> u8,
) -> u8 {
    let index = s.index(b);
    let (held, pressed) = inputs(b, index);
    let direction = held & 3;
    if direction == 0 {
        s.set_p8(b, panel::FLAG_27, 0);
        return choice;
    }
    let right = direction == 1;
    let edge = if right { pressed & 1 != 0 } else { pressed & 2 != 0 };
    if !edge {
        let held_frames = s.p8(b, panel::FLAG_27).wrapping_add(1);
        s.set_p8(b, panel::FLAG_27, held_frames);
        if (held_frames as i8) < REPEAT_FRAMES {
            return choice;
        }
    }
    s.set_p8(b, panel::FLAG_27, 0);
    if right {
        let mut next = choice.wrapping_add(if choice & 1 != 0 { 1 } else { 2 });
        if next > 0xb {
            next = 0;
        }
        resolve(b, next, false)
    } else {
        let mut next = choice.wrapping_sub(if choice & 1 != 0 { 3 } else { 2 });
        if (next as i8) < 0 {
            next = 0xa;
        }
        resolve(b, next, true)
    }
}

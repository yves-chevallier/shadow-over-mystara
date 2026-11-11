use super::*;

// ---------------------------------------------------------------------------------------------
// Type 23: players' vote for the next path (variant 0: ballot; 1: a player's cursor;
// 2: countdown)
// ---------------------------------------------------------------------------------------------

pub(super) mod vote {
    pub const TEXT: u32 = 0xff2072;
    pub const SAVE: u32 = 0x910000;
    pub const ANIM_SET: crate::world::data::RomAddr = crate::world::data::RomAddr(0x1c03a6);
    /// Ballot: pointer (creator `+0x50`), remaining voters `+0x42`, living cursors `+0x44`,
    /// counted `+0x53`, duration `+0x54` (zero: time up), votes per choice `+0x58..+0x5b`.
    pub const BALLOT: u32 = 0x50;
}

pub(super) fn spawn_vote<R: Bench>(x: &mut Cx<'_, R>, variant: u8, mode: u8) -> Option<Obj> {
    let o = x.alloc(&pools::EFFCT)?;
    wr8(x.b, o.0, 1);
    wr8(x.b, o.0 + 2, 23);
    wr8(x.b, o.0 + 3, variant);
    wr8(x.b, o.0 + 0x2f, mode);
    Some(o)
}

pub(super) fn vote<R: Bench>(x: &mut Cx<'_, R>) {
    match x.u8(f::VARIANT) {
        0 => ballot(x),
        1 => ballot_cursor(x),
        2 => ballot_timer(x),
        _ => {}
    }
}

/// Part of `table` for the number of players (`A5+0x72`: 2 or 4): the table starts with its addresses (longs), the
/// parts follow; returns the offset of the part in the table.
pub(super) fn by_players<R: Bench>(x: &mut Cx<'_, R>, table: &Table) -> u32 {
    let k = (vars::CABINET_PLAYERS.get(x.b).wrapping_sub(2).wrapping_mul(2)) as i16 as i32 as u32;
    table.at32(x.b, k).wrapping_sub(table.rom)
}

pub(super) fn ballot<R: Bench>(x: &mut Cx<'_, R>) {
    match x.u8(f::STATE) {
        0 => {
            x.set8(Ballot::COUNTED, 0);
            x.add8(f::STATE, 2);
            x.set16(Ballot::DURATION, 0xffff);
            x.set32(Ballot::VOTES, 0);
            let mode = x.u8(Ballot::MODE) as u32;
            let choices = tables().vote_choices.at8(x.b, mode);
            x.set8(Ballot::LAST_CHOICE, choices);
            // one cursor per player in the game
            let mut voters = 0u8;
            for k in 0..4 {
                let p = crate::world::ram::PLAYERS.nth(k);
                if !p.is_active(x.b) || (p.u8(x.b, f::STATE) as i8) >= 4 {
                    continue;
                }
                if let Some(o) = spawn_vote(x, 1, voters) {
                    wr16(x.b, o.0 + 0x50, x.o.0 as u16);
                    wr16(x.b, o.0 + 0x54, p.0 as u16);
                    let index = p.u8(x.b, Obj::KIND);
                    wr8(x.b, o.0 + 0x2f, index);
                    wr8(x.b, o.0 + 0x43, choices);
                    voters += 1;
                }
            }
            x.set8(Ballot::VOTERS, voters);
            x.set8(Ballot::CURSORS, voters);
            if let Some(o) = spawn_vote(x, 2, voters) {
                wr16(x.b, o.0 + 0x50, x.o.0 as u16);
            }
        }
        2 => {
            if x.u8(Ballot::VOTERS) == 0 {
                x.set16(Ballot::DURATION, 0);
                x.add8(f::STATE, 2);
                x.set8(f::SUB_STATE, 0);
            }
        }
        4 => ballot_count(x),
        6 => ballot_end(x),
        _ => {}
    }
}

/// 0xb01e2: end, when all the cursors have left; notifies the creator.
pub(super) fn ballot_end<R: Bench>(x: &mut Cx<'_, R>) {
    if x.u8(Ballot::CURSORS) != 0 {
        return;
    }
    let creator = x.linked(vote::BALLOT);
    creator.set_u16(x.b, VoteCreator::DONE, 1);
    free(x);
}

pub(super) fn ballot_count<R: Bench>(x: &mut Cx<'_, R>) {
    match x.u8(f::SUB_STATE) {
        0 => {
            // winning choice: the most votes, at random among ties
            let creator = x.linked(vote::BALLOT);
            let mut best = x.u8(Ballot::VOTES) as i8;
            for k in 1..4 {
                best = best.max(x.u8(Ballot::VOTES + k) as i8);
            }
            for k in 0..4 {
                let v = if x.u8(Ballot::VOTES + k) as i8 == best { 0xff } else { 0 };
                x.set8(Ballot::VOTES + k, v);
            }
            let rng = x.random();
            let mut k = rng as u32 & 3;
            while x.u8(Ballot::VOTES + k) == 0 {
                k = (k + 1) & 3;
            }
            let choice = k as u8;
            creator.set_u8(x.b, VoteCreator::RESULT, choice);
            x.set8(Ballot::VOTES, choice);
            x.set8(Ballot::COUNTED, 1);
            x.add8(f::SUB_STATE, 2);
        }
        2 => {
            x.add8(f::SUB_STATE, 2);
            let table = &tables().vote_text_zones;
            let zones = by_players(x, table);
            let mode = x.u8(Ballot::MODE) as u32;
            let mut off = table.at16(x.b, zones + 2 * mode);
            off = off.wrapping_add((x.u8(Ballot::VOTES) as u16) << 4);
            let level = (crate::world::ram::PLAYERS_ALIVE.get(x.b).wrapping_sub(1) as u16) << 2;
            off = off.wrapping_add(level);
            let zone = zones.wrapping_add(off as i16 as i32 as u32);
            let count = table.at16(x.b, zone);
            let size = table.at16(x.b, zone + 2);
            x.set16(Ballot::TEXT_SIZE, size);
            ballot_save_text(x, count);
            x.set8(Ballot::BLINKS, 8);
            x.set8(Ballot::BLINK_TIMER, 3);
            x.set8(Ballot::TEXT_SHOWN, 1);
        }
        4 => {
            let t = x.u8(Ballot::BLINK_TIMER).wrapping_sub(1);
            x.set8(Ballot::BLINK_TIMER, t);
            if t != 0 {
                return;
            }
            x.set8(Ballot::BLINK_TIMER, 2);
            let at = x.u32(Ballot::TEXT_AT);
            let n = x.u16(Ballot::TEXT_SIZE);
            let shown = x.u8(Ballot::TEXT_SHOWN) ^ 1;
            x.set8(Ballot::TEXT_SHOWN, shown);
            if shown == 0 {
                return ballot_erase_text(x, at, n);
            }
            x.set8(Ballot::BLINK_TIMER, 3);
            let format = text_format(x);
            text_copy(x, format, vote::SAVE, at, n);
            let left = x.u8(Ballot::BLINKS).wrapping_sub(1);
            x.set8(Ballot::BLINKS, left);
            if left == 0 {
                x.add8(f::SUB_STATE, 2);
                x.set8(Ballot::BLINK_TIMER, 0x1e);
            }
        }
        6 => {
            let t = x.u8(Ballot::BLINK_TIMER).wrapping_sub(1);
            x.set8(Ballot::BLINK_TIMER, t);
            if t == 0 {
                let creator = x.linked(vote::BALLOT);
                creator.set_u16(x.b, VoteCreator::DONE, 1);
                x.add8(f::STATE, 2);
            }
        }
        _ => {}
    }
}

/// Text of the text layer, in words (`A5+0x72` = 2) or in bytes (other languages).
/// In words: 0xffff introduces a 6-byte command, 0xfffe a 4-byte one, the other words are
/// characters. In bytes: 0xff introduces a 3-byte command, 0xfe a 2-byte one, a byte ≤ 0x0f a
/// 2-byte character, the other bytes are characters.
#[derive(Clone, Copy, PartialEq)]
pub(super) enum TextFormat {
    Words,
    Bytes,
}

pub(super) fn text_format<R: Bench>(x: &mut Cx<'_, R>) -> TextFormat {
    if vars::CABINET_PLAYERS.get(x.b) == 2 { TextFormat::Words } else { TextFormat::Bytes }
}

/// Length of the text element at `at`, and whether it is a character.
pub(super) fn text_token<R: Bench>(x: &mut Cx<'_, R>, format: TextFormat, at: u32) -> (u32, bool) {
    match format {
        TextFormat::Words => match rd16(x.b, at) {
            0xffff => (6, false),
            0xfffe => (4, false),
            _ => (2, true),
        },
        TextFormat::Bytes => match rd8(x.b, at) {
            0xff => (3, false),
            0xfe => (2, false),
            c if c <= 0xf => (2, true),
            _ => (1, true),
        },
    }
}

/// Advances by `n + 1` characters (commands included) from `at`, calling `each` on each
/// element (address, length, character?). Returns the address that follows.
pub(super) fn text_walk<R: Bench>(x: &mut Cx<'_, R>, format: TextFormat, mut at: u32, n: u16, mut each: impl FnMut(&mut Cx<'_, R>, u32, u32, bool)) -> u32 {
    let mut n = n;
    loop {
        let (len, is_char) = text_token(x, format, at);
        each(x, at, len, is_char);
        at += len;
        if is_char {
            n = n.wrapping_sub(1);
            if n == 0xffff {
                return at;
            }
        }
    }
}

/// Copies `n + 1` characters (commands included) from `from` to `to`.
pub(super) fn text_copy<R: Bench>(x: &mut Cx<'_, R>, format: TextFormat, from: u32, to: u32, n: u16) {
    text_walk(x, format, from, n, |x, at, len, _| {
        for k in 0..len {
            let v = rd8(x.b, at + k);
            wr8(x.b, to + (at - from) + k, v);
        }
    });
}

/// 0xb0102: skips `count + 1` characters, notes the address and saves the winning choice's text.
pub(super) fn ballot_save_text<R: Bench>(x: &mut Cx<'_, R>, count: u16) {
    let format = text_format(x);
    let at = text_walk(x, format, vote::TEXT, count, |_, _, _, _| {});
    x.set32(Ballot::TEXT_AT, at);
    let size = x.u16(Ballot::TEXT_SIZE);
    text_copy(x, format, at, vote::SAVE, size);
}

/// 0xb0076: clears the text (blank character: 0x1800 in words; 0x80, or 1 then 0 for a
/// 2-byte character), commands kept.
pub(super) fn ballot_erase_text<R: Bench>(x: &mut Cx<'_, R>, at: u32, n: u16) {
    let format = text_format(x);
    text_walk(x, format, at, n, |x, at, len, is_char| {
        if !is_char {
            return;
        }
        match (format, len) {
            (TextFormat::Words, _) => wr16(x.b, at, 0x1800),
            (TextFormat::Bytes, 2) => {
                wr8(x.b, at, 1);
                wr8(x.b, at + 1, 0);
            }
            _ => wr8(x.b, at, 0x80),
        }
    });
}

/// 0xb03c6: places the cursor on its choice (`+0x42`), according to the ballot's mode and the player.
pub(super) fn cursor_place<R: Bench>(x: &mut Cx<'_, R>) {
    let ballot = x.linked(vote::BALLOT);
    let table = &tables().vote_cursor_places;
    let places = by_players(x, table);
    let mode = ballot.u8(x.b, Ballot::MODE) as u32;
    let places = places.wrapping_add(table.at16(x.b, places + 2 * mode) as i16 as i32 as u32);
    let k = ((x.u8(VoteCursor::CHOICE) as u32) << 4) + 4 * x.u8(VoteCursor::PLAYER) as u32;
    let (px, pz) = (table.at16(x.b, places + k), table.at16(x.b, places + k + 2));
    let (cx_, cz) = if crate::world::ram::CAMERA_ALT_ACTIVE.get(x.b) != 0 {
        (crate::world::ram::CAMERA_ALT_X.get(x.b), crate::world::ram::CAMERA_ALT_Y.get(x.b))
    } else {
        (crate::world::ram::CAMERA_X.get(x.b), crate::world::ram::CAMERA_Y.get(x.b))
    };
    x.set16(Obj::X, px.wrapping_add(cx_));
    x.set16(Obj::Z, pz.wrapping_add(cz));
}

pub(super) fn ballot_cursor<R: Bench>(x: &mut Cx<'_, R>) {
    match x.u8(f::STATE) {
        0 => {
            x.add8(f::STATE, 2);
            x.set8(VoteCursor::CHOICE, 0);
            x.set32(Obj::Y, 0xffff_ff00);
            cursor_place(x);
            let player = x.u8(VoteCursor::PLAYER);
            x.add32(Obj::Y, player as u32);
            x.set8(VoteCursor::OWN_PALETTE, 1);
            x.set8(VoteCursor::PALETTE, 0x12u8.wrapping_add(player));
            x.anim_start(vote::ANIM_SET.0, (0x173 + player as u32) as u16);
            x.reinsert();
        }
        2 => {
            let ballot = x.linked(vote::BALLOT);
            if ballot.u16(x.b, Ballot::DURATION) == 0 {
                return cursor_vote(x);
            }
            x.anim_step();
            // 0xb0390: the player's inputs (left/right, and up/down for some modes)
            let mut mask: u8 = 0xfc;
            let ballot_mode = ballot.u8(x.b, Ballot::MODE) as u32;
            if tables().vote_vertical.at8(x.b, ballot_mode) != 0 {
                mask = 0xf3;
            }
            let player = x.u8(VoteCursor::PLAYER) as u32;
            let input = crate::world::ram::Byte(vars::INPUTS.0 + player).get(x.b) & mask;
            if input == 0 {
                return cursor_place(x);
            }
            let step: u8 = if input & 5 != 0 {
                1
            } else if input & 0xa != 0 {
                0xff
            } else {
                0
            };
            x.add8(VoteCursor::CHOICE, step);
            let last = x.u8(VoteCursor::LAST_CHOICE);
            let choice = x.u8(VoteCursor::CHOICE);
            if last < choice {
                // beyond the last choice: back to the first, or to the last below the first
                x.set8(VoteCursor::CHOICE, if (last.wrapping_sub(choice) as i8) < 0 { 0 } else { last });
            }
            if input & 0xf0 == 0 {
                return cursor_place(x);
            }
            cursor_vote(x);
        }
        4 => {
            let ballot = x.linked(vote::BALLOT);
            if ballot.u8(x.b, Ballot::COUNTED) == 0 {
                return cursor_place(x);
            }
            let n = ballot.u8(x.b, Ballot::CURSORS).wrapping_sub(1);
            ballot.set_u8(x.b, Ballot::CURSORS, n);
            free(x);
        }
        _ => {}
    }
}

/// 0xb02b4: the player votes for the current choice.
pub(super) fn cursor_vote<R: Bench>(x: &mut Cx<'_, R>) {
    let ballot = x.linked(vote::BALLOT);
    let n = ballot.u8(x.b, Ballot::VOTERS).wrapping_sub(1);
    ballot.set_u8(x.b, Ballot::VOTERS, n);
    let choice = x.u8(VoteCursor::CHOICE) as u32;
    let votes = ballot.u8(x.b, Ballot::VOTES + choice).wrapping_add(1);
    ballot.set_u8(x.b, Ballot::VOTES + choice, votes);
    x.add8(f::STATE, 2);
    cursor_place(x);
    x.unlink();
}

/// 0xb0990: the countdown is placed on the camera, at the place given by the ballot's mode.
pub(super) fn timer_place<R: Bench>(x: &mut Cx<'_, R>) -> Obj {
    let ballot = x.linked(vote::BALLOT);
    let (cx_, cz) = if crate::world::ram::CAMERA_ALT_ACTIVE.get(x.b) != 0 {
        (crate::world::ram::CAMERA_ALT_X.get(x.b), crate::world::ram::CAMERA_ALT_Y.get(x.b))
    } else {
        (crate::world::ram::CAMERA_X.get(x.b), crate::world::ram::CAMERA_Y.get(x.b))
    };
    x.set16(Obj::X, cx_);
    x.set16(Obj::Z, cz);
    let table = &tables().vote_timer_places;
    let places = by_players(x, table);
    let k = 4 * ballot.u8(x.b, Ballot::MODE) as u32;
    let (dx, dz) = (table.at16(x.b, places + k), table.at16(x.b, places + k + 2));
    x.add16(Obj::X, dx);
    x.add16(Obj::Z, dz);
    ballot
}

pub(super) fn timer_palette<R: Bench>(x: &mut Cx<'_, R>, src: &Table) {
    for k in 0..8 {
        let v = src.nth32(x.b, k) | 0xf000_f000;
        wr32(x.b, 0x91c1c0 + 4 * k, v);
    }
}

/// 0xb0330: countdown digit (seconds remaining, high byte of `+0x54`).
pub(super) fn timer_show<R: Bench>(x: &mut Cx<'_, R>) {
    let anim = 0xe3u8.wrapping_sub(x.u8(VoteTimer::SECONDS));
    x.anim_start(vote::ANIM_SET.0, (anim as u32) as u16);
    x.set8(VoteTimer::FRAME_ATTRIBUTE, 6);
    x.reinsert();
}

pub(super) fn ballot_timer<R: Bench>(x: &mut Cx<'_, R>) {
    match x.u8(f::STATE) {
        0 => {
            x.add8(f::STATE, 2);
            timer_palette(x, crate::services::palette::library_excerpt!("vote_timer_palette", 0x28ff20, 0x28ff40, 4));
            x.set8(VoteTimer::OWN_PALETTE, 1);
            x.set8(VoteTimer::PALETTE, 0xe);
            x.set32(Obj::Y, 0);
            timer_place(x);
            x.set16(VoteTimer::SECONDS, 0x143c);
            timer_show(x);
        }
        2 => {
            let ballot = timer_place(x);
            if ballot.u16(x.b, Ballot::DURATION) == 0 {
                return ballot_end(x);
            }
            let t = x.u8(VoteTimer::FRAMES).wrapping_sub(1);
            x.set8(VoteTimer::FRAMES, t);
            if (t as i8) >= 0 {
                return;
            }
            x.set8(VoteTimer::FRAMES, 0x3c);
            let s = x.u8(VoteTimer::SECONDS).wrapping_sub(1);
            x.set8(VoteTimer::SECONDS, s);
            if s != 0 {
                return timer_show(x);
            }
            // time elapsed: the ballot ends
            ballot.set_u16(x.b, Ballot::DURATION, 0);
            timer_palette(x, crate::services::palette::library_excerpt!("vote_timer_palette_end", 0x2531c0, 0x2531e0, 4));
            free(x);
        }
        // Original dead code: state 4 (0xb0374), which no step leads to; the original clears word `+0x54` there
        // of the object designated by the A6 that the previous code leaves, restores the palette and releases the
        // object (see
        // docs/code-mort.md, "vote timer, state 4")
        4 => unreachable!("vote timer (effct_vote 0xafe20, variant 2): no step leads to state 4 (0xb0374)"),
        // state 6 (set by the off-screen removal): release (0xb02e8)
        6 => free(x),
        _ => {}
    }
}


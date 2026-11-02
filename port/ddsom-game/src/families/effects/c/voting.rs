use super::*;

// ---------------------------------------------------------------------------------------------
// Type 23: players' vote for the next path (variant 0: ballot; 1: a player's cursor;
// 2: countdown)
// ---------------------------------------------------------------------------------------------

pub(super) mod vote {
    /// Number of choices per mode `+0x2f` (bytes, data).
    pub const CHOICES: u32 = 0xafec8;
    /// Cursor positions: by number of players (`A5+0x72`), offset table per mode, then
    /// (x, z) per choice and per player (data).
    pub const CURSOR_PLACES: u32 = 0xb042c;
    /// Countdown position, same indexing (data).
    pub const TIMER_PLACES: u32 = 0xb09e0;
    /// Text zones of the choices to blink (data); save buffer.
    pub const TEXT_ZONES: u32 = 0xb0a9c;
    pub const TEXT: u32 = 0xff2072;
    pub const SAVE: u32 = 0x910000;
    /// Modes where the up/down directions also count (bytes, program space).
    pub const VERTICAL: u32 = 0xb03b6;
    pub const ANIM_SET: u32 = 0x1c03a6;
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

/// Word of the table indexed by the number of players (`A5+0x72`: 2 or 4).
pub(super) fn by_players<R: Bench>(x: &mut Cx<'_, R>, table: u32) -> u32 {
    let k = (x.a5_u16(0x72).wrapping_sub(2).wrapping_mul(2)) as i16 as i32 as u32;
    x.data32(table.wrapping_add(k))
}

pub(super) fn ballot<R: Bench>(x: &mut Cx<'_, R>) {
    match x.u8(f::STATE) {
        0 => {
            x.set8(0x53, 0);
            x.add8(f::STATE, 2);
            x.set16(0x54, 0xffff);
            x.set32(0x58, 0);
            let mode = x.u8(0x2f) as u32;
            let choices = x.data8(vote::CHOICES + mode);
            x.set8(0x43, choices);
            // one cursor per player in the game
            let mut voters = 0u8;
            for k in 0..4 {
                let p = Obj(A5 + 0x5de + 0x200 * k);
                if !p.is_active(x.b) || (p.u8(x.b, f::STATE) as i8) >= 4 {
                    continue;
                }
                if let Some(o) = spawn_vote(x, 1, voters) {
                    wr16(x.b, o.0 + 0x50, x.o.0 as u16);
                    wr16(x.b, o.0 + 0x54, p.0 as u16);
                    let index = p.u8(x.b, 2);
                    wr8(x.b, o.0 + 0x2f, index);
                    wr8(x.b, o.0 + 0x43, choices);
                    voters += 1;
                }
            }
            x.set8(0x42, voters);
            x.set8(0x44, voters);
            if let Some(o) = spawn_vote(x, 2, voters) {
                wr16(x.b, o.0 + 0x50, x.o.0 as u16);
            }
        }
        2 => {
            if x.u8(0x42) == 0 {
                x.set16(0x54, 0);
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
    if x.u8(0x44) != 0 {
        return;
    }
    let creator = x.linked(vote::BALLOT);
    creator.set_u16(x.b, 0x32, 1);
    free(x);
}

pub(super) fn ballot_count<R: Bench>(x: &mut Cx<'_, R>) {
    match x.u8(f::SUB_STATE) {
        0 => {
            // winning choice: the most votes, at random among ties
            let creator = x.linked(vote::BALLOT);
            let mut best = x.u8(0x58) as i8;
            for k in 1..4 {
                best = best.max(x.u8(0x58 + k) as i8);
            }
            for k in 0..4 {
                let v = if x.u8(0x58 + k) as i8 == best { 0xff } else { 0 };
                x.set8(0x58 + k, v);
            }
            let rng = x.random();
            let mut k = rng as u32 & 3;
            while x.u8(0x58 + k) == 0 {
                k = (k + 1) & 3;
            }
            let choice = k as u8;
            creator.set_u8(x.b, 0x23, choice);
            x.set8(0x58, choice);
            x.set8(0x53, 1);
            x.add8(f::SUB_STATE, 2);
        }
        2 => {
            x.add8(f::SUB_STATE, 2);
            let zones = by_players(x, vote::TEXT_ZONES);
            let mode = x.u8(0x2f) as u32;
            let mut off = x.data16(zones + 2 * mode);
            off = off.wrapping_add((x.u8(0x58) as u16) << 4);
            let level = (x.a5_u8(0x35).wrapping_sub(1) as u16) << 2;
            off = off.wrapping_add(level);
            let zone = zones.wrapping_add(off as i16 as i32 as u32);
            let count = x.data16(zone);
            let size = x.data16(zone + 2);
            x.set16(0x34, size);
            ballot_save_text(x, count);
            x.set8(0x36, 8);
            x.set8(0x37, 3);
            x.set8(0x38, 1);
        }
        4 => {
            let t = x.u8(0x37).wrapping_sub(1);
            x.set8(0x37, t);
            if t != 0 {
                return;
            }
            x.set8(0x37, 2);
            let at = x.u32(0x30);
            let n = x.u16(0x34);
            let shown = x.u8(0x38) ^ 1;
            x.set8(0x38, shown);
            if shown == 0 {
                return ballot_erase_text(x, at, n);
            }
            x.set8(0x37, 3);
            let format = text_format(x);
            text_copy(x, format, vote::SAVE, at, n);
            let left = x.u8(0x36).wrapping_sub(1);
            x.set8(0x36, left);
            if left == 0 {
                x.add8(f::SUB_STATE, 2);
                x.set8(0x37, 0x1e);
            }
        }
        6 => {
            let t = x.u8(0x37).wrapping_sub(1);
            x.set8(0x37, t);
            if t == 0 {
                let creator = x.linked(vote::BALLOT);
                creator.set_u16(x.b, 0x32, 1);
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
    if x.a5_u16(0x72) == 2 { TextFormat::Words } else { TextFormat::Bytes }
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
    x.set32(0x30, at);
    let size = x.u16(0x34);
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
    let places = by_players(x, vote::CURSOR_PLACES);
    let mode = ballot.u8(x.b, 0x2f) as u32;
    let places = places.wrapping_add(x.data16(places + 2 * mode) as i16 as i32 as u32);
    let k = ((x.u8(0x42) as u32) << 4) + 4 * x.u8(0x2f) as u32;
    let (px, pz) = (x.data16(places + k), x.data16(places + k + 2));
    let (cx_, cz) = if crate::ram::CAMERA_ALT_ACTIVE.get(x.b) != 0 {
        (crate::ram::CAMERA_ALT_X.get(x.b), crate::ram::CAMERA_ALT_Y.get(x.b))
    } else {
        (crate::ram::CAMERA_X.get(x.b), crate::ram::CAMERA_Y.get(x.b))
    };
    x.set16(Obj::X, px.wrapping_add(cx_));
    x.set16(Obj::Z, pz.wrapping_add(cz));
}

pub(super) fn ballot_cursor<R: Bench>(x: &mut Cx<'_, R>) {
    match x.u8(f::STATE) {
        0 => {
            x.add8(f::STATE, 2);
            x.set8(0x42, 0);
            x.set32(Obj::Y, 0xffff_ff00);
            cursor_place(x);
            let player = x.u8(0x2f);
            x.add32(Obj::Y, player as u32);
            x.set8(0x2a, 1);
            x.set8(0x2b, 0x12u8.wrapping_add(player));
            x.anim_start(vote::ANIM_SET, (0x173 + player as u32) as u16);
            x.reinsert();
        }
        2 => {
            let ballot = x.linked(vote::BALLOT);
            if ballot.u16(x.b, 0x54) == 0 {
                return cursor_vote(x);
            }
            x.anim_step();
            // 0xb0390: the player's inputs (left/right, and up/down for some modes)
            let mut mask: u8 = 0xfc;
            let ballot_mode = ballot.u8(x.b, 0x2f) as u32;
            if x.rom8(vote::VERTICAL + ballot_mode) != 0 {
                mask = 0xf3;
            }
            let player = x.u8(0x2f) as u32;
            let input = x.a5_u8(0xc + player) & mask;
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
            x.add8(0x42, step);
            let last = x.u8(0x43);
            let choice = x.u8(0x42);
            if last < choice {
                // beyond the last choice: back to the first, or to the last below the first
                x.set8(0x42, if (last.wrapping_sub(choice) as i8) < 0 { 0 } else { last });
            }
            if input & 0xf0 == 0 {
                return cursor_place(x);
            }
            cursor_vote(x);
        }
        4 => {
            let ballot = x.linked(vote::BALLOT);
            if ballot.u8(x.b, 0x53) == 0 {
                return cursor_place(x);
            }
            let n = ballot.u8(x.b, 0x44).wrapping_sub(1);
            ballot.set_u8(x.b, 0x44, n);
            free(x);
        }
        _ => {}
    }
}

/// 0xb02b4: the player votes for the current choice.
pub(super) fn cursor_vote<R: Bench>(x: &mut Cx<'_, R>) {
    let ballot = x.linked(vote::BALLOT);
    let n = ballot.u8(x.b, 0x42).wrapping_sub(1);
    ballot.set_u8(x.b, 0x42, n);
    let choice = x.u8(0x42) as u32;
    let votes = ballot.u8(x.b, 0x58 + choice).wrapping_add(1);
    ballot.set_u8(x.b, 0x58 + choice, votes);
    x.add8(f::STATE, 2);
    cursor_place(x);
    x.unlink();
}

/// 0xb0990: the countdown is placed on the camera, at the place given by the ballot's mode.
pub(super) fn timer_place<R: Bench>(x: &mut Cx<'_, R>) -> Obj {
    let ballot = x.linked(vote::BALLOT);
    let (cx_, cz) = if crate::ram::CAMERA_ALT_ACTIVE.get(x.b) != 0 {
        (crate::ram::CAMERA_ALT_X.get(x.b), crate::ram::CAMERA_ALT_Y.get(x.b))
    } else {
        (crate::ram::CAMERA_X.get(x.b), crate::ram::CAMERA_Y.get(x.b))
    };
    x.set16(Obj::X, cx_);
    x.set16(Obj::Z, cz);
    let places = by_players(x, vote::TIMER_PLACES);
    let k = 4 * ballot.u8(x.b, 0x2f) as u32;
    let (dx, dz) = (x.data16(places + k), x.data16(places + k + 2));
    x.add16(Obj::X, dx);
    x.add16(Obj::Z, dz);
    ballot
}

pub(super) fn timer_palette<R: Bench>(x: &mut Cx<'_, R>, src: u32) {
    for k in 0..8 {
        let v = x.data32(src + 4 * k) | 0xf000_f000;
        wr32(x.b, 0x91c1c0 + 4 * k, v);
    }
}

/// 0xb0330: countdown digit (seconds remaining, high byte of `+0x54`).
pub(super) fn timer_show<R: Bench>(x: &mut Cx<'_, R>) {
    let anim = 0xe3u8.wrapping_sub(x.u8(0x54));
    x.anim_start(vote::ANIM_SET, (anim as u32) as u16);
    x.set8(0x3f, 6);
    x.reinsert();
}

pub(super) fn ballot_timer<R: Bench>(x: &mut Cx<'_, R>) {
    match x.u8(f::STATE) {
        0 => {
            x.add8(f::STATE, 2);
            timer_palette(x, 0x28ff20);
            x.set8(0x2a, 1);
            x.set8(0x2b, 0xe);
            x.set32(Obj::Y, 0);
            timer_place(x);
            x.set16(0x54, 0x143c);
            timer_show(x);
        }
        2 => {
            let ballot = timer_place(x);
            if ballot.u16(x.b, 0x54) == 0 {
                return ballot_end(x);
            }
            let t = x.u8(0x55).wrapping_sub(1);
            x.set8(0x55, t);
            if (t as i8) >= 0 {
                return;
            }
            x.set8(0x55, 0x3c);
            let s = x.u8(0x54).wrapping_sub(1);
            x.set8(0x54, s);
            if s != 0 {
                return timer_show(x);
            }
            // time elapsed: the ballot ends
            ballot.set_u16(x.b, 0x54, 0);
            timer_palette(x, 0x2531c0);
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


use super::*;
use crate::actors::families::set::tables as set_tables;
use crate::world::objects::Obj;
use crate::actors::player::Player;
use crate::actors::families::set::fields::NameCursorObj;

pub fn run<R: Bench>(x: &mut Cx<'_, R>) {
    match x.u8(Obj::STATE) {
        0 => init(x),
        2 => step(x),
        4 => {}
        _ => finish_and_free(x),
    }
}

fn owner<R: Bench>(x: &mut Cx<'_, R>) -> Obj {
    at(x, OWNER)
}

/// 0xc9a74: state 0. A player who already has a name (non-empty buffer) is marked finished; otherwise
/// the cursor goes to its player's starting cell.
fn init<R: Bench>(x: &mut Cx<'_, R>) {
    let player = owner(x);
    x.set8(ACTIVE, 0);
    x.set8(DONE, 0);
    x.set8(HELD_FRAMES, 0);
    if player.u8(x.b, Player::ACTIVE) == 0 || (player.u8(x.b, Player::MAIN) as i8) >= 4 {
        return;
    }
    if x.a5_u8(GAME_MODE) != 1 && !(player.u32(x.b, BUFFER) == u32::MAX && player.u32(x.b, BUFFER + 4) == u32::MAX) {
        x.set8(DONE, 1);
        x.set8(Obj::STATE, 4);
        return;
    }
    let wide = x.a5_u16(WIDE_FLAG) != 0;
    x.set8(WIDE, wide as u8);
    let index = ((x.u8(PLAYER) << 2) as u16).wrapping_add((wide as u16) << 4) as u32;
    let (col, row) = (set_tables::pc_word(x.b, START_CELLS + index), set_tables::pc_word(x.b, START_CELLS + 2 + index));
    x.set16(COL, col);
    x.set16(ROW, row);
    x.add8(Obj::STATE, 2);
    x.set8(ACTIVE, 1);
    x.set8(LENGTH, 0);
    x.set8(VOICED_PENDING, 0);
    x.set8(NO_NAME, 0);
    x.set8(REPEAT, 0x1e);
    step(x);
}

/// 0xc9b9e: state 2, one frame of entry.
fn step<R: Bench>(x: &mut Cx<'_, R>) {
    x.set8(TYPED, 0);
    let (c, r) = (x.u16(COL), x.u16(ROW));
    x.set16(PREV_X, c);
    x.set16(PREV_Y, r);
    handle_input(x);
    x.set8(COUNT_ACTIVE, 1);
    if x.u16(COL) == x.u16(PREV_X) && x.u16(ROW) == x.u16(PREV_Y) {
        x.set8(COUNT_ACTIVE, 0);
    }
    let player = x.u8(PLAYER) as u32;
    let (held, _) = inputs(x, player);
    if held & 0x80 != 0 {
        x.add8(HELD_FRAMES, 1);
    }
}

/// A move: the grid repeats the direction every 6 frames, 30 the first time.
fn handle_input<R: Bench>(x: &mut Cx<'_, R>) {
    let player = x.u8(PLAYER) as u32;
    let (held, pressed) = inputs(x, player);
    let mut repeat = 0x1e;
    for dir in 0..4u32 {
        if held >> dir & 1 != 0 {
            if pressed >> dir & 1 != 0 {
                return step_cursor(x, dir, repeat);
            }
            repeat = 6;
            let t = x.u8(REPEAT).wrapping_sub(1);
            x.set8(REPEAT, t);
            if t == 0 {
                return step_cursor(x, dir, repeat);
            }
            break;
        }
    }
    if pressed & 0x10 != 0 {
        return confirm_button(x);
    }
    if pressed & 0x20 != 0 {
        return erase_button(x);
    }
    // start button: validated when all the other buttons are released
    let released = !x.a5_u8(BUTTONS_OLD) & x.a5_u8(BUTTONS_MASK);
    if released >> (x.u8(PLAYER) as u32 & 31) & 1 == 0 {
        return;
    }
    if x.u8(LENGTH) == 0 {
        load_default_name(x);
    }
    finish(x);
}

/// 0xc9ef8: moves the cursor one cell in direction `dir`.
fn step_cursor<R: Bench>(x: &mut Cx<'_, R>, dir: u32, repeat: u8) {
    x.set8(REPEAT, repeat);
    let (col, row) = (x.u16(COL), x.u16(ROW));
    let slot = (dir * 2 + if wide(x) { 8 } else { 0 }) as u16;
    move_cursor(x, slot, col, row);
}

/// Movement routines (table 0xc9f26): `slot` = direction × 2 (+ 8 in the wide grid).
fn move_cursor<R: Bench>(x: &mut Cx<'_, R>, slot: u16, col: u16, row: u16) {
    let width = |x: &mut Cx<'_, R>, table: u32, row: u16| set_tables::word(x.b, table.wrapping_add(row.wrapping_mul(2) as i16 as i32 as u32));
    match slot {
        // to the right, narrow grid
        0 => {
            x.add16(COL, 1);
            let w = width(x, ROW_WIDTH_NARROW, row);
            if w < x.u16(COL) {
                x.set16(COL, 0);
                x.add16(ROW, 1);
                if x.u16(ROW) > 5 {
                    x.set16(ROW, 0);
                }
            }
        }
        // to the left, narrow grid
        2 => {
            x.add16(COL, 0xffff);
            if (x.u16(COL) as i16) < 0 {
                x.add16(ROW, 0xffff);
                if (x.u16(ROW) as i16) < 0 {
                    x.set16(ROW, 5);
                }
                let r = x.u16(ROW);
                let w = width(x, ROW_WIDTH_NARROW, r);
                x.set16(COL, w);
            }
        }
        // down, narrow grid
        4 => {
            if col as u8 == 0xa && row as u8 == 4 {
                x.set16(COL, 9);
            }
            x.add16(ROW, 1);
            if x.u16(ROW) > 5 {
                x.set16(ROW, 0);
            }
        }
        // up, narrow grid
        6 => {
            if col as u8 == 0xa && row as u8 == 1 {
                x.set16(COL, 9);
            }
            x.add16(ROW, 0xffff);
            if (x.u16(ROW) as i16) < 0 {
                x.set16(ROW, 5);
            }
        }
        // to the right, wide grid
        8 => {
            x.add16(COL, 1);
            let w = width(x, ROW_WIDTH_WIDE, row);
            if w < x.u16(COL) {
                x.set16(COL, 0);
                x.add16(ROW, 1);
                if x.u16(ROW) > 4 {
                    x.set16(ROW, 0);
                }
            }
        }
        // to the left, wide grid
        0xa => {
            x.add16(COL, 0xffff);
            if (x.u16(COL) as i16) < 0 {
                x.add16(ROW, 0xffff);
                if (x.u16(ROW) as i16) < 0 {
                    x.set16(ROW, 4);
                }
                let r = x.u16(ROW);
                let w = width(x, ROW_WIDTH_WIDE, r);
                x.set16(COL, w);
            }
        }
        // down, wide grid
        0xc => {
            x.add16(ROW, 1);
            if (x.u16(ROW) as i16) > 4 {
                x.set16(ROW, 0);
            }
            clamp_wide(x);
        }
        // up, wide grid
        _ => {
            x.add16(ROW, 0xffff);
            if (x.u16(ROW) as i16) < 0 {
                x.set16(ROW, 4);
            }
            clamp_wide(x);
        }
    }
    redraw_cell(x, col, row);
}

/// Brings the column back within the width of the current row (wide grid).
fn clamp_wide<R: Bench>(x: &mut Cx<'_, R>) {
    let r = x.u16(ROW);
    let w = set_tables::word(x.b, ROW_WIDTH_WIDE.wrapping_add(r.wrapping_mul(2) as i16 as i32 as u32));
    if w < x.u16(COL) {
        x.set16(COL, w);
    }
}

/// 0xc9c1a: button 1 — chooses the current cell: command (special column) or character.
fn confirm_button<R: Bench>(x: &mut Cx<'_, R>) {
    let special = if !wide(x) {
        x.u16(COL) == 0xa
    } else {
        x.u16(ROW) == 4 && (x.u16(COL) as i16) >= 3
    };
    if special {
        return special_command(x);
    }
    let ch = char_under_cursor(x);
    let buffer = owner(x).0 + BUFFER;
    let len = x.u8(LENGTH);
    enter_char(x, ch, buffer, len);
}

/// 0xcaaae: character of the current cell.
fn char_under_cursor<R: Bench>(x: &mut Cx<'_, R>) -> u8 {
    let index = x.u16(COL).wrapping_add(x.u16(ROW) << 4);
    let table = if wide(x) { GRID_WIDE } else { GRID_NARROW };
    set_tables::pc_byte(x.b, table.wrapping_add(index as i16 as i32 as u32))
}

/// 0xc9cd8: command of the special column according to the row: 1 confirm (0x81), 2 default
/// name, 3 erase, 4 finish.
fn special_command<R: Bench>(x: &mut Cx<'_, R>) {
    let n = if !wide(x) { x.u16(ROW) } else { x.u16(COL).wrapping_sub(2) };
    match n {
        1 => {
            let buffer = owner(x).0 + BUFFER;
            let len = x.u8(LENGTH);
            enter_char(x, ENTER, buffer, len);
        }
        2 => load_default_name(x),
        3 => erase_button(x),
        4 => finish(x),
        _ => {}
    }
}

/// 0xcab86: adds character `ch`; 0x9f and 0x9d transform the previous letter.
fn enter_char<R: Bench>(x: &mut Cx<'_, R>, mut ch: u8, buffer: u32, len: u8) {
    if x.u8(NO_NAME) != 0 {
        return;
    }
    if ch == 0x9f || ch == 0x9d {
        let table = if ch == 0x9f { VOICED } else { SEMI_VOICED };
        if len == 0 {
            return;
        }
        let previous = rd8(x.b, buffer + len as u32 - 1);
        if previous == 0x6f {
            return;
        }
        let converted = set_tables::byte(x.b, table + previous as u32);
        if converted == 0 {
            return;
        }
        ch = converted;
        x.add8(LENGTH, 0xff);
        x.set8(VOICED_PENDING, 0);
    }
    if x.u8(VOICED_PENDING) != 0 {
        return;
    }
    store_char(x, ch, buffer);
}

/// 0xc9c90: writes the character into the buffer; buffer full → cursor on "confirm".
fn store_char<R: Bench>(x: &mut Cx<'_, R>, ch: u8, buffer: u32) {
    let len = x.u8(LENGTH) as u32;
    wr8(x.b, buffer + len, ch);
    x.set8(TYPED, 1);
    x.add8(LENGTH, 1);
    if x.u8(LENGTH) != 6 {
        return;
    }
    let (col, row) = (x.u16(COL), x.u16(ROW));
    x.set8(VOICED_PENDING, 1);
    x.set16(ROW, 4);
    let target = if wide(x) { 6 } else { 0xa };
    x.set16(COL, target);
    redraw_cell(x, col, row);
}

/// 0xc9e6a: erases the last character (or the whole buffer after a default name).
fn erase_button<R: Bench>(x: &mut Cx<'_, R>) {
    if x.u8(LENGTH) == 0 {
        return;
    }
    let player = owner(x);
    if x.u8(NO_NAME) == 0 {
        x.set8(VOICED_PENDING, 0);
        x.add8(LENGTH, 0xff);
        let len = x.u8(LENGTH) as u32;
        wr8(x.b, player.0 + BUFFER + len, EMPTY);
    } else {
        x.set8(VOICED_PENDING, 0);
        x.set8(NO_NAME, 0);
        player.set_u32(x.b, BUFFER, u32::MAX);
        player.set_u32(x.b, BUFFER + 4, u32::MAX);
        x.set8(LENGTH, 0);
    }
}

/// 0xc9d16: loads the player's default name into the buffer.
fn load_default_name<R: Bench>(x: &mut Cx<'_, R>) {
    x.set8(NO_NAME, 0);
    let player = owner(x);
    let mut offset = (player.u8(x.b, Player::CHARACTER) as u16) << 3;
    if wide(x) {
        offset += 0x60;
    }
    let mut length = 0u8;
    for i in 0..8u32 {
        let mut c = set_tables::byte(x.b, DEFAULT_NAMES + offset as u32 + i);
        wr8(x.b, player.0 + BUFFER + i, c);
        length = length.wrapping_add(1);
        if c == NAME_END {
            length = length.wrapping_sub(1);
            c = EMPTY;
            wr8(x.b, player.0 + BUFFER + i, c);
        }
    }
    x.set8(LENGTH, length);
    if length == 6 {
        x.set8(VOICED_PENDING, 1);
    }
    if player.u8(x.b, BUFFER) >= NAME_END {
        x.set8(NO_NAME, 1);
        x.set8(LENGTH, 1);
    }
    let (col, row) = (x.u16(COL), x.u16(ROW));
    redraw_cell(x, col, row);
    x.set16(ROW, 4);
    let target = if wide(x) { 6 } else { 0xa };
    x.set16(COL, target);
}

/// 0xc9eb0: finishes the entry (empty name → default name), checks the secret codes, computes
/// the check digit and writes the result into the player.
fn finish<R: Bench>(x: &mut Cx<'_, R>) {
    let n;
    if x.u8(LENGTH) == 0 {
        load_default_name(x);
        x.add16(ROW, 0xfffe);
        n = special_codes(x);
        scan_reset_codes(x, n);
    } else {
        let (col, row) = (x.u16(COL), x.u16(ROW));
        redraw_cell(x, col, row);
        n = special_codes(x);
        scan_reset_codes(x, n);
        check_repeated_name(x);
    }
    check_digit(x);
    commit_name(x);
}

/// 0xca33c: a buffer of identical letters or entirely "confirm" becomes the default
/// name again.
fn check_repeated_name<R: Bench>(x: &mut Cx<'_, R>) {
    let player = owner(x);
    let b = |x: &mut Cx<'_, R>, i: u32| player.u8(x.b, BUFFER + i);
    let first = b(x, 0);
    let mut same = true;
    for i in 0..5 {
        if b(x, i) != first {
            same = false;
            break;
        }
    }
    if same {
        return load_default_name(x);
    }
    for i in 0..6 {
        let c = b(x, i);
        if c == EMPTY {
            return load_default_name(x);
        }
        if c != ENTER {
            return;
        }
    }
    load_default_name(x);
}

/// 0xca374: looks in the buffer for a sequence from the reset-code table.
/// `n`: N flag left by 0xca524 (the scan only takes place if it is raised).
fn scan_reset_codes<R: Bench>(x: &mut Cx<'_, R>, n: bool) {
    if !n {
        return;
    }
    let buffer = owner(x).0 + BUFFER;
    if find_code(x, buffer, RESET_CODES, 0).is_some() {
        load_default_name(x);
    }
}

/// Looks in the buffer's 6 bytes for a sequence from the table (length byte, characters,
/// then `trailer` action bytes), like routines 0xca384 and 0xca54c: the first letter
/// is searched further along in the buffer; a mismatch re-reads the faulty letter as the start of the
/// sequence without rewinding the counter. Returns the address following the characters of the sequence found.
fn find_code<R: Bench>(x: &mut Cx<'_, R>, buffer: u32, table: u32, trailer: u32) -> Option<u32> {
    enum Phase {
        Start,
        ReadLength,
        Search,
        Continue,
        NextPattern,
    }
    let (mut step, mut a4, mut a3, mut a1) = (Phase::Start, table, table, buffer);
    let (mut left, mut remaining, mut first) = (0u16, 5i32, 0u8);
    loop {
        match step {
            Phase::Start => {
                a1 = buffer;
                remaining = 5;
                a3 = a4;
                step = Phase::ReadLength;
            }
            Phase::ReadLength => {
                let length = set_tables::byte(x.b, a4);
                a4 += 1;
                if (length as i8) < 0 {
                    return None;
                }
                left = length as u16;
                first = set_tables::byte(x.b, a4);
                a4 += 1;
                step = Phase::Search;
            }
            Phase::Search => {
                let c = rd8(x.b, a1);
                a1 += 1;
                if c == first {
                    step = Phase::Continue;
                } else {
                    remaining -= 1;
                    if remaining < 0 {
                        step = Phase::NextPattern;
                    }
                }
            }
            Phase::Continue => {
                left = left.wrapping_sub(1);
                if left == 0 {
                    return Some(a4);
                }
                remaining -= 1;
                if remaining < 0 {
                    step = Phase::NextPattern;
                    continue;
                }
                let want = set_tables::byte(x.b, a4);
                a4 += 1;
                let c = rd8(x.b, a1);
                a1 += 1;
                if want != c {
                    a1 -= 1;
                    a4 = a3;
                    step = Phase::ReadLength;
                }
            }
            Phase::NextPattern => {
                a4 = a3;
                let length = set_tables::byte(x.b, a4) as u32;
                a4 += 1 + length + trailer;
                step = Phase::Start;
            }
        }
    }
}

/// 0xca524: if the player holds the three buttons (0x70) for more than a second, looks for a
/// special code and replaces the buffer. Returns the N flag left by the routine.
fn special_codes<R: Bench>(x: &mut Cx<'_, R>) -> bool {
    let player = x.u8(PLAYER) as u32;
    let (_, pressed) = inputs(x, player);
    let n_of = |a: u8, b: u8| (a.wrapping_sub(b) as i8) < 0;
    if pressed != 0x70 {
        return n_of(pressed, 0x70);
    }
    let held_frames = x.u8(HELD_FRAMES);
    if held_frames <= 0x3c {
        return n_of(held_frames, 0x3c);
    }
    let buffer = owner(x).0 + BUFFER;
    match find_code(x, buffer, SPECIAL_CODES, 1) {
        None => true,
        Some(after) => {
            let action = set_tables::byte(x.b, after) as u32;
            let src = SPECIAL_REPLACEMENTS + (action << 3);
            let (a, b) = (set_tables::long(x.b, src), set_tables::long(x.b, src + 4));
            wr32(x.b, buffer, a);
            wr32(x.b, buffer + 4, b);
            (b as i32) < 0
        }
    }
}

/// 0xca5fe: BCD check digit of the name (`+0x2a..+0x2d`, result in `+0x2b`).
fn check_digit<R: Bench>(x: &mut Cx<'_, R>) {
    let buffer = owner(x).0 + BUFFER;
    let first = rd8(x.b, buffer) as i8 as i16;
    let d0 = (-first).wrapping_sub(2) as u16;
    if d0 < 0x10 {
        let v = set_tables::pc_byte(x.b, DEFAULT_CHECK + (d0 & 0xff) as u32);
        x.set8(NameCursorObj::CHECK, v);
        return;
    }
    for i in 0..6 {
        let c = rd8(x.b, buffer + i);
        if c == 0xff {
            continue;
        }
        x.set8(NameCursorObj::ADDEND_HIGH, 0);
        let digit = set_tables::byte(x.b, CHAR_DIGITS + c as u32);
        x.set8(NameCursorObj::ADDEND, digit);
        add_bcd(x);
    }
    while x.u8(NameCursorObj::CARRY) != 0 {
        let carry_in = x.u8(NameCursorObj::CARRY);
        x.set8(NameCursorObj::ADDEND, carry_in);
        x.set8(NameCursorObj::ADDEND_HIGH, 0);
        x.set8(NameCursorObj::CARRY, 0);
        add_bcd(x);
    }
    while x.u8(NameCursorObj::CHECK) >= 0x10 {
        let high = x.u8(NameCursorObj::CHECK) >> 4;
        let low = x.u8(NameCursorObj::CHECK) & 0xf;
        x.set8(NameCursorObj::CHECK, low);
        x.set8(NameCursorObj::ADDEND, high);
        let (v, _) = abcd(x.u8(NameCursorObj::ADDEND), x.u8(NameCursorObj::CHECK), false);
        x.set8(NameCursorObj::CHECK, v);
    }
}

/// Adds the BCD value (`+0x2c:+0x2d`) to (`+0x2a:+0x2b`) with carry (two ABCD).
fn add_bcd<R: Bench>(x: &mut Cx<'_, R>) {
    let (low, carry) = abcd(x.u8(NameCursorObj::ADDEND), x.u8(NameCursorObj::CHECK), false);
    x.set8(NameCursorObj::CHECK, low);
    let (high, _) = abcd(x.u8(NameCursorObj::ADDEND_HIGH), x.u8(NameCursorObj::CARRY), carry);
    x.set8(NameCursorObj::CARRY, high);
}

/// ABCD: `dst + src + carry` in BCD; returns (result, carry).
fn abcd(src: u8, dst: u8, carry: bool) -> (u8, bool) {
    let mut r = (src & 0xf) as u16 + (dst & 0xf) as u16 + carry as u16;
    if r > 9 {
        r += 6;
    }
    r += (src & 0xf0) as u16 + (dst & 0xf0) as u16;
    let carry = r > 0x99;
    if carry {
        r -= 0xa0;
    }
    (r as u8, carry)
}

/// 0xca0f4: writes the result: end indicator on screen, player record
/// (validated name, check digit), the cursor goes to state 4.
fn commit_name<R: Bench>(x: &mut Cx<'_, R>) {
    let player = owner(x);
    let kind = player.u8(x.b, Player::INDEX) as u16;
    let mut index = kind.wrapping_mul(2);
    if x.a5_u8(BLINK_FLAGS) & 1 != 0 {
        index = index.wrapping_add(8);
    }
    let offset = set_tables::pc_word(x.b, FINISH_POSITIONS.wrapping_add(index as i16 as i32 as u32));
    let mut a1 = TILEMAP.wrapping_add(offset as i16 as i32 as u32);
    let mut tile: u32 = 0x6700013;
    tile = tile & !0xff | (tile as u8).wrapping_add(kind as u8) as u32;
    for _ in 0..4 {
        wr32(x.b, a1, tile);
        a1 += 0x80;
        tile = tile.wrapping_add(0x10000);
    }
    let swapped = tile.rotate_left(16);
    let swapped = swapped & !0xff | (swapped as u8).wrapping_add(x.u8(NameCursorObj::CHECK)) as u32;
    wr32(x.b, a1, swapped.rotate_left(16));
    x.set32(Obj::STATE, 0x4000000);
    x.set8(DONE, 1);
    x.set8(ACTIVE, 0);
    let check = (x.u8(NameCursorObj::CHECK) << 2) as u32;
    let record = {
        let at = FINISH_RECORDS + check;
        set_tables::pc_long(x.b, at)
    };
    let variant = (player.u8(x.b, Player::CHARACTER) >> 1) as u32;
    let a1 = record + variant * 5;
    let (i0, v0, i1, v1, flag) = (set_tables::byte(x.b, a1) as u32, set_tables::byte(x.b, a1 + 1), set_tables::byte(x.b, a1 + 2) as u32, set_tables::byte(x.b, a1 + 3), set_tables::byte(x.b, a1 + 4) as u32);
    wr8(x.b, player.0 + 0x160 + i0, v0);
    if (i1 as u8 as i8) >= 0 {
        wr8(x.b, player.0 + 0x16b + i1, v1);
    }
    wr8(x.b, FLAGS_ARRAY + flag, 1);
}

/// 0xca2f8: state 6. If the player is still there and the entry is not finished, finishes it automatically;
/// then release.
fn finish_and_free<R: Bench>(x: &mut Cx<'_, R>) {
    let player = owner(x);
    if player.u8(x.b, Player::ACTIVE) != 0 && (player.u8(x.b, Player::MAIN) as i8) < 4 && x.u8(DONE) == 0 {
        x.set8(DONE, 1);
        finish(x);
    }
    x.free(&pools::SET);
}

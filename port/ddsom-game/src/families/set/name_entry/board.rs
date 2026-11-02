use super::*;

/// 0xc979e: state 0. Resets the records to zero and loads four palettes (with the
/// brightness of the page's first color).
pub fn init<R: Bench>(x: &mut Cx<'_, R>) {
    x.add8(0x4, 2);
    for off in [0x30, 0x34, 0x38, 0x3c] {
        x.set32(off, 0);
    }
    x.set8(COUNT_DONE, 0);
    x.set8(PAIRS, 0);
    let wide = x.a5_u16(WIDE_FLAG) != 0;
    x.set8(WIDE, wide as u8);
    let keep = rd32(x.b, 0x91c000) & KEEP_BRIGHTNESS;
    for i in 0..4u32 {
        for j in (0..PALETTE_ENTRY).step_by(4) {
            let v = x.data32(PALETTE_SOURCE + i * 0x100 + j);
            wr32(x.b, PALETTE_BANK + i * PALETTE_ENTRY + j, v | keep);
        }
    }
}

/// States 4 and 6: releases the cursors then the board.
pub fn release<R: Bench>(x: &mut Cx<'_, R>) {
    for i in 0..4 {
        let slave = at(x, SLAVES + 2 * i);
        slave.set_u32(x.b, 4, FREE_ON_NEXT);
    }
    x.free(&pools::SET);
}

/// 0xc9810: one frame. Counts the cursors, spots the shared cells, animates and draws.
pub fn update<R: Bench>(x: &mut Cx<'_, R>) {
    count_cursors(x);
    count_finished(x);
    // records of the previous frame (shared cells to ignore according to the count)
    for (dst, src) in [(0x40, 0x30), (0x48, 0x38), (0x44, 0x34), (0x4c, 0x3c)] {
        let v = x.u32(src);
        x.set32(dst, v);
    }
    if x.u8(PAIRS) != 2 {
        if x.u8(PAIRS) == 0 {
            x.set32(0x40, u32::MAX);
            x.set32(0x44, u32::MAX);
        }
        x.set32(0x48, u32::MAX);
        x.set32(0x4c, u32::MAX);
    }
    for off in [0x30, 0x34, 0x38, 0x3c] {
        x.set32(off, 0);
    }
    let pairs = find_shared_cells(x);
    x.set8(PAIRS, pairs);
    animate_records(x);
    draw_cursors(x);
    for i in 0..pairs as u32 {
        let rec = x.o.0 + RECORDS + 8 * i;
        let (col, row) = (rd16(x.b, rec + REC_COL), rd16(x.b, rec + REC_ROW));
        let (a1, size) = cell_region(x, col, row);
        let mut tile: u16 = 0x18;
        if shared_in_special(x, col, row) {
            tile = 0;
        }
        tile = (tile & 0xff00) | (tile as u8).wrapping_add(rd8(x.b, rec + REC_DIR)) as u16;
        fill_tiles(x, a1, size, tile);
    }
}

/// Cells where at least two cursors overlap: records (column, row, player
/// mask) for the first two, row-by-row scan. Returns their number.
fn find_shared_cells<R: Bench>(x: &mut Cx<'_, R>) -> u8 {
    let mut found = 0u8;
    let (mut col, mut row) = (0u16, 0u16);
    loop {
        let rec = x.o.0 + RECORDS + 8 * found as u32;
        let mut hits = 0u16;
        for i in 0..4 {
            let slave = at(x, SLAVES + 2 * i);
            if slave.u8(x.b, ACTIVE) != 0 && slave.u16(x.b, COL) == col && slave.u16(x.b, ROW) == row {
                wr16(x.b, rec + REC_COL, col);
                wr16(x.b, rec + REC_ROW, row);
                let player = slave.u8(x.b, PLAYER) as u32;
                let mask = rd8(x.b, rec + REC_MASK) | 1u8.wrapping_shl(player & 7);
                wr8(x.b, rec + REC_MASK, mask);
                hits += 1;
            }
        }
        let last = row == GRID_ROWS - 1 && col == GRID_COLS - 1;
        if !last {
            let mask = rd8(x.b, rec + REC_MASK);
            wr8(x.b, rec + REC_MASK, 0);
            if hits >= 2 {
                found += 1;
                wr8(x.b, rec + REC_MASK, mask);
            }
            col += 1;
            if col >= GRID_COLS {
                row += 1;
                col = 0;
            }
            if found == 2 {
                break;
            }
            continue;
        }
        break;
    }
    found
}

/// 0xc9912 and following: for each shared cell, maintains the "featured" player
/// (`REC_DIR`) and their display time (`REC_TIMER`) taking the previous frame into account.
fn animate_records<R: Bench>(x: &mut Cx<'_, R>) {
    for k in 0..2u32 {
        let rec = x.o.0 + RECORDS + 8 * k;
        if rd8(x.b, rec + REC_MASK) == 0 {
            return;
        }
        let (col, row) = (rd16(x.b, rec + REC_COL), rd16(x.b, rec + REC_ROW));
        let mask = rd8(x.b, rec + REC_MASK);
        let previous = (0..2u32).map(|p| x.o.0 + PREV_RECORDS + 8 * p).find(|&p| rd16(x.b, p + REC_COL) == col && rd16(x.b, p + REC_ROW) == row);
        if let Some(old) = previous {
            let dir = rd8(x.b, old + REC_DIR);
            wr8(x.b, rec + REC_DIR, dir);
            let timer = rd8(x.b, old + REC_TIMER);
            wr8(x.b, rec + REC_TIMER, timer);
            let old_mask = rd8(x.b, old + REC_MASK);
            if old_mask != mask {
                let new_bits = !old_mask & mask;
                if new_bits != 0 {
                    // a new player arrives: they become featured
                    let first = new_bits.trailing_zeros() as u8;
                    wr8(x.b, rec + REC_DIR, first);
                    wr8(x.b, rec + REC_TIMER, 0xd);
                } else if mask >> (dir & 7) & 1 != 0 {
                    // the featured player is still there
                } else {
                    next_player(x, rec, mask);
                    continue;
                }
            }
            let timer = rd8(x.b, rec + REC_TIMER).wrapping_sub(1);
            wr8(x.b, rec + REC_TIMER, timer);
            if timer == 0 {
                next_player(x, rec, mask);
            }
        } else {
            // new cell: players who already have a name entered are preferred
            let mut with_name = 0u8;
            for bit in (0..4u32).rev() {
                let slave = at(x, SLAVES + 2 * bit);
                if mask >> bit & 1 != 0 && slave.u8(x.b, COUNT_ACTIVE) != 0 {
                    with_name |= 1 << bit;
                }
            }
            let choose = if with_name != 0 { with_name } else { mask };
            wr8(x.b, rec + REC_TIMER, 0xc);
            let mut d = 0u8;
            while choose >> d & 1 == 0 {
                d = (d + 1) & 3;
            }
            wr8(x.b, rec + REC_DIR, d);
        }
    }
}

/// 0xc998a: next player in the mask, in turn (display time 12 frames).
fn next_player<R: Bench>(x: &mut Cx<'_, R>, rec: u32, mask: u8) {
    let mut d = rd8(x.b, rec + REC_DIR);
    loop {
        d = d.wrapping_add(1) & 3;
        if mask >> d & 1 != 0 {
            break;
        }
    }
    wr8(x.b, rec + REC_DIR, d);
    wr8(x.b, rec + REC_TIMER, 0xc);
}

/// 0xca7ea: is the shared cell in the special column? (base tile 0 otherwise 0x18).
fn shared_in_special<R: Bench>(x: &mut Cx<'_, R>, col: u16, row: u16) -> bool {
    if !wide(x) {
        col == 0xa
    } else {
        row == 4 && (col as i16) >= 3
    }
}

/// 0xca6b0: number of active cursors (`+0x17`) and of those that typed a letter (`+0x14`).
fn count_cursors<R: Bench>(x: &mut Cx<'_, R>) {
    let (mut active, mut typed) = (0u8, 0u8);
    for i in 0..4 {
        let slave = at(x, SLAVES + 2 * i);
        let player = Obj(slave.u16(x.b, OWNER) as i16 as i32 as u32 & 0xff_ffff);
        if player.u8(x.b, 0) != 0 && (player.u8(x.b, 4) as i8) < 4 {
            active += 1;
            if slave.u8(x.b, TYPED) != 0 {
                typed += 1;
            }
        }
    }
    x.set8(COUNT_ACTIVE, active);
    x.set8(COUNT_TYPED, typed);
}

/// 0xca6ea: number of cursors that have finished their entry (`+0x16`).
fn count_finished<R: Bench>(x: &mut Cx<'_, R>) {
    let n = (0..4).filter(|&i| at(x, SLAVES + 2 * i).u8(x.b, DONE) != 0).count();
    x.set8(COUNT_DONE, n as u8);
}

/// 0xca78c: draws the cursors (their cell, special tint in the commands column).
fn draw_cursors<R: Bench>(x: &mut Cx<'_, R>) {
    for i in 0..4 {
        let slave = at(x, SLAVES + 2 * i);
        if slave.u8(x.b, ACTIVE) == 0 {
            continue;
        }
        let (col, row) = (slave.u16(x.b, COL), slave.u16(x.b, ROW));
        let (a1, size) = cell_region(x, col, row);
        let base: u8 = if !wide(x) {
            if col == 0xa { 0 } else { 0x18 }
        } else if row == 4 && (col as i16) >= 3 {
            0
        } else {
            0x18
        };
        let tile = base.wrapping_add(slave.u8(x.b, PLAYER)) as u16;
        fill_tiles(x, a1, size, tile);
    }
}

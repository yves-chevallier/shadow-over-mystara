//! Pop-ups (U1): state machine that takes the floating texts out of their queue, one at a time, and draws them.
//!
//! `A5-0x6e80`: current pop-up (4 words: identifier, two parameters, remaining duration in frames).
//! The queue (`A5+0x20e`, 8-byte entries, read at `A5+0x20d`) is filled by `create_popup`
//! (0x19dce, see `events.rs`). At each frame (0x19e20):
//!
//! * no current pop-up (zero duration): the next one, if any, becomes current (its slot is
//!   freed), its drawing is started (0x19f24); without a second parameter its duration is 0x60 frames;
//! * current pop-up: its duration decreases; at zero it is erased (0x19ee0) and then replaced by the next one;
//!   if another one is waiting and the remaining duration is at least 4 frames, it is brought down to 4.
//!
//! ## Drawing
//!
//! The identifier (word) gives in its high byte the location of the bubble on the text layer
//! (bit 1: shift of one cell) and in its low byte the text number. A bubble is a frame
//! of 8x8 tiles (width and height in pixels: 2nd and 3rd words) with the text at the top; in a two-player
//! game (`A5+0x20` bit 0) the bubble is halved.

use cps2::Cps2Bus;

use crate::mem::{prog16, rd16, rd8, wr16, wr32, wr8, A5};
use crate::text::clear_cells;

const EMPTY: u16 = 0xffff;
/// Current pop-up (4 words); the remaining duration is the last one.
pub(crate) const CURRENT: u32 = 0xffff_9180;
const DURATION: u32 = 6;
pub(crate) const INPUTS: u32 = 0x20e;
const READ: u32 = 0x20d;
const ENTRY_SIZE: u8 = 8;
/// Default duration of a pop-up without a second parameter (frames), and shortened duration.
const DEFAULT_DURATION: u16 = 0x60;
const SHORT_DURATION: u16 = 4;
/// Bit 0: reduced bubbles.
const REDUCED: u32 = 0x20;

/// Program ROM data.
mod rom {
    /// Rectangles to erase (column, row, width, height), one per slot: large, then reduced.
    pub const CLEARS: [u32; 2] = [0x19f02, 0x19f12];
    /// Origin of the bubble in the text layer, per slot (4 large, then 4 reduced): read
    /// as a PC-relative table, hence in the decrypted program.
    pub const ORIGINALS: u32 = 0x19f6c;
    /// Offsets of the texts (by number) to strings of tile indices terminated by `/`.
    pub const TEXTS: u32 = 0x1a01a;
    /// Tiles of the bottom edge of the frame (by height remainder from 1 to 8 pixels).
    pub const EDGES: u32 = 0x1a25e;
}

const TEXT_LAYER: u32 = 0x912000;
const COLUMN: u32 = 0x80;
/// End of a text string.
const TEXT_END: u8 = 0x2f;
/// Frame cells (tile, attribute 0xd): edge of the last column, full edge, and base of the corner cell.
const EDGE_CURRENT: u32 = 0x1318_000d;
const EDGE_FULL: u32 = 0x1310_000d;
const EDGE_COIN: u16 = 0x1310;
const ATTR_FRAME: u16 = 0xd;
const ATTR_TEXT: u16 = 2;

fn reduced(b: &mut Cps2Bus, a5: u32) -> bool {
    rd8(b, a5 + REDUCED) & 1 != 0
}

/// 0x19ee0: erases the bubble of pop-up `entry`.
fn clear(b: &mut Cps2Bus, a5: u32, entry: u32) {
    let table = rom::CLEARS[reduced(b, a5) as usize];
    let case = table + 4 * rd8(b, entry) as u32;
    let (col, row, width, height) = (rd8(b, case), rd8(b, case + 1), rd8(b, case + 2), rd8(b, case + 3));
    clear_cells(b, col, row, width, height);
}

/// 0x19f24: draws the bubble of pop-up `entry`.
fn draw(b: &mut Cps2Bus, a5: u32, entry: u32) {
    let id = rd16(b, entry);
    let mut width = rd16(b, entry + 2);
    let mut height = rd16(b, entry + 4);
    if (height as i16) < 0 {
        // parameter absent
        height = 0;
    }
    let mut slot = (id >> 8) << 1;
    if width as i16 > 0xa8 {
        width >>= 1;
        height >>= 1;
    }
    let small = reduced(b, a5);
    if small {
        slot += 8;
        width >>= 1;
        height >>= 1;
    }
    let original = prog16(b, rom::ORIGINALS + slot as u32) as i16 as i32 as u32;
    let a4 = TEXT_LAYER.wrapping_add(original);
    text(b, a4, id, small);
    frame(b, a4, id, small, width as i16, height as i16);
}

/// Column of the text and of the frame: shifted by one cell when bit 9 of the identifier is set.
fn shifted(id: u16, small: bool) -> bool {
    !small && id & 0x200 != 0
}

/// 0x19fd2: the text, one tile per cell, in a column.
fn text(b: &mut Cps2Bus, a4: u32, id: u16, small: bool) {
    let mut case = a4 + 0x100 + if shifted(id, small) { 4 } else { 0 };
    let start = prog16(b, rom::TEXTS + 2 * (id & 0xff) as u32) as i16 as i32 as u32;
    let mut s = rom::TEXTS.wrapping_add(start);
    loop {
        let tile = rd8(b, s);
        s += 1;
        if tile == TEXT_END {
            return;
        }
        wr16(b, case, tile as u16);
        wr16(b, case + 2, ATTR_TEXT);
        case += COLUMN;
    }
}

/// 0x1a20e: the frame under the text. `width` and `height` in pixels: whole edge columns, then
/// the remainder column (a tile chosen by the number of remaining pixels).
fn frame(b: &mut Cps2Bus, a4: u32, id: u16, small: bool, mut d1: i16, mut d2: i16) {
    let mut case = a4 + 0x104 - if shifted(id, small) { 4 } else { 0 };
    loop {
        d1 = d1.wrapping_sub(8);
        d2 = d2.wrapping_sub(8);
        if d2 < 0 {
            break;
        }
        wr32(b, case, EDGE_CURRENT);
        case += COLUMN;
    }
    loop {
        d2 = d2.wrapping_add(8);
        d1 = d1.wrapping_add(8);
        if d1 == 0 {
            return;
        }
        if d1 <= 8 {
            let edge = prog16(b, rom::EDGES.wrapping_add_signed(2 * d1 as i32));
            wr16(b, case, edge.wrapping_add(d2 as u16));
            wr16(b, case + 2, ATTR_FRAME);
            return;
        }
        wr16(b, case, EDGE_COIN.wrapping_add(d2 as u16));
        wr16(b, case + 2, ATTR_FRAME);
        case += COLUMN;
        d1 = d1.wrapping_sub(8);
        d2 = -8;
        loop {
            d1 = d1.wrapping_sub(8);
            if d1 < 0 {
                break;
            }
            wr32(b, case, EDGE_FULL);
            case += COLUMN;
        }
    }
}

/// Makes the next pop-up of the queue the current pop-up (if there is one) and starts its drawing. Returns the read
/// index read.
fn next(b: &mut Cps2Bus, current: u32) -> u8 {
    let index = rd8(b, A5 + READ);
    let case = A5 + INPUTS + index as u32;
    if rd16(b, case) == EMPTY {
        return index;
    }
    for offset in [0, 2, 4, 6] {
        let word = rd16(b, case + offset);
        wr16(b, current + offset, word);
        wr16(b, case + offset, EMPTY);
    }
    if rd16(b, current + 4) == 0 {
        wr16(b, current + DURATION, DEFAULT_DURATION);
    }
    draw(b, A5, current);
    wr8(b, A5 + READ, index.wrapping_add(ENTRY_SIZE));
    index
}

/// 0x19e20: one step of the pop-up state machine. Returns the queue read index if it was read (the D0.b of the
/// original). Core.
pub fn animate_obj(b: &mut Cps2Bus) -> Option<u8> {
    let current = A5.wrapping_add(CURRENT);
    let duration = rd16(b, current + DURATION);
    if duration != 0 {
        let duration = duration - 1;
        wr16(b, current + DURATION, duration);
        if duration != 0 {
            // the current pop-up continues; if another is waiting, it is cut short
            if duration < SHORT_DURATION {
                return None;
            }
            let index = rd8(b, A5 + READ);
            if rd16(b, A5 + INPUTS + index as u32) != EMPTY {
                wr16(b, current + DURATION, SHORT_DURATION);
            }
            return Some(index);
        }
        clear(b, A5, current);
    }
    Some(next(b, current))
}

/// 0x19eb0: erases the bubbles of all slots (4, or 3 reduced), from last to first. Returns the erase table
/// read (the A2 of the original). Core.
pub fn clear_all(b: &mut Cps2Bus) -> u32 {
    let small = reduced(b, A5);
    let table = rom::CLEARS[small as usize];
    let last = if small { 2 } else { 3 };
    for slot in (0..=last).rev() {
        let case = table + 4 * slot;
        let (col, row, width, height) = (rd8(b, case), rd8(b, case + 1), rd8(b, case + 2), rd8(b, case + 3));
        clear_cells(b, col, row, width, height);
    }
    table
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn bubble_offset() {
        assert!(shifted(0x0200, false));
        assert!(!shifted(0x0200, true));
        assert!(!shifted(0x0100, false));
    }
}

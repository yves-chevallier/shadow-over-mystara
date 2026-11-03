//! Small routines of the end-of-level screens: clears of the character layer, copies of palette
//! pages, resets of state blocks (W3).

use cps2::Cps2Bus;

use super::tiles::{BLANK_TILE, COLUMN};
use crate::hw::video::{Cell, Vram, SCRATCH, TEXT};
use crate::ram::Long;
use crate::Native;

/// Rectangle of empty cells: `columns` columns of `rows` cells starting at `at`.
pub fn blank_rect(b: &mut Cps2Bus, at: u32, columns: u32, rows: u32) {
    for col in 0..columns {
        for row in 0..rows {
            Cell(at + col * COLUMN + 4 * row).put(b, BLANK_TILE, 0);
        }
    }
}

/// 0xf683c: clears the text area of the end screens (48 columns of 22 cells from row 5).
/// The original leaves D1.w = -1, re-read by the bonus screen as a setting (written out in `bonus.rs`).
pub fn clear_text(b: &mut Cps2Bus) {
    const AT: u32 = TEXT.at(0x14).0;
    blank_rect(b, AT, 48, 22);
}

/// 0xf688c: clears the frame of the bonus screen (40 columns of 21 cells from row 6, column 5).
pub fn clear_bonus_frame(b: &mut Cps2Bus) {
    blank_rect(b, TEXT.at(0x298).0, 40, 21);
}

/// 0xf617e: copy of a palette page (0x400 bytes, in the graphics RAM).
pub fn copy_page(b: &mut Cps2Bus, from: u32, to: u32) {
    for i in (0..0x400).step_by(4) {
        let v = Vram(from + i).long(b);
        Vram(to + i).set_long(b, v);
    }
}

/// 0xfa240: zeroes 0x800 bytes of layer 0x920000.
pub fn clear_920000(b: &mut Cps2Bus) {
    for i in (0..0x800).step_by(4) {
        SCRATCH.offset(i).set_long(b, 0);
    }
}

/// Zeroes `n` longs starting at `at`.
pub fn clear_longs(b: &mut Cps2Bus, at: u32, n: u32) {
    for k in 0..n {
        Long(at + 4 * k).set(b, 0);
    }
}

/// State blocks reset by the initialisation routines.
pub mod blocks {
    use crate::mem::A5;
    /// Scene exit block (phase 16), 8 longs (0xf3130).
    pub const EXIT: u32 = A5 - 0x55ca;
    /// Its state (`+2`, step of 4) and the sub-state of the shared states (`+3`, step of 2).
    pub const EXIT_STEP: crate::ram::Byte = crate::ram::Byte(EXIT + 2);
    pub const EXIT_SUB_STEP: crate::ram::Byte = crate::ram::Byte(EXIT + 3);
    /// Shop block, 5 longs (0xf7916).
    pub const SHOP: u32 = A5 - 0x5560;
    /// Transition script variables (shared state 0xf7ecc), 4 longs (0xf7ee0).
    pub const TRANSITION: u32 = A5 - 0x5520;
    /// Ranking screen block, 4 longs (0xfc562).
    pub const RANKING: u32 = A5 - 0x54f0;
}

macro_rules! clear_native {
    ($name:ident, $entry:expr, $label:expr, $at:expr, $n:expr) => {
        pub const $name: Native = Native {
            entry: $entry,
            name: $label,
            run: crate::Body::Memory(|b| clear_longs(b, $at, $n)),
            touches_sound: false,
            outputs: 0,
        };
    };
}

/// 0xf3130: no more calls by address (scene entry calls `clear_longs`); A4 and D6 left by the original
/// loop are read by nobody (code-mort.md).
pub const RESET_EXIT: Native = Native {
    entry: 0xf3130,
    name: "scene_exit_reset",
    run: crate::Body::Memory(|b| clear_longs(b, blocks::EXIT, 8)),
    touches_sound: false,
    outputs: 0,
};
clear_native!(RESET_SHOP, 0xf7916, "scene_shop_reset", blocks::SHOP, 5);
pub const RESET_TRANSITION: Native = Native {
    entry: 0xf7ee0,
    name: "scene_transition_reset",
    run: crate::Body::Memory(|b| clear_longs(b, blocks::TRANSITION, 4)),
    touches_sound: false,
    outputs: 0,
};
clear_native!(RESET_RANKING, 0xfc562, "ranking_reset", blocks::RANKING, 4);

pub const CLEAR_TEXT: Native = Native {
    entry: 0xf683c,
    name: "scene_clear_text",
    run: crate::Body::Memory(clear_text),
    touches_sound: false,
    outputs: 0,
};

/// No more calls by address to these five natives: registers left by the original not reproduced.
pub const NATIVES: &[Native] = &[RESET_EXIT, RESET_SHOP, RESET_TRANSITION, RESET_RANKING, CLEAR_TEXT];

#[cfg(test)]
mod tests {
    use super::*;
    use crate::mem::A5;

    #[test]
    fn block_addresses() {
        assert_eq!(blocks::EXIT, 0xff2a36);
        assert_eq!(blocks::RANKING, 0xff2b10);
        assert_eq!(A5 - 0x5520, 0xff2ae0);
    }
}

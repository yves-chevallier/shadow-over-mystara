//! Credits screen (C1): credit and coin texts and digits displayed while
//! waiting for a start (`0x148be` credit text, `0x14b74` text of a player, `0x14c1e` erasure,
//! `0x14652` digits). The credits task (`credits_task.rs`) calls them with the first coin slot
//! (`A5+0x44e`, see `credits.rs`, A1 of the original); they never cede control.
//!
//! The texts are tables of ROM pointers, indexed by the number of credits or of coins
//! waiting: the index is computed as 4 bytes per step. The digits are written into the character
//! layer (GFX RAM 0x912000) by `credits::write_digit` (0x14e46).
//!
//! Variables: `A5+0x20` shared coin slot, `+0x21` number of coin slots minus one, `+0x22` separate
//! counters, `+0x1e` coins per credit, `+0x456` big-digit screen. In a coin slot: `+1` coins
//! waiting, `+2` credits, `+9` two-coin credit, `+0xb` last text displayed.

use cps2::Cps2Bus;

use crate::credits::write_digit;
use crate::mem::{prog16, rd32, rd8, set_low_byte, wr8, A5};
use crate::text;

/// ROM tables: text pointers by credit (one table per variant), and erased area.
mod table {
    pub const TEXT_ONE: u32 = 0x1ae10;
    pub const TEXT_ONE_EMPTY: u32 = 0x1ae80;
    pub const TEXT_TWO_COINS: u32 = 0x1aea4;
    pub const PLAYER_TEXT: [[u32; 2]; 3] = [[0x1aecc, 0x1af18], [0x1af24, 0x1af70], [0x1af7c, 0x1afc8]];
    pub const DIGIT_TEXT: [u32; 2] = [0x1ad98, 0x1ade8];
    pub const TALL_TEXT: u32 = 0x1adc0;
    /// Offsets (in 0x912000) of each player's digits.
    pub const DIGIT_OFFSETS: u32 = 0x14740;
    pub const DIGIT_OFFSETS_TWO_COINS: u32 = 0x14764;
    pub const TALL_OFFSETS: u32 = 0x1489a;
    pub const RECT: u32 = 0x14e54;
}

/// First coin slot (`A5+0x44e`).
pub const FIRST_COIN_SLOT: u32 = A5 + 0x44e;

const CHARACTERS: u32 = 0x912000;

fn v8(b: &mut Cps2Bus, off: u32) -> u8 {
    rd8(b, A5 + off)
}

/// Text pointer `table[index]` (A4 of the original).
fn text_at(b: &mut Cps2Bus, table: u32, index: u32) -> u32 {
    rd32(b, table + (index & 0xffff))
}

/// Writes a digit (attribute 1) through `0x14e46`; returns the cell that follows.
fn digit(b: &mut Cps2Bus, case: u32, value: u8) -> u32 {
    write_digit(b, case, value, 1)
}

/// Remainder of `value` modulo `by` by subtractions (`sub.b ; bgt`), like the original.
fn modulo(value: u8, by: u8) -> u8 {
    let mut v = value;
    loop {
        v = v.wrapping_sub(by);
        if (v as i8) <= 0 {
            break;
        }
    }
    v.wrapping_add(by)
}

// ---------------------------------------------------------------------------------------------
// 0x14c1e and 0x14b74

/// 0x14c1e: erases the credit text rectangle.
pub fn clear_credit_text(b: &mut Cps2Bus) {
    text::clear_box(b, table::RECT);
}

/// Index (in bytes) of the credit text of coin slot `m`: 4 bytes per whole credit, plus a fraction of coins
/// when the credit is at one coin.
fn credit_index(b: &mut Cps2Bus, m: u32) -> u32 {
    let (coins, credits, per_credit) = (rd8(b, m + 1), rd8(b, m + 2), v8(b, 0x1e));
    let mut d0: u32;
    if rd8(b, m + 9) != 0 {
        d0 = (credits as u32) << 2 & 0xffff;
        if d0 as u8 > 8 {
            set_low_byte(&mut d0, 8);
        }
    } else {
        d0 = 0;
        if credits == 0 {
            d0 = (per_credit as u32) << 2;
            if coins != 0 {
                let mut byte = 0x28u8;
                let mut left = per_credit.wrapping_sub(coins);
                loop {
                    byte = byte.wrapping_add(4);
                    left = left.wrapping_sub(1);
                    if left == 0 {
                        break;
                    }
                }
                set_low_byte(&mut d0, byte);
            }
        }
    }
    d0
}

/// `0x14b74`: credit text of coin slot `m` at column `col` (D5), row 0xd (D6).
fn count_text(b: &mut Cps2Bus, m: u32, col: u8) {
    let index = credit_index(b, m);
    let two_coins = (rd8(b, m + 9) != 0) as usize;
    let variant = match v8(b, 0x21) {
        3 => 2,
        2 => 1,
        _ => 0,
    };
    let t = text_at(b, table::PLAYER_TEXT[variant][two_coins], index);
    text::print_tall_at(b, t, col, 0xd);
}

// ---------------------------------------------------------------------------------------------
// 0x148be: credit text

/// `0x148be`: credit text, from the first coin slot `m`.
pub fn credit_text(b: &mut Cps2Bus, m: u32) {
    if v8(b, 0x22) != 0 {
        return credit_text_separate(b);
    }
    let two_coins = rd8(b, m + 9) != 0;
    if two_coins {
        // two-coin credit: the text follows the number of credits, capped according to the sharing
        let limit = match v8(b, 0x20) {
            0 => 4,
            1 => 6,
            _ => 8,
        };
        let d0 = (rd8(b, m + 2) as i8).min(limit);
        let index = (d0 as u8 as u32) << 2;
        if index as u8 == rd8(b, m + 0xb) {
            return;
        }
        wr8(b, m + 0xb, index as u8);
        clear_credit_text(b);
        let index = rd8(b, m + 0xb) as u32;
        let t = text_at(b, table::TEXT_TWO_COINS, index);
        text::print_tall(b, t);
        return;
    }
    let credits = rd8(b, m + 2);
    let shared = v8(b, 0x20);
    // 4 bytes per whole credit up to 3 (depending on the sharing), then the fraction of coins
    let mut d0 = 0u8;
    let mut fraction = true;
    if credits >= 2 {
        d0 = 0x24;
        if shared == 0 {
            fraction = false;
        } else if credits >= 3 {
            d0 = 0x48;
            if shared == 1 {
                fraction = false;
            } else if credits >= 4 {
                d0 = 0x6c;
                fraction = false;
            }
        }
    }
    if fraction {
        d0 = fraction_index(b, m, d0);
    }
    wr8(b, m + 0xb, d0);
    clear_credit_text(b);
    let table = if rd8(b, m + 2) != 0 { table::TEXT_ONE } else { table::TEXT_ONE_EMPTY };
    let index = rd8(b, m + 0xb) as u32;
    let t = text_at(b, table, index);
    text::print_tall(b, t);
}

/// Intermediate text (0x14912): `d0` plus 4 bytes per coin missing for the next credit.
fn fraction_index(b: &mut Cps2Bus, m: u32, d0: u8) -> u8 {
    let mut d1 = 0u8;
    let coins = rd8(b, m + 1);
    if coins != 0 {
        let mut left = v8(b, 0x1e).wrapping_sub(coins);
        loop {
            d1 = d1.wrapping_add(4);
            left = left.wrapping_sub(1);
            if left == 0 {
                break;
            }
        }
    }
    d0.wrapping_add(d1)
}

/// Separate counters (0x149aa): one text per player, at the columns of each configuration.
fn credit_text_separate(b: &mut Cps2Bus) {
    clear_credit_text(b);
    const P1: u32 = 0x44e;
    const P2: u32 = 0x45e;
    const P3: u32 = 0x46e;
    const P4: u32 = 0x47e;
    // (coin slot, column); the original also passes a row (D4: 0, 4, 8, 0xc) that the text does not read
    let layout: &[(u32, u8)] = if v8(b, 0x21) == 3 {
        &[(P1, 1), (P2, 0xd), (P3, 0x19), (P4, 0x25)]
    } else if v8(b, 0x21) == 2 {
        &[(P1, 1), (P2, 0x11), (P3, 0x21)]
    } else if v8(b, 0x20) == 1 {
        &[(P1, 1), (P1, 0x11), (P2, 0x21)]
    } else if v8(b, 0x20) == 2 {
        if v8(b, 0x21) == 1 {
            &[(P1, 0xa), (P2, 0x18)]
        } else {
            &[(P1, 1), (P1, 0xd), (P2, 0x19), (P2, 0x25)]
        }
    } else {
        &[(P1, 0xa), (P2, 0x18)]
    };
    for &(coin_slot, col) in layout {
        count_text(b, A5 + coin_slot, col);
    }
}

// ---------------------------------------------------------------------------------------------
// 0x14652: credit and coin digits

/// Variant of the player array: 0 for one coin slot, 2 for two, 5 for three and four.
fn rank(b: &mut Cps2Bus) -> u8 {
    match v8(b, 0x21) {
        1 => 0,
        2 => 2,
        _ => 5,
    }
}

/// Cell of the character layer at a signed offset (word) from its start.
fn case(offset: u32) -> u32 {
    CHARACTERS.wrapping_add(offset as u16 as i16 as i32 as u32)
}

/// `0x14652`: credit and coin digits, from the first coin slot `m`.
pub fn digits(b: &mut Cps2Bus, m: u32) {
    let tall = v8(b, 0x456) != 0;
    if v8(b, 0x21) == 0 || v8(b, 0x22) == 0 {
        return if tall { digits_tall_single(b) } else { digits_single(b, m) };
    }
    let mut a2 = A5 + 0x44e;
    for player in 0..=v8(b, 0x21) as u32 {
        let index = (rank(b) as u32 + player) << 2;
        if tall {
            let t = text_at(b, table::TALL_TEXT, index);
            text::print_over(b, t);
            let offset = rd32(b, table::TALL_OFFSETS + index);
            let credits = rd8(b, a2 + 2);
            let mut at = digit(b, case(offset), credits);
            at = at.wrapping_sub(0x7e);
            if v8(b, 0x21) != 3 {
                at = at.wrapping_add(0x17c);
            }
            let coins = modulo(rd8(b, a2 + 1), v8(b, 0x1e));
            at = digit(b, at, coins);
            at = at.wrapping_add(0xfe);
            let per = v8(b, 0x1e);
            digit(b, at, per);
        } else {
            let two_coins = rd8(b, m + 9) != 0;
            let t = text_at(b, table::DIGIT_TEXT[two_coins as usize], index);
            text::print(b, t);
            // the offset table is read PC-relative (decrypted program)
            let mut offset = prog16(b, table::DIGIT_OFFSETS + index + 2) as u32;
            if rd8(b, m + 9) != 0 {
                offset = rd32(b, table::DIGIT_OFFSETS_TWO_COINS + index);
            }
            let credits = rd8(b, a2 + 2);
            digit(b, case(offset), credits);
        }
        a2 += 0x10;
    }
}

/// A single array (0x14700): credits of one coin slot.
fn digits_single(b: &mut Cps2Bus, m: u32) {
    let two_coins = rd8(b, m + 9) != 0;
    let t = text_at(b, table::DIGIT_TEXT[two_coins as usize], 0x24);
    text::print(b, t);
    let at = if rd8(b, m + 9) != 0 { 0x912df4 } else { 0x912e74 };
    let credits = v8(b, 0x450);
    digit(b, at, credits);
}

/// Big digits, a single array (0x1484c).
fn digits_tall_single(b: &mut Cps2Bus) {
    let t = text_at(b, table::TALL_TEXT, 0x24);
    text::print_over(b, t);
    let credits = v8(b, 0x450);
    let mut at = digit(b, 0x912cf4, credits);
    at = at.wrapping_add(0xfe);
    let coins = modulo(v8(b, 0x44f), v8(b, 0x1e));
    at = digit(b, at, coins);
    at = at.wrapping_add(0xfe);
    let per = v8(b, 0x1e);
    digit(b, at, per);
}

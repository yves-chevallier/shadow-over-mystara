//! Coins and credits (C1): reading of the coin slots in VBLANK (0x143da), start buttons
//! (0x141e0) and locking of the coin counters (0x14290).
//!
//! Port `IN2` (0x804020) carries the coin slot lines (bits 4 to 7: coins 1 to 4); `IN2+1`
//! (0x804021) the service buttons. Up to four coin slots, each with a 16-byte record
//! at `A5+0x44e + 0x10 k`:
//!
//! | Byte | Role |
//! |---|---|
//! | +0 | debounce: consecutive frames where the line stayed low (capped at 50) |
//! | +1 | coins inserted not yet converted into credits |
//! | +2 | credits (capped at 9) |
//! | +3 | total of coins inserted |
//! | +7 | "start taken into account" |
//!
//! Common counters (`SEPARATE_MODE` zero): everything happens in the record of the first coin slot,
//! the others only serve for debouncing. Separate counters: each coin slot has its own.
//!
//! The routines read, decide and write like the original; the coin sound, the EEPROM bookkeeping and the
//! locks (0xb96/0xbac) are called through their Rust functions.

use cps2::Cps2Bus;

use crate::mem::{rd16, rd8, wr8, A5};
use crate::Native;

/// Coin slot port and service button port.
const PORT_PIECES: u32 = 0x804020;
const PORT_SERVICE: u32 = 0x804021;

/// Settings and states, relative to A5.
mod ram {
    /// Coins needed for one credit.
    pub const COINS_PER_CREDIT: u32 = 0x1e;
    /// Credits granted when the threshold is reached.
    pub const CREDITS_PER_THRESHOLD: u32 = 0x1f;
    /// Coin sharing mode (≠ 0: the 2nd coin slot reads line 3; lockout of the 2nd).
    pub const SHARED_MODE: u32 = 0x20;
    /// Number of coin slots minus one.
    pub const COIN_SLOTS: u32 = 0x21;
    /// Separate counters per coin slot (0: common counters).
    pub const SEPARATE_MODE: u32 = 0x22;
    /// Free play: no locking.
    pub const FREE: u32 = 0x25;
    /// Game in progress.
    pub const GAME: u32 = 0x30;
    /// Non-zero word during the test menu: nothing is read.
    pub const MENU_TEST: u32 = 0x7fcc;
    /// Flag that, with `GAME`, enables the coin sound (A5 - 0x713c).
    pub const COIN_SOUND: u32 = 0xffff_8ec4;
    /// First coin slot record, and stride.
    pub const COIN_SLOT_1: u32 = 0x44e;
    pub const STEP: u32 = 0x10;
}

/// 0x14cb8: the four coin slot records zeroed, then their settings: `+0xb` = 0xff, `+8` =
/// "more than one coin per credit", `+9` = 4 and `+0xf` = "free start" (`A5+0x24`) in coin
/// mode 17; free play (`A5+0x25`) in mode 18.
pub fn initialize(b: &mut Cps2Bus) {
    use crate::mem::{wr32, wr8, A5};
    const COIN_MODE: u32 = 0x1c;
    const FREE_START: u32 = 0x24;
    let coin_slot = |k: u32| A5 + ram::COIN_SLOT_1 + ram::STEP * k;
    for i in 0..16 {
        wr32(b, A5 + ram::COIN_SLOT_1 + 4 * i, 0);
    }
    for k in 0..4 {
        wr8(b, coin_slot(k) + 0xb, 0xff);
    }
    let mode = rd8(b, A5 + COIN_MODE);
    wr8(b, A5 + ram::FREE, (mode == 18) as u8);
    let several = (rd8(b, A5 + ram::COINS_PER_CREDIT) as i8 > 1) as u8;
    let setting_9 = if mode == 17 { 4 } else { 0 };
    let free_start = rd8(b, A5 + FREE_START);
    for k in 0..4 {
        wr8(b, coin_slot(k) + 8, several);
        wr8(b, coin_slot(k) + 9, setting_9);
        wr8(b, coin_slot(k) + 0xf, free_start);
    }
}

/// Debounce: saturation, and minimum duration of a valid coin (in frames).
const DEBOUNCE_MAX: u8 = 50;
const DEBOUNCE_MIN: u8 = 2;
/// Maximum credits.
const CREDITS_MAX: u8 = 9;

/// Coin insertion sound (sound wrapper).
const COIN_NOISE: u32 = 0xe18f8;

/// One coin slot line: record index and mask in `IN2`.
#[derive(Clone, Copy)]
struct Line {
    k: u32,
    mask: u8,
}

/// Walks the configured coin slots in the original's order: the lines to examine and, between
/// them, the decisions of the tests on the number of coin slots (`apres_ligne(k)` is called after each
/// line). The second coin slot reads line 3 if the coins are shared.
fn walk(b: &mut Cps2Bus, mut line: impl FnMut(&mut Cps2Bus, Line)) {
    let n = rd8(b, A5 + ram::COIN_SLOTS) as i8;
    line(b, Line { k: 0, mask: 0x10 });
    if n == 0 {
        return;
    }
    if n == 1 {
        let shared = rd8(b, A5 + ram::SHARED_MODE) != 0;
        line(b, Line { k: 1, mask: if shared { 0x40 } else { 0x20 } });
        return;
    }
    line(b, Line { k: 1, mask: 0x20 });
    if n < 2 {
        return;
    }
    line(b, Line { k: 2, mask: 0x40 });
    if n < 3 {
        return;
    }
    line(b, Line { k: 3, mask: 0x80 });
}

// ---------------------------------------------------------------------------------------------
// Reading the coin slots (0x143da)

/// Processes a line (0x14546 common counters, 0x145cc separate counters). `level`: `IN2 & mask`.
fn handle_line(b: &mut Cps2Bus, separate: bool, l: Line, level: u8) {
    let a0 = A5 + ram::COIN_SLOT_1 + ram::STEP * l.k;
    let counters = if separate { a0 } else { A5 + ram::COIN_SLOT_1 };
    let debounce = rd8(b, a0);
    if !(level != 0) {
        // line low: the debounce counts frames, capped
        wr8(b, a0, debounce.wrapping_add(1).min(DEBOUNCE_MAX));
        return;
    }
    // line high: the coin is counted if the line had stayed low long enough, without saturating
    if (debounce < DEBOUNCE_MIN) || (debounce == DEBOUNCE_MAX) {
        wr8(b, a0, 0);
        return;
    }
    let total = rd8(b, counters + 3).wrapping_add(1);
    wr8(b, counters + 3, total);
    let credits = rd8(b, counters + 2);
    if !(credits as i8 >= CREDITS_MAX as i8) {
        let pieces = rd8(b, counters + 1).wrapping_add(1);
        wr8(b, counters + 1, pieces);
    }
    let pieces = rd8(b, counters + 1);
    let threshold = rd8(b, A5 + ram::COINS_PER_CREDIT);
    if !(pieces < threshold) {
        wr8(b, counters + 1, pieces - threshold);
        let gain = rd8(b, A5 + ram::CREDITS_PER_THRESHOLD);
        let credits = rd8(b, counters + 2).wrapping_add(gain);
        let cap_reached = !(credits < 10);
        wr8(b, counters + 2, if cap_reached { CREDITS_MAX } else { credits });
    }
    // coin sound if a game is in progress or if the sound flag is raised
    let sum = rd8(b, A5 + ram::GAME).wrapping_add(rd8(b, A5.wrapping_add(ram::COIN_SOUND)));
    if !(sum == 0) {
        crate::sound::play_global(b, COIN_NOISE);
    }
    // 0x2e448: bookkeeping
    crate::eeprom::count_coin(b);
    wr8(b, a0, 0);
}

/// 0x143da (VBLANK): reads the coin slot lines. The original leaves D0 and A0 intact and VBLANK restores all the
/// registers: nothing comes out in the registers.
pub fn read_coin_slots(b: &mut Cps2Bus) {
    let menu = rd16(b, A5 + ram::MENU_TEST);
    if menu != 0 {
        return;
    }
    let separate = rd8(b, A5 + ram::SEPARATE_MODE) != 0;
    walk(b, |b, l| {
        let level = rd8(b, PORT_PIECES) & l.mask;
        handle_line(b, separate, l, level);
    });
}


// ---------------------------------------------------------------------------------------------
// Start buttons (0x141e0)

/// Absolute address of the first coin slot's record (compared to A0 by the original).
const FIRST_ABSOLUTE: u32 = 0xff844e;

/// 0x1422e: start button of coin slot `coin_slot` (`service` = byte of IN2+1). A start adds a free credit
/// to the coin slot (0x1427a), with bookkeeping for the first.
fn line_start(b: &mut Cps2Bus, coin_slot: u32, service: u8) {
    if service & 4 != 0 {
        wr8(b, coin_slot + 7, 0);
        return;
    }
    if rd8(b, coin_slot + 7) != 0 {
        return;
    }
    let several = rd8(b, A5 + ram::COIN_SLOTS) != 0;
    if !several && coin_slot != FIRST_ABSOLUTE {
        return;
    }
    wr8(b, coin_slot + 7, 1);
    if coin_slot == FIRST_ABSOLUTE {
        crate::eeprom::count_start(b);
    }
    // the credit added is byte `+7` (reread after the bookkeeping: 1)
    let credits = rd8(b, coin_slot + 2).wrapping_add(rd8(b, coin_slot + 7));
    wr8(b, coin_slot + 2, credits);
    if credits >= 10 {
        wr8(b, coin_slot + 2, CREDITS_MAX);
    }
}

/// 0x141e0: the start buttons of the configured coin slots.
pub fn read_starts(b: &mut Cps2Bus) {
    let service = rd8(b, PORT_SERVICE);
    let absolute = |k: u32| FIRST_ABSOLUTE + ram::STEP * k;
    line_start(b, absolute(0), service);
    for k in 1..4 {
        // tst.b COIN_SLOTS; beq (k = 1) then SEPARATE_MODE; beq, then cmpi; beq after each line
        let coin_slots = rd8(b, A5 + ram::COIN_SLOTS);
        let continue_game = match k {
            1 => coin_slots != 0 && rd8(b, A5 + ram::SEPARATE_MODE) != 0,
            _ => coin_slots != k as u8 - 1,
        };
        if !continue_game {
            return;
        }
        line_start(b, absolute(k), service);
    }
}

// ---------------------------------------------------------------------------------------------
// Locking of the coin counters (0x14290)

/// Decides the lock for coin slot `coin_slot` (0x14322): locked (true) if the game is not free and
/// the credits plus the gain of a threshold stay below 10.
fn simple_lock(b: &mut Cps2Bus, coin_slot: u32) -> bool {
    if rd8(b, A5 + ram::FREE) != 0 {
        return false;
    }
    let sum = rd8(b, coin_slot + 2).wrapping_add(rd8(b, A5 + ram::CREDITS_PER_THRESHOLD));
    (sum as i8) < 10
}

/// Same for coin slots 2 to 4 (0x14300): `rank` is their number.
fn rank_lock(b: &mut Cps2Bus, coin_slot: u32, rank: u8) -> bool {
    if rd8(b, A5 + ram::FREE) != 0 {
        return false;
    }
    let coin_slots = rd8(b, A5 + ram::COIN_SLOTS);
    let shared = rd8(b, A5 + ram::SHARED_MODE) != 0;
    if rank as i8 > coin_slots as i8 {
        // coin slot absent: locked only if it shares the coins of the first (except rank 3)
        if coin_slots != 1 || !shared || rank == 3 {
            return false;
        }
        return simple_lock(b, coin_slot);
    }
    // a single extra coin slot in shared mode: no lock of its own
    if coin_slots == 1 && shared {
        return false;
    }
    simple_lock(b, coin_slot)
}

fn apply_lock(b: &mut Cps2Bus, locked: bool, mask: u8) {
    if locked {
        crate::video::lock_counters(b, mask);
    } else {
        crate::video::unlock_counters(b, mask);
    }
}

/// 0x14290: locks or releases the coin counter of each coin slot.
pub fn lock_counters(b: &mut Cps2Bus) {
    let separate = rd8(b, A5 + ram::SEPARATE_MODE) != 0;
    let coin_slot = |k: u32| A5 + ram::COIN_SLOT_1 + ram::STEP * k;
    let lock = simple_lock(b, coin_slot(0));
    apply_lock(b, lock, 0x10);
    for (k, mask) in [(1u32, 0x20u8), (2, 0x40), (3, 0x80)] {
        let m = coin_slot(if separate { k } else { 0 });
        let lock = rank_lock(b, m, k as u8);
        apply_lock(b, lock, mask);
    }
}

pub const READ_COIN_SLOTS: Native = Native {
    entry: 0x143da,
    name: "credits_lire_monnayeurs",
    run: crate::Body::Memory(read_coin_slots),
    touches_sound: false,
    outputs: 0,
};

// ---------------------------------------------------------------------------------------------
// Digit of the credits display (0x14e46)

/// 0x14e46: writes the digit `value & 15` (tile) then the attribute `attr` into the character layer at
/// `case`. Returns the cell following the tile (the original's A4 only advances by one word).
pub fn write_digit(b: &mut Cps2Bus, case: u32, value: u8, attr: u16) -> u32 {
    // addi.w #0,D0: offset of the base tile of the digits, zero in this ROM
    let tile = (value & 15) as u16;
    crate::mem::wr16(b, case, tile);
    crate::mem::wr16(b, case.wrapping_add(2), attr);
    case.wrapping_add(2)
}
/// Original subroutine rewritten in the body of the credits task, presented as a native for the differential
/// fuzz (`examples/fuzz_couv`); the game does not call it by its address.
pub const TEST_NATIVES: &[Native] = &[Native {
    // 0x1422e: start button of a coin slot (A0 coin slot, D1.b byte of IN2+1)
    entry: 0x1422e,
    name: "credits_depart_ligne",
    run: crate::Body::Processeur(|c, b| {
        let (a0, service) = (c.a[0], c.d[1] as u8);
        line_start(b, a0, service);
        crate::mem::rts(c, b);
    }),
    touches_sound: false,
    outputs: 0,
}];

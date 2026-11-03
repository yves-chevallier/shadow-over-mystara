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

use crate::hw::io::{IN2, IN2_SERVICE};
use crate::hw::video::Cell;
use crate::ram::{Byte, Long, Table};
use crate::record::record;
use crate::Native;

/// Settings and states, relative to A5.
pub(crate) mod ram {
    use crate::ram::{Byte, Word};
    /// Coins needed for one credit.
    pub const COINS_PER_CREDIT: Byte = Byte::a5(0x1e);
    /// Credits granted when the threshold is reached.
    pub const CREDITS_PER_THRESHOLD: Byte = Byte::a5(0x1f);
    /// Coin sharing mode (≠ 0: the 2nd coin slot reads line 3; lockout of the 2nd).
    pub const SHARED_MODE: Byte = Byte::a5(0x20);
    /// Number of coin slots minus one.
    pub const COIN_SLOTS: Byte = Byte::a5(0x21);
    /// Separate counters per coin slot (0: common counters).
    pub const SEPARATE_MODE: Byte = Byte::a5(0x22);
    /// Free play: no locking.
    pub const FREE: Byte = Byte::a5(0x25);
    /// Game in progress.
    pub const GAME: Byte = Byte::a5(0x30);
    /// Non-zero word during the test menu: nothing is read.
    pub const MENU_TEST: Word = Word::a5(0x7fcc);
    /// Screen of the big credit digits.
    pub const BIG_DIGITS: Byte = Byte::a5(0x456);
    /// Flag that, with `GAME`, enables the coin sound (A5 - 0x713c).
    pub const COIN_SOUND: Byte = Byte(0xff_0ec4);
}

/// The four coin slot records (16 bytes, `A5+0x44e`).
pub const COIN_SLOTS: Table = Table { base: 0xff_844e, step: 0x10, count: 4 };

/// A coin slot record (see the module), designated by its address (A0 or A1 of the original).
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub struct CoinSlot(pub u32);

impl CoinSlot {
    /// Record `k` (0 to 3).
    pub const fn nth(k: u32) -> CoinSlot {
        CoinSlot(COIN_SLOTS.base + COIN_SLOTS.step * k)
    }
    pub fn at(self, off: u32) -> u32 {
        self.0.wrapping_add(off)
    }
}

record! {
    impl CoinSlot {
        /// Debounce: consecutive frames where the line stayed low (capped at 50).
        debounce, set_debounce, DEBOUNCE: u8 @ 0x00;
        /// Coins inserted not yet converted into credits.
        coins, set_coins, COINS: u8 @ 0x01;
        /// Credits (capped at 9).
        credits, set_credits, CREDITS: u8 @ 0x02;
        /// Coins counted, not yet pulsed on the coin counter.
        counted, set_counted, COUNTED: u8 @ 0x03;
        /// Credits screen already prepared (first record).
        screen_ready, set_screen_ready, SCREEN_READY: u8 @ 0x06;
        /// Start taken into account.
        started, set_started, STARTED: u8 @ 0x07;
        /// More than one coin per credit.
        several_coins, set_several_coins, SEVERAL_COINS: u8 @ 0x08;
        /// Two-coin credit (4 in coin mode 17).
        two_coins, set_two_coins, TWO_COINS: u8 @ 0x09;
        /// Last credit text displayed (byte index into its table).
        shown_text, set_shown_text, SHOWN_TEXT: u8 @ 0x0b;
        /// Free start (`A5+0x24`, coin mode 17).
        free_start, set_free_start, FREE_START: u8 @ 0x0f;
    }
}

/// 0x14cb8: the four coin slot records zeroed, then their settings: `+0xb` = 0xff, `+8` =
/// "more than one coin per credit", `+9` = 4 and `+0xf` = "free start" (`A5+0x24`) in coin
/// mode 17; free play (`A5+0x25`) in mode 18.
pub fn initialize(b: &mut Cps2Bus) {
    const COIN_MODE: Byte = Byte::a5(0x1c);
    const FREE_START: Byte = Byte::a5(0x24);
    for slot in COIN_SLOTS.iter() {
        for i in 0..4 {
            Long(slot.0 + 4 * i).set(b, 0);
        }
    }
    for k in 0..4 {
        CoinSlot::nth(k).set_shown_text(b, 0xff);
    }
    let mode = COIN_MODE.get(b);
    ram::FREE.set(b, (mode == 18) as u8);
    let several = (ram::COINS_PER_CREDIT.get(b) as i8 > 1) as u8;
    let setting_9 = if mode == 17 { 4 } else { 0 };
    let free_start = FREE_START.get(b);
    for k in 0..4 {
        let slot = CoinSlot::nth(k);
        slot.set_several_coins(b, several);
        slot.set_two_coins(b, setting_9);
        slot.set_free_start(b, free_start);
    }
}

/// Debounce: saturation, and minimum duration of a valid coin (in frames).
const DEBOUNCE_MAX: u8 = 50;
const DEBOUNCE_MIN: u8 = 2;
/// Maximum credits.
const CREDITS_MAX: u8 = 9;

/// Coin insertion sound (sound wrapper).
const COIN_NOISE: crate::sound::Sound = crate::sound::CMD_4F;

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
    let n = ram::COIN_SLOTS.get(b) as i8;
    line(b, Line { k: 0, mask: 0x10 });
    if n == 0 {
        return;
    }
    if n == 1 {
        let shared = ram::SHARED_MODE.get(b) != 0;
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
    let a0 = CoinSlot::nth(l.k);
    let counters = if separate { a0 } else { CoinSlot::nth(0) };
    let debounce = a0.debounce(b);
    if !(level != 0) {
        // line low: the debounce counts frames, capped
        a0.set_debounce(b, debounce.wrapping_add(1).min(DEBOUNCE_MAX));
        return;
    }
    // line high: the coin is counted if the line had stayed low long enough, without saturating
    if (debounce < DEBOUNCE_MIN) || (debounce == DEBOUNCE_MAX) {
        a0.set_debounce(b, 0);
        return;
    }
    let total = counters.counted(b).wrapping_add(1);
    counters.set_counted(b, total);
    let credits = counters.credits(b);
    if !(credits as i8 >= CREDITS_MAX as i8) {
        let pieces = counters.coins(b).wrapping_add(1);
        counters.set_coins(b, pieces);
    }
    let pieces = counters.coins(b);
    let threshold = ram::COINS_PER_CREDIT.get(b);
    if !(pieces < threshold) {
        counters.set_coins(b, pieces - threshold);
        let gain = ram::CREDITS_PER_THRESHOLD.get(b);
        let credits = counters.credits(b).wrapping_add(gain);
        let cap_reached = !(credits < 10);
        counters.set_credits(b, if cap_reached { CREDITS_MAX } else { credits });
    }
    // coin sound if a game is in progress or if the sound flag is raised
    let sum = ram::GAME.get(b).wrapping_add(ram::COIN_SOUND.get(b));
    if !(sum == 0) {
        crate::sound::play_global(b, COIN_NOISE);
    }
    // 0x2e448: bookkeeping
    crate::eeprom::count_coin(b);
    a0.set_debounce(b, 0);
}

/// 0x143da (VBLANK): reads the coin slot lines. The original leaves D0 and A0 intact and VBLANK restores all the
/// registers: nothing comes out in the registers.
pub fn read_coin_slots(b: &mut Cps2Bus) {
    let menu = ram::MENU_TEST.get(b);
    if menu != 0 {
        return;
    }
    let separate = ram::SEPARATE_MODE.get(b) != 0;
    walk(b, |b, l| {
        let level = IN2.get(b) & l.mask;
        handle_line(b, separate, l, level);
    });
}

// ---------------------------------------------------------------------------------------------
// Start buttons (0x141e0)

/// 0x1422e: start button of coin slot `coin_slot` (`service` = byte of IN2+1). A start adds a free credit
/// to the coin slot (0x1427a), with bookkeeping for the first.
pub(crate) fn line_start(b: &mut Cps2Bus, coin_slot: CoinSlot, service: u8) {
    // the original compares A0 with the absolute address of the first record
    let first = coin_slot == CoinSlot::nth(0);
    if service & 4 != 0 {
        coin_slot.set_started(b, 0);
        return;
    }
    if coin_slot.started(b) != 0 {
        return;
    }
    let several = ram::COIN_SLOTS.get(b) != 0;
    if !several && !first {
        return;
    }
    coin_slot.set_started(b, 1);
    if first {
        crate::eeprom::count_start(b);
    }
    // the credit added is byte `+7` (reread after the bookkeeping: 1)
    let credits = coin_slot.credits(b).wrapping_add(coin_slot.started(b));
    coin_slot.set_credits(b, credits);
    if credits >= 10 {
        coin_slot.set_credits(b, CREDITS_MAX);
    }
}

/// 0x141e0: the start buttons of the configured coin slots.
pub fn read_starts(b: &mut Cps2Bus) {
    let service = IN2_SERVICE.get(b);
    line_start(b, CoinSlot::nth(0), service);
    for k in 1..4 {
        // tst.b COIN_SLOTS; beq (k = 1) then SEPARATE_MODE; beq, then cmpi; beq after each line
        let coin_slots = ram::COIN_SLOTS.get(b);
        let continue_game = match k {
            1 => coin_slots != 0 && ram::SEPARATE_MODE.get(b) != 0,
            _ => coin_slots != k as u8 - 1,
        };
        if !continue_game {
            return;
        }
        line_start(b, CoinSlot::nth(k), service);
    }
}

// ---------------------------------------------------------------------------------------------
// Locking of the coin counters (0x14290)

/// Decides the lock for coin slot `coin_slot` (0x14322): locked (true) if the game is not free and
/// the credits plus the gain of a threshold stay below 10.
fn simple_lock(b: &mut Cps2Bus, coin_slot: CoinSlot) -> bool {
    if ram::FREE.get(b) != 0 {
        return false;
    }
    let sum = coin_slot.credits(b).wrapping_add(ram::CREDITS_PER_THRESHOLD.get(b));
    (sum as i8) < 10
}

/// Same for coin slots 2 to 4 (0x14300): `rank` is their number.
fn rank_lock(b: &mut Cps2Bus, coin_slot: CoinSlot, rank: u8) -> bool {
    if ram::FREE.get(b) != 0 {
        return false;
    }
    let coin_slots = ram::COIN_SLOTS.get(b);
    let shared = ram::SHARED_MODE.get(b) != 0;
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
    let separate = ram::SEPARATE_MODE.get(b) != 0;
    let lock = simple_lock(b, CoinSlot::nth(0));
    apply_lock(b, lock, 0x10);
    for (k, mask) in [(1u32, 0x20u8), (2, 0x40), (3, 0x80)] {
        let m = CoinSlot::nth(if separate { k } else { 0 });
        let lock = rank_lock(b, m, k as u8);
        apply_lock(b, lock, mask);
    }
}

pub const READ_COIN_SLOTS: Native = Native {
    entry: 0x143da,
    name: "credits_read_coin_slots",
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
    Cell(case).put(b, tile, attr);
    case.wrapping_add(2)
}

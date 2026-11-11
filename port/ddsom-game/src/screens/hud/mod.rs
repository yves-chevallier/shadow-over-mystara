//! Player interface (F3): status panels, "INSERT COIN / PUSH START" banners, character
//! choice and continue countdown. See `docs/hud.md`.
//!
//! It is a permanent task (`0x150ac`, created at the end of phase 0 of the game), which loops on
//! `0x15178`: once per frame (cession `0xb64`), it updates the character layer
//! (`0x912000`, see `layer.rs`) for each of the 1 to 4 player slots.
//!
//! ## One turn (`tour`)
//!
//! * `A5+0x3c` is the *display mode*: negative, the panel area is cleared (once, when the mode
//!   changes); 0, nothing is drawn; 1 to 4, the panels are updated (`slot.rs`). When the mode changes,
//!   each slot redraws its zones (`mode_change`).
//! * a slot (`Slot`) has three work RAM records: the panel (`A5+0x30e + 0x50 k`,
//!   fields in `panel`), the player (`A5+0x5de + 0x200 k`, fields in `player`) and the coin counter
//!   (`A5+0x44e + 0x10 k`, see `credits.rs`). Depending on `A5+0x344` (layout: 4, 3, 2 or 1 players; bit 0
//!   selects the narrow columns), only the first slots exist.
//! * `status.rs`: the common status line (`0x17814`) and the end-of-game countdown (`0x175a8`).
//!
//! ## State machine of a slot (`panel::STATE`, step of 2)
//!
//! | State | Role |
//! |---|---|
//! | 0 | panel initialization from the player (`enter`) |
//! | 2 | waiting: "INSERT COIN" or "PUSH START" banner; start if a credit is available (`idle`) |
//! | 4 | player in the game: update of the panel gauges and digits (`play`) |
//! | 6 | character choice, with countdown (`choose`) |
//! | 8 | continue: countdown from 9 to 0 (`resume`) |
//! | 0xa | end of the continue (`resume_end`) |
//! | 0xc | character choice of the continue (`resume_choose`) |
//!
//! The original routines not rewritten (accounting `0x2e4ec`, sounds `0x2e630`/`0x2e696`, music
//! `0xe12b8`...) are called through `call`.

mod choice;
mod draw;
mod layer;
mod select;
pub(crate) mod slot;
mod status;

pub use slot::forget_displayed;

use crate::screens::credits::CoinSlot;
use crate::screens::tables::Rom;
use crate::world::World;

use crate::world::mem::A5;
use crate::screens::modes::Flow;
use crate::screens::scene::Ctx;
use crate::Native;
use crate::world::ram::Byte;
use crate::world::objects::Obj;

/// Work RAM variables (absolute addresses or offsets from A5).
pub mod var {
    use crate::world::ram::Byte;
    /// Start buttons: current state and previous state (one bit per player).
    pub const START_NOW: Byte = Byte::a5(0x04);
    pub const START_BEFORE: Byte = Byte::a5(0x0a);
    /// Inputs per player (4 bytes) and rising edges (`+0x0c`, 4 bytes).
    pub const INPUT: u32 = 0x00;
    pub const PRESSED: u32 = 0x0c;
    /// Coin slot mode (0 to 11): selects the coin counter of a slot.
    pub const COIN_MODE: Byte = Byte::a5(0x1d);
    /// Price of a game in coins.
    pub const PRICE: Byte = Byte::a5(0x1e);
    /// Two-credit game.
    pub const PRICE_TWO: Byte = Byte::a5(0x457);
    /// "Continue" mode: credits are used to resume a game.
    pub const CONTINUE_MODE: Byte = Byte::a5(0x25);
    /// Continue allowed.
    pub const CONTINUE_ALLOWED: Byte = Byte::a5(0x23);
    pub const GAME_ON: Byte = Byte::a5(0x30);
    pub const PHASE: Byte = Byte::a5(0x2c);
    pub const STEP: Byte = Byte::a5(0x2d);
    /// Display mode of the panels (see the module).
    pub const DISPLAY: Byte = Byte::a5(0x3c);
    /// Pause: one bit per player.
    pub const PAUSE: Byte = Byte::a5(0x4a);
    /// Settings byte (bit 5: second set of texts).
    pub const SETTINGS: Byte = Byte::a5(0x6d);
    /// Non-zero in front of a boss (h): the frames of players 3 and 4 start one row lower.
    pub const BOSS_ROW: Byte = Byte::a5(0x50);
    /// Language of the texts (word: non-zero, the second set of tables).
    pub const LANGUAGE: crate::world::ram::Word = crate::world::ram::Word::a5(0x70);
    /// Layout: number of slots = 4 - value; bit 0: narrow columns.
    pub const LAYOUT: Byte = Byte::a5(0x344);
    /// First panel record (0x50 bytes each).
    pub const PANELS: u32 = 0x30e;
    /// Last display mode processed, and last pause value (task variables).
    pub const SEEN_DISPLAY: Byte = Byte(0xff_0f06);
    pub const SEEN_PAUSE: Byte = Byte(0xff_0f07);
    /// Frozen screen (A5-0x5b8e): the banners are not cleared.
    pub const FROZEN: Byte = Byte(0xff_2472);
}

/// Fields of a panel (0x50-byte record).
pub mod panel {
    pub const SIZE: u32 = 0x50;
    /// Player number (0 to 3).
    pub const INDEX: u32 = 0x00;
    pub const STATE: u32 = 0x04;
    /// Last HP and maximum HP displayed (words), last score and last money displayed (longs).
    pub const SHOWN_HP: u32 = 0x0e;
    pub const SHOWN_HP_MAX: u32 = 0x10;
    pub const SHOWN_SCORE: u32 = 0x12;
    pub const SHOWN_MONEY: u32 = 0x16;
    /// Gauge to redraw (non-zero HP word: gauge displayed).
    pub const HP_BAR: u32 = 0x08;
    pub const HP_DIRTY: u32 = 0x09;
    pub const SCORE_DIRTY: u32 = 0x0a;
    pub const MONEY_DIRTY: u32 = 0x0b;
    pub const CLASS: u32 = 0x0c;
    /// Displayed items (two longs) and "to redraw" flag.
    pub const SHOWN_ITEMS: u32 = 0x1c;
    pub const PLAYER_DIRTY: u32 = 0x1b;
    pub const DISPLAYED_A: u32 = 0x24;
    pub const DISPLAYED_B: u32 = 0x25;
    /// Selected character (step of 2, bit 0: variant).
    pub const CHOICE: u32 = 0x26;
    pub const FLAG_27: u32 = 0x27;
    /// BCD countdown (`+0x28`) and frame sub-counter (`+0x29`).
    pub const COUNTDOWN: u32 = 0x28;
    pub const COUNTDOWN_FRAMES: u32 = 0x29;
    pub const FLAG_2A: u32 = 0x2a;
    pub const STEP: u32 = 0x2c;
    pub const SUBSTEP: u32 = 0x2d;
    pub const DELAY_2E: u32 = 0x2e;
    /// Byte cleared at the task entry (h: unused).
    pub const BYTE_30: u32 = 0x30;
    pub const LAST_MODE: u32 = 0x31;
    /// Frames elapsed in the state (long).
    pub const FRAMES: u32 = 0x32;
    pub const CHOSEN: u32 = 0x37;
    pub const HINT: u32 = 0x38;
    pub const CLAIM: u32 = 0x39;
    pub const SHOWN_NAME: u32 = 0x40;
    /// Coins counted when the continue opened.
    pub const SHOWN_COINS: u32 = 0x2b;
}

/// Fields of the player (0x200-byte record).
pub mod player {
    pub const SIZE: u32 = 0x200;
    pub const ACTIVE: u32 = 0x00;
    pub const FLAGS: u32 = 0x01;
    pub const NUMBER: u32 = 0x02;
    /// Character (step of 2, bit 7: choice locked).
    pub const CHARACTER: u32 = 0x03;
    pub const STATE: u32 = 0x04;
    pub const HP_MAX: u32 = 0x60;
    pub const HP: u32 = 0x62;
    pub const CLASS: u32 = 0x10c;
    pub const SCORE: u32 = 0x110;
    pub const MONEY: u32 = 0x114;
    pub const ITEMS: u32 = 0x118;
    /// Low byte of the score: its last two digits.
    pub const SCORE_LOW: u32 = SCORE + 3;
    /// Selection rows (16 bytes per family), then the row of each entry.
    pub const SELECTIONS: u32 = 0x120;
    pub const SELECTION_ROWS: u32 = 0x150;
    /// Selections held, and selection family (2: none).
    pub const SELECTION_COUNT: u32 = 0x15c;
    pub const SELECTION_KIND: u32 = 0x15d;
    /// Name: eight letters.
    pub const NAME: u32 = 0x160;
    /// Speed class, and a setting given to character 5 (h).
    pub const SPEED_CLASS: u32 = 0x162;
    pub const SETTING_16B: u32 = 0x16b;
}

/// The three records of a slot.
#[derive(Clone, Copy)]
pub struct Slot {
    pub panel: u32,
    pub player: u32,
    pub coins: CoinSlot,
}

impl Slot {
    pub fn new(k: u32) -> Slot {
        Slot {
            panel: A5 + var::PANELS + panel::SIZE * k,
            player: crate::world::ram::PLAYERS.nth(k).0,
            coins: CoinSlot::nth(k),
        }
    }

    /// Byte, word, long of the panel.
    pub fn p8(&self, b: &mut World, off: u32) -> u8 {
        Obj(self.panel).u8(b, off)
    }
    pub fn p16(&self, b: &mut World, off: u32) -> u16 {
        Obj(self.panel).u16(b, off)
    }
    pub fn p32(&self, b: &mut World, off: u32) -> u32 {
        Obj(self.panel).u32(b, off)
    }
    pub fn set_p8(&self, b: &mut World, off: u32, v: u8) {
        Obj(self.panel).set_u8(b, off, v)
    }
    pub fn set_p16(&self, b: &mut World, off: u32, v: u16) {
        Obj(self.panel).set_u16(b, off, v)
    }
    pub fn set_p32(&self, b: &mut World, off: u32, v: u32) {
        Obj(self.panel).set_u32(b, off, v)
    }

    /// Byte, word, long of the player.
    pub fn g8(&self, b: &mut World, off: u32) -> u8 {
        Obj(self.player).u8(b, off)
    }
    pub fn g16(&self, b: &mut World, off: u32) -> u16 {
        Obj(self.player).u16(b, off)
    }
    pub fn g32(&self, b: &mut World, off: u32) -> u32 {
        Obj(self.player).u32(b, off)
    }
    pub fn set_g8(&self, b: &mut World, off: u32, v: u8) {
        Obj(self.player).set_u8(b, off, v)
    }

    /// Player number.
    pub fn index(&self, b: &mut World) -> u8 {
        self.p8(b, panel::INDEX)
    }
}


/// The slots updated according to the layout: 4, 3, then always at least 2.
fn slot_count(b: &mut World) -> u32 {
    match var::LAYOUT.get(b) as i8 {
        0 => 4,
        1 => 3,
        _ => 2,
    }
}

/// Layout per cabinet type (`A5+0x20`), PC-relative byte table.
const LAYOUTS: Rom = Rom::at(0x15164);

/// Cabinet type (number of players, h): selects the layout.
const CABINET: Byte = Byte::a5(0x20);

crate::steps! {
    /// Task steps (the top of the loop, `0x15178`, is never a resume: the entry chains into it).
    pub enum Step {
        Entry = 0x150ac => entry,
        /// Wait for the game start.
        Starting = 0x15146 => starting,
        /// After the cession of a turn.
        AfterWait = 0x15212 => tour,
    }
}

/// 0x150ac: task entry. Panels reset (player number in `+0`), task variables, then
/// wait for the game start.
fn entry(x: &mut Ctx) -> Flow {
    for k in 0..4 {
        let at = Obj(A5 + var::PANELS + k * panel::SIZE);
        at.set_u32(x.b, panel::INDEX, 0);
        at.set_u32(x.b, panel::STATE, 0);
        for off in [panel::STEP, panel::SUBSTEP, panel::BYTE_30] {
            at.set_u8(x.b, off, 0);
        }
        at.set_u32(x.b, panel::FRAMES, 0);
    }
    let over = Obj(status::over::RECORD);
    over.set_u32(x.b, status::over::STATE, 0);
    over.set_u8(x.b, status::over::ACTIVE, 0);
    var::SEEN_DISPLAY.set(x.b, 0x80);
    status::line::SEEN_VALUE.set(x.b, 0);
    status::line::ON.set(x.b, 1);
    let pause = var::PAUSE.get(x.b);
    var::SEEN_PAUSE.set(x.b, pause);
    for k in 1..4 {
        Slot::new(k).set_p8(x.b, panel::INDEX, k as u8);
    }
    crate::core::tasks::one_frame(x, Step::Starting)
}

/// 0x15146: layout according to the cabinet; outside the game, panels displayed in layout 0; in the game, wait for
/// phase 4 (character choice) before the first update.
fn starting(x: &mut Ctx) -> Flow {
    let cabinet = CABINET.get(x.b);
    let layout = LAYOUTS.pc_byte(x.b, cabinet as u32);
    var::LAYOUT.set(x.b, layout);
    if var::GAME_ON.get(x.b) == 0 {
        var::DISPLAY.set(x.b, 1);
        var::LAYOUT.set(x.b, 0);
        slot::forget_displayed(x.b);
    } else if (var::PHASE.get(x.b) as i8) < 4 {
        return crate::core::tasks::one_frame(x, Step::Starting);
    }
    tour(x)
}

/// 0x1507a, called by other modules.
pub const FORGET_DISPLAYED: Native = Native {
    entry: 0x1507a,
    name: "hud_forget_displayed",
    run: crate::Body::Memory(slot::forget_displayed),
    touches_sound: false,
    outputs: 0,
};

/// One turn of the task: panel update according to the display mode, services, then cession.
fn tour(x: &mut Ctx) -> Flow {
    let mode = var::DISPLAY.get(x.b);
    let seen = var::SEEN_DISPLAY.get(x.b);
    if (mode as i8) < 0 {
        if mode != seen {
            var::SEEN_DISPLAY.set(x.b, mode);
            draw::clear_panels(x.b);
        }
    } else {
        if mode != seen {
            slot::forget_displayed(x.b);
        }
        var::SEEN_DISPLAY.set(x.b, mode);
        for k in 0..slot_count(x.b) {
            slot::step(x, Slot::new(k));
        }
        status::status_line(x.b);
        let pause = var::PAUSE.get(x.b);
        var::SEEN_PAUSE.set(x.b, pause);
        status::countdown(x);
    }
    // 0x19b2a, 0x19b9c: the pending events credited to the players' counters
    crate::world::events::credit_points(x.b);
    crate::world::events::credit_counter(x.b);
    crate::core::tasks::one_frame(x, Step::AfterWait)
}

/// Native routines of the player interface.
pub const NATIVES: &[Native] = &[FORGET_DISPLAYED];

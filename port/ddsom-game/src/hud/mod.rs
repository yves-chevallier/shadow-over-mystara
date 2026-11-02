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
mod slot;
mod status;

pub use slot::forget_displayed;

use cps2::Cps2Bus;

use crate::mem::{rd16, rd32, rd8, wr16, wr32, wr8, A5};
use crate::modes::Flow;
use crate::scene::Ctx;
use crate::Native;

/// Work RAM variables (absolute addresses or offsets from A5).
pub mod var {
    /// Start buttons: current state and previous state (one bit per player).
    pub const START_NOW: u32 = 0x04;
    pub const START_BEFORE: u32 = 0x0a;
    /// Inputs per player (4 bytes) and rising edges (`+0x0c`, 4 bytes).
    pub const INPUT: u32 = 0x00;
    pub const PRESSED: u32 = 0x0c;
    /// Coin slot mode (0 to 11): selects the coin counter of a slot.
    pub const COIN_MODE: u32 = 0x1d;
    /// Price of a game in coins.
    pub const PRICE: u32 = 0x1e;
    /// Two-credit game.
    pub const PRICE_TWO: u32 = 0x457;
    /// "Continue" mode: credits are used to resume a game.
    pub const CONTINUE_MODE: u32 = 0x25;
    /// Continue allowed.
    pub const CONTINUE_ALLOWED: u32 = 0x23;
    pub const GAME_ON: u32 = 0x30;
    pub const PHASE: u32 = 0x2c;
    pub const STEP: u32 = 0x2d;
    /// Display mode of the panels (see the module).
    pub const DISPLAY: u32 = 0x3c;
    /// Pause: one bit per player.
    pub const PAUSE: u32 = 0x4a;
    /// Settings byte (bit 5: second set of texts).
    pub const SETTINGS: u32 = 0x6d;
    /// Layout: number of slots = 4 - value; bit 0: narrow columns.
    pub const LAYOUT: u32 = 0x344;
    /// First panel record, first player, first coin counter.
    pub const PANELS: u32 = 0x30e;
    pub const PLAYERS: u32 = 0x5de;
    pub const COINS: u32 = 0x44e;
    /// Last display mode processed, and last pause value (task variables).
    pub const SEEN_DISPLAY: u32 = 0xff_0f06;
    pub const SEEN_PAUSE: u32 = 0xff_0f07;
    /// Frozen screen (A5-0x5b8e): the banners are not cleared.
    pub const FROZEN: u32 = 0xff_2472;
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
}

/// The three records of a slot.
#[derive(Clone, Copy)]
pub struct Slot {
    pub panel: u32,
    pub player: u32,
    pub coins: u32,
}

impl Slot {
    pub fn new(k: u32) -> Slot {
        Slot {
            panel: A5 + var::PANELS + panel::SIZE * k,
            player: A5 + var::PLAYERS + player::SIZE * k,
            coins: A5 + var::COINS + 0x10 * k,
        }
    }

    /// Byte, word, long of the panel.
    pub fn p8(&self, b: &mut Cps2Bus, off: u32) -> u8 {
        rd8(b, self.panel + off)
    }
    pub fn p16(&self, b: &mut Cps2Bus, off: u32) -> u16 {
        rd16(b, self.panel + off)
    }
    pub fn p32(&self, b: &mut Cps2Bus, off: u32) -> u32 {
        rd32(b, self.panel + off)
    }
    pub fn set_p8(&self, b: &mut Cps2Bus, off: u32, v: u8) {
        wr8(b, self.panel + off, v)
    }
    pub fn set_p16(&self, b: &mut Cps2Bus, off: u32, v: u16) {
        wr16(b, self.panel + off, v)
    }
    pub fn set_p32(&self, b: &mut Cps2Bus, off: u32, v: u32) {
        wr32(b, self.panel + off, v)
    }

    /// Byte, word, long of the player.
    pub fn g8(&self, b: &mut Cps2Bus, off: u32) -> u8 {
        rd8(b, self.player + off)
    }
    pub fn g16(&self, b: &mut Cps2Bus, off: u32) -> u16 {
        rd16(b, self.player + off)
    }
    pub fn g32(&self, b: &mut Cps2Bus, off: u32) -> u32 {
        rd32(b, self.player + off)
    }
    pub fn set_g8(&self, b: &mut Cps2Bus, off: u32, v: u8) {
        wr8(b, self.player + off, v)
    }

    /// Player number.
    pub fn index(&self, b: &mut Cps2Bus) -> u8 {
        self.p8(b, panel::INDEX)
    }
}

/// Work RAM byte at an offset from A5.
pub fn v8(b: &mut Cps2Bus, off: u32) -> u8 {
    rd8(b, A5 + off)
}

/// The slots updated according to the layout: 4, 3, then always at least 2.
fn slot_count(b: &mut Cps2Bus) -> u32 {
    match v8(b, var::LAYOUT) as i8 {
        0 => 4,
        1 => 3,
        _ => 2,
    }
}

/// Addresses of the original (task resume).
mod at {
    /// Task entry, and resume of the wait for the game start.
    /// Layout per cabinet type (`A5+0x20`), PC-relative byte table.
    pub const LAYOUTS: u32 = 0x15164;
}

/// Cabinet type (number of players, h): selects the layout.
const CABINET: u32 = 0x20;

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
        let at = A5 + var::PANELS + k * panel::SIZE;
        wr32(x.b, at, 0);
        wr32(x.b, at + 4, 0);
        for off in [panel::STEP, panel::SUBSTEP, 0x30] {
            wr8(x.b, at + off, 0);
        }
        wr32(x.b, at + panel::FRAMES, 0);
    }
    wr32(x.b, 0xff_0eca, 0);
    wr8(x.b, 0xff_0ec6, 0);
    wr8(x.b, var::SEEN_DISPLAY, 0x80);
    wr8(x.b, 0xff_0f08, 0);
    x.set_v8(0x5f, 1);
    let pause = x.v8(var::PAUSE);
    wr8(x.b, var::SEEN_PAUSE, pause);
    for k in 1..4 {
        x.set_v8(var::PANELS + k * panel::SIZE + panel::INDEX, k as u8);
    }
    crate::tasks::one_frame(x, Step::Starting)
}

/// 0x15146: layout according to the cabinet; outside the game, panels displayed in layout 0; in the game, wait for
/// phase 4 (character choice) before the first update.
fn starting(x: &mut Ctx) -> Flow {
    let cabinet = x.v8(CABINET);
    let layout = x.prog8(at::LAYOUTS + cabinet as u32);
    x.set_v8(var::LAYOUT, layout);
    if x.v8(var::GAME_ON) == 0 {
        x.set_v8(var::DISPLAY, 1);
        x.set_v8(var::LAYOUT, 0);
        slot::forget_displayed(x.b);
    } else if (x.v8(var::PHASE) as i8) < 4 {
        return crate::tasks::one_frame(x, Step::Starting);
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
    let mode = x.v8(var::DISPLAY);
    let seen = rd8(x.b, var::SEEN_DISPLAY);
    if (mode as i8) < 0 {
        if mode != seen {
            wr8(x.b, var::SEEN_DISPLAY, mode);
            draw::clear_panels(x.b);
        }
    } else {
        if mode != seen {
            slot::forget_displayed(x.b);
        }
        wr8(x.b, var::SEEN_DISPLAY, mode);
        for k in 0..slot_count(x.b) {
            slot::step(x, Slot::new(k));
        }
        status::status_line(x.b);
        let pause = x.v8(var::PAUSE);
        wr8(x.b, var::SEEN_PAUSE, pause);
        status::countdown(x);
    }
    // 0x19b2a, 0x19b9c: the pending events credited to the players' counters
    crate::events::credit_points(x.b);
    crate::events::credit_counter(x.b);
    crate::tasks::one_frame(x, Step::AfterWait)
}

/// Native routines of the player interface.
pub const NATIVES: &[Native] = &[FORGET_DISPLAYED];

/// Original subroutines rewritten inside the task body, presented as natives for the
/// differential fuzz (`examples/fuzz_couv`); the game does not call them by their address.
pub const TEST_NATIVES: &[Native] = &[Native {
    // 0x15216: a slot (A0 panel, A6 player, A4 coin counter)
    entry: 0x15216,
    name: "hud_slot",
    run: crate::Body::WithCpu(|c, b| {
        let s = Slot { panel: c.a[0], player: c.a[6], coins: c.a[4] };
        slot::step(&mut Ctx::new(c, b), s);
        crate::mem::rts(c, b);
    }),
    touches_sound: false,
    outputs: 0,
}];

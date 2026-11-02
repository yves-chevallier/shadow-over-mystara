//! Credits task (C1, `0x13c0a`): coin slots, "credits" screen, game starts and resumption
//! (continue). It is one of the scheduler's permanent tasks (`kernel.rs`).
//!
//! It loops: each turn reads the start buttons (`0x141e0`), handles the credits
//! state (`0x13c2c`), locks the coin counters (`0x14290`) and reads the continue
//! buttons (`0x1436e`), then cedes one frame (`0xb64`). As long as the TEST switch is not
//! operated (bit 1 of `IN2+1`, active low, test at `0x14d74`), it continues; otherwise it tears itself down
//! (`0x14d8c`): screen reset, interface task (`0x29162`) created, task ended.
//!
//! ## Credits state (`0x13c2c`)
//!
//! According to the credits and the starts requested (coin slot records at `A5+0x44e`, see
//! `credits.rs`), it prepares the credits screen at the first credit (`init`, which clears the screen and
//! cedes a frame), displays it, then calls `start` (`0x140b4`) for each player: if the player's
//! start button is pressed and there are enough credits, the player enters the game (game task
//! `0x1c45a` created at the first start). When a player has started (`A5+0x31` ≠ 0), the game task
//! is launched in place of the credits screen (`end`).
//!
//! The original cessions (`0xc32` clears the screen while ceding, `0x1460` fades to black, `0x1436e`, `0xb64`)
//! cut the code into pieces: each is a step of the task's state machine ([`Step`]), at the return
//! address of the original.

use crate::credits_screen::{self, FIRST_COIN_SLOT};
use crate::scene::fading;
use crate::modes::{Flow, Sequence};
use crate::mem::{rd16, rd8, set_low_word, wr16, wr32, wr8, A5};
use crate::objects::{pools, Obj};
use crate::scene::Ctx;
use crate::tasks;
use crate::video::{load_grid, Grid};

/// Sounds of the credits screen.
mod sound {
    pub const SCREEN_A: u32 = 0xe1060;
    pub const SCREEN_B: u32 = 0xe18b8;
    pub const START: u32 = 0xe181c;
}

/// Entry of the game task, and of the players' interface task.
const GAME_TASK: u32 = 0x1c45a;
const UI_TASK: u32 = 0x29162;
/// Texts displayed by the credits screen (addresses of pointers or strings in ROM).
const TEXT_CREDITS: u32 = 0x14ea2;
const TEXT_TABLE: u32 = 0x15060;

/// Hardware registers.
const IN2: u32 = 0x804020;
const IN2_SERVICE: u32 = 0x804021;
const CPS_VIDEO_A: u32 = 0x804122;
const CPS_VIDEO_B: u32 = 0x804166;
const CPS_A_OBJ: u32 = 0x804100;
const CPS_A_SCROLL: u32 = 0x80410c;
const OUTPUT_4: u32 = 0x400004;
const VIDEO_ON: u16 = 0x12ce;
const OUTPUT_ON: u16 = 0x1355;

/// Work RAM variables (offsets from A5).
mod var {
    /// Game in progress.
    pub const GAME: u32 = 0x30;
    /// Players who have started (one bit per player).
    pub const STARTED: u32 = 0x31;
    /// "Continue" mode: the credits serve to resume a game.
    pub const CONTINUE: u32 = 0x25;
    pub const SEPARATE: u32 = 0x22;
    pub const COIN_COUNT: u32 = 0x21;
    pub const SHARED: u32 = 0x20;
    /// First coin slot, and stride.
    pub const COIN_1: u32 = 0x44e;
    /// Credits screen already prepared (byte +6 of the first coin slot).
    pub const SCREEN_READY: u32 = COIN_1 + 6;
    /// Coin sound flag (A5 - 0x713c).
    pub const COIN_SOUND: u32 = 0xffff_8ec4;
    /// Requested scrolls (see `vblank.rs`).
    pub const SCROLLS: [u32; 6] = [0x4b6, 0x4ba, 0x506, 0x50a, 0x556, 0x55a];
    pub const FLAGS: u32 = 0x12;
    pub const VIDEO_B: u32 = 0x14;
    pub const SCROLL_MODE: u32 = 0x53c;
    pub const GAME_LANGUAGE: u32 = 0x70;
    /// Flag (A5 - 0x7155) and byte (A5 - 0x5b8e) reset to zero / set by the screen.
    pub const CLEAR_FLAG: u32 = 0xffff_8eab;
    pub const SCREEN_FLAG: u32 = 0xffff_a472;
    pub const FRAME_SKIP: u32 = 0x5bc;
    pub const FRAME_SKIP_2: u32 = 0x5ba;
    pub const TILES_OFF: u32 = 0x33;
}

/// Return addresses of the original after each call that may cede control.
mod at {
    /// Returns of the `bsr 0x143a6` of `0x1436e`, and resumption of the pulse after its first cession.
    pub const LOCKOUT: [u32; 3] = [0x14376, 0x14386, 0x14398];
    pub const PULSE_AFTER_WAIT: u32 = 0x143c8;
    /// After `bsr 0x13c2c`, `0x14290`, `0x1436e` of the loop, and after the cession.
    pub const AFTER_STATE: u32 = 0x13c18;
    pub const AFTER_CONTINUE: u32 = 0x13c20;
    pub const AFTER_WAIT: u32 = 0x13c2a;
    /// After clearing the initialisation screen.
    pub const AFTER_INIT_CLEAR: u32 = 0x13cc6;
    /// Returns of the four start calls of each sequence (the last of B is the end, `0x13fa8`).
    pub const SEQ_A: [u32; 4] = [0x13eb6, 0x13ec8, 0x13ee2, 0x13efe];
    pub const SEQ_B: [u32; 4] = [0x13f72, 0x13f84, 0x13f96, 0x13fa8];
    pub const SEQ_C: [u32; 4] = [0x13f26, 0x13f38, 0x13f4a, 0x13f5c];
    /// After the fade and the clearing of the game resumption.
    pub const AFTER_FADE: u32 = 0x14066;
    pub const AFTER_FADE_CLEAR: u32 = 0x1406c;
    /// Resumption of `0x140b4` after the screen is cleared.
    pub const START_AFTER_CLEAR: u32 = 0x14174;
    /// Teardown: after the clearing.
    pub const SHUTDOWN_AFTER_CLEAR: u32 = 0x14e24;
}

fn a5(off: u32) -> u32 {
    A5.wrapping_add(off)
}

/// Work RAM byte at a signed offset (`A5 - n` is given as 32 bits).
fn v8(x: &mut Ctx, off: u32) -> u8 {
    rd8(x.b, a5(off))
}
fn set8(x: &mut Ctx, off: u32, v: u8) {
    wr8(x.b, a5(off), v)
}
fn set16(x: &mut Ctx, off: u32, v: u16) {
    wr16(x.b, a5(off), v)
}

/// Kills the tasks at these offsets in the table (`0xa14`, D0 = offset).
fn kill(x: &mut Ctx, slots: &[u16]) {
    for &s in slots {
        crate::kernel::kill_task(x.b, s as u32);
    }
}

fn clear_scrolls(x: &mut Ctx) {
    for off in var::SCROLLS {
        set16(x, off, 0);
    }
}

/// `A5+0x14 = 0 ; 0xbc6`: restarts the planes.
fn video_on(x: &mut Ctx) {
    set16(x, var::VIDEO_B, 0);
    crate::video::turn_layers_on(x.b, VIDEO_ON);
}

// ---------------------------------------------------------------------------------------------
// The task loop

/// Steps of the task: its entry, and each return from a call that may cede control.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Step {
    /// `0x13c0a`.
    Entry,
    /// After `bsr 0x13c2c` (credits state), `0x1436e` (locks), and after the turn's cession.
    AfterState,
    AfterContinue,
    AfterWait,
    /// Teardown, after the screen is cleared.
    ShutdownAfterClear,
    /// First preparation of the screen, after the clearing.
    AfterInitClear,
    /// Return of the `k`-th start call of a sequence (the last of sequence B lands on `0x13fa8`, the end).
    Sequence(Seq, usize),
    /// Game resumption: after the fade to black, then after the clearing.
    AfterFade,
    AfterFadeClear,
    /// Start of a player in "continue" mode, after the screen is cleared.
    StartAfterClear(GameStart),
    /// Lock pulse after its first cession; `d0` is the D0 that the original keeps on the stack
    /// (`move.l D0,-(SP)`).
    PulseAfterWait { d0: u32 },
    /// Return of the pulse of coin slot `k - 1`: that of coin slot `k` (1 to 3).
    Lock(usize),
}

impl Step {
    /// Address that the original resumes at in this step.
    pub fn address(self) -> u32 {
        match self {
            Step::Entry => 0x13c0a,
            Step::AfterState => at::AFTER_STATE,
            Step::AfterContinue => at::AFTER_CONTINUE,
            Step::AfterWait => at::AFTER_WAIT,
            Step::ShutdownAfterClear => at::SHUTDOWN_AFTER_CLEAR,
            Step::AfterInitClear => at::AFTER_INIT_CLEAR,
            Step::Sequence(seq, k) => seq.returns()[k],
            Step::AfterFade => at::AFTER_FADE,
            Step::AfterFadeClear => at::AFTER_FADE_CLEAR,
            Step::StartAfterClear(_) => at::START_AFTER_CLEAR,
            Step::PulseAfterWait { .. } => at::PULSE_AFTER_WAIT,
            Step::Lock(k) => at::LOCKOUT[k - 1],
        }
    }

    pub fn step(self, x: &mut Ctx) -> Flow {
        match self {
            Step::Entry | Step::AfterWait => tour(x),
            Step::AfterState => after_state(x),
            Step::AfterContinue => after_continue(x),
            Step::ShutdownAfterClear => shutdown_after_clear(x),
            Step::AfterInitClear => after_init_clear(x),
            Step::Sequence(seq, k) => sequence(x, seq, k + 1),
            Step::AfterFade => x.subroutine(crate::video::black_screen::start, Step::AfterFadeClear),
            Step::AfterFadeClear => after_fade_clear(x),
            Step::StartAfterClear(d) => start_after_clear(x, d),
            Step::PulseAfterWait { d0 } => pulse_after_wait(x, d0),
            Step::Lock(k) => lockouts(x, k),
        }
    }
}

/// `0x13c10`: one turn.
fn tour(x: &mut Ctx) -> Flow {
    crate::credits::read_starts(x.b);
    x.push_return(Step::AfterState);
    credits(x)
}

fn after_state(x: &mut Ctx) -> Flow {
    crate::credits::lock_counters(x.b);
    x.push_return(Step::AfterContinue);
    lockouts(x, 0)
}

fn after_continue(x: &mut Ctx) -> Flow {
    // 0x14d74: the task continues as long as the TEST switch is not operated (the ROM flag 0xc8 is zero)
    if x.rd8(IN2_SERVICE) & 2 != 0 {
        tasks::one_frame(x, Step::AfterWait)
    } else {
        shutdown(x)
    }
}

/// `0x14d8c`: teardown of the task.
fn shutdown(x: &mut Ctx) -> Flow {
    kill(x, &[0, 0x10, 0x30, 0xb0, 0xd0]);
    wr16(x.b, CPS_VIDEO_A, 0xe);
    wr16(x.b, CPS_VIDEO_B, 0x12ce);
    set8(x, var::FRAME_SKIP, 1);
    wr16(x.b, CPS_A_OBJ, 0x7080);
    set16(x, var::FRAME_SKIP_2, 0x7080);
    for k in 0..3 {
        wr32(x.b, CPS_A_SCROLL + 4 * k, 0);
    }
    for off in var::SCROLLS {
        wr32(x.b, a5(off), 0);
    }
    set8(x, var::TILES_OFF, 0);
    let flags = rd16(x.b, a5(var::FLAGS)) & !1;
    set16(x, var::FLAGS, flags);
    set8(x, var::SCROLL_MODE, 0);
    set8(x, var::CLEAR_FLAG, 0);
    x.subroutine(crate::video::black_screen::start, Step::ShutdownAfterClear)
}

/// End of the teardown: the TEST menu task takes slot 0, the task ends.
fn shutdown_after_clear(x: &mut Ctx) -> Flow {
    crate::palette::load_all_palettes(x.b);
    crate::sound::service(x.b, crate::sound::Service::GeneralStop);
    crate::kernel::create_task(x.b, 0, UI_TASK);
    tasks::finish(x)
}

// ---------------------------------------------------------------------------------------------
// Credits state (0x13c2c)

/// `0x13c2c`.
fn credits(x: &mut Ctx) -> Flow {
    if v8(x, var::GAME) != 0 {
        return Flow::Done;
    }
    if v8(x, var::CONTINUE) == 0 {
        // credits or coins anywhere?
        let sum = [0x44f, 0x45f, 0x46f, 0x47f, 0x450, 0x460, 0x470, 0x480].iter().fold(0u8, |s, &o| s.wrapping_add(v8(x, o)));
        if sum == 0 {
            return Flow::Done;
        }
        if v8(x, var::SCREEN_READY) == 0 {
            return init(x);
        }
        return display(x);
    }
    starts(x)
}

/// `0x13c6c`: first time: prepares the credits screen, after the screen is cleared.
fn init(x: &mut Ctx) -> Flow {
    set8(x, var::SCREEN_READY, 1);
    kill(x, &[0, 0x10, 0x30, 0xd0, 0xf0]);
    set8(x, var::SCROLL_MODE, 0);
    clear_scrolls(x);
    x.subroutine(crate::video::black_screen::start, Step::AfterInitClear)
}

fn after_init_clear(x: &mut Ctx) -> Flow {
    let flags = rd16(x.b, a5(var::FLAGS)) & !1;
    set16(x, var::FLAGS, flags);
    set8(x, var::CLEAR_FLAG, 0);
    set8(x, var::SCREEN_FLAG, 1);
    crate::tasks::game::clear_variables(x.c, x.b);
    crate::player::init::clear_all_players(x.c, x.b);
    crate::sprites::clear_lists(x.b);
    wr16(x.b, OUTPUT_4, OUTPUT_ON);
    video_on(x);
    crate::palette::load_palette_set(x.b, 1);
    pools::SOBJ.reset(x.b);
    // five system objects: (variant, X, Z)
    for (variant, px, pz) in [(0xe7, 0x70, 0xd0), (0xe8, 0x70, 0xd0), (0xe4, 0x20, 0xb0), (0xe5, 0x00, 0x70), (0xe6, 0x13c, 0x60)] {
        // Original glitch: the allocation failure is not tested (see `Pool::alloc_unchecked`).
        let Obj(o) = pools::SOBJ.alloc_unchecked(x.b);
        wr8(x.b, o, 1);
        wr8(x.b, o + 2, 0);
        wr8(x.b, o + 3, variant);
        wr16(x.b, o + 8, px);
        wr16(x.b, o + 0xc, pz);
    }
    crate::dispatch::SOBJ.update_no_next(x.c, x.b);
    // two grids of each kind, the second following the first
    let end = load_grid(x.b, Grid::Blocks4x4, 5, 0x912000);
    load_grid(x.b, Grid::Blocks4x4, 6, end.destination);
    let end = load_grid(x.b, Grid::Layer2, 0x201, 0x917000);
    load_grid(x.b, Grid::Layer2, 0x202, end.destination);
    let end = load_grid(x.b, Grid::Layer3, 8, 0x91b800);
    load_grid(x.b, Grid::Layer3, 8, end.destination);
    crate::palette::load_palette_page(x.b, 1, 2);
    crate::palette::load_palette_page(x.b, 2, 0x55);
    crate::palette::load_palette_page(x.b, 3, 0x55);
    fading::PAGES.set(x.b, 0xf);
    crate::palette::full_brightness(x.b);
    video_on(x);
    crate::sound::service(x.b, crate::sound::Service::GeneralStop);
    crate::sound::play_global(x.b, sound::SCREEN_A);
    crate::sound::play_global(x.b, sound::SCREEN_B);
    set8(x, var::COIN_SOUND, 1);
    display(x)
}

/// `0x13e46`: displays the credits screen (counters, texts, sprites), then processes the starts.
fn display(x: &mut Ctx) -> Flow {
    credits_screen::credit_text(x.b, FIRST_COIN_SLOT);
    credits_screen::digits(x.b, FIRST_COIN_SLOT);
    crate::text::print(x.b, TEXT_CREDITS);
    let text = language_text(x);
    crate::text::print(x.b, text);
    crate::sprites::build_68k(x.c, x.b);
    starts(x)
}

/// Text of the game's language (`A5+0x70`, two bytes per language in the pointer table).
fn language_text(x: &mut Ctx) -> u32 {
    let d0 = (rd16(x.b, a5(var::GAME_LANGUAGE)) as u32 * 2) & 0xffff;
    x.rd32(TEXT_TABLE + d0)
}

// ---------------------------------------------------------------------------------------------
// Counter lock pulses (0x1436e)

/// Offsets of the four coin slots.
const COINS: [u32; 4] = [0x44e, 0x45e, 0x46e, 0x47e];

/// `0x1436e`: for each coin slot whose counter `+3` is not zero, a lock
/// pulse (see `pulse`), further and further apart according to the number of coin slots (`A5+0x21`). Each
/// pulse cedes control: its continuation is the step `Lock(k + 1)`.
fn lockouts(x: &mut Ctx, k: usize) -> Flow {
    // coin slot k only exists if `A5+0x21` ≥ k
    if k > 0 && v8(x, var::COIN_COUNT) as usize == k - 1 {
        return Flow::Done;
    }
    x.c.a[0] = a5(COINS[k]);
    if k < 3 {
        x.push_return(Step::Lock(k + 1));
    }
    pulse(x)
}

/// `0x143a6`: if coin slot A0 has a counted coin (`+3`), counts it down, locks the coin
/// counter for 16 frames then unlocks it and waits another 16 frames.
fn pulse(x: &mut Ctx) -> Flow {
    set_low_word(&mut x.c.d[0], 1);
    let counter = rd8(x.b, x.c.a[0] + 3);
    if counter == 0 {
        return Flow::Done;
    }
    wr8(x.b, x.c.a[0] + 3, counter - 1);
    // D0 is kept during the cession (`move.l D0,-(SP)` of the original): in the resumption step
    let d0 = x.c.d[0];
    crate::video::lock_counters(x.b, d0 as u8);
    set_low_word(&mut x.c.d[0], 0x10);
    crate::kernel::wait(x.c, x.b, Step::PulseAfterWait { d0 });
    Flow::Yield
}

/// `0x143c8`: unlocks, then the second wait (`jmp 0xa40`): the task will resume at the continuation of the caller.
fn pulse_after_wait(x: &mut Ctx, d0: u32) -> Flow {
    x.c.d[0] = d0;
    crate::video::unlock_counters(x.b, d0 as u8);
    set_low_word(&mut x.c.d[0], 0x10);
    let next = crate::tasks::table::pop_return(x.c, x.b);
    crate::kernel::wait(x.c, x.b, next);
    Flow::Yield
}

// ---------------------------------------------------------------------------------------------
// Game starts (0x13e7a)

/// A `start` call: coin slot (offset of A1), button mask (D1), player (D2), player record
/// (offset of A3).
type Start = (u32, u8, u32, u32);

const SEQ_A: [Start; 4] = [(0x44e, 1, 0, 0x5de), (0x44e, 2, 1, 0x7de), (0x44e, 4, 2, 0x9de), (0x44e, 8, 3, 0xbde)];
const SEQ_B: [Start; 4] = [(0x44e, 1, 0, 0x5de), (0x44e, 2, 1, 0x7de), (0x45e, 4, 2, 0x9de), (0x45e, 8, 3, 0xbde)];
const SEQ_C: [Start; 4] = [(0x44e, 1, 0, 0x5de), (0x45e, 2, 1, 0x7de), (0x46e, 4, 2, 0x9de), (0x47e, 8, 3, 0xbde)];

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Seq {
    A,
    B,
    C,
}

impl Seq {
    fn calls(self) -> &'static [Start; 4] {
        match self {
            Seq::A => &SEQ_A,
            Seq::B => &SEQ_B,
            Seq::C => &SEQ_C,
        }
    }
    fn returns(self) -> &'static [u32; 4] {
        match self {
            Seq::A => &at::SEQ_A,
            Seq::B => &at::SEQ_B,
            Seq::C => &at::SEQ_C,
        }
    }
}

/// `0x13e7a`: chooses the sequence of start calls.
fn starts(x: &mut Ctx) -> Flow {
    if v8(x, var::SEPARATE) != 0 {
        let seq = if v8(x, var::COIN_COUNT) == 1 && v8(x, var::SHARED) != 0 { Seq::B } else { Seq::C };
        return sequence(x, seq, 0);
    }
    if v8(x, var::CONTINUE) == 0 {
        let needed = if v8(x, var::COIN_1 + 9) != 0 { 2 } else { 1 };
        if v8(x, var::COIN_1 + 2) < needed {
            return end(x);
        }
    }
    sequence(x, Seq::A, 0)
}

/// Runs the sequence's start calls from the `k`-th. Each call returns to the step
/// `Sequence(seq, k)`, which resumes at the next call.
fn sequence(x: &mut Ctx, seq: Seq, k: usize) -> Flow {
    if k == 4 {
        return end(x);
    }
    if let Seq::A = seq {
        // the third player only exists if A5+0x20 ≠ 0, the fourth if it is ≥ 2
        let shared = v8(x, var::SHARED) as i8;
        if (k == 2 && shared == 0) || (k == 3 && shared <= 1) {
            return end(x);
        }
    }
    let (coin_slot, mask, player, record) = seq.calls()[k];
    x.push_return(Step::Sequence(seq, k));
    start_player(x, GameStart { coin_slot: a5(coin_slot), player: player as u8, record: a5(record), cost: 0 }, mask)
}

/// `0x13fa8`: if a player has started, launches the game.
fn end(x: &mut Ctx) -> Flow {
    if v8(x, var::GAME) != 0 || v8(x, var::STARTED) == 0 {
        return Flow::Done;
    }
    if v8(x, var::CONTINUE) == 0 {
        kill(x, &[0, 0x10]);
        set8(x, var::COIN_SOUND, 0);
        set8(x, var::GAME, 1);
        set8(x, var::COIN_1 + 6, 0);
        crate::text::erase(x.b, TEXT_CREDITS);
        let text = language_text(x);
        crate::text::erase(x.b, text);
        return create_game_task(x);
    }
    // game resumption: the continue screen is torn down, faded to black and cleared
    crate::sprites::clear_lists(x.b);
    pools::SOBJ.reset(x.b);
    let flags = rd16(x.b, a5(var::FLAGS)) & !1;
    set16(x, var::FLAGS, flags);
    set8(x, var::SCROLL_MODE, 0);
    set8(x, var::CLEAR_FLAG, 0);
    kill(x, &[0, 0x10, 0xd0]);
    fading::PAGES.set(x.b, 0xf);
    crate::palette::full_brightness(x.b);
    fading::DELAY.set(x.b, 2);
    fading::PAGES.set(x.b, 0xf);
    x.subroutine(crate::tasks::fading::descent, Step::AfterFade)
}

fn after_fade_clear(x: &mut Ctx) -> Flow {
    wr16(x.b, OUTPUT_4, OUTPUT_ON);
    video_on(x);
    set8(x, var::GAME, 1);
    set8(x, var::COIN_1 + 6, 0);
    clear_scrolls(x);
    create_game_task(x)
}

/// `jmp 0x9b6` with A0 = game task, D0 = slot 0: the game task is created, and we return to the
/// caller of `0x13c2c`.
fn create_game_task(x: &mut Ctx) -> Flow {
    crate::kernel::create_task(x.b, 0, GAME_TASK);
    Flow::Done
}

// ---------------------------------------------------------------------------------------------
// Start of a player (0x140b4)

/// A player start (A1, D2, A3 and D3 of the original, kept while the screen is cleared in a start in
/// "continue" mode).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct GameStart {
    /// Record of the paying coin slot.
    coin_slot: u32,
    /// Player number.
    player: u8,
    /// Player record.
    record: u32,
    /// Credits debited.
    cost: u8,
}

/// `0x140b4`: the player whose start button is `mask` (IN2) enters the game.
fn start_player(x: &mut Ctx, mut d: GameStart, mask: u8) -> Flow {
    let button = x.rd8(IN2) & mask;
    if v8(x, var::CONTINUE) == 0 {
        d.cost = if rd8(x.b, d.coin_slot + 9) != 0 { 2 } else { 1 };
        if rd8(x.b, d.coin_slot + 2) < d.cost || button != 0 {
            return Flow::Done;
        }
        return join(x, d);
    }
    if button != 0 {
        return Flow::Done;
    }
    if rd8(x.b, d.coin_slot + 6) == 0 {
        wr8(x.b, d.coin_slot + 6, 1);
        kill(x, &[0, 0x10, 0x30, 0xd0, 0xf0]);
        clear_scrolls(x);
        return x.subroutine(crate::video::black_screen::start, Step::StartAfterClear(d));
    }
    join(x, d)
}

fn start_after_clear(x: &mut Ctx, d: GameStart) -> Flow {
    wr16(x.b, OUTPUT_4, OUTPUT_ON);
    video_on(x);
    let flags = rd16(x.b, a5(var::FLAGS)) & !1;
    set16(x, var::FLAGS, flags);
    crate::tasks::game::clear_variables(x.c, x.b);
    crate::player::init::clear_all_players(x.c, x.b);
    set8(x, var::SCROLL_MODE, 0);
    crate::sprites::clear_lists(x.b);
    join(x, d)
}

/// `0x141a6`: the player starts: sound, credit debited, player record created, bookkeeping (`jmp`, which
/// returns to the caller).
fn join(x: &mut Ctx, d: GameStart) -> Flow {
    crate::sound::play_global(x.b, sound::START);
    crate::sound::service(x.b, crate::sound::Service::Ff07 { lock: 0, line: 4 });
    let credits = rd8(x.b, d.coin_slot + 2).wrapping_sub(d.cost);
    wr8(x.b, d.coin_slot + 2, credits);
    crate::tasks::game::clear_variables(x.c, x.b);
    wr8(x.b, d.record, 1);
    wr8(x.b, d.record + 2, d.player);
    let started = rd8(x.b, a5(var::STARTED)) | 1 << (d.player & 7);
    wr8(x.b, a5(var::STARTED), started);
    crate::eeprom::count_game(x.b);
    Flow::Done
}

/// Original subroutine rewritten in the body of the task, presented as a native for the differential fuzz
/// (`examples/fuzz_couv`); the game does not call it by its address. If it cedes control, the trial is discarded
/// (panic).
pub const TEST_NATIVES: &[crate::Native] = &[crate::Native {
    // 0x13c2c: credits state (startup, resumption, rallying of the four slots)
    entry: 0x13c2c,
    name: "credits_state",
    run: crate::Body::WithCpu(|c, b| {
        if let Flow::Yield = credits(&mut Ctx::new(c, b)) {
            panic!("credits_etat: cedes control (outside the fuzz frame)");
        }
        crate::mem::rts(c, b);
    }),
    touches_sound: false,
    outputs: 0,
}];


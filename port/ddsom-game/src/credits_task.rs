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
use crate::credits::CoinSlot;
use crate::data::RomAddr;
use crate::hw::io::{IN2, IN2_SERVICE};
use crate::hw::video::regs;
use crate::player::Player;
use crate::ram::{Long, PLAYERS};
use crate::screens::tables::Rom;
use crate::objects::{pools, Obj};
use crate::scene::Ctx;
use crate::tasks;
use crate::video::{load_grid, Grid};

/// Sounds of the credits screen.
mod sound {
    pub const SCREEN_A: crate::sound::Sound = crate::sound::CMD_01;
    pub const SCREEN_B: crate::sound::Sound = crate::sound::CMD_4E;
    pub const START: crate::sound::Sound = crate::sound::CMD_4C;
}

/// Entry of the game task, and of the players' interface task.
const GAME_TASK: RomAddr = RomAddr(0x1c45a);
const UI_TASK: RomAddr = RomAddr(0x29162);
/// Texts displayed by the credits screen (addresses of pointers or strings in ROM).
const TEXT_CREDITS: Rom = Rom::at(0x14ea2);
const TEXT_TABLE: Rom = Rom::at(0x15060);

/// Values of the video registers.
const VIDEO_ON: u16 = 0x12ce;
const OUTPUT_ON: u16 = 0x1355;

/// Work RAM variables (offsets from A5).
mod var {
    use crate::ram::{Byte, Word};
    /// Game in progress.
    pub const GAME: Byte = Byte::a5(0x30);
    /// Players who have started (one bit per player).
    pub const STARTED: Byte = Byte::a5(0x31);
    /// "Continue" mode: the credits serve to resume a game.
    pub const CONTINUE: Byte = Byte::a5(0x25);
    pub const SEPARATE: Byte = Byte::a5(0x22);
    pub const COIN_COUNT: Byte = Byte::a5(0x21);
    pub const SHARED: Byte = Byte::a5(0x20);
    /// First coin slot, and stride.
    use crate::credits::CoinSlot;
    pub const COIN_1: CoinSlot = CoinSlot::nth(0);
    /// Credits screen already prepared (byte +6 of the first coin slot).
    pub const SCREEN_READY: Byte = Byte(COIN_1.0 + CoinSlot::SCREEN_READY);
    /// Coin sound flag (A5 - 0x713c).
    pub const COIN_SOUND: Byte = Byte(0xff_0ec4);
    /// Requested scrolls (see `vblank.rs`).
    pub const SCROLLS: [Word; 6] = [Word::a5(0x4b6), Word::a5(0x4ba), Word::a5(0x506), Word::a5(0x50a), Word::a5(0x556), Word::a5(0x55a)];
    pub const FLAGS: Word = Word::a5(0x12);
    pub const VIDEO_B: Word = Word::a5(0x14);
    pub const SCROLL_MODE: Byte = Byte::a5(0x53c);
    pub const GAME_LANGUAGE: Word = Word::a5(0x70);
    /// Flag (A5 - 0x7155) and byte (A5 - 0x5b8e) reset to zero / set by the screen.
    pub const CLEAR_FLAG: Byte = Byte(0xff_0eab);
    pub const SCREEN_FLAG: Byte = Byte(0xff_2472);
    pub const FRAME_SKIP: Byte = Byte::a5(0x5bc);
    pub const FRAME_SKIP_2: Word = Word::a5(0x5ba);
    pub const TILES_OFF: Byte = Byte::a5(0x33);
}

/// Return addresses of the original after each call that may cede control.
mod at {
    /// Returns of the `bsr 0x143a6` of `0x1436e`, and resumption of the pulse after its first cession.
    pub const LOCKOUT: [u32; 3] = [0x14376, 0x14386, 0x14398];
    /// Returns of the four start calls of each sequence (the last of B is the end, `0x13fa8`).
    pub const SEQ_A: [u32; 4] = [0x13eb6, 0x13ec8, 0x13ee2, 0x13efe];
    pub const SEQ_B: [u32; 4] = [0x13f72, 0x13f84, 0x13f96, 0x13fa8];
    pub const SEQ_C: [u32; 4] = [0x13f26, 0x13f38, 0x13f4a, 0x13f5c];
}

/// Kills the tasks at these offsets in the table (`0xa14`, D0 = offset).
fn kill(x: &mut Ctx, slots: &[u16]) {
    for &s in slots {
        crate::kernel::kill_task(x.b, s as u32);
    }
}

fn clear_scrolls(x: &mut Ctx) {
    for v in var::SCROLLS {
        v.set(x.b, 0);
    }
}

/// `A5+0x14 = 0 ; 0xbc6`: restarts the planes.
fn video_on(x: &mut Ctx) {
    var::VIDEO_B.set(x.b, 0);
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
    /// Lock pulse after its first cession.
    PulseAfterWait,
    /// Return of the pulse of coin slot `k - 1`: that of coin slot `k` (1 to 3).
    Lock(usize),
}

impl Step {
    /// Address that the original resumes at in this step.
    pub fn address(self) -> u32 {
        match self {
            Step::Entry => 0x13c0a,
            Step::AfterState => 0x13c18,
            Step::AfterContinue => 0x13c20,
            Step::AfterWait => 0x13c2a,
            Step::ShutdownAfterClear => 0x14e24,
            Step::AfterInitClear => 0x13cc6,
            Step::Sequence(seq, k) => seq.returns()[k],
            Step::AfterFade => 0x14066,
            Step::AfterFadeClear => 0x1406c,
            Step::StartAfterClear(_) => 0x14174,
            Step::PulseAfterWait => 0x143c8,
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
            Step::PulseAfterWait => pulse_after_wait(x),
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
    if IN2_SERVICE.get(x.b) & 2 != 0 {
        tasks::one_frame(x, Step::AfterWait)
    } else {
        shutdown(x)
    }
}

/// `0x14d8c`: teardown of the task.
fn shutdown(x: &mut Ctx) -> Flow {
    kill(x, &[0, 0x10, 0x30, 0xb0, 0xd0]);
    regs::VIDEO_CONTROL.set(x.b, 0xe);
    regs::LAYER_CONTROL.set(x.b, 0x12ce);
    var::FRAME_SKIP.set(x.b, 1);
    regs::OBJECT_BASE.set(x.b, 0x7080);
    var::FRAME_SKIP_2.set(x.b, 0x7080);
    for k in 0..3 {
        regs::SCROLLS.nth(2 * k).set_long(x.b, 0);
    }
    for v in var::SCROLLS {
        Long(v.0).set(x.b, 0);
    }
    var::TILES_OFF.set(x.b, 0);
    let flags = var::FLAGS.get(x.b) & !1;
    var::FLAGS.set(x.b, flags);
    var::SCROLL_MODE.set(x.b, 0);
    var::CLEAR_FLAG.set(x.b, 0);
    x.subroutine(crate::video::black_screen::start, Step::ShutdownAfterClear)
}

/// End of the teardown: the TEST menu task takes slot 0, the task ends.
fn shutdown_after_clear(x: &mut Ctx) -> Flow {
    crate::palette::load_all_palettes(x.b);
    crate::sound::service(x.b, crate::sound::Service::GeneralStop);
    crate::kernel::create_task(x.b, 0, UI_TASK.0);
    tasks::finish(x)
}

// ---------------------------------------------------------------------------------------------
// Credits state (0x13c2c)

/// `0x13c2c`.
pub(crate) fn credits(x: &mut Ctx) -> Flow {
    if var::GAME.get(x.b) != 0 {
        return Flow::Done;
    }
    if var::CONTINUE.get(x.b) == 0 {
        // credits or coins anywhere?
        let coins = (0..4).map(|k| CoinSlot::nth(k).coins(x.b)).fold(0u8, u8::wrapping_add);
        let sum = (0..4).map(|k| CoinSlot::nth(k).credits(x.b)).fold(coins, u8::wrapping_add);
        if sum == 0 {
            return Flow::Done;
        }
        if var::SCREEN_READY.get(x.b) == 0 {
            return init(x);
        }
        return display(x);
    }
    starts(x)
}

/// `0x13c6c`: first time: prepares the credits screen, after the screen is cleared.
fn init(x: &mut Ctx) -> Flow {
    var::SCREEN_READY.set(x.b, 1);
    kill(x, &[0, 0x10, 0x30, 0xd0, 0xf0]);
    var::SCROLL_MODE.set(x.b, 0);
    clear_scrolls(x);
    x.subroutine(crate::video::black_screen::start, Step::AfterInitClear)
}

fn after_init_clear(x: &mut Ctx) -> Flow {
    let flags = var::FLAGS.get(x.b) & !1;
    var::FLAGS.set(x.b, flags);
    var::CLEAR_FLAG.set(x.b, 0);
    var::SCREEN_FLAG.set(x.b, 1);
    crate::tasks::game::clear_variables(x.b);
    crate::player::init::clear_all_players(x.b);
    crate::sprites::clear_lists(x.b);
    regs::OBJECT_OUTPUT.set(x.b, OUTPUT_ON);
    video_on(x);
    crate::palette::load_palette_set(x.b, 1);
    pools::SOBJ.reset(x.b);
    // five system objects: (variant, X, Z)
    for (variant, px, pz) in [(0xe7, 0x70, 0xd0), (0xe8, 0x70, 0xd0), (0xe4, 0x20, 0xb0), (0xe5, 0x00, 0x70), (0xe6, 0x13c, 0x60)] {
        // Original glitch: the allocation failure is not tested (see `Pool::alloc_unchecked`).
        let o = pools::SOBJ.alloc_unchecked(x.b);
        o.set_active(x.b, 1);
        o.set_kind(x.b, 0);
        o.set_variant(x.b, variant);
        o.set_u16(x.b, Obj::X, px);
        o.set_u16(x.b, Obj::Z, pz);
    }
    crate::dispatch::SOBJ.update_no_next(x.b);
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
    var::COIN_SOUND.set(x.b, 1);
    display(x)
}

/// `0x13e46`: displays the credits screen (counters, texts, sprites), then processes the starts.
fn display(x: &mut Ctx) -> Flow {
    credits_screen::credit_text(x.b, FIRST_COIN_SLOT);
    credits_screen::digits(x.b, FIRST_COIN_SLOT);
    crate::text::print(x.b, TEXT_CREDITS.addr());
    let text = language_text(x);
    crate::text::print(x.b, text);
    crate::sprites::build_in_game(x.b);
    starts(x)
}

/// Text of the game's language (`A5+0x70`, two bytes per language in the pointer table).
fn language_text(x: &mut Ctx) -> u32 {
    let d0 = (var::GAME_LANGUAGE.get(x.b) as u32 * 2) & 0xffff;
    TEXT_TABLE.long(x.b, d0)
}

// ---------------------------------------------------------------------------------------------
// Counter lock pulses (0x1436e)

/// `0x1436e`: for each coin slot whose counter `+3` is not zero, a lock
/// pulse (see `pulse`), further and further apart according to the number of coin slots (`A5+0x21`). Each
/// pulse cedes control: its continuation is the step `Lock(k + 1)`.
fn lockouts(x: &mut Ctx, k: usize) -> Flow {
    // coin slot k only exists if `A5+0x21` ≥ k
    if k > 0 && var::COIN_COUNT.get(x.b) as usize == k - 1 {
        return Flow::Done;
    }
    if k < 3 {
        x.push_return(Step::Lock(k + 1));
    }
    pulse(x, CoinSlot::nth(k as u32))
}

/// Coin counter bit locked and unlocked by a pulse (D0.w = 1 of the original, kept during the cession).
const PULSE_BITS: u8 = 1;
/// Frames of each half of a pulse (D0.w = 0x10).
const PULSE_FRAMES: u8 = 0x10;

/// `0x143a6`: if coin slot `slot` (A0 of the original) has a counted coin (`+3`), counts it down, locks the coin
/// counter for 16 frames then unlocks it and waits another 16 frames.
fn pulse(x: &mut Ctx, slot: CoinSlot) -> Flow {
    let counter = slot.counted(x.b);
    if counter == 0 {
        return Flow::Done;
    }
    slot.set_counted(x.b, counter - 1);
    crate::video::lock_counters(x.b, PULSE_BITS);
    crate::tasks::wait(x, PULSE_FRAMES, Step::PulseAfterWait)
}

/// `0x143c8`: unlocks, then the second wait (`jmp 0xa40`): the task will resume at the continuation of the caller.
fn pulse_after_wait(x: &mut Ctx) -> Flow {
    crate::video::unlock_counters(x.b, PULSE_BITS);
    crate::tasks::wait_then_return(x, PULSE_FRAMES)
}

// ---------------------------------------------------------------------------------------------
// Game starts (0x13e7a)

/// A `start` call: coin slot (A1), button mask (D1), player (D2); the player record (A3) is the player's.
type Start = (u32, u8, u32);

const SEQ_A: [Start; 4] = [(0, 1, 0), (0, 2, 1), (0, 4, 2), (0, 8, 3)];
const SEQ_B: [Start; 4] = [(0, 1, 0), (0, 2, 1), (1, 4, 2), (1, 8, 3)];
const SEQ_C: [Start; 4] = [(0, 1, 0), (1, 2, 1), (2, 4, 2), (3, 8, 3)];

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
    if var::SEPARATE.get(x.b) != 0 {
        let seq = if var::COIN_COUNT.get(x.b) == 1 && var::SHARED.get(x.b) != 0 { Seq::B } else { Seq::C };
        return sequence(x, seq, 0);
    }
    if var::CONTINUE.get(x.b) == 0 {
        let needed = if var::COIN_1.two_coins(x.b) != 0 { 2 } else { 1 };
        if var::COIN_1.credits(x.b) < needed {
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
        let shared = var::SHARED.get(x.b) as i8;
        if (k == 2 && shared == 0) || (k == 3 && shared <= 1) {
            return end(x);
        }
    }
    let (coin_slot, mask, player) = seq.calls()[k];
    x.push_return(Step::Sequence(seq, k));
    start_player(x, GameStart { coin_slot: CoinSlot::nth(coin_slot), player: player as u8, record: PLAYERS.nth(player).0, cost: 0 }, mask)
}

/// `0x13fa8`: if a player has started, launches the game.
fn end(x: &mut Ctx) -> Flow {
    if var::GAME.get(x.b) != 0 || var::STARTED.get(x.b) == 0 {
        return Flow::Done;
    }
    if var::CONTINUE.get(x.b) == 0 {
        kill(x, &[0, 0x10]);
        var::COIN_SOUND.set(x.b, 0);
        var::GAME.set(x.b, 1);
        var::COIN_1.set_screen_ready(x.b, 0);
        crate::text::erase(x.b, TEXT_CREDITS.addr());
        let text = language_text(x);
        crate::text::erase(x.b, text);
        return create_game_task(x);
    }
    // game resumption: the continue screen is torn down, faded to black and cleared
    crate::sprites::clear_lists(x.b);
    pools::SOBJ.reset(x.b);
    let flags = var::FLAGS.get(x.b) & !1;
    var::FLAGS.set(x.b, flags);
    var::SCROLL_MODE.set(x.b, 0);
    var::CLEAR_FLAG.set(x.b, 0);
    kill(x, &[0, 0x10, 0xd0]);
    fading::PAGES.set(x.b, 0xf);
    crate::palette::full_brightness(x.b);
    fading::DELAY.set(x.b, 2);
    fading::PAGES.set(x.b, 0xf);
    x.subroutine(crate::tasks::fading::descent, Step::AfterFade)
}

fn after_fade_clear(x: &mut Ctx) -> Flow {
    regs::OBJECT_OUTPUT.set(x.b, OUTPUT_ON);
    video_on(x);
    var::GAME.set(x.b, 1);
    var::COIN_1.set_screen_ready(x.b, 0);
    clear_scrolls(x);
    create_game_task(x)
}

/// `jmp 0x9b6` with A0 = game task, D0 = slot 0: the game task is created, and we return to the
/// caller of `0x13c2c`.
fn create_game_task(x: &mut Ctx) -> Flow {
    crate::kernel::create_task(x.b, 0, GAME_TASK.0);
    Flow::Done
}

// ---------------------------------------------------------------------------------------------
// Start of a player (0x140b4)

/// A player start (A1, D2, A3 and D3 of the original, kept while the screen is cleared in a start in
/// "continue" mode).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct GameStart {
    /// Record of the paying coin slot.
    coin_slot: CoinSlot,
    /// Player number.
    player: u8,
    /// Player record.
    record: u32,
    /// Credits debited.
    cost: u8,
}

/// `0x140b4`: the player whose start button is `mask` (IN2) enters the game.
fn start_player(x: &mut Ctx, mut d: GameStart, mask: u8) -> Flow {
    let button = IN2.get(x.b) & mask;
    if var::CONTINUE.get(x.b) == 0 {
        d.cost = if d.coin_slot.two_coins(x.b) != 0 { 2 } else { 1 };
        if d.coin_slot.credits(x.b) < d.cost || button != 0 {
            return Flow::Done;
        }
        return join(x, d);
    }
    if button != 0 {
        return Flow::Done;
    }
    if d.coin_slot.screen_ready(x.b) == 0 {
        d.coin_slot.set_screen_ready(x.b, 1);
        kill(x, &[0, 0x10, 0x30, 0xd0, 0xf0]);
        clear_scrolls(x);
        return x.subroutine(crate::video::black_screen::start, Step::StartAfterClear(d));
    }
    join(x, d)
}

fn start_after_clear(x: &mut Ctx, d: GameStart) -> Flow {
    regs::OBJECT_OUTPUT.set(x.b, OUTPUT_ON);
    video_on(x);
    let flags = var::FLAGS.get(x.b) & !1;
    var::FLAGS.set(x.b, flags);
    crate::tasks::game::clear_variables(x.b);
    crate::player::init::clear_all_players(x.b);
    var::SCROLL_MODE.set(x.b, 0);
    crate::sprites::clear_lists(x.b);
    join(x, d)
}

/// `0x141a6`: the player starts: sound, credit debited, player record created, bookkeeping (`jmp`, which
/// returns to the caller).
fn join(x: &mut Ctx, d: GameStart) -> Flow {
    crate::sound::play_global(x.b, sound::START);
    crate::sound::service(x.b, crate::sound::Service::Ff07 { lock: 0, line: 4 });
    let credits = d.coin_slot.credits(x.b).wrapping_sub(d.cost);
    d.coin_slot.set_credits(x.b, credits);
    crate::tasks::game::clear_variables(x.b);
    Player(d.record).obj().set_active(x.b, 1);
    Player(d.record).obj().set_u8(x.b, Player::INDEX, d.player);
    let started = var::STARTED.get(x.b) | 1 << (d.player & 7);
    var::STARTED.set(x.b, started);
    crate::eeprom::count_game(x.b);
    Flow::Done
}

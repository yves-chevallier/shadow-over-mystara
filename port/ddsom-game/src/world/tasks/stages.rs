//! The stages of the Rust tasks: one `enum` per module whose code cedes control, one variant per resume point
//! (`world::tasks`, "Stages").
//!
//! The task table holds the stage where each task resumes and the continuations of its calls in progress: these types
//! are therefore declared here, in the world, with what they carry (pure data: objects, counters, leftovers). The
//! code of each stage stays in its module, which re-exports its `enum` as `Step` and maps each variant to its original
//! address and to its function (`steps!`, or an `impl` written by hand); `core::tasks` runs them.

use crate::world::objects::Obj;
use crate::world::Leftovers;

// ---------------------------------------------------------------------------------------------
// Core tasks (`core::tasks`) and the game loop

// Run by `core/tasks/game.rs`.
/// Stages of the game task (the game step, `0x1c4b0`, remains a native).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Game {
    Entry,
    /// Start of turn: return from the frame end, or from a phase that pops its own return.
    Tour,
    /// The game step: the current phase. `d4`: the D4 that the previous step left and that the phase re-reads
    /// (leftover of the dispatch loop, `Leftovers::d4`: the pilotless raft, 0xad7b0, adds it to the players'
    /// position); the original keeps it in its registers, which the cession saves and restores.
    Frame { d4: u32 },
    /// Return from the phase: frame end (`game::frame_end`), then the next turn.
    FrameEnd,
}

// Run by `actors/update.rs` and `core/game.rs`.
/// Resumptions of the game loop when an object handler made the task cede control.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum World {
    /// Return from the dispatch of an object class, step `step` of the order `ORDERS[order_index]`: the world
    /// update continues at the next step, then returns to its caller.
    World { order_index: u8, step: u8 },
    /// Return of the world update in phase 12 (`0x1c702`): end of the phase.
    AfterWorld,
}

// Run by `core/tasks/freeze.rs`.
/// Stages of the task: its entry, and the returns from its two cessions.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Freeze {
    Entry,
    /// After a cession with no effect.
    AfterCession,
    /// After a cession that counted down the main task.
    AfterCountdown,
}

// Run by `actors/dispatch.rs`.
/// Resumption of a dispatch loop in which a handler made the task cede control.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Dispatch {
    /// Return of the handler of the current slot of class `CLASSES[class]`: the walk continues at
    /// the next slot, then returns to the loop's caller. The original keeps the object (A0) and the counter
    /// (D7.w) in its registers, which the cession saves and restores; here they are in the step, noted at the call of
    /// the handler (no handler that cedes control changes them).
    Dispatch { class: u8, object: Obj, counter: u16 },
}

// ---------------------------------------------------------------------------------------------
// Palette fades (`services/fading.rs`)

/// Stages of a fade: the start of a fade task (from its entry), then the next step after each cession.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Fading {
    /// Start of a brightness fade task; `step`: imposed number of steps (otherwise that of the state).
    BrightnessTask { fading: Brightness, step: Option<u8> },
    /// Start of a per-component fade task (`whiten`: task `0x1ced4` first whitens the pages).
    ComponentTask { fading: Component, whiten: bool },
    /// Next step of a brightness fade.
    Brightness { fading: Brightness },
    /// Next step of a per-component fade; `saved`: the leftovers D2-D5 saved on entry.
    Component { fading: Component, saved: Leftovers },
}

/// The fade tasks (created by `0x9b6` in slot 1), by entry.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Fade {
    /// `0x14ba`: full rise of the brightness (16 steps).
    Rise,
    /// `0x14c8`: rise by the number of steps of the state.
    RiseSteps,
    /// `0x14d0`: full descent of the brightness (16 steps).
    Descent,
    /// `0x14de`: descent by the number of steps of the state.
    DescentSteps,
    /// `0x1536`: rise of the end of page 0 only.
    PartialRise,
    /// `0x153e`: descent of the end of page 0 only.
    PartialDescent,
    /// `0x1ccf8`: per component, toward white.
    ToWhite,
    /// `0x1ce64`: per component, toward the colours.
    ToColours,
    /// `0x1ced4`: whitens the pages, then per component toward the palettes.
    FromWhite,
    /// `0x1cf12`: per component toward the palettes, without whitening first.
    ToPalettes,
}

/// A brightness fade: direction (D1 of the original), form (subroutine or task), pages (complete or partial).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Brightness {
    pub(crate) delta: u32,
    pub(crate) task: bool,
    pub(crate) partial: bool,
}

/// A per-component fade: target, form and address following the step's cession.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Component {
    pub(crate) target: Target,
    pub(crate) task: bool,
    pub(crate) resume: u32,
}

/// Target of a per-component fade (`palette::component_step`).
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Target {
    /// White (`0x1cd32`).
    White,
    /// One colour per page, the high word of `SOURCES[k]` (`0x1cdd6`).
    Colours,
    /// A full palette per page, at address `SOURCES[k]` (`0x1cd7c`).
    Palettes,
}

// ---------------------------------------------------------------------------------------------
// Screen tasks (`screens/`)

// Run by `screens/service/task.rs`.
/// Stages of the task.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Service {
    Entry,
    /// After the wait frame: one turn.
    Turn,
    /// Return of the handler when it ceded control on the way: the wait frame.
    HandlerReturn,
    /// Resume of the menu after the wait frame that follows the clearing of the screen (`service::show_menu`).
    MenuAfterClear,
}

// Run by `screens/fixed_screen.rs`.
/// Stages of the task: the entry and the cession return, each of which does one turn.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Scores {
    Entry,
    Resume,
    /// Return from the stage call (`jsr (table,PC)`): the objects, the sprites, a cession frame.
    AfterStep,
}

// Run by `screens/prompt.rs`.
/// Stages: the entry, then after the display and after the erasure, for each of the two families of texts.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Prompt {
    Entry,
    CoinsShown,
    CoinsHidden,
    PlainShown,
    PlainHidden,
}

// Run by `screens/hud/mod.rs`.
/// Task steps (the top of the loop, `0x15178`, is never a resume: the entry chains into it).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Interface {
    Entry,
    /// Wait for the game start.
    Starting,
    /// After the cession of a turn.
    AfterWait,
}

// ---------------------------------------------------------------------------------------------
// Mode machine (`screens/modes/`)

// Run by `screens/modes/mod.rs`.
/// Steps of the mode machine loop (`0x133fe`).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Modes {
    /// Loop entry (task 0 restarted after a game, or end of startup).
    Entry,
    /// The handler yielded: one yield frame, then a new turn.
    AfterHandler,
    /// After the yield: a new turn.
    AfterWait,
}

// Run by `screens/modes/boot.rs`.
/// Steps (resume points) of the state machine.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Boot {
    /// Task entry (`0x11b16`): black screen, then initialisation.
    Entry,
    Init,
    WarningTick,
    NoticeTick,
}

// Run by `screens/modes/attract.rs`.
/// Steps (resume points) of the state machine.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Attract {
    FixedScreenAfterFade,
    LegalScreenSetup,
    LegalScreenTiles,
    LegalScreenSound,
    LegalScreenFaded,
    LegalScreenCleared,
    LegalScreenPalette,
    SecondSetup,
    SecondTiles,
    SecondFadeIn,
    SecondFaded,
    SecondCleared,
    SecondPalette,
    NoticeTextShown,
    NoticeFadeIn,
    NoticeCountdown,
    NoticeFaded,
    NoticeCleared,
}

// Run by `screens/modes/title.rs`.
/// Steps (resume points) of the state machine.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Title {
    T00Load,
    T00Fade,
    T06Object,
    T06Retry,
    T08Next,
    T0aRetry,
    T10Object,
    T10Retry,
    T12Scenery,
    T12Fade,
    T18Retry,
    T18Loop,
    T18After,
    T18SubRetry,
    T1aRetry,
    T1cText,
    T1eWait,
    T1eCleared,
    T1eDone,
    S02Load,
    S02Retry,
    S02Show,
    S02Palette,
    S04Retry,
    S04Show,
    S08Wait,
    S08Cleared,
    S08Done,
}

// Run by `screens/modes/select.rs`.
/// Steps (resume points) of the state machine.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Select {
    FadeStart,
    FadeLoop,
    Step6Faded,
    Step6Clear,
    Step6Palette,
    Step8Show,
    StepEFaded,
    StepEHide,
    StepEClear,
    StepEFade,
}

// Run by `screens/modes/ranking.rs`.
/// Steps (resume points) of the state machine.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Ranking {
    Return,
    S0Video,
    S0Palettes,
    S0Tiles,
    SaFaded,
    SaVideo,
    SaCleared,
}

// Run by `screens/modes/demo/mod.rs`.
/// Steps (resume points) of the state machine.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Demo {
    ReturnA,
    LoopA,
    AfterYieldA,
    SceneEndClearA,
    SceneEndNextA,
    EndClear,
    EndShow,
    /// The jump from `0x1fce4` to `0x1fc68` leaves the `bsr` address on the stack: the terrain is rendered twice.
    PresentAgain,
    P4Hide,
    P4Clear,
    ReturnB,
    LoopB,
    AfterYieldB,
    SceneNextClearB,
    SceneNextNextB,
    EndingReset,
    EndingClear,
    EndingShow,
    /// Phase returns (`jsr (A0)` of the two loops): a phase may cede control, then come back here.
    AfterPhaseA,
    AfterPhaseB,
    /// Return from the world update in demo phase 12 (`jsr 0x1c86e` at `0x1fa44`), when
    /// an object handler made the task cede control.
    AfterWorld,
}

// Run by `screens/modes/ending/mod.rs`.
/// Steps (resume points) of the state machine.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Ending {
    Return,
    S00Video,
    S00Palette,
    S02Cleared,
    S04Fade,
    S04Timer,
    S06Retry,
    S08Fade,
    S08Faded,
    S08Picture,
    S08Retry,
    S0aRetry,
    S0cObject,
    S0cRetry,
    S0eLoop,
    S0eMusic,
    S0eObjects,
    S10Retry,
    S14Retry,
    S18Retry,
    S1aLoop,
    S1cScroll,
    S1cPalette,
    S1cFadeLoop,
    S1eLoop,
    S1eRetry,
    S22Loop,
    S28Loop,
    S2aPlanes,
    S2aScrolls,
    S2aWait,
    S2aRetry,
    S2aShow,
    S2cLoop,
    S2cPlanes,
    S2cCleared,
    S2cTimer,
    S2eOpened,
    S2eClosed,
    S30Wait,
    S32Faded,
    S32Cleared,
}

// Run by `screens/modes/gameover.rs`.
/// Steps (resume points) of the state machine.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Gameover {
    Faded,
    Cleared,
}

// ---------------------------------------------------------------------------------------------
// Credits task (`screens/credits_task.rs`)

/// Steps of the task: its entry, and each return from a call that may cede control.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Credits {
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

/// The sequence of start calls (`0x13e7a`).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Seq {
    A,
    B,
    C,
}

/// A player start (A1, D2, A3 and D3 of the original, kept while the screen is cleared in a start in
/// "continue" mode).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct GameStart {
    /// Record of the paying coin slot.
    pub(crate) coin_slot: CoinSlot,
    /// Player number.
    pub(crate) player: u8,
    /// Player record.
    pub(crate) record: u32,
    /// Credits debited.
    pub(crate) cost: u8,
}

/// A coin slot record (`screens::credits`, which names its fields), designated by its address (A0 or A1 of the
/// original).
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub struct CoinSlot(pub u32);

// ---------------------------------------------------------------------------------------------
// Scene sequences (`screens/scene/`)

// Run by `screens/scene/black_screen.rs`.
/// Steps of the black screen.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum BlackScreen {
    /// Objects and layers cleared, then one frame.
    Clear,
    /// Return to the caller.
    End,
}

// Run by `screens/scene/blackout.rs`.
/// Steps (resume points) of the state machine.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Blackout {
    Faded,
    /// Layers cleared, scrolls and cameras reset.
    Cleared,
    Finale1,
    Finale2,
}

// Run by `screens/scene/transition/mod.rs`.
/// Steps (resume points) of the state machine.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Transition {
    FadeInAfterFrame,
    SpritesAfterFrame,
}

// Run by `screens/scene/transition/early/mod.rs`.
/// Steps (resume points) of the state machine.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum TransitionEarly {
    S1Shown,
    S1Retry,
    S2Shown,
    S2Hidden,
    S2Drawn,
    S2Faded,
    S2Cleared,
    S5Retry,
    S5Faded,
    S5Hidden,
    S5Shown,
    S5FadedIn,
    S5Retry2,
    S5TextFaded,
    S5TextHidden,
    S5TextShown,
    S5TextFadedIn,
    S5EndFaded,
    S5EndDrawn,
    S5End,
    S4Shown,
    S4TextGone,
    S4Hidden,
    S4SecondText,
    S4SecondDrawn,
    S4SecondShown,
    S4Faded,
    S4FadedHidden,
    S4Homed,
    S4Dark,
    S4DarkIn,
    S4Retry92,
    S4RetryWizard,
    S4SecondFaded,
    S4SecondClear,
    S4SecondHidden,
    S4SecondHomed,
    S4RetryObject,
    S4Scenery,
    S4SceneryShown,
    S4FadingIn,
    S4FadingInNext,
    S4Retry16,
    S4FadingOut,
    S4FadingOutNext,
    S4FadedOut,
    S4End,
    S4EndHidden,
    S11RetryObject,
    S11Drawn,
    S11Shown,
    S11Retry17,
    S11Retry18,
    S11Faded,
    S11DrawnOut,
    S11Hidden,
    S11End,
    S13RetryObject,
    S13Drawn,
    S13Shown,
    S13LeftDrawn,
    S13LeftWaited,
    S13LeftHidden,
    S13LeftScenery,
    S13Retry11,
    S13Set11Drawn,
    S13Set11Shown,
    S13OutFaded,
    S13OutScenery,
    S13OutSceneryDrawn,
    S13Retry9,
    S13OutDrawn,
    /// state 6: new terrain, dialogue 30
    S13Closing,
    S13ClosingWaited,
    S13ClosingHidden,
    S13ClosingDrawn,
    S13ClosingShown,
    S13ClosingIn,
    S13Retry30,
    S13Faded,
    S13End,
}

// Run by `screens/scene/transition/late.rs`.
/// Steps (resume points) of the state machine.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum TransitionLate {
    S31Shown,
    S31Retry,
    S33RetryObject,
    S33Drawn,
    S33Shown,
    S33Retry39,
    S33Faded,
    S33Clear,
    S33Hidden,
    S33Dark,
    S33Drawn2,
    S33Shown2,
    S33Retry40,
    S33EndFaded,
    S33EndHidden,
    S33End,
    S32Shown,
    /// Original glitch: the resume does not reload D0: the original then reopens message 1 (D0 of the wait).
    S32Retry59,
    S32Hidden,
    S32Retry60,
    S32SecondShown,
    S32SecondIn,
    S32Retry61,
    S32Coloured,
    S32Retry62,
    S32Faded,
    S32End,
    S38Drawn,
    S38Closed,
    S38Retry73,
    S38Gone,
    S38Hidden,
    S38Homed,
    S38Scenery,
    S38SceneryShown,
    S38SceneryIn,
    S38Retry98,
    S38LastFaded,
    S38LastHidden,
    S38LastScenery,
    S38LastShown,
    S38LastIn,
    S38Retry74,
    S38End,
    S46Homed,
    S46Drawn,
    S46Shown,
    S46Gone,
    S46Hidden,
    S46End,
    FinaleFaded,
    FinaleBlack,
    FinalePalettes,
    FinaleEnd,
}

// Run by `screens/scene/bonus.rs`.
/// Steps (resume points) of the state machine.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Bonus {
    /// The cameras are set aside and placed on the bonus terrain.
    FadedOut,
    SetUp,
    /// The bonus title (SET object type 0, variant 69).
    Started,
    /// The bonus screen is cleared; outside scene 1 the state is finished, in scene 1 the palette pages come back
    /// after four frames.
    Ended,
    Scene1Waited,
    /// Last frame: fade from black, then the next state of the sequence.
    LastFrame,
    FadedIn,
}

// Run by `screens/scene/shop.rs`.
/// Steps (resume points) of the shop, and the portrait task.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Shop {
    /// Entry of the portrait task.
    Portraits,
    /// Opening: shop number, portrait task, music, shop and player objects, then the
    /// shop scene (69 + number) is loaded and its spawn script restarted.
    OpenFaded,
    LeaveFaded,
    /// Return from the shop: layers cleared, next state of the sequence, return scene.
    LeaveWaited,
    ChainFaded,
    ReopenWaited,
}

// Run by `screens/scene/tally.rs`.
/// Steps (resume points) of the state machine.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Tally {
    FadeWait,
    /// End of sub-state 0: the banner is drawn off-screen, then its first step.
    AfterFade,
}

// Run by `screens/scene/exit/mod.rs`.
/// Steps of the scene exit: dispatch tails of the sequences (return of a shared state that ceded control) and
/// resumes after the yield of the final state.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Exit {
    /// Dispatch tail of the sequence of the given index (`entry + 10`).
    DispatchTail(u8),
    /// `0xf32cc`: `0xc0c` then `0xe0882`.
    Both,
    /// `0xf481a`: `0xc0c`.
    Sound,
    /// `0xf4b48`: `0xe0882`.
    Command,
    /// `0xf3cec`: `0xc0c`, then two waiting frames before `TwiceEnd`.
    Twice,
    /// `0xf3cfa`: `0xe0882`.
    TwiceEnd,
}

// Run by `screens/scene/characters.rs`.
/// Steps (resume points) of the state machine.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Characters {
    /// Screen cleared, cameras at the origin, interlude palettes (84); then the first player.
    FadedOut,
    /// After a wait, new attempt to create objects 5, 6 and 7.
    Retry5(InterludePlayer),
    Retry6(InterludePlayer),
    Retry7(InterludePlayer),
    NextWaited,
}

/// The player of the current interlude: its record (A6 of the original) and its `+3` byte (D1), which the original
/// keeps in registers during the yields.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct InterludePlayer {
    pub(crate) record: u32,
    pub(crate) byte: u8,
}

// ---------------------------------------------------------------------------------------------
// Object handlers that make the game task cede control (`actors/families/`)

// Run by `actors/families/effects/g/rockets.rs`.
/// Resumptions of the game task when the fireworks (type 80) made it wait (`hero_rockets`); the
/// context returned by the core has the object in A0.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Firework {
    /// After the four-frame wait: fade back.
    AfterWait,
    /// Return from the jump through the state table (`jsr (table,PC)`): end of the handler.
    AfterState,
}

// Run by `actors/families/set/exit_fading.rs`.
/// Where the task resumes after the fade of a SET door (return address of the original's call to 0x1460).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum DoorSet {
    /// Exit door (SET 7), change of zone (0xcb896): `exit_gate::after_fade`.
    Zone(Obj),
    /// Exit door (SET 7), next scene (0xcb9c0): `exit_gate::next_scene`.
    Scene(Obj),
    /// Passage door (SET 29, 0xd8f06): `set_b::door_b::after_fade`.
    Passage(Obj),
}

//! Phase 16 (`0xf3142`): scene exit.
//!
//! When a scene is won, the phase goes from 12 to 16: an *exit sequence* (specific to the scene,
//! 36 variants) walks the players out, shows the end-of-level screens, picks the next scene
//! ("route choice") then restarts phase 8 (entry of the next scene).
//!
//! The state block `A5-0x55ca` (A0 in the original) holds: `+0` state (0: initialisation, 2:
//! sequence), `+1` sequence number (twice the index, from the scene table `0xf3170`), `+2`
//! sequence state (step of 4), `+4` timer, `+0x10`, `+0x12`, `+0x16` objects (messages, effects).
//!
//! Each sequence is an array of states ([`St`]) run one per frame, followed by an end-of-frame "wake"
//! ([`Update`]) that updates the world except at the final state. The five *shared states* (0xf590e,
//! 0xf5938, 0xf6492, 0xf792e, 0xf7ecc: tally, bonus, character interlude, shop, fade to black)
//! live in their own modules ([`super::tally`], [`super::bonus`], [`super::characters`],
//! [`super::shop`], [`super::blackout`]); they are called like the original dispatch (`jsr (A1)`, its
//! continuation stored in the pending calls) and often cede control to the scheduler: the task then resumes at
//! a step of their module, which ends by returning to the dispatch tail of the sequence (`entry + 10`,
//! [`Step::DispatchTail`]).
//!
//! The final state of a sequence (`Leave`) prepares the next scene then yields once
//! (`0xa40`, a few frames) before ending with sound calls (`0xc0c`, `0xe0882`): the task resumes at a
//! step that finishes the work ([`Step`]: `Both`, `Sound`, `Command`, `Twice`).
//!
//! Sequences 0, 6, 20, 29, 31 and 33 have no `jsr` dispatch: their states end the
//! frame themselves (`Update::Nothing`). Sequence 27 becomes 29 when `A5+0x66` is set.

use cps2::Cps2Bus;
use super::fading;
use m68k::decode::Fetch;
use m68k::Cpu;

use super::{player, scratch, var, Ctx, PLAYERS, PLAYER_STRIDE};
use crate::dispatch;
use crate::mem::{rd16, rd8, rts, wr16, wr32, wr8, A5};
use crate::modes::Sequence as _;
use crate::objects::Obj;
use crate::tasks::table::Next;
use crate::Native;
use crate::objects::pools;

/// Scene exit state block (A0 in the original) and its fields.
const BASE: u32 = A5 - 0x55ca;
mod blk {
    pub const STATE: u32 = 0;
    pub const SEQUENCE: u32 = 1;
    pub const STEP: u32 = 2;
    pub const TIMER: u32 = 4;
    pub const OBJECT: u32 = 0x10;
    pub const MESSAGE: u32 = 0x12;
    pub const NEXT_SCENE: u32 = 0x14;
    pub const EFFECT: u32 = 0x16;
}

/// ROM tables: sequence number per scene (bytes) and branches (words, relative to the table).
const SEQUENCE_OF_SCENE: u32 = 0xf3170;
const SEQUENCE_JUMPS: u32 = 0xf31ea;

/// Sound command from the exit to the end of the game.
const SOUND_E57D6: u32 = 0xe57d6;

/// Dispatch loops of the object classes, in the order of the reduced update (without the bosses).
const CLASSES: [&dispatch::Class; 9] = [
    &dispatch::PSHL,
    &dispatch::PEFF,
    &dispatch::ENEMY,
    &dispatch::ESHL,
    &dispatch::EFFCT,
    &dispatch::SET,
    &dispatch::BOX,
    &dispatch::ITEM,
    &dispatch::SOBJ,
];

/// Player: fields written when the players leave the screen.
const WALK_OUT_STATE: u32 = 0x000c_0000;
const P_STATE_LONG: u32 = 4;
const WALK_OUT_CLEAR_BYTES: [u32; 3] = [0x66, 0x8a, 0x15c];
const P_RANK: u32 = 0x7d;
/// Player longs cleared on leaving (state `+0x24` of each).
const PLAYER_LONGS: [u32; 4] = [0x602, 0x802, 0xa02, 0xc02];

/// Jumping forbidden (`A5+0x3d`), input lock, animation flags.
const PAUSE: u32 = 0x3d;
const PHASE_ENTRY: u32 = 0x0800_0000;
const PHASE_ENDING: u32 = 0x1c00_0000;
const FLAGS: u32 = 0x14;
const FLAGS_KEEP: u16 = 0xfff1;

/// Context of a scene exit frame.
pub struct Ex<'a> {
    pub x: Ctx<'a>,
    /// Control was handed to the scheduler or to the original: no return (RTS) to perform.
    handed_off: bool,
    /// Original address of the current sequence (to cede control to it).
    entry: u32,
    /// Where the original dispatch resumes after the state (`jsr (A1)`), empty for sequence 0.
    dispatch_return: Option<Next>,
    /// The state left A0 elsewhere than on the block (`set_dispatch_a0`, world update): the original dispatch then
    /// compares byte `+2` of what A0 designates (not the sequence state) with the final state.
    a0: Option<u32>,
}

/// Original glitch: the dispatch of a scene-exit sequence compares byte `+2` of the block designated by A0 with the
/// final state, at the end of a shared state or on the return of a cession (`Step::DispatchTail`). The dispatch puts
/// A0 on the block, and A0 survives the cessions; the black screen moves it (the last object unlinked from the lists,
/// 0xf7f12), the shop puts it back on the block. The value is kept in the task table across the cessions.
pub fn set_dispatch_a0(b: &mut Cps2Bus, a0: u32) {
    crate::tasks::table::table(b).scene_exit_a0 = a0;
}

/// The A0 that the world update leaves (end of the last dispatch loop: the one past the last SOBJ slot).
fn a0_after_world() -> u32 {
    let pool = &crate::objects::pools::SOBJ;
    pool.slot(0).0.wrapping_add(pool.stride * pool.slots as u32)
}

impl<'a> Ex<'a> {
    // --- state block ----------------------------------------------------------------------------

    fn b8(&mut self, off: u32) -> u8 {
        rd8(self.x.b, BASE + off)
    }
    fn set_b8(&mut self, off: u32, v: u8) {
        wr8(self.x.b, BASE + off, v)
    }
    fn b16(&mut self, off: u32) -> u16 {
        rd16(self.x.b, BASE + off)
    }
    fn set_b16(&mut self, off: u32, v: u16) {
        wr16(self.x.b, BASE + off, v)
    }
    fn next(&mut self) {
        let step = self.b8(blk::STEP).wrapping_add(4);
        self.set_b8(blk::STEP, step);
    }
    fn set_step(&mut self, step: u8) {
        self.set_b8(blk::STEP, step);
    }
    /// Subtracts one frame from the timer; true when it reaches zero.
    fn countdown(&mut self) -> bool {
        let left = self.b16(blk::TIMER).wrapping_sub(1);
        self.set_b16(blk::TIMER, left);
        left == 0
    }

    /// Starts the fade task `entry` (delay `delay`).
    fn fade(&mut self, delay: u16, entry: u32) {
        fading::DELAY.set(self.x.b, delay);
        fading::PAGES.set(self.x.b, 15);
        crate::kernel::create_task(self.x.b, 0x10, entry);
    }

    /// Opens message `n` (0xc8312); its (16-bit) address goes to `+0x12`. Original glitch: the failure is not
    /// tested, the A1 left by the allocation is stored.
    fn message(&mut self, n: u8) {
        let o = crate::families::creators::scene_set_or_rest(self.x.b, n);
        self.set_b16(blk::MESSAGE, o.0 as u16);
    }
    fn message_done(&mut self) -> bool {
        let m = self.object(blk::MESSAGE);
        m.u8(self.x.b, 0x22) != 0
    }
    /// Object designated by a block word (`movea.w`: sign-extended address).
    fn object(&mut self, off: u32) -> Obj {
        Obj(self.b16(off) as i16 as i32 as u32 & 0xff_ffff)
    }

    /// Players in the game and alive walk out: "walk" state and indicators cleared. If `rank`,
    /// each one gets its arrival rank (`+0x7d`).
    fn release_players(&mut self, rank: bool) {
        let mut n = 0u8;
        for k in 0..4 {
            let p = PLAYERS + k * PLAYER_STRIDE;
            if rd8(self.x.b, p + player::ACTIVE) == 0 || rd16(self.x.b, p + player::HEALTH) == 0 {
                continue;
            }
            wr8(self.x.b, p + player::ACTIVE, 1);
            wr32(self.x.b, p + P_STATE_LONG, WALK_OUT_STATE);
            if rank {
                wr8(self.x.b, p + P_RANK, n);
                n += 1;
            }
            for off in WALK_OUT_CLEAR_BYTES {
                wr8(self.x.b, p + off, 0);
            }
        }
        // Original glitch: the rank count left in D5 is re-read by the handlers of the next frame (leftover D5 of the
        // game); A1 and D6 of the loop by nobody
        if rank {
            crate::tasks::table::table(self.x.b).leftovers.d5 = n as u32;
        }
    }
    /// Start of the exit: the pause is lifted, the buttons released, the players walk out.
    fn start_walk_out(&mut self, rank: bool) {
        self.x.set_v8(PAUSE, 0);
        self.x.set_v8(var::INPUT_LOCK, 0);
        crate::inputs::clear_inputs(self.x.b);
        self.release_players(rank);
    }
    /// Are all players in the game and alive in state `state` (byte `+6`)?
    fn all_in_state(&mut self, state: u8) -> bool {
        (0..4).all(|k| {
            let p = PLAYERS + k * PLAYER_STRIDE;
            rd8(self.x.b, p + player::ACTIVE) == 0
                || rd16(self.x.b, p + player::HEALTH) == 0
                || rd8(self.x.b, p + player::STATE) == state
        })
    }

    /// 0xf7ee0: transition script variables cleared.
    fn reset_transition(&mut self) {
        super::screen::clear_longs(self.x.b, super::screen::blocks::TRANSITION, 4);
    }
    /// 0xf7916: shop block cleared.
    fn reset_shop(&mut self) {
        super::screen::clear_longs(self.x.b, super::screen::blocks::SHOP, 5);
    }
    /// 0xf683c: end-screen text area cleared.
    fn clear_text(&mut self) {
        super::screen::clear_text(self.x.b);
    }

    /// Sequence or step without a rewrite: the per-scene sequence table (0xf3170) only designates rewritten sequences,
    /// whose steps stay within their states (code-mort.md, batch "exit of the Cpu").
    fn outside_table(&mut self) -> ! {
        let (sequence, step) = (self.b8(blk::SEQUENCE), self.b8(blk::STEP));
        unreachable!("scene exit: sequence {sequence:#x}, step {step:#x} without rewrite (entry {:#x})", self.entry)
    }
    /// Cedes control to the scheduler for `frames` frames; resumption happens at `resume` (otherwise after
    /// the caller).
    fn wait_frames(&mut self, frames: u8, resume: Option<Step>) {
        if let Some(back) = self.dispatch_return {
            self.x.push_return(back);
        }
        // `jsr 0xa40` (or `jmp 0xa40` without its own resume: the task will resume after the current call)
        let resume_point = match resume {
            Some(resume) => resume.into(),
            None => crate::tasks::table::pop_return(self.x.b),
        };
        crate::kernel::wait(self.x.b, frames, resume_point);
        self.handed_off = true;
    }
}

// ---------------------------------------------------------------------------------------------
// States of a sequence

/// How the exit state of a sequence ends: calls after the yield.
#[derive(Clone, Copy, PartialEq, Eq)]
enum Tail {
    /// `0xc0c` then `0xe0882` (resume `RESUME_BOTH`).
    Both,
    /// `0xc0c` alone.
    Sound,
    /// `0xe0882` alone.
    Command,
    /// `0xc0c`, two more frames, then `0xe0882`.
    Twice,
    /// None: `jmp 0xa40`, the yield returns directly to the caller of the phase.
    Jump,
}

/// Scene change on exit.
#[derive(Clone, Copy)]
enum NextScene {
    Same,
    Plus1,
    /// Next scene, unless the frame counter `A5+0x54` exceeds 0x1500: scene 10.
    Plus1UnlessLate,
    Fixed(u8),
    /// Scene stored in the block (`+0x14`).
    FromBlock,
}

/// Waiting for the end of the fade before leaving (`A5+0x48e` zero): what the state does meanwhile.
#[derive(Clone, Copy, PartialEq, Eq)]
enum WaitFade {
    No,
    /// World update and sprites (the dispatcher has nothing to add).
    World,
    /// Inputs, world update and sprites.
    InputsWorld,
    /// Nothing: the dispatcher does the end of frame.
    Nothing,
}

#[derive(Clone, Copy)]
struct Leave {
    wait_fade: WaitFade,
    /// Palettes to black before leaving (`A5+0x48e` = 15, `0x168c`).
    black: bool,
    /// RAM word cleared (sequence 1).
    clear_word: Option<u32>,
    clear_flags: bool,
    next_scene: NextScene,
    phase: u32,
    /// Sound played before the yield.
    extra_sound: Option<u32>,
    frames: u32,
    tail: Tail,
}

/// Ordinary exit: next scene unchanged, 4 frames, `0xc0c` then `0xe0882`.
const LEAVE: Leave = Leave {
    wait_fade: WaitFade::No,
    black: true,
    clear_word: None,
    clear_flags: true,
    next_scene: NextScene::Same,
    phase: PHASE_ENTRY,
    extra_sound: None,
    frames: 4,
    tail: Tail::Both,
};
const fn leave_to(scene: u8) -> St {
    St::Leave(Leave { next_scene: NextScene::Fixed(scene), ..LEAVE })
}

/// A state of a sequence.
#[derive(Clone, Copy)]
enum St {
    /// Does nothing (waits for an external event).
    Idle,
    /// State shared by several sequences (end-of-level screens).
    Shared(Shared),
    /// Clears the transition variables (`0xf7ee0`) and the shop block (`0xf7916`); timer
    /// `+4` to `timer`.
    Clear { timer: Option<u16> },
    /// Waits for the timer to end, then walks the players out; `timer`: new timer.
    WalkWait { timer: Option<u16> },
    /// Waits for the timer to end, then moves to the next state.
    TimerWait,
    /// Fade `0x14ba` (delay 2) and text area cleared (`0xf683c`), timer `timer`.
    FadeScript { timer: u16 },
    /// Waits for all players to be in state `state`, then moves to the next state with `timer`.
    PlayersThen { state: u8, timer: Option<u16> },
    /// Custom state.
    Custom(fn(&mut Ex)),
    /// Final state: prepares the next scene.
    Leave(Leave),
}

/// End of frame of the sequence, after its state (except at the final state).
#[derive(Clone, Copy, PartialEq, Eq)]
enum Update {
    /// Inputs, world, sprites, collisions.
    Full,
    /// Inputs, world, sprites.
    NoCollisions,
    /// World and sprites.
    World,
    /// Inputs, players, then the classes except BOSS and cameras, sprites.
    Reduced,
    /// Nothing: the states do the update themselves (sequence 0).
    Nothing,
}

struct Sequence {
    update: Update,
    /// State after which the end of frame is omitted.
    last: u8,
    states: &'static [St],
}

use Update::{Full, NoCollisions, Nothing, Reduced, World};
use St::Idle;

/// The shared states of the exit sequences, at their original address.
#[derive(Clone, Copy, PartialEq, Eq)]
enum Shared {
    /// 0xf590e: end-of-level tally ([`super::tally`]).
    Tally,
    /// 0xf5938: bonus screen.
    Bonus,
    /// 0xf6492: scene specific to some characters.
    Characters,
    /// 0xf792e: shop.
    Shop,
    /// 0xf7ecc: fade to black and screen reset.
    Blackout,
}
const TALLY: St = St::Shared(Shared::Tally);
const BONUS: St = St::Shared(Shared::Bonus);
const CHARACTERS: St = St::Shared(Shared::Characters);
const SHOP: St = St::Shared(Shared::Shop);
const BLACKOUT: St = St::Shared(Shared::Blackout);

const CLEAR: St = St::Clear { timer: None };
const WALK: St = St::WalkWait { timer: None };

// ---------------------------------------------------------------------------------------------
// Running a frame

pub fn phase_16(b: &mut Cps2Bus) -> bool {
    // the original dispatch works with A0 on the block
    set_dispatch_a0(b, BASE);
    let mut ex = Ex { x: Ctx::new(b), handed_off: false, entry: 0, dispatch_return: None, a0: None };
    match ex.b8(blk::STATE) {
        0 => init(&mut ex),
        2 => run_sequence(&mut ex),
        _ => {}
    }
    ex.handed_off
}

/// State 0: player lock and choice of the sequence from the scene.
fn init(ex: &mut Ex) {
    let state = ex.b8(blk::STATE).wrapping_add(2);
    ex.set_b8(blk::STATE, state);
    wr8(ex.x.b, A5 - 0x5b8e, 0);
    ex.x.set_v8(var::PLAYERS_FROZEN, 1);
    let scene = ex.x.v8(var::SCENE) as u32;
    let sequence = ex.x.b.fetch16((SEQUENCE_OF_SCENE + scene) & !1);
    let sequence = if scene & 1 == 0 { (sequence >> 8) as u8 } else { sequence as u8 };
    ex.set_b8(blk::SEQUENCE, sequence);
}

/// State 2: the chosen sequence.
fn run_sequence(ex: &mut Ex) {
    let number = ex.b8(blk::SEQUENCE) as u32;
    let jump = ex.x.prog16(SEQUENCE_JUMPS + number);
    ex.entry = SEQUENCE_JUMPS + jump as u32;
    let (sequence, dispatcher) = match number as u8 / 2 {
        // sequence 27 becomes 29 when `A5+0x66` is set; otherwise its dispatch follows this test (10 bytes)
        27 if ex.x.v8(0x66) != 0 => (&SEQ_29, None),
        27 => (&SEQ_27, Some(ex.entry + 20)),
        index => {
            let Some(s) = sequence(index) else { ex.outside_table() };
            // the original dispatch (10 bytes) calls the state with `jsr (A1)`: its stack keeps the return;
            // the sequences whose states end the frame themselves jump to the state (`jmp (A1)`)
            (s, (s.update != Nothing).then(|| ex.entry + 10))
        }
    };
    ex.dispatch_return = dispatcher.map(dispatch_tail_at);
    let step = ex.b8(blk::STEP);
    match sequence.states.get(step as usize / 4) {
        None => ex.outside_table(),
        Some(&St::Shared(shared)) => run_shared(ex, shared),
        Some(&state) => run_state(ex, state),
    }
    if !ex.handed_off {
        finish_frame(ex, sequence);
    }
}

/// A shared state, called like the original dispatch (`jsr (A1)`, A0 on the block): if it cedes control,
/// the resume will end up returning to the original dispatch; otherwise the end of frame compares byte `+2` of the
/// block designated by A0, which the state may have changed.
fn run_shared(ex: &mut Ex, shared: Shared) {
    use crate::modes::{Flow, Sequence};
    let run: fn(&mut Ctx) -> Flow = match shared {
        Shared::Tally => super::tally::run,
        Shared::Bonus => super::bonus::run,
        Shared::Characters => super::characters::run,
        Shared::Shop => super::shop::run,
        Shared::Blackout => super::blackout::run,
    };
    let Some(back) = ex.dispatch_return else { ex.outside_table() };
    set_dispatch_a0(ex.x.b, BASE);
    ex.x.push_return(back);
    if run(&mut ex.x) == Flow::Yield {
        ex.handed_off = true;
        return;
    }
    ex.x.drop_return();
    ex.a0 = Some(crate::tasks::table::table(ex.x.b).scene_exit_a0);
}

/// A state jumps to the start of the original dispatch (`bra entry`) after changing step: the state of the
/// new step runs in the same frame, called like the dispatch (A0 on the block).
fn redispatch(ex: &mut Ex, sequence: &Sequence) {
    let step = ex.b8(blk::STEP);
    ex.a0 = None;
    match sequence.states.get(step as usize / 4) {
        None => ex.outside_table(),
        Some(&St::Shared(shared)) => run_shared(ex, shared),
        Some(&state) => run_state(ex, state),
    }
}

/// End of frame: except at the final state, world update.
fn finish_frame(ex: &mut Ex, sequence: &Sequence) {
    let step = match ex.a0 {
        Some(a0) => rd8(ex.x.b, a0.wrapping_add(blk::STEP)),
        None => ex.b8(blk::STEP),
    };
    if step == sequence.last {
        return;
    }
    run_update(ex, sequence.update);
}

fn run_update(ex: &mut Ex, update: Update) {
    if update == Update::Nothing {
        return;
    }
    if update != Update::World {
        ex.x.inputs();
    }
    match update {
        Update::Reduced => {
            crate::game::update_players(ex.x.b);
            crate::game::update_player_parts(ex.x.b);
            for class in CLASSES {
                class.update_no_next(ex.x.b);
            }
            crate::sprites::build_in_game(ex.x.b);
        }
        _ => {
            ex.x.world();
            if update == Update::Full {
                crate::game::update_combat(ex.x.b);
            }
        }
    }
}

/// Waits for the timer to end, then the players walk out.
fn walk_wait(ex: &mut Ex, timer: Option<u16>) {
    if !ex.countdown() {
        return;
    }
    ex.next();
    if let Some(t) = timer {
        ex.set_b16(blk::TIMER, t);
    }
    ex.start_walk_out(false);
}

fn run_state(ex: &mut Ex, state: St) {
    match state {
        St::Idle | St::Shared(_) => {}
        St::Clear { timer } => {
            ex.next();
            if let Some(t) = timer {
                ex.set_b16(blk::TIMER, t);
            }
            ex.reset_transition();
            ex.reset_shop();
        }
        St::WalkWait { timer } => walk_wait(ex, timer),
        St::TimerWait => {
            if ex.countdown() {
                ex.next();
            }
        }
        St::FadeScript { timer } => {
            ex.next();
            ex.set_b16(blk::TIMER, timer);
            ex.fade(2, fading::RISE);
            ex.clear_text();
        }
        St::PlayersThen { state, timer } => {
            if ex.all_in_state(state) {
                ex.next();
                if let Some(t) = timer {
                    ex.set_b16(blk::TIMER, t);
                }
            }
        }
        St::Custom(f) => f(ex),
        St::Leave(leave) => do_leave(ex, &leave),
    }
}

/// Final state: the next scene is prepared, then control is yielded to the scheduler.
fn do_leave(ex: &mut Ex, leave: &Leave) {
    if leave.wait_fade != WaitFade::No && fading::PAGES.get(ex.x.b) != 0 {
        match leave.wait_fade {
            WaitFade::World => ex.x.world(),
            WaitFade::InputsWorld => {
                ex.x.inputs();
                ex.x.world();
            }
            _ => {}
        }
        if leave.wait_fade != WaitFade::Nothing {
            ex.a0 = Some(a0_after_world());
        }
        return;
    }
    ex.x.set_v8(0x42, 0);
    if leave.black {
        fading::PAGES.set(ex.x.b, 15);
        crate::palette::no_brightness(ex.x.b);
    }
    if let Some(w) = leave.clear_word {
        wr16(ex.x.b, w, 0);
    }
    for off in PLAYER_LONGS {
        wr32(ex.x.b, A5 + off, 0);
    }
    crate::sprites::clear_lists(ex.x.b);
    if leave.clear_flags {
        let flags = ex.x.v16(FLAGS) & FLAGS_KEEP;
        ex.x.set_v16(FLAGS, flags);
    }
    match leave.next_scene {
        NextScene::Same => {}
        NextScene::Plus1 => {
            let scene = ex.x.v8(var::SCENE).wrapping_add(1);
            ex.x.set_v8(var::SCENE, scene);
        }
        NextScene::Plus1UnlessLate => {
            let scene = ex.x.v8(var::SCENE).wrapping_add(1);
            ex.x.set_v8(var::SCENE, scene);
            if ex.x.v16(0x54) > 0x1500 {
                ex.x.set_v8(var::SCENE, 10);
            }
        }
        NextScene::Fixed(scene) => ex.x.set_v8(var::SCENE, scene),
        NextScene::FromBlock => {
            let scene = ex.b8(blk::NEXT_SCENE);
            ex.x.set_v8(var::SCENE, scene);
        }
    }
    ex.x.set_v32(var::PHASE, leave.phase);
    if let Some(sound) = leave.extra_sound {
        crate::sound::play_global(ex.x.b, sound);
    }
    let resume = match leave.tail {
        Tail::Both => Some(Step::Both),
        Tail::Sound => Some(Step::Sound),
        Tail::Command => Some(Step::Command),
        Tail::Twice => Some(Step::Twice),
        Tail::Jump => None,
    };
    ex.wait_frames(leave.frames as u8, resume);
}

// ---------------------------------------------------------------------------------------------
// Resumes

/// Steps of the scene exit: dispatch tails of the sequences (return of a shared state that ceded control) and
/// resumes after the yield of the final state.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Step {
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

/// Entries of the `jsr (A1)` dispatches, by sequence index (27 has its dispatch after a 10-byte test).
const DISPATCHERS: [(u8, u32); 29] = [
    (1, 0xf32d8), (2, 0xf33fe), (3, 0xf3594), (4, 0xf3686), (5, 0xf3812), (7, 0xf39f4), (8, 0xf3b18),
    (9, 0xf3c0a), (10, 0xf3d00), (11, 0xf3e3c), (12, 0xf3f2e), (13, 0xf4020), (14, 0xf4176), (15, 0xf434a),
    (16, 0xf4480), (17, 0xf455e), (18, 0xf464a), (19, 0xf4738), (21, 0xf493a), (22, 0xf4a68), (23, 0xf4b4e),
    (25, 0xf4c6e), (26, 0xf4da2), (27, 0xf4ea4), (28, 0xf4f9c), (30, 0xf5390), (32, 0xf565e), (34, 0xf52a4),
    (35, 0xf5782),
];

/// Dispatch tail at address `at` (`entry + 10`).
fn dispatch_tail_at(at: u32) -> Next {
    match DISPATCHERS.iter().find(|&&(_, entry)| entry + 10 == at) {
        Some(&(index, _)) => Step::DispatchTail(index).into(),
        None => unreachable!("scene exit: dispatch {at:#x} absent from DISPATCHERS"),
    }
}

impl Step {
    /// Address the original resumes at for this step.
    pub fn address(self) -> u32 {
        match self {
            Step::DispatchTail(index) => DISPATCHERS.iter().find(|d| d.0 == index).expect("dispatch").1 + 10,
            Step::Both => 0xf32cc,
            Step::Sound => 0xf481a,
            Step::Command => 0xf4b48,
            Step::Twice => 0xf3cec,
            Step::TwiceEnd => 0xf3cfa,
        }
    }

    pub fn step(self, x: &mut Ctx) -> crate::modes::Flow {
        match self {
            Step::DispatchTail(index) => dispatch_tail(x, index),
            Step::Both => {
                crate::video::black_screen(x.b);
                crate::sound::service(x.b, crate::sound::Service::GeneralStop);
            }
            Step::Sound => crate::video::black_screen(x.b),
            Step::Command | Step::TwiceEnd => crate::sound::service(x.b, crate::sound::Service::GeneralStop),
            Step::Twice => {
                crate::video::black_screen(x.b);
                crate::kernel::wait(x.b, 2, Step::TwiceEnd);
                return crate::modes::Flow::Yield;
            }
        }
        crate::modes::Flow::Done
    }
}

pub const PHASE_16: Native = Native {
    entry: 0xf3142,
    name: "scene_phase_16",
    run: crate::Body::WithCpu(|c, b| {
        if !phase_16(b) {
            rts(c, b);
        }
    }),
    touches_sound: false,
    outputs: 0,
};

/// End of frame of the original dispatch of a sequence (`entry + 10`: `cmpi.b #last,2(A0)`, then the
/// update), where the resumes after a yield return. A0 is the one the state left.
fn dispatch_tail(x: &mut Ctx, index: u8) {
    let sequence = if index == 27 { &SEQ_27 } else { sequence(index).expect("rewritten sequence") };
    let a0 = crate::tasks::table::table(x.b).scene_exit_a0;
    let mut ex = Ex { x: Ctx::new(x.b), handed_off: false, entry: 0, dispatch_return: None, a0: Some(a0) };
    finish_frame(&mut ex, sequence);
}

pub const NATIVES: &[Native] = &[PHASE_16];

// ---------------------------------------------------------------------------------------------
// The sequences (index = sequence number / 2; see `docs/scenes.md` for the scenes)

/// Final state without clearing the flags, 8-frame yield, only the final command afterwards.
const L_SILENT: Leave = Leave { clear_flags: false, frames: 8, tail: Tail::Command, ..LEAVE };

static SEQ_0: Sequence = Sequence {
    update: Nothing,
    last: 0xff,
    states: &[
        St::Custom(seq0_start),
        St::Leave(Leave { wait_fade: WaitFade::World, next_scene: NextScene::Plus1, ..LEAVE }),
    ],
};
static SEQ_1: Sequence = Sequence {
    update: Reduced,
    last: 24,
    states: &[
        St::Custom(walk_out_now),
        St::Custom(seq1_clear),
        Idle,
        BONUS,
        Idle,
        BLACKOUT,
        St::Leave(Leave { clear_word: Some(scratch(0xa53a)), ..LEAVE }),
    ],
};
static SEQ_2: Sequence = Sequence {
    update: Full,
    last: 40,
    states: &[
        St::Custom(seq2_start),
        St::Custom(seq2_wait_exit),
        St::Custom(seq2_message_done),
        St::Custom(seq2_boss_state),
        St::Custom(seq2_countdown),
        St::TimerWait,
        TALLY,
        BONUS,
        CHARACTERS,
        BLACKOUT,
        St::Leave(LEAVE),
    ],
};
/// Common model (sequences 3, 8, 11, 12): frame, exit, shared states, departure.
static SEQ_3: Sequence =
    Sequence { update: Full, last: 28, states: &[CLEAR, WALK, TALLY, BONUS, CHARACTERS, SHOP, BLACKOUT, St::Leave(LEAVE)] };
static SEQ_4: Sequence = Sequence {
    update: Full,
    last: 40,
    states: &[
        CLEAR,
        St::WalkWait { timer: Some(144) },
        St::Custom(seq4_probe),
        St::Custom(seq4_effect),
        St::Custom(message_then_next),
        TALLY,
        BONUS,
        CHARACTERS,
        SHOP,
        BLACKOUT,
        St::Leave(LEAVE),
    ],
};
static SEQ_5: Sequence = Sequence {
    update: World,
    last: 8,
    states: &[
        St::Clear { timer: Some(32) },
        St::Custom(walk_with_effect),
        St::Leave(Leave { wait_fade: WaitFade::World, next_scene: NextScene::Plus1UnlessLate, ..LEAVE }),
    ],
};
/// Sequence 6 (scene 7): like sequence 0, with sound command 0xe0a82(4).
static SEQ_6: Sequence = Sequence {
    update: Nothing,
    last: 0xff,
    states: &[
        St::Custom(seq6_start),
        St::Leave(Leave { wait_fade: WaitFade::World, next_scene: NextScene::Plus1, ..LEAVE }),
    ],
};
static SEQ_7: Sequence = Sequence {
    update: World,
    last: 8,
    states: &[
        St::Clear { timer: Some(32) },
        St::Custom(walk_with_effect),
        St::Leave(Leave { wait_fade: WaitFade::World, next_scene: NextScene::Plus1, ..LEAVE }),
    ],
};
static SEQ_9: Sequence = Sequence {
    update: Full,
    last: 24,
    states: &[CLEAR, WALK, TALLY, BONUS, CHARACTERS, BLACKOUT, St::Leave(Leave { frames: 2, tail: Tail::Twice, ..LEAVE })],
};
static SEQ_10: Sequence = Sequence {
    update: NoCollisions,
    last: 36,
    states: &[
        St::Custom(seq10_clear),
        St::Custom(seq10_walk),
        St::PlayersThen { state: 10, timer: Some(112) },
        Idle,
        St::TimerWait,
        TALLY,
        BONUS,
        CHARACTERS,
        BLACKOUT,
        St::Leave(LEAVE),
    ],
};
static SEQ_13: Sequence = Sequence {
    update: Full,
    last: 32,
    states: &[
        CLEAR,
        WALK,
        TALLY,
        St::FadeScript { timer: 60 },
        Idle,
        St::PlayersThen { state: 16, timer: None },
        BONUS,
        CHARACTERS,
        leave_to(106),
    ],
};
static SEQ_14: Sequence = Sequence {
    update: Full,
    last: 44,
    states: &[
        CLEAR,
        St::WalkWait { timer: Some(96) },
        TALLY,
        St::FadeScript { timer: 60 },
        St::Custom(seq14_wait_players),
        St::Custom(seq14_message_done),
        St::Custom(seq14_countdown),
        St::Custom(seq14_message_done_wait),
        St::TimerWait,
        BONUS,
        CHARACTERS,
        leave_to(27),
    ],
};
static SEQ_15: Sequence = Sequence {
    update: Full,
    last: 36,
    states: &[
        St::Custom(seq15_clear),
        WALK,
        TALLY,
        St::Custom(seq15_fade),
        Idle,
        BLACKOUT,
        BONUS,
        CHARACTERS,
        SHOP,
        St::Leave(LEAVE),
    ],
};
static SEQ_16: Sequence = Sequence {
    update: NoCollisions,
    last: 24,
    states: &[St::Custom(seq16_start), BLACKOUT, BONUS, CHARACTERS, SHOP, BLACKOUT, St::Leave(LEAVE)],
};
static SEQ_17: Sequence = Sequence {
    update: Full,
    last: 28,
    states: &[CLEAR, WALK, TALLY, BONUS, CHARACTERS, SHOP, BLACKOUT, St::Leave(Leave { tail: Tail::Sound, ..LEAVE })],
};
static SEQ_18: Sequence =
    Sequence { update: Full, last: 24, states: &[CLEAR, WALK, TALLY, BONUS, CHARACTERS, BLACKOUT, St::Leave(LEAVE)] };
static SEQ_19: Sequence = Sequence {
    update: NoCollisions,
    last: 24,
    states: &[St::Clear { timer: Some(32) }, WALK, BONUS, CHARACTERS, SHOP, BLACKOUT, St::Leave(Leave { tail: Tail::Sound, ..LEAVE })],
};
/// Sequence 20 (scenes 35 and 111): players leave after 48 more frames, fade, scene 33.
static SEQ_20: Sequence = Sequence {
    update: Nothing,
    last: 0xff,
    states: &[
        St::Custom(seq20_walk),
        St::Custom(fade_out_world),
        St::Leave(Leave { wait_fade: WaitFade::World, next_scene: NextScene::Fixed(33), ..LEAVE }),
    ],
};
static SEQ_21: Sequence = Sequence {
    update: NoCollisions,
    last: 12,
    states: &[
        St::Custom(seq21_start),
        St::Custom(seq21_route),
        St::Leave(Leave { wait_fade: WaitFade::Nothing, next_scene: NextScene::FromBlock, ..LEAVE }),
    ],
};
static SEQ_22: Sequence =
    Sequence { update: Full, last: 28, states: &[CLEAR, WALK, TALLY, BONUS, CHARACTERS, SHOP, BLACKOUT, St::Leave(L_SILENT)] };
static SEQ_23: Sequence = Sequence {
    update: NoCollisions,
    last: 8,
    states: &[
        St::Clear { timer: Some(128) },
        St::Custom(seq23_walk),
        St::Leave(Leave { wait_fade: WaitFade::InputsWorld, next_scene: NextScene::Fixed(44), ..LEAVE }),
    ],
};
static SEQ_25: Sequence = Sequence {
    update: Full,
    last: 36,
    states: &[CLEAR, WALK, TALLY, St::Custom(seq25_route), St::TimerWait, BONUS, CHARACTERS, SHOP, BLACKOUT, St::Leave(L_SILENT)],
};
static SEQ_26: Sequence =
    Sequence { update: Full, last: 28, states: &[CLEAR, WALK, TALLY, BLACKOUT, BONUS, CHARACTERS, SHOP, leave_to(38)] };
/// Sequence 27 (scene 54), when `A5+0x66` is zero: tally, bonus, scene 55.
static SEQ_27: Sequence =
    Sequence { update: Full, last: 16, states: &[St::Custom(seq27_clear), WALK, TALLY, BONUS, leave_to(55)] };
static SEQ_28: Sequence =
    Sequence { update: Full, last: 24, states: &[St::Custom(seq28_clear), WALK, TALLY, BONUS, CHARACTERS, SHOP, leave_to(59)] };
/// Sequence 29 (scene 59, and 54 when `A5+0x66` is set): exit and fade, next scene.
static SEQ_29: Sequence = Sequence {
    update: Nothing,
    last: 0xff,
    states: &[
        St::Custom(seq29_start),
        St::Custom(seq29_walk),
        St::Leave(Leave { wait_fade: WaitFade::InputsWorld, next_scene: NextScene::Plus1, ..LEAVE }),
    ],
};
static SEQ_30: Sequence = Sequence {
    update: Full,
    last: 52,
    states: &[
        CLEAR,
        St::WalkWait { timer: Some(96) },
        TALLY,
        St::FadeScript { timer: 60 },
        St::PlayersThen { state: 12, timer: Some(64) },
        St::Custom(seq30_message_79),
        St::Custom(seq30_message_done),
        St::Custom(seq30_effect),
        St::Custom(seq30_players_wait),
        St::TimerWait,
        BONUS,
        CHARACTERS,
        SHOP,
        leave_to(65),
    ],
};
/// Sequence 31 (scene 65): fade after 192 frames, next scene without black palettes or sound.
static SEQ_31: Sequence = Sequence {
    update: Nothing,
    last: 0xff,
    states: &[
        St::Custom(seq31_start),
        St::Custom(fade_out_world),
        St::Leave(Leave {
            wait_fade: WaitFade::World,
            black: false,
            clear_flags: false,
            next_scene: NextScene::Plus1,
            tail: Tail::Jump,
            ..LEAVE
        }),
    ],
};
static SEQ_32: Sequence = Sequence {
    update: Full,
    last: 24,
    states: &[St::Custom(seq32_clear), WALK, TALLY, BONUS, St::Custom(seq32_route), CHARACTERS, leave_to(105)],
};
/// Sequence 33 (scene 67): players leave and fade, scene 38.
static SEQ_33: Sequence = Sequence {
    update: Nothing,
    last: 0xff,
    states: &[
        St::Custom(seq33_start),
        St::Custom(seq33_walk),
        St::Leave(Leave { wait_fade: WaitFade::World, next_scene: NextScene::Fixed(38), tail: Tail::Sound, ..LEAVE }),
    ],
};
static SEQ_34: Sequence = Sequence {
    update: NoCollisions,
    last: 28,
    states: &[
        St::Clear { timer: Some(4) },
        WALK,
        SHOP,
        BONUS,
        CHARACTERS,
        SHOP,
        BLACKOUT,
        St::Leave(Leave { tail: Tail::Sound, ..LEAVE }),
    ],
};
static SEQ_35: Sequence = Sequence {
    update: Full,
    last: 20,
    states: &[
        St::Custom(seq35_clear),
        St::Custom(seq35_walk),
        BLACKOUT,
        TALLY,
        St::Custom(seq35_fade),
        St::Custom(seq35_message),
        St::Custom(seq35_boss),
        St::Leave(Leave { phase: PHASE_ENDING, extra_sound: Some(SOUND_E57D6), ..LEAVE }),
    ],
};

/// Rewritten sequence of index `index`; `None`: ceded to the original.
fn sequence(index: u8) -> Option<&'static Sequence> {
    Some(match index {
        0 => &SEQ_0,
        1 => &SEQ_1,
        2 => &SEQ_2,
        3 | 8 | 11 | 12 => &SEQ_3,
        4 => &SEQ_4,
        5 => &SEQ_5,
        6 => &SEQ_6,
        7 => &SEQ_7,
        9 => &SEQ_9,
        10 => &SEQ_10,
        13 => &SEQ_13,
        14 => &SEQ_14,
        15 => &SEQ_15,
        16 => &SEQ_16,
        17 => &SEQ_17,
        18 => &SEQ_18,
        19 => &SEQ_19,
        20 => &SEQ_20,
        21 => &SEQ_21,
        22 | 24 => &SEQ_22,
        23 => &SEQ_23,
        25 => &SEQ_25,
        26 => &SEQ_26,
        28 => &SEQ_28,
        29 => &SEQ_29,
        30 => &SEQ_30,
        31 => &SEQ_31,
        32 => &SEQ_32,
        33 => &SEQ_33,
        34 => &SEQ_34,
        35 => &SEQ_35,
        _ => return None,
    })
}

// ---------------------------------------------------------------------------------------------
// Custom states

/// Sequence 0 (the most common): players leave, fade.
fn seq0_start(ex: &mut Ex) {
    ex.next();
    ex.fade(2, fading::DESCENT);
    ex.x.set_v8(var::INPUT_LOCK, 0);
    crate::inputs::clear_inputs(ex.x.b);
    ex.x.world();
}

/// Sequence 6: like `seq0_start`, with sound command 0xe0a82(4).
fn seq6_start(ex: &mut Ex) {
    ex.next();
    ex.fade(2, fading::DESCENT);
    ex.x.set_v8(var::INPUT_LOCK, 0);
    crate::inputs::clear_inputs(ex.x.b);
    crate::sound::service(ex.x.b, crate::sound::Service::Ff07 { lock: 0, line: 4 });
    ex.x.world();
}

/// Sequence 20: players leave at the end of the timer (48 more frames); full update.
fn seq20_walk(ex: &mut Ex) {
    walk_wait(ex, Some(48));
    run_update(ex, Update::Full);
}

/// Sequences 20 and 31: exit fade at the end of the timer; the world goes on.
fn fade_out_world(ex: &mut Ex) {
    if ex.countdown() {
        ex.next();
        ex.fade(2, fading::DESCENT);
    }
    ex.x.world();
}

/// Sequence 27: camera, timer + 90, transition variables and shop block cleared.
fn seq27_clear(ex: &mut Ex) {
    ex.next();
    ex.x.set_v16(0x51e, 4);
    let t = ex.b16(blk::TIMER).wrapping_add(90);
    ex.set_b16(blk::TIMER, t);
    ex.reset_transition();
    ex.reset_shop();
}

/// Sequence 29: timer + 80, transition variables cleared, then the wait in the same frame.
fn seq29_start(ex: &mut Ex) {
    ex.next();
    let t = ex.b16(blk::TIMER).wrapping_add(80);
    ex.set_b16(blk::TIMER, t);
    ex.reset_transition();
    seq29_walk(ex);
}

/// Sequence 29: at the end of the timer, players leave and the fade starts; full update.
fn seq29_walk(ex: &mut Ex) {
    if ex.countdown() {
        ex.next();
        ex.start_walk_out(false);
        ex.fade(2, fading::DESCENT);
    }
    run_update(ex, Update::Full);
}

/// Sequence 31: timer 192, inputs released.
fn seq31_start(ex: &mut Ex) {
    ex.next();
    ex.set_b16(blk::TIMER, 192);
    ex.x.set_v8(var::INPUT_LOCK, 0);
    crate::inputs::clear_inputs(ex.x.b);
    ex.x.world();
}

/// Sequence 33: transition variables and shop block cleared, then the wait in the same frame.
fn seq33_start(ex: &mut Ex) {
    ex.next();
    ex.reset_transition();
    ex.reset_shop();
    seq33_walk(ex);
}

/// Sequence 33: at the end of the timer, exit fade then players leave.
fn seq33_walk(ex: &mut Ex) {
    if ex.countdown() {
        ex.next();
        ex.x.set_v8(PAUSE, 0);
        ex.x.set_v8(var::INPUT_LOCK, 0);
        crate::inputs::clear_inputs(ex.x.b);
        ex.fade(2, fading::DESCENT);
        ex.release_players(false);
    }
    ex.x.world();
}

/// Sequence 10: transition variables cleared, timer 128.
fn seq10_clear(ex: &mut Ex) {
    ex.next();
    ex.set_b16(blk::TIMER, 128);
    ex.reset_transition();
}

/// Sequence 32: depending on the route (`A5+0x5c`), the exit or the bonus screen 0xf6492.
fn seq32_route(ex: &mut Ex) {
    if ex.x.v8(0x5c) != 0 {
        ex.set_step(24);
        if let Some(St::Leave(leave)) = SEQ_32.states.get(6) {
            do_leave(ex, leave);
        }
    } else {
        ex.set_step(20);
        redispatch(ex, &SEQ_32);
    }
}

/// Sequence 1: players leave immediately.
fn walk_out_now(ex: &mut Ex) {
    ex.next();
    ex.start_walk_out(false);
}

/// Sequence 1: transition variables cleared, then event 0xdc5fe(1).
fn seq1_clear(ex: &mut Ex) {
    ex.next();
    ex.reset_transition();
    crate::families::creators::scene_object(ex.x.b, 1);
}

/// Sequence 2: the timer (word) is decremented once, then the players leave if there are any.
fn seq2_start(ex: &mut Ex) {
    // the timer (word) starts at 0: it is decremented as long as it stays positive
    let left = ex.b16(blk::TIMER).wrapping_sub(1);
    ex.set_b16(blk::TIMER, left);
    if left as i16 >= 0 || ex.x.v8(var::PLAYERS) == 0 {
        return;
    }
    ex.next();
    ex.start_walk_out(false);
}

/// Sequence 2: waits for all players to be in exit state 0xc0c00, then dialogue 7.
fn seq2_wait_exit(ex: &mut Ex) {
    ex.reset_transition();
    for k in 0..4 {
        let p = PLAYERS + k * PLAYER_STRIDE;
        if rd8(ex.x.b, p) != 0 && rd16(ex.x.b, p + player::HEALTH) != 0 && crate::mem::rd32(ex.x.b, p + P_STATE_LONG) != 0x000c_0c00 {
            return;
        }
    }
    ex.next();
    ex.message(7);
}

/// Sequence 2: message done, the boss of object `+0x10` goes to state 0x04020a00.
fn seq2_message_done(ex: &mut Ex) {
    if ex.message_done() {
        ex.next();
        let o = ex.object(blk::OBJECT);
        wr32(ex.x.b, o.0 + 4, 0x0402_0a00);
        ex.set_b16(blk::TIMER, 240);
    }
}

fn seq2_boss_state(ex: &mut Ex) {
    let o = ex.object(blk::OBJECT);
    if o.u8(ex.x.b, 4) == 6 {
        ex.next();
        ex.set_b16(blk::TIMER, 160);
    }
}

fn seq2_countdown(ex: &mut Ex) {
    if ex.countdown() {
        ex.next();
        ex.set_b16(blk::TIMER, 128);
        crate::camera_modes::start_shake(ex.x.b, crate::camera_modes::shake::B, 30);
        crate::sound::play_global(ex.x.b, 0xe4830);
    }
}

/// Sequence 4: after the exit, an effect appears; if a player is in state 4 or 5, dialogue 33.
fn seq4_probe(ex: &mut Ex) {
    if !ex.countdown() {
        return;
    }
    ex.next();
    for k in 0..4 {
        let p = PLAYERS + k * PLAYER_STRIDE;
        if rd8(ex.x.b, p) != 0 && rd16(ex.x.b, p + player::HEALTH) != 0 {
            let character = rd8(ex.x.b, p + 3);
            if character == 4 || character == 5 {
                ex.next();
                ex.message(33);
                return;
            }
        }
    }
    let effect = ex.x.spawn(&pools::EFFCT, 0x59);
    ex.set_b16(blk::EFFECT, effect.0 as u16);
}

fn seq4_effect(ex: &mut Ex) {
    let e = ex.object(blk::EFFECT);
    if e.u8(ex.x.b, 5) == 2 {
        ex.next();
        ex.message(33);
    }
}

fn message_then_next(ex: &mut Ex) {
    if ex.message_done() {
        ex.next();
    }
}

/// Sequences 5 and 7: players leave, effect `A5+0x46` receives 0x20000 at `+0x58`, exit fade.
fn walk_with_effect(ex: &mut Ex) {
    if !ex.countdown() {
        return;
    }
    ex.next();
    ex.x.set_v8(PAUSE, 0);
    ex.x.set_v8(var::INPUT_LOCK, 0);
    crate::inputs::clear_inputs(ex.x.b);
    ex.set_b16(blk::TIMER, 144);
    ex.release_players(false);
    let effect = Obj(ex.x.v16(0x46) as i16 as i32 as u32 & 0xff_ffff);
    wr32(ex.x.b, effect.0 + 0x58, 0x2_0000);
    ex.fade(1, fading::DESCENT);
}

/// Sequence 10: players receive their rank then 0xd5f12.
fn seq10_walk(ex: &mut Ex) {
    if !ex.countdown() {
        return;
    }
    ex.next();
    ex.start_walk_out(true);
    crate::families::set_residue::clear_columns(ex.x.b);
}

/// Sequence 14: waits for state 12 of the players, then dialogue 35.
fn seq14_wait_players(ex: &mut Ex) {
    if ex.all_in_state(12) {
        ex.next();
        ex.message(35);
    }
}

fn seq14_message_done(ex: &mut Ex) {
    if ex.message_done() {
        ex.next();
        ex.set_b16(blk::TIMER, 256);
        crate::video::clear_layer1_start(ex.x.b);
    }
}

fn seq14_countdown(ex: &mut Ex) {
    if ex.countdown() {
        ex.next();
        ex.message(36);
        crate::sound::play_global(ex.x.b, 0xe61ca);
    }
}

fn seq14_message_done_wait(ex: &mut Ex) {
    if ex.message_done() {
        ex.next();
        ex.set_b16(blk::TIMER, 288);
    }
}

/// Sequence 15: clears the temporary variable A5-0x65ac, resets transition and shop.
fn seq15_clear(ex: &mut Ex) {
    ex.next();
    ex.x.wr16(scratch(0x9a54), 0);
    ex.reset_transition();
    ex.reset_shop();
}

/// Sequence 15: fade, fade script, event 0xdc5fe(3).
fn seq15_fade(ex: &mut Ex) {
    ex.next();
    ex.fade(2, fading::RISE);
    ex.clear_text();
    crate::families::creators::scene_object(ex.x.b, 3);
}

/// Sequence 16: transition and shop cleared, players leave.
fn seq16_start(ex: &mut Ex) {
    ex.next();
    ex.reset_transition();
    ex.reset_shop();
    ex.start_walk_out(false);
}

/// Sequence 21: players leave, dialogue 70; the next scene depends on the result.
fn seq21_start(ex: &mut Ex) {
    ex.next();
    ex.x.set_v8(PAUSE, 0);
    ex.x.set_v8(var::INPUT_LOCK, 0);
    crate::inputs::clear_inputs(ex.x.b);
    ex.reset_transition();
    ex.reset_shop();
    ex.message(70);
    ex.release_players(false);
}

/// Sequence 21: the finished message gives an index (`+0x23`) into a table of next scenes.
fn seq21_route(ex: &mut Ex) {
    const NEXT_SCENES: u32 = 0xf4a0c;
    let m = ex.object(blk::MESSAGE);
    if m.u8(ex.x.b, 0x22) != 0 {
        ex.next();
        let index = m.u8(ex.x.b, 0x23) as u32;
        let scene = ex.x.prog16((NEXT_SCENES + index) & !1);
        let scene = if (NEXT_SCENES + index) & 1 == 0 { (scene >> 8) as u8 } else { scene as u8 };
        ex.set_b8(blk::NEXT_SCENE, scene);
        ex.fade(1, fading::DESCENT);
    }
}

/// Sequence 23: players leave then exit fade.
fn seq23_walk(ex: &mut Ex) {
    if !ex.countdown() {
        return;
    }
    ex.next();
    ex.start_walk_out(false);
    ex.fade(2, fading::DESCENT);
}

/// Sequence 25: depending on the route (`A5+0x5c`), jumps to state 20 (bonus screen) or fades.
fn seq25_route(ex: &mut Ex) {
    if ex.x.v8(0x5c) == 0 {
        ex.set_step(20);
        return redispatch(ex, &SEQ_25);
    }
    ex.next();
    ex.set_b16(blk::TIMER, 128);
    ex.fade(2, fading::RISE);
    ex.clear_text();
}

/// Sequence 28: like `Clear`, the timer increases by 90.
fn seq28_clear(ex: &mut Ex) {
    ex.next();
    let t = ex.b16(blk::TIMER).wrapping_add(90);
    ex.set_b16(blk::TIMER, t);
    ex.reset_transition();
    ex.reset_shop();
}

/// Sequence 30: series of dialogues (79, 80) around an effect 75 and the exit.
fn seq30_message_79(ex: &mut Ex) {
    if ex.countdown() {
        ex.next();
        ex.message(79);
    }
}

fn seq30_message_done(ex: &mut Ex) {
    if ex.message_done() {
        ex.next();
        ex.set_b16(blk::TIMER, 80);
    }
}

fn seq30_effect(ex: &mut Ex) {
    if ex.b16(blk::TIMER) == 32 {
        let e = ex.x.spawn(&pools::EFFCT, 75);
        e.set_u8(ex.x.b, Obj::VARIANT, 1);
        crate::sound::play_global(ex.x.b, 0xed0c0);
    }
    if ex.countdown() {
        ex.next();
        ex.message(80);
    }
}

fn seq30_players_wait(ex: &mut Ex) {
    if ex.message_done() {
        ex.next();
        let players = ex.x.v8(var::PLAYERS) as u32;
        let t = ex.x.prog16(0xf5542 + 2 * players);
        ex.set_b16(blk::TIMER, t);
    }
}

/// Sequence 32: clears, draws; if the route is taken (`A5+0x5c`) jumps to state 12.
fn seq32_clear(ex: &mut Ex) {
    ex.next();
    ex.reset_transition();
    ex.reset_shop();
    if ex.x.v8(0x5c) != 0 {
        ex.set_step(12);
        return redispatch(ex, &SEQ_32);
    }
    walk_wait(ex, None);
}

/// Sequence 35: transition variables cleared, then players leave after the wait.
fn seq35_clear(ex: &mut Ex) {
    ex.next();
    ex.reset_transition();
    seq35_walk(ex);
}

fn seq35_walk(ex: &mut Ex) {
    if !ex.countdown() {
        return;
    }
    ex.next();
    ex.x.set_v8(PAUSE, 0);
    ex.x.wr16(scratch(0xaa84), 0x1400);
    ex.x.set_v8(var::INPUT_LOCK, 0);
    crate::inputs::clear_inputs(ex.x.b);
    ex.release_players(false);
}

fn seq35_fade(ex: &mut Ex) {
    ex.next();
    ex.set_b16(blk::TIMER, 64);
    ex.fade(2, fading::RISE);
    ex.clear_text();
    crate::sound::service(ex.x.b, crate::sound::Service::Ff07 { lock: 0, line: 4 });
}

fn seq35_message(ex: &mut Ex) {
    if !ex.countdown() {
        return;
    }
    ex.next();
    ex.message(88);
    // Original glitch: the failure is not tested, the A1 left by the allocation is stored
    let set = crate::families::creators::set_15_pure(ex.x.b, 24).unwrap_or_else(|a1| a1);
    ex.set_b16(blk::OBJECT, set.0 as u16);
    crate::camera_modes::start_shake(ex.x.b, crate::camera_modes::shake::B, 0x7fff);
    crate::sound::play_global(ex.x.b, 0xe5776);
}

fn seq35_boss(ex: &mut Ex) {
    if ex.message_done() {
        ex.next();
        let o = ex.object(blk::OBJECT);
        o.set_u8(ex.x.b, 4, 4);
        ex.x.set_v16(0x544, 1);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn block_and_temp_var_addresses() {
        assert_eq!(BASE, 0xff_2a36);
        assert_eq!(scratch(0x9a2a), 0xff_1a2a);
    }

    /// Each sequence has a final state that is a multiple of 4 (or 0xff) and ends with an exit state.
    #[test]
    fn consistent_sequences() {
        for index in 0..36 {
            let Some(s) = sequence(index) else { continue };
            assert!(s.last % 4 == 0 || s.last == 0xff, "sequence {index}");
            assert!(matches!(s.states.last(), Some(St::Leave(_)) | Some(St::Custom(_))), "sequence {index}");
        }
    }
}

/// Shared state 0xf792e (end-of-level screen): after the message, 0xf7b62 picks the continuation from byte `+0xc` of
/// its block (A6) via a 20-byte table (0xf7b88). For demo scenes 109 to 114, the byte is outside
/// the table and the original jumps to an address read from code: it crashes and restarts. Outside the table: no
/// continuation (zero table byte, branch at 0xf792c). Same fix in the original code (reference machine).
pub const SHARED_NEXT_ENTRY: u32 = 0xf7b62;
const SHARED_NEXT_TABLE_LEN: u8 = 20;
const SHARED_NEXT_NONE: u32 = 0xf792c;

pub fn patch_shared_next(c: &mut Cpu, b: &mut Cps2Bus) -> bool {
    if rd8(b, c.a[6] + 0xc) < SHARED_NEXT_TABLE_LEN {
        return false;
    }
    wr16(b, c.a[6] + 0x12, 0x65);
    c.d[0] &= 0xffff_0000;
    c.a[1] = 0xf7b88;
    c.pc = SHARED_NEXT_NONE;
    true
}

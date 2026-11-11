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

use crate::world::World;
use super::fading;

use super::{player, scratch, var, Ctx, PLAYERS, PLAYER_STRIDE};
use crate::actors::dispatch;
use crate::hw::mem::A5;
use crate::world::tasks::Sequence as _;
use crate::world::objects::Obj;
use crate::world::tasks::table::Next;
pub use crate::world::tasks::stages::Exit as Step;
use crate::Native;
use crate::world::objects::pools;

mod sequences;
mod custom;
use sequences::*;
use custom::*;

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
const SEQUENCE_OF_SCENE: crate::world::screen_tables::Rom = crate::world::screen_tables::Rom::at(0xf3170);
const SEQUENCE_JUMPS: crate::world::screen_tables::Rom = crate::world::screen_tables::Rom::at(0xf31ea);

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
const WALK_OUT_STATE: u32 = u32::from_be_bytes([0, 0x0c, 0, 0]);
const P_STATE_LONG: u32 = 4;
const WALK_OUT_CLEAR_BYTES: [u32; 3] = [0x66, 0x8a, 0x15c];
const P_RANK: u32 = 0x7d;

/// Jumping forbidden (`A5+0x3d`), input lock, animation flags.
const PAUSE: Byte = Byte::a5(0x3d);
const PHASE_ENTRY: u32 = 0x0800_0000;
const PHASE_ENDING: u32 = 0x1c00_0000;
const FLAGS: Word = Word::a5(0x14);
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
pub fn set_dispatch_a0(b: &mut World, a0: u32) {
    crate::world::tasks::table::table(b).scene_exit_a0 = a0;
}

/// The A0 that the world update leaves (end of the last dispatch loop: the one past the last SOBJ slot).
fn a0_after_world() -> u32 {
    let pool = &crate::world::objects::pools::SOBJ;
    pool.slot(0).0.wrapping_add(pool.stride * pool.slots)
}

impl<'a> Ex<'a> {
    // --- state block ----------------------------------------------------------------------------

    fn b8(&mut self, off: u32) -> u8 {
        Obj(BASE).u8(self.x.b, off)
    }
    fn set_b8(&mut self, off: u32, v: u8) {
        Obj(BASE).set_u8(self.x.b, off, v)
    }
    fn b16(&mut self, off: u32) -> u16 {
        Obj(BASE).u16(self.x.b, off)
    }
    fn set_b16(&mut self, off: u32, v: u16) {
        Obj(BASE).set_u16(self.x.b, off, v)
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

    /// Starts the fade task `task` (delay `delay`).
    fn fade(&mut self, delay: u16, task: crate::world::tasks::Task) {
        fading::DELAY.set(self.x.b, delay);
        fading::PAGES.set(self.x.b, 15);
        crate::world::tasks::traps::create_task(self.x.b, 0x10, task);
    }

    /// Opens message `n` (0xc8312); its (16-bit) address goes to `+0x12`. Original glitch: the failure is not
    /// tested, the A1 left by the allocation is stored.
    fn message(&mut self, n: u8) {
        let o = crate::actors::families::creators::scene_set_or_rest(self.x.b, n);
        self.set_b16(blk::MESSAGE, o.0 as u16);
    }
    fn message_done(&mut self) -> bool {
        let m = self.object(blk::MESSAGE);
        m.u8(self.x.b, crate::screens::obj::MESSAGE_DONE) != 0
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
            if Obj(p).u8(self.x.b, player::ACTIVE) == 0 || Obj(p).u16(self.x.b, player::HEALTH) == 0 {
                continue;
            }
            Obj(p).set_u8(self.x.b, player::ACTIVE, 1);
            Obj(p).set_u32(self.x.b, P_STATE_LONG, WALK_OUT_STATE);
            if rank {
                Obj(p).set_u8(self.x.b, P_RANK, n);
                n += 1;
            }
            for off in WALK_OUT_CLEAR_BYTES {
                Obj(p).set_u8(self.x.b, off, 0);
            }
        }
        // Original glitch: the rank count left in D5 is re-read by the handlers of the next frame (leftover D5 of the
        // game); A1 and D6 of the loop by nobody
        if rank {
            self.x.b.leftovers.d5 = n as u32;
        }
    }
    /// Start of the exit: the pause is lifted, the buttons released, the players walk out.
    fn start_walk_out(&mut self, rank: bool) {
        PAUSE.set(self.x.b, 0);
        var::INPUT_LOCK.set(self.x.b, 0);
        crate::services::inputs::clear_inputs(self.x.b);
        self.release_players(rank);
    }
    /// Are all players in the game and alive in state `state` (byte `+6`)?
    fn all_in_state(&mut self, state: u8) -> bool {
        (0..4).all(|k| {
            let p = PLAYERS + k * PLAYER_STRIDE;
            Obj(p).u8(self.x.b, player::ACTIVE) == 0
                || Obj(p).u16(self.x.b, player::HEALTH) == 0
                || Obj(p).u8(self.x.b, player::STATE) == state
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
            None => crate::world::tasks::table::pop_return(self.x.b),
        };
        crate::world::tasks::traps::wait(self.x.b, frames, resume_point);
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
pub(crate) struct Leave {
    wait_fade: WaitFade,
    /// Palettes to black before leaving (`A5+0x48e` = 15, `0x168c`).
    black: bool,
    /// RAM word cleared (sequence 1).
    clear_word: Option<u32>,
    clear_flags: bool,
    next_scene: NextScene,
    phase: u32,
    /// Sound played before the yield.
    extra_sound: Option<crate::services::sound::Sound>,
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
pub(crate) enum St {
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
pub(crate) enum Update {
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

use Update::{Full, NoCollisions, Nothing, Reduced};
use St::Idle;
use crate::world::ram::{Byte, Word};

/// The shared states of the exit sequences, at their original address.
#[derive(Clone, Copy, PartialEq, Eq)]
pub(crate) enum Shared {
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

pub fn phase_16(b: &mut World) -> bool {
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
    crate::screens::hud::var::FROZEN.set(ex.x.b, 0);
    var::PLAYERS_FROZEN.set(ex.x.b, 1);
    let scene = var::SCENE.get(ex.x.b) as u32;
    let sequence = SEQUENCE_OF_SCENE.pc_byte(ex.x.b, scene);
    ex.set_b8(blk::SEQUENCE, sequence);
}

/// State 2: the chosen sequence.
fn run_sequence(ex: &mut Ex) {
    let number = ex.b8(blk::SEQUENCE) as u32;
    let jump = SEQUENCE_JUMPS.pc_word(ex.x.b, number);
    ex.entry = SEQUENCE_JUMPS.addr() + jump as u32;
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
    use crate::world::tasks::{Flow, Sequence};
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
    ex.a0 = Some(crate::world::tasks::table::table(ex.x.b).scene_exit_a0);
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
        Some(a0) => Obj(a0).u8(ex.x.b, blk::STEP),
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
            crate::actors::update::update_players(ex.x.b);
            crate::actors::update::update_player_parts(ex.x.b);
            for class in CLASSES {
                class.update_no_next(ex.x.b);
            }
            crate::services::sprites::build_in_game(ex.x.b);
        }
        _ => {
            ex.x.world();
            if update == Update::Full {
                crate::actors::update::update_combat(ex.x.b);
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
        crate::services::palette::no_brightness(ex.x.b);
    }
    if let Some(w) = leave.clear_word {
        Word(w).set(ex.x.b, 0);
    }
    for k in 0..4 {
        Obj(crate::world::ram::PLAYERS.nth(k).0).set_u32(ex.x.b, crate::world::objects::Obj::PREV, 0);
    }
    crate::services::sprites::clear_lists(ex.x.b);
    if leave.clear_flags {
        let flags = FLAGS.get(ex.x.b) & FLAGS_KEEP;
        FLAGS.set(ex.x.b, flags);
    }
    match leave.next_scene {
        NextScene::Same => {}
        NextScene::Plus1 => {
            let scene = var::SCENE.get(ex.x.b).wrapping_add(1);
            var::SCENE.set(ex.x.b, scene);
        }
        NextScene::Plus1UnlessLate => {
            let scene = var::SCENE.get(ex.x.b).wrapping_add(1);
            var::SCENE.set(ex.x.b, scene);
            if ex.x.v16(0x54) > 0x1500 {
                var::SCENE.set(ex.x.b, 10);
            }
        }
        NextScene::Fixed(scene) => var::SCENE.set(ex.x.b, scene),
        NextScene::FromBlock => {
            let scene = ex.b8(blk::NEXT_SCENE);
            var::SCENE.set(ex.x.b, scene);
        }
    }
    ex.x.set_v32(var::PHASE, leave.phase);
    if let Some(sound) = leave.extra_sound {
        crate::services::sound::play_global(ex.x.b, sound);
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

    pub fn step(self, x: &mut Ctx) -> crate::world::tasks::Flow {
        match self {
            Step::DispatchTail(index) => dispatch_tail(x, index),
            Step::Both => {
                crate::services::video::black_screen(x.b);
                crate::services::sound::service(x.b, crate::services::sound::Service::GeneralStop);
            }
            Step::Sound => crate::services::video::black_screen(x.b),
            Step::Command | Step::TwiceEnd => crate::services::sound::service(x.b, crate::services::sound::Service::GeneralStop),
            Step::Twice => {
                crate::services::video::black_screen(x.b);
                crate::world::tasks::traps::wait(x.b, 2, Step::TwiceEnd);
                return crate::world::tasks::Flow::Yield;
            }
        }
        crate::world::tasks::Flow::Done
    }
}

/// End of frame of the original dispatch of a sequence (`entry + 10`: `cmpi.b #last,2(A0)`, then the
/// update), where the resumes after a yield return. A0 is the one the state left.
fn dispatch_tail(x: &mut Ctx, index: u8) {
    let sequence = if index == 27 { &SEQ_27 } else { sequence(index).expect("rewritten sequence") };
    let a0 = crate::world::tasks::table::table(x.b).scene_exit_a0;
    let mut ex = Ex { x: Ctx::new(x.b), handed_off: false, entry: 0, dispatch_return: None, a0: Some(a0) };
    finish_frame(&mut ex, sequence);
}

pub const NATIVES: &[Native] = &[];

/// Shared state 0xf792e (end-of-level screen): after the message, 0xf7b62 picks the continuation from byte `+0xc` of
/// its block (A6) via a 20-byte table (0xf7b88). For demo scenes 109 to 114, the byte is outside
/// the table and the original jumps to an address read from code: it crashes and restarts. Outside the table: no
/// continuation (zero table byte, branch at 0xf792c). Same fix in the original code (reference machine).
pub const SHARED_NEXT_ENTRY: crate::world::data::RomAddr = crate::world::data::RomAddr(0xf7b62);

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

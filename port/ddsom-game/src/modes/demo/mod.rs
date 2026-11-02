//! Attract demonstrations: the two scene-script interpreters of the mode machine
//! (sub-modes 4 and 14: `0x1f32e`; sub-mode 10: `0x20926`).
//!
//! A *script* (in ROM, long pointer at `FF0EA2`) is a sequence of 32-byte entries:
//!
//! | Byte | Content |
//! |---|---|
//! | `+0` | operation: `0x80..` end of script, `0` scene, `2` sound command then scene (2 more bytes), other: character presentation (3 bytes) |
//! | `+1` | scene number (`A5+0x34`) |
//! | `+2` | duration in frames (word) |
//! | `+4` | four 6-byte players: character (negative: absent), variant, three optional parameters |
//! | `+28` | a long copied to `A5-0x5aa8` |
//!
//! A demonstration scene runs the game itself: on each frame the interpreter calls the
//! current phase handler (8: scene entry; 12: the game, driven by the AI) until the end of the
//! duration. Screen steps (`FF0E72`): 0 script start, 2 scene running, 6 character presentation.

use cps2::Cps2Bus;
use crate::scene::fading;

use super::{mode, Block, Flow, Sequence};
use crate::effects_aids::scenery_6;
use crate::families::creators::set_15_pure;
use crate::objects::Obj;
use crate::video::{clear_layer, Layer};
use crate::mem::{rd16, rd32, rd8, wr16, wr32, wr8, A5};
use crate::objects::pools;
use crate::scene::Ctx;
use crate::Native;

mod presentation;
mod recorder;
use presentation::*;
use recorder::*;
pub use recorder::{reset_recorder, stop_recorder};
pub(crate) use recorder::reset_recorder_by;

/// Fields of the mode machine state block.
mod f {
    pub const SUB: u32 = 1;
    pub const STEP: u32 = 2;
    /// Sub-state of the character presentation (byte, step of 2).
    pub const SUBSTEP: u32 = 3;
    pub const TIMER: u32 = 8;
    pub const PRESENT_9: u32 = 9;
    /// Link (word) to the portrait object.
    pub const LINK: u32 = 0x10;
    /// Pointeur de script (long).
    pub const SCRIPT: u32 = 0x32;
    /// End of script requested (interpreter B).
    pub const ENDING: u32 = 0x3a;
    pub const PRESENT_KIND: u32 = 0x3c;
    pub const PRESENT_CHARACTER: u32 = 0x3d;
    /// Alternates between the two scripts (0 / 4).
    pub const ALTERNATE: u32 = 0x3e;
}

/// Work RAM variables (absolute addresses).
mod var {
    use crate::mem::A5;
    pub const SCREEN_WAIT: u32 = A5 - 0x7155;
    pub const YIELDED: u32 = A5 - 0x660e;
    pub const SKIP_YIELD: u32 = A5 - 0x65d8;
    pub const SAVED_TIME: u32 = A5 - 0x65ea;
    pub const SAVED_FRAMES: u32 = A5 - 0x65e8;
    pub const SAVED_RANK: u32 = A5 - 0x65e6;
    pub const SCRIPT_TAIL: u32 = A5 - 0x5aa8;
    pub const PLAYERS_PRESENT: u32 = A5 + 0x35;
    pub const SCENE: u32 = A5 + 0x34;
    pub const PHASE: u32 = A5 + 0x2c;
    pub const STEP: u32 = A5 + 0x2d;
    pub const VIDEO: u32 = A5 + 0x14;
    pub const VIDEO_FLAGS_B: u32 = A5 + 0x13;
    pub const FRAME_COUNTER: u32 = A5 + 0x6e;
    pub const DEMO_TIME: u32 = A5 + 0x6c;
    pub const RANK: u32 = A5 + 0x7f92;
    /// Game in progress; settings word.
    pub const GAME: u32 = A5 + 0x30;
    pub const DSW: u32 = A5 + 0x7fd0;
}

mod ext {
    /// Player interface task.
    pub const PLAYER_UI: u32 = 0x150ac;
}

/// ROM tables (data, or addresses read by the original code relative to the PC).
mod table {
    /// Pointers of the two starting scripts, alternating (longs, read through the PC).
    pub const SCRIPTS: u32 = 0x1f348;
    /// Sound command routines (longs, byte index).
    pub const COMMANDS: u32 = 0x1f3aa;
    /// Phase handlers of loop A and loop B.
    pub const PHASES_A: u32 = 0x1f990;
    pub const PHASES_B: u32 = 0x20a9c;
    /// Script of interpreter B.
    pub const SCRIPT_B: u32 = 0x1f756;
    /// Presentation tiles (two words per character), frame, texts.
    pub const TILES: u32 = 0x1fd04;
    pub const PRESENT_DATA: u32 = 0x1febe;
    pub const FRAME_ATTR: u32 = 0x1ffa2;
    pub const BOX_ATTR: u32 = 0x1ffd0;
    pub const TEXT_1: u32 = 0x20070;
    pub const TEXT_2: u32 = 0x200ca;
}

/// Resume and return addresses of the original.
mod at {
    pub const RETURN_A: u32 = 0x1f33a;
    pub const LOOP_A: u32 = 0x1f932;
    pub const AFTER_YIELD_A: u32 = 0x1f962;
    pub const AFTER_PHASE_A: u32 = 0x1f98c;
    pub const RETURN_B: u32 = 0x20932;
    pub const LOOP_B: u32 = 0x209d4;
    pub const AFTER_YIELD_B: u32 = 0x20a04;
    pub const AFTER_PHASE_B: u32 = 0x20a98;
}

const PLAYERS: u32 = A5 + 0x5de;
const PLAYER_STRIDE: u32 = 0x200;

/// Screen of sub-mode `sub`, if it is rewritten here.
pub fn screen(x: &mut Ctx, sub: u8) -> Option<Flow> {
    match sub {
        4 | 14 => Some(interpreter_a(x)),
        10 => Some(interpreter_b(x)),
        _ => None,
    }
}

fn prog32(x: &mut Ctx, addr: u32) -> u32 {
    (x.prog16(addr) as u32) << 16 | x.prog16(addr + 2) as u32
}

fn rd_state(x: &mut Ctx, off: u32) -> u8 {
    rd8(x.b, mode::BASE + off)
}

// ---------------------------------------------------------------------------------------------
// Interpreter A (`0x1f32e`)

fn interpreter_a(x: &mut Ctx) -> Flow {
    let step = rd_state(x, f::STEP);
    x.push_return(Step::ReturnA);
    match step {
        0 => start_a(x),
        2 => {
            crate::tasks::prompt::launch(x.b);
            loop_a(x)
        }
        6 => presentation_step(x),
        // step 4 (0x1fb20, end of scene): never written, the loop leads there directly (`scene_end_a`); beyond:
        // outside table 0x1f340 (code-mort.md, batch "CPU removal")
        _ => unreachable!("demo A: step {step:#x} never produced"),
    }
}
crate::steps! {
    /// Steps (resume points) of the state machine.
    pub enum Step {
        ReturnA = at::RETURN_A => |_| Flow::Done,
        LoopA = at::LOOP_A => loop_a,
        AfterYieldA = at::AFTER_YIELD_A => after_yield_a,
        SceneEndClearA = SCENE_END_CLEAR_A => |x| scene_end_clear(x, Step::SceneEndNextA),
        SceneEndNextA = SCENE_END_NEXT_A => next_operation,
        EndClear = END_CLEAR => |x| {
            let v = rd16(x.b, var::VIDEO) & !0xe;
            wr16(x.b, var::VIDEO, v);
            clear_layers(x, &[Layer::One, Layer::Two, Layer::Three]);
            clear_players_and_pools(x);
            x.wait(4, Step::EndShow)
        },
        EndShow = END_SHOW => |x| {
            let v = rd16(x.b, var::VIDEO) | 0xe;
            wr16(x.b, var::VIDEO, v);
            Flow::Done
        },
        /// The jump from `0x1fce4` to `0x1fc68` leaves the `bsr` address on the stack: the terrain is rendered twice.
        PresentAgain = 0x1fdba => p_render,
        P4Hide = P4_HIDE => |x| {
            let v = rd16(x.b, var::VIDEO) & !0xe;
            wr16(x.b, var::VIDEO, v);
            x.wait(1, Step::P4Clear)
        },
        P4Clear = P4_CLEAR => |x| {
            clear_layers(x, &[Layer::One, Layer::Two, Layer::Three]);
            next_operation(x)
        },
        ReturnB = at::RETURN_B => |_| Flow::Done,
        LoopB = at::LOOP_B => loop_b,
        AfterYieldB = at::AFTER_YIELD_B => after_yield_b,
        SceneNextClearB = SCENE_NEXT_CLEAR_B => |x| scene_end_clear(x, Step::SceneNextNextB),
        SceneNextNextB = SCENE_NEXT_NEXT_B => next_operation_b,
        EndingReset = ENDING_RESET => |x| {
            clear_lists_and_objects(x);
            x.wait(1, Step::EndingClear)
        },
        EndingClear = ENDING_CLEAR => |x| {
            let v = rd16(x.b, var::VIDEO) & !0xc;
            wr16(x.b, var::VIDEO, v);
            clear_layers(x, &[Layer::Two, Layer::Three]);
            clear_players_and_pools(x);
            x.wait(4, Step::EndingShow)
        },
        EndingShow = ENDING_SHOW => |x| {
            let v = rd16(x.b, var::VIDEO) | 0xc;
            wr16(x.b, var::VIDEO, v);
            crate::sound::service(x.b, crate::sound::Service::Ff05);
            Flow::Done
        },
        /// Phase returns (`jsr (A0)` of the two loops): a phase may cede control, then come back here.
        AfterPhaseA = at::AFTER_PHASE_A => after_phase,
        AfterPhaseB = at::AFTER_PHASE_B => after_phase,
        /// Return from the world update in demo phase 12 (`jsr 0x1c86e` at `0x1fa44`), when
        /// an object handler made the task cede control.
        AfterWorld = 0x1fa4a => demo_after_world,
    }
}

/// Step 0: chooses one of the two scripts (alternately), keeps the demonstration clock.
/// Cleared layers (0xca8, 0xcc8, 0xce8).
fn clear_layers(x: &mut Ctx, layers: &[Layer]) {
    for &layer in layers {
        clear_layer(x.b, layer);
    }
}

/// Players cleared (0x2d9e) and pools reset (0x2062).
fn clear_players_and_pools(x: &mut Ctx) {
    crate::player::init::clear_all_players(x.b);
    for pool in pools::POOLS {
        pool.reset(x.b);
    }
}

/// Lists undone and emptied, objects cleared (0x1f184, 0x1de7c, 0xc58).
fn clear_lists_and_objects(x: &mut Ctx) {
    crate::objects::lists::unlink_all(x.b);
    crate::sprites::clear_lists(x.b);
    crate::video::clear_objects(x.b);
}

fn start_a(x: &mut Ctx) -> Flow {
    crate::tasks::prompt::stop(x.b);
    x.next_step();
    let which = rd_state(x, f::ALTERNATE);
    wr8(x.b, mode::BASE + f::ALTERNATE, which ^ 4);
    let script = prog32(x, table::SCRIPTS + which as u32);
    wr32(x.b, mode::BASE + f::SCRIPT, script);
    let t = rd16(x.b, var::DEMO_TIME);
    wr16(x.b, var::SAVED_TIME, t);
    let t = rd16(x.b, var::FRAME_COUNTER);
    wr16(x.b, var::SAVED_FRAMES, t);
    next_operation(x)
}

/// `0x1f376`: reads the next script operation.
fn next_operation(x: &mut Ctx) -> Flow {
    wr8(x.b, mode::BASE + 3, 0);
    let script = rd32(x.b, mode::BASE + f::SCRIPT);
    let op = rd8(x.b, script);
    if op & 0x80 != 0 {
        return end_of_script(x);
    }
    match op {
        0 => scene_start(x, script + 1),
        2 => {
            // sound command: the next byte indexes the routine table (0x1f3aa: 0xe0ef8, 0xe0f70, 0xe0f34,
            // 0xe1024), called by `jsr (A6)`
            let index = rd8(x.b, script + 1) as u32;
            let routine = prog32(x, table::COMMANDS + index);
            crate::sound::play_global(x.b, crate::sound::Sound::at(routine));
            scene_start(x, script + 3)
        }
        _ => presentation(x, script + 1),
    }
}

/// `0x1f818`: start of a demonstration scene from the script (`a1`: after the operation).
fn scene_start(x: &mut Ctx, mut a1: u32) -> Flow {
    wr8(x.b, mode::BASE + f::STEP, 2);
    let scene = rd8(x.b, a1);
    wr8(x.b, var::SCENE, scene);
    let duration = rd16(x.b, a1 + 1);
    wr16(x.b, mode::BASE + f::TIMER, duration);
    a1 += 3;
    wr16(x.b, var::FRAME_COUNTER, 0);
    wr16(x.b, var::DEMO_TIME, 0);
    let rank = rd16(x.b, var::RANK);
    wr16(x.b, var::SAVED_RANK, rank);
    wr16(x.b, var::RANK, 0xe);
    wr8(x.b, var::PLAYERS_PRESENT, 0);
    wr8(x.b, var::PHASE, 8);
    wr8(x.b, var::STEP, 0);
    crate::rng::seed(x.b);
    let mut a0 = PLAYERS;
    for d6 in (0..4u32).rev() {
        if rd8(x.b, a1) & 0x80 == 0 {
            let character = rd8(x.b, a1);
            wr8(x.b, a0 + 3, character);
            let variant = rd8(x.b, a1 + 1);
            wr8(x.b, a0 + 0x15d, variant);
            // the routine of table 0x1f8a8 (indexed by the counter, counting down) is the player's initialisation
            // of that number (0x2e58...: `player::init`), D6/A0/A1 kept around the call (`movem.l`)
            crate::player::init::initialize(x.b, 3 - d6);
            apply_record(x, a0, a1);
        }
        a1 += 6;
        a0 += PLAYER_STRIDE;
    }
    let v = rd32(x.b, a1);
    wr32(x.b, var::SCRIPT_TAIL, v);
    a1 += 4;
    wr32(x.b, mode::BASE + f::SCRIPT, a1);
    wr8(x.b, A5 + 0x31, 0);
    wr8(x.b, var::SCREEN_WAIT, 3);
    crate::kernel::create_task(x.b, 0x30, ext::PLAYER_UI);
    Flow::Done
}

/// `0x1f8b8`: optional parameters of a demonstration player (negative byte: not supplied).
fn apply_record(x: &mut Ctx, a0: u32, a1: u32) {
    let b2 = rd8(x.b, a1 + 2);
    if b2 & 0x80 == 0 {
        wr8(x.b, a0 + 0x10d, b2);
    }
    let b3 = rd8(x.b, a1 + 3);
    if b3 & 0x80 == 0 {
        wr8(x.b, a0 + 0x10e, b3);
    }
    let b4 = rd8(x.b, a1 + 4);
    if b4 & 0x80 == 0 {
        let slot = rd8(x.b, a1 + 1) as u32;
        wr8(x.b, a0 + 0x120 + ((slot << 4) & 0xffff), b4);
        for i in 0..10 {
            wr8(x.b, a0 + 0x150 + i, 9);
        }
    }
}

// --- the scene runs ---------------------------------------------------------------------------

/// `0x1f932`: one frame of the demonstration. The game runs two steps per frame: one step without yielding
/// when the frame has just started (`SKIP_YIELD` <= 1), one step after the yield.
fn loop_a(x: &mut Ctx) -> Flow {
    let n = rd16(x.b, var::FRAME_COUNTER).wrapping_add(1);
    wr16(x.b, var::FRAME_COUNTER, n);
    let skip = rd8(x.b, var::SKIP_YIELD) as i8;
    if rd8(x.b, var::SCREEN_WAIT) != 0 || rd8(x.b, var::YIELDED) != 0 || skip > 1 {
        wr8(x.b, var::SKIP_YIELD, 0);
        return x.wait(1, Step::AfterYieldA);
    }
    wr8(x.b, var::SKIP_YIELD, skip.wrapping_add(3) as u8);
    after_yield_a(x)
}

fn after_yield_a(x: &mut Ctx) -> Flow {
    if rd8(x.b, var::PHASE) == 0xc {
        let left = rd16(x.b, mode::BASE + f::TIMER).wrapping_sub(1);
        wr16(x.b, mode::BASE + f::TIMER, left);
        if left == 0 {
            return scene_end_a(x);
        }
    }
    run_phase(x, table::PHASES_A, Step::LoopA, Step::AfterPhaseA)
}

/// Calls the current phase handler (`jsr (A0)` preceded by the loop's `pea`): it returns to `after`
/// (`after_phase`), which returns to the top of the loop; a phase may also return directly to the top of the loop,
/// popping the return of its call.
fn run_phase(x: &mut Ctx, phases: u32, loop_top: Step, after: Step) -> Flow {
    wr8(x.b, var::YIELDED, 0xff);
    x.push_return(loop_top);
    x.push_return(after);
    let phase = rd8(x.b, var::PHASE) as u32;
    // Original glitch: A0 stays the phase handler address (`jsr (A0)`); scene entry re-reads it as
    // object creator (`scene::entry`: finale, effects 0x13)
    let handler = prog32(x, phases + phase);
    // the two tables only have phases 8 (scene entry) and 12 (the game); the other entries are null
    // (code-mort.md, batch "CPU removal")
    match phase {
        8 => {
            crate::scene::entry::phase_8(x.b, handler);
            Flow::Done
        }
        12 => demo_phase_12(x),
        _ => unreachable!("demo: phase {phase:#x} without handler (table {phases:#x})"),
    }
}

/// `0x1f9b0`: after the phase, handles the loading screen and empties the collision queues; the `rts`
/// goes back to the top of the loop.
fn after_phase(x: &mut Ctx) -> Flow {
    let wait = rd8(x.b, var::SCREEN_WAIT);
    if wait == 0 {
        let v = rd16(x.b, var::VIDEO) | 0xe;
        wr16(x.b, var::VIDEO, v);
    } else {
        let left = wait - 1;
        wr8(x.b, var::SCREEN_WAIT, left);
        if left != 0 {
            let v = rd16(x.b, var::VIDEO) & !0xe;
            wr16(x.b, var::VIDEO, v);
            let longs = [0, 0x1800_0000, 0, 0x1800_0000, 0x8000_8000, 0];
            for (i, l) in longs.into_iter().enumerate() {
                wr32(x.b, 0x708000 + 4 * i as u32, l);
            }
        }
    }
    reset_queues(x);
    Flow::Done
}

/// The collision queues restart empty (like `game::FRAME_END`).
fn reset_queues(x: &mut Ctx) {
    for queue in [&pools::QUEUE_PSHL, &pools::QUEUE_ESHL, &pools::QUEUE_ENEMY_BOSS, &pools::QUEUE_BOX, &pools::QUEUE_ITEM] {
        queue.reset(x.b);
    }
}

// --- end of a scene, end of the script -------------------------------------------------------------

/// Reset common to the end of a scene or a script.
fn scene_cleanup_1(x: &mut Ctx) {
    wr8(x.b, A5 + 0x3d, 0);
    let v = rd8(x.b, var::VIDEO_FLAGS_B) & !1;
    wr8(x.b, var::VIDEO_FLAGS_B, v);
    wr8(x.b, A5 + 0x53c, 0);
}
/// `0x1fb20`: duration elapsed: everything is cleared, then the next operation.
fn scene_end_a(x: &mut Ctx) -> Flow {
    let rank = rd16(x.b, var::SAVED_RANK);
    wr16(x.b, var::RANK, rank);
    scene_cleanup_1(x);
    wr8(x.b, A5 + 0x50, 0);
    scene_end_common(x, Step::SceneEndClearA)
}
/// Chains the common cleanup then the yield of one frame whose resume is `resume`.
fn scene_end_common(x: &mut Ctx, resume: Step) -> Flow {
    stop_recorder(x.b);
    wr8(x.b, var::PHASE, 0);
    wr8(x.b, var::PLAYERS_PRESENT, 0);
    crate::kernel::kill_task(x.b, 0x30);
    clear_lists_and_objects(x);
    x.wait(1, resume)
}

/// After the yield: clears the planes and the queues, yields again, then reads the next operation.
fn scene_end_clear(x: &mut Ctx, resume: Step) -> Flow {
    let v = rd16(x.b, var::VIDEO) & !0xe;
    wr16(x.b, var::VIDEO, v);
    clear_layers(x, &[Layer::One, Layer::Two, Layer::Three]);
    clear_players_and_pools(x);
    reset_queues(x);
    x.wait(1, resume)
}
const SCENE_END_CLEAR_A: u32 = 0x1fb6a;
const SCENE_END_NEXT_A: u32 = 0x1fbd2;

/// `0x1fa92`: end of script: return of the demonstration clock, everything cleared, next screen.
fn end_of_script(x: &mut Ctx) -> Flow {
    let s = rd8(x.b, mode::BASE + f::SUB).wrapping_add(2);
    wr8(x.b, mode::BASE + f::SUB, s);
    restore_clock(x);
    scene_cleanup_1(x);
    stop_recorder(x.b);
    wr8(x.b, var::PHASE, 0);
    wr16(x.b, mode::BASE + f::STEP, 0);
    wr8(x.b, var::PLAYERS_PRESENT, 0);
    crate::kernel::kill_task(x.b, 0x30);
    clear_lists_and_objects(x);
    x.wait(1, Step::EndClear)
}
fn restore_clock(x: &mut Ctx) {
    let t = rd16(x.b, var::SAVED_TIME);
    wr16(x.b, var::DEMO_TIME, t);
    let t = rd16(x.b, var::SAVED_FRAMES);
    wr16(x.b, var::FRAME_COUNTER, t);
    let rank = rd16(x.b, var::SAVED_RANK);
    wr16(x.b, var::RANK, rank);
}
const END_CLEAR: u32 = 0x1faec;
const END_SHOW: u32 = 0x1fb18;

// ---------------------------------------------------------------------------------------------
// Interpreter B (`0x20926`)

fn interpreter_b(x: &mut Ctx) -> Flow {
    let step = rd_state(x, f::STEP);
    x.push_return(Step::ReturnB);
    match step {
        0 => {
            crate::tasks::prompt::stop(x.b);
            x.next_step();
            wr32(x.b, mode::BASE + f::SCRIPT, table::SCRIPT_B);
            let t = rd16(x.b, var::DEMO_TIME);
            wr16(x.b, var::SAVED_TIME, t);
            let t = rd16(x.b, var::FRAME_COUNTER);
            wr16(x.b, var::SAVED_FRAMES, t);
            wr8(x.b, mode::BASE + f::ENDING, 0);
            next_operation_b(x)
        }
        2 => {
            crate::tasks::prompt::launch(x.b);
            loop_b(x)
        }
        4 => scene_next_b(x),
        _ => Flow::Done,
    }
}

/// `0x20960`: next operation: end of script (negative byte) or scene.
fn next_operation_b(x: &mut Ctx) -> Flow {
    wr8(x.b, mode::BASE + 3, 0);
    let script = rd32(x.b, mode::BASE + f::SCRIPT);
    let op = rd8(x.b, script);
    if op & 0x80 != 0 {
        return end_of_script(x);
    }
    scene_start(x, script + 1)
}

fn loop_b(x: &mut Ctx) -> Flow {
    let n = rd16(x.b, var::FRAME_COUNTER).wrapping_add(1);
    wr16(x.b, var::FRAME_COUNTER, n);
    let skip = rd8(x.b, var::SKIP_YIELD) as i8;
    if rd8(x.b, var::SCREEN_WAIT) != 0 || rd8(x.b, var::YIELDED) != 0 || skip > 1 {
        wr8(x.b, var::SKIP_YIELD, 0);
        return x.wait(1, Step::AfterYieldB);
    }
    wr8(x.b, var::SKIP_YIELD, skip.wrapping_add(3) as u8);
    after_yield_b(x)
}

fn after_yield_b(x: &mut Ctx) -> Flow {
    if rd8(x.b, var::SCENE) == 0x6f || rd8(x.b, var::PHASE) == 0xc {
        let timer = rd16(x.b, mode::BASE + f::TIMER);
        if timer == 0 {
            let ending = rd_state(x, f::ENDING);
            if ending & 0x80 != 0 {
                return ending_b(x);
            }
            if ending == 0 && fading::STEP.get(x.b) == 0 {
                return scene_next_b(x);
            }
        } else {
            let left = timer - 1;
            wr16(x.b, mode::BASE + f::TIMER, left);
            if left == 0 {
                let script = rd32(x.b, mode::BASE + f::SCRIPT);
                if rd16(x.b, script) == 0xffff {
                    wr8(x.b, mode::BASE + f::ENDING, 1);
                    let _ = scenery_6(x.b, 8);
                } else {
                    fading::DELAY.set(x.b, 2);
                    fading::PAGES.set(x.b, 15);
                    crate::kernel::create_task(x.b, 0x10, crate::scene::fading::DESCENT);
                    crate::sound::service(x.b, crate::sound::Service::Ff07 { lock: 0, line: 0 });
                }
            }
        }
    }
    run_phase(x, table::PHASES_B, Step::LoopB, Step::AfterPhaseB)
}

/// `0x20abc`: next scene: like `0x1fb20` but also clears `A5+0x59`.
fn scene_next_b(x: &mut Ctx) -> Flow {
    let rank = rd16(x.b, var::SAVED_RANK);
    wr16(x.b, var::RANK, rank);
    scene_cleanup_1(x);
    wr8(x.b, A5 + 0x59, 0);
    wr8(x.b, A5 + 0x50, 0);
    scene_end_common(x, Step::SceneNextClearB)
}
const SCENE_NEXT_CLEAR_B: u32 = 0x20b0a;
const SCENE_NEXT_NEXT_B: u32 = 0x20b72;

/// `0x20b86`: end of the game demonstration: end screen.
fn ending_b(x: &mut Ctx) -> Flow {
    let s = rd8(x.b, mode::BASE + f::SUB).wrapping_add(2);
    wr8(x.b, mode::BASE + f::SUB, s);
    restore_clock(x);
    scene_cleanup_1(x);
    stop_recorder(x.b);
    wr8(x.b, var::PHASE, 0);
    wr16(x.b, mode::BASE + f::STEP, 0);
    wr8(x.b, var::PLAYERS_PRESENT, 0);
    crate::kernel::kill_task(x.b, 0x30);
    clear_layer(x.b, Layer::One);
    crate::video::clear_layer1_start(x.b);
    wr16(x.b, 0x91c41c, 0xf112);
    wr16(x.b, A5 + 0x4ba, 256);
    wr16(x.b, A5 + 0x4b6, 0);
    x.wait(1, Step::EndingReset)
}
const ENDING_RESET: u32 = 0x20bec;
const ENDING_CLEAR: u32 = 0x20c06;
const ENDING_SHOW: u32 = 0x20c2c;

// ---------------------------------------------------------------------------------------------
// Phase 12 of the demonstration (`0x1fa38`)

/// `0x1fa38`: AI inputs, then the world update (an object handler may make the task cede control there; it
/// will resume at [`Step::AfterWorld`]).
pub(crate) fn demo_phase_12(x: &mut Ctx) -> Flow {
    // Original glitch: D2-D4 of the input copy (0x2aec) are re-read by handlers (see game.rs)
    crate::inputs::copy_in_game(x.b);
    demo_inputs(x);
    x.push_return(Step::AfterWorld);
    if crate::game::update_world(x.b) == Flow::Yield {
        return Flow::Yield;
    }
    x.drop_return();
    demo_after_world(x)
}

/// `0x1fa4a`: like game phase 12 (`game::after_world`); the original ends with `jmp 0x28d8` (the end of
/// the game, then its `rts`).
fn demo_after_world(x: &mut Ctx) -> Flow {
    crate::game::after_world(x.b);
    Flow::Done
}

pub const NATIVES: &[Native] = &[
    
    
    RECORDER_STOP,
];

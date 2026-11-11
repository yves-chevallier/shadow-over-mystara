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

use crate::hw::video::{palette, OBJECTS};
use crate::world::ram::Word;
use crate::world::screen_tables::{self as tables, Rom};
use crate::screens::vars as sv;
use crate::world::World;
use crate::screens::scene::fading;

use super::{mode, Block, Flow, Sequence};
use crate::actors::effects_aids::scenery_6;
use crate::actors::families::creators::set_15_pure;
use crate::world::objects::Obj;
use crate::services::video::{clear_layer, Layer};
use crate::world::objects::pools;
use crate::screens::scene::Ctx;
use crate::Native;

mod presentation;
mod recorder;
use presentation::*;
use recorder::*;
pub use recorder::{reset_recorder, stop_recorder};
#[cfg(feature = "tools")]
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
    /// Word set to 300 at the end of a presentation (h).
    pub const WORD_30: u32 = 0x30;
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
    use crate::world::ram::{Byte, Long, Word};
    
    pub const SCREEN_WAIT: Byte = Byte(0xff_0eab);
    pub const YIELDED: Byte = Byte(0xff_19f2);
    pub const SKIP_YIELD: Byte = Byte(0xff_1a28);
    pub const SAVED_TIME: Word = Word(0xff_1a16);
    pub const SAVED_FRAMES: Word = Word(0xff_1a18);
    pub const SAVED_RANK: Word = Word(0xff_1a1a);
    pub const SCRIPT_TAIL: Long = Long(0xff_2558);
    pub const PLAYERS_PRESENT: Byte = Byte::a5(0x35);
    pub const SCENE: Byte = Byte::a5(0x34);
    pub const PHASE: Byte = Byte::a5(0x2c);
    pub const STEP: Byte = Byte::a5(0x2d);
    pub const VIDEO: Word = Word::a5(0x14);
    pub const VIDEO_FLAGS_B: Byte = Byte::a5(0x13);
    pub const FRAME_COUNTER: Word = Word::a5(0x6e);
    pub const DEMO_TIME: Word = Word::a5(0x6c);
    pub const RANK: Word = Word::a5(0x7f92);
    /// Game in progress; settings word.
    pub const GAME: Byte = Byte::a5(0x30);
    pub const DSW: Word = Word::a5(0x7fd0);
}

mod ext {
    /// Player interface task.
    pub const PLAYER_UI: crate::core::tasks::Task = crate::core::tasks::Task::Interface;
}

/// ROM tables (data, or addresses read by the original code relative to the PC).
mod table {
    use crate::world::screen_tables::Rom;
    /// Pointers of the two starting scripts, alternating (longs, read through the PC).
    pub const SCRIPTS: Rom = Rom::at(0x1f348);
    /// Sound command routines (longs, byte index).
    pub const COMMANDS: Rom = Rom::at(0x1f3aa);
    /// Phase handlers of loop A and loop B.
    pub const PHASES_A: Rom = Rom::at(0x1f990);
    pub const PHASES_B: Rom = Rom::at(0x20a9c);
    /// Script of interpreter B.
    pub const SCRIPT_B: Rom = Rom::at(0x1f756);
    /// Presentation tiles (two words per character), palettes, data per character, frame, texts.
    pub const TILES: Rom = Rom::at(0x1fd04);
    pub const PRESENT_PALETTES: Rom = Rom::at(0x1fe3e);
    pub const PRESENT_DATA: Rom = Rom::at(0x1febe);
    pub const FRAME_ATTR: Rom = Rom::at(0x1ffa2);
    pub const BOX_ATTR: Rom = Rom::at(0x1ffd0);
    pub const TEXT_1: Rom = Rom::at(0x20070);
    pub const TEXT_2: Rom = Rom::at(0x200ca);
}

/// Player fields set by a demonstration script: weapons (`+0x10d`, `+0x10e`), selection rows and their entries.
mod player {
    pub const WEAPON_A: u32 = 0x10d;
    pub const WEAPON_B: u32 = 0x10e;
    pub const SELECTIONS: u32 = 0x120;
    pub const SELECTION_ROWS: u32 = 0x150;
    pub const SELECTION_KIND: u32 = 0x15d;
}

/// Work RAM bytes cleared between the demonstration scenes: jump forbidden (`A5+0x3d`), boss row (`A5+0x50`),
/// `A5+0x59` (h), players who started (`A5+0x31`).
mod clear {
    use crate::world::ram::Byte;
    pub const NO_JUMP: Byte = Byte::a5(0x3d);
    pub const BOSS_ROW: Byte = Byte::a5(0x50);
    pub const BYTE_59: Byte = Byte::a5(0x59);
    pub const STARTED: Byte = Byte::a5(0x31);
}

/// Screen of sub-mode `sub`, if it is rewritten here.
pub fn screen(x: &mut Ctx, sub: u8) -> Option<Flow> {
    match sub {
        4 | 14 => Some(interpreter_a(x)),
        10 => Some(interpreter_b(x)),
        _ => None,
    }
}

fn rd_state(x: &mut Ctx, off: u32) -> u8 {
    Obj(mode::BASE).u8(x.b, off)
}

// ---------------------------------------------------------------------------------------------
// Interpreter A (`0x1f32e`)

fn interpreter_a(x: &mut Ctx) -> Flow {
    let step = rd_state(x, f::STEP);
    x.push_return(Step::ReturnA);
    match step {
        0 => start_a(x),
        2 => {
            crate::core::tasks::prompt::launch(x.b);
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
        ReturnA = 0x1f33a => |_| Flow::Done,
        LoopA = 0x1f932 => loop_a,
        AfterYieldA = 0x1f962 => after_yield_a,
        SceneEndClearA = 0x1fb6a => |x| scene_end_clear(x, Step::SceneEndNextA),
        SceneEndNextA = 0x1fbd2 => next_operation,
        EndClear = 0x1faec => |x| {
            let v = var::VIDEO.get(x.b) & !0xe;
            var::VIDEO.set(x.b, v);
            clear_layers(x, &[Layer::One, Layer::Two, Layer::Three]);
            clear_players_and_pools(x);
            x.wait(4, Step::EndShow)
        },
        EndShow = 0x1fb18 => |x| {
            let v = var::VIDEO.get(x.b) | 0xe;
            var::VIDEO.set(x.b, v);
            Flow::Done
        },
        /// The jump from `0x1fce4` to `0x1fc68` leaves the `bsr` address on the stack: the terrain is rendered twice.
        PresentAgain = 0x1fdba => p_render,
        P4Hide = 0x1ff5e => |x| {
            let v = var::VIDEO.get(x.b) & !0xe;
            var::VIDEO.set(x.b, v);
            x.wait(1, Step::P4Clear)
        },
        P4Clear = 0x1ff6c => |x| {
            clear_layers(x, &[Layer::One, Layer::Two, Layer::Three]);
            next_operation(x)
        },
        ReturnB = 0x20932 => |_| Flow::Done,
        LoopB = 0x209d4 => loop_b,
        AfterYieldB = 0x20a04 => after_yield_b,
        SceneNextClearB = 0x20b0a => |x| scene_end_clear(x, Step::SceneNextNextB),
        SceneNextNextB = 0x20b72 => next_operation_b,
        EndingReset = 0x20bec => |x| {
            clear_lists_and_objects(x);
            x.wait(1, Step::EndingClear)
        },
        EndingClear = 0x20c06 => |x| {
            let v = var::VIDEO.get(x.b) & !0xc;
            var::VIDEO.set(x.b, v);
            clear_layers(x, &[Layer::Two, Layer::Three]);
            clear_players_and_pools(x);
            x.wait(4, Step::EndingShow)
        },
        EndingShow = 0x20c2c => |x| {
            let v = var::VIDEO.get(x.b) | 0xc;
            var::VIDEO.set(x.b, v);
            crate::services::sound::service(x.b, crate::services::sound::Service::Ff05);
            Flow::Done
        },
        /// Phase returns (`jsr (A0)` of the two loops): a phase may cede control, then come back here.
        AfterPhaseA = 0x1f98c => after_phase,
        AfterPhaseB = 0x20a98 => after_phase,
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
    crate::actors::player::init::clear_all_players(x.b);
    for pool in pools::POOLS {
        pool.reset(x.b);
    }
}

/// Lists undone and emptied, objects cleared (0x1f184, 0x1de7c, 0xc58).
fn clear_lists_and_objects(x: &mut Ctx) {
    crate::world::objects::lists::unlink_all(x.b);
    crate::services::sprites::clear_lists(x.b);
    crate::services::video::clear_objects(x.b);
}

fn start_a(x: &mut Ctx) -> Flow {
    crate::core::tasks::prompt::stop(x.b);
    x.next_step();
    let which = rd_state(x, f::ALTERNATE);
    Obj(mode::BASE).set_u8(x.b, f::ALTERNATE, which ^ 4);
    let script = table::SCRIPTS.pc_long(x.b, which as u32);
    Obj(mode::BASE).set_u32(x.b, f::SCRIPT, script);
    let t = var::DEMO_TIME.get(x.b);
    var::SAVED_TIME.set(x.b, t);
    let t = var::FRAME_COUNTER.get(x.b);
    var::SAVED_FRAMES.set(x.b, t);
    next_operation(x)
}

/// `0x1f376`: reads the next script operation.
fn next_operation(x: &mut Ctx) -> Flow {
    Obj(mode::BASE).set_u8(x.b, f::SUBSTEP, 0);
    let script = Obj(mode::BASE).u32(x.b, f::SCRIPT);
    let op = tables::byte(x.b, script);
    if op & 0x80 != 0 {
        return end_of_script(x);
    }
    match op {
        0 => scene_start(x, script + 1),
        2 => {
            // sound command: the next byte indexes the routine table (0x1f3aa: 0xe0ef8, 0xe0f70, 0xe0f34,
            // 0xe1024), called by `jsr (A6)`
            let index = tables::byte(x.b, script + 1) as u32;
            let routine = table::COMMANDS.pc_long(x.b, index);
            crate::services::sound::play_global(x.b, crate::services::sound::Sound::at(routine));
            scene_start(x, script + 3)
        }
        _ => presentation(x, script + 1),
    }
}

/// `0x1f818`: start of a demonstration scene from the script (`a1`: after the operation).
fn scene_start(x: &mut Ctx, mut a1: u32) -> Flow {
    Obj(mode::BASE).set_u8(x.b, f::STEP, 2);
    let scene = tables::byte(x.b, a1);
    var::SCENE.set(x.b, scene);
    let duration = tables::word(x.b, a1 + 1);
    Obj(mode::BASE).set_u16(x.b, f::TIMER, duration);
    a1 += 3;
    var::FRAME_COUNTER.set(x.b, 0);
    var::DEMO_TIME.set(x.b, 0);
    let rank = var::RANK.get(x.b);
    var::SAVED_RANK.set(x.b, rank);
    var::RANK.set(x.b, 0xe);
    var::PLAYERS_PRESENT.set(x.b, 0);
    var::PHASE.set(x.b, 8);
    var::STEP.set(x.b, 0);
    crate::world::rng::seed(x.b);
    for (k, d6) in (0..4u32).rev().enumerate() {
        let a0 = Obj(crate::world::ram::PLAYERS.nth(k as u32).0);
        if tables::byte(x.b, a1) & 0x80 == 0 {
            let character = tables::byte(x.b, a1);
            a0.set_u8(x.b, Obj::VARIANT, character);
            let variant = tables::byte(x.b, a1 + 1);
            a0.set_u8(x.b, player::SELECTION_KIND, variant);
            // the routine of table 0x1f8a8 (indexed by the counter, counting down) is the player's initialisation
            // of that number (0x2e58...: `player::init`), D6/A0/A1 kept around the call (`movem.l`)
            crate::actors::player::init::initialize(x.b, 3 - d6);
            apply_record(x, a0, a1);
        }
        a1 += 6;
    }
    let v = tables::long(x.b, a1);
    var::SCRIPT_TAIL.set(x.b, v);
    a1 += 4;
    Obj(mode::BASE).set_u32(x.b, f::SCRIPT, a1);
    clear::STARTED.set(x.b, 0);
    var::SCREEN_WAIT.set(x.b, 3);
    crate::core::kernel::create_task(x.b, 0x30, ext::PLAYER_UI);
    Flow::Done
}

/// `0x1f8b8`: optional parameters of a demonstration player (negative byte: not supplied).
fn apply_record(x: &mut Ctx, a0: Obj, a1: u32) {
    let b2 = tables::byte(x.b, a1 + 2);
    if b2 & 0x80 == 0 {
        a0.set_u8(x.b, player::WEAPON_A, b2);
    }
    let b3 = tables::byte(x.b, a1 + 3);
    if b3 & 0x80 == 0 {
        a0.set_u8(x.b, player::WEAPON_B, b3);
    }
    let b4 = tables::byte(x.b, a1 + 4);
    if b4 & 0x80 == 0 {
        let slot = tables::byte(x.b, a1 + 1) as u32;
        a0.set_u8(x.b, player::SELECTIONS + ((slot << 4) & 0xffff), b4);
        for i in 0..10 {
            a0.set_u8(x.b, player::SELECTION_ROWS + i, 9);
        }
    }
}

// --- the scene runs ---------------------------------------------------------------------------

/// `0x1f932`: one frame of the demonstration. The game runs two steps per frame: one step without yielding
/// when the frame has just started (`SKIP_YIELD` <= 1), one step after the yield.
fn loop_a(x: &mut Ctx) -> Flow {
    let n = var::FRAME_COUNTER.get(x.b).wrapping_add(1);
    var::FRAME_COUNTER.set(x.b, n);
    let skip = var::SKIP_YIELD.get(x.b) as i8;
    if var::SCREEN_WAIT.get(x.b) != 0 || var::YIELDED.get(x.b) != 0 || skip > 1 {
        var::SKIP_YIELD.set(x.b, 0);
        return x.wait(1, Step::AfterYieldA);
    }
    var::SKIP_YIELD.set(x.b, skip.wrapping_add(3) as u8);
    after_yield_a(x)
}

fn after_yield_a(x: &mut Ctx) -> Flow {
    if var::PHASE.get(x.b) == 0xc {
        let left = Obj(mode::BASE).u16(x.b, f::TIMER).wrapping_sub(1);
        Obj(mode::BASE).set_u16(x.b, f::TIMER, left);
        if left == 0 {
            return scene_end_a(x);
        }
    }
    run_phase(x, table::PHASES_A, Step::LoopA, Step::AfterPhaseA)
}

/// Calls the current phase handler (`jsr (A0)` preceded by the loop's `pea`): it returns to `after`
/// (`after_phase`), which returns to the top of the loop; a phase may also return directly to the top of the loop,
/// popping the return of its call.
fn run_phase(x: &mut Ctx, phases: Rom, loop_top: Step, after: Step) -> Flow {
    var::YIELDED.set(x.b, 0xff);
    x.push_return(loop_top);
    x.push_return(after);
    let phase = var::PHASE.get(x.b) as u32;
    // Original glitch: A0 stays the phase handler address (`jsr (A0)`); scene entry re-reads it as
    // object creator (`scene::entry`: finale, effects 0x13)
    let handler = phases.pc_long(x.b, phase);
    // the two tables only have phases 8 (scene entry) and 12 (the game); the other entries are null
    // (code-mort.md, batch "CPU removal")
    match phase {
        8 => {
            crate::screens::scene::entry::phase_8(x.b, handler);
            Flow::Done
        }
        12 => demo_phase_12(x),
        _ => unreachable!("demo: phase {phase:#x} without handler (table {:#x})", phases.addr()),
    }
}

/// `0x1f9b0`: after the phase, handles the loading screen and empties the collision queues; the `rts`
/// goes back to the top of the loop.
fn after_phase(x: &mut Ctx) -> Flow {
    let wait = var::SCREEN_WAIT.get(x.b);
    if wait == 0 {
        let v = var::VIDEO.get(x.b) | 0xe;
        var::VIDEO.set(x.b, v);
    } else {
        let left = wait - 1;
        var::SCREEN_WAIT.set(x.b, left);
        if left != 0 {
            let v = var::VIDEO.get(x.b) & !0xe;
            var::VIDEO.set(x.b, v);
            let longs = [0, 0x1800_0000, 0, 0x1800_0000, 0x8000_8000, 0];
            for (i, l) in longs.into_iter().enumerate() {
                OBJECTS.offset(4 * i as u32).set_long(x.b, l);
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
    clear::NO_JUMP.set(x.b, 0);
    let v = var::VIDEO_FLAGS_B.get(x.b) & !1;
    var::VIDEO_FLAGS_B.set(x.b, v);
    sv::SCROLL_MODE.set(x.b, 0);
}
/// `0x1fb20`: duration elapsed: everything is cleared, then the next operation.
fn scene_end_a(x: &mut Ctx) -> Flow {
    let rank = var::SAVED_RANK.get(x.b);
    var::RANK.set(x.b, rank);
    scene_cleanup_1(x);
    clear::BOSS_ROW.set(x.b, 0);
    scene_end_common(x, Step::SceneEndClearA)
}
/// Chains the common cleanup then the yield of one frame whose resume is `resume`.
fn scene_end_common(x: &mut Ctx, resume: Step) -> Flow {
    stop_recorder(x.b);
    var::PHASE.set(x.b, 0);
    var::PLAYERS_PRESENT.set(x.b, 0);
    crate::core::kernel::kill_task(x.b, 0x30);
    clear_lists_and_objects(x);
    x.wait(1, resume)
}

/// After the yield: clears the planes and the queues, yields again, then reads the next operation.
fn scene_end_clear(x: &mut Ctx, resume: Step) -> Flow {
    let v = var::VIDEO.get(x.b) & !0xe;
    var::VIDEO.set(x.b, v);
    clear_layers(x, &[Layer::One, Layer::Two, Layer::Three]);
    clear_players_and_pools(x);
    reset_queues(x);
    x.wait(1, resume)
}

/// `0x1fa92`: end of script: return of the demonstration clock, everything cleared, next screen.
fn end_of_script(x: &mut Ctx) -> Flow {
    let s = Obj(mode::BASE).u8(x.b, f::SUB).wrapping_add(2);
    Obj(mode::BASE).set_u8(x.b, f::SUB, s);
    restore_clock(x);
    scene_cleanup_1(x);
    stop_recorder(x.b);
    var::PHASE.set(x.b, 0);
    Obj(mode::BASE).set_u16(x.b, f::STEP, 0);
    var::PLAYERS_PRESENT.set(x.b, 0);
    crate::core::kernel::kill_task(x.b, 0x30);
    clear_lists_and_objects(x);
    x.wait(1, Step::EndClear)
}
fn restore_clock(x: &mut Ctx) {
    let t = var::SAVED_TIME.get(x.b);
    var::DEMO_TIME.set(x.b, t);
    let t = var::SAVED_FRAMES.get(x.b);
    var::FRAME_COUNTER.set(x.b, t);
    let rank = var::SAVED_RANK.get(x.b);
    var::RANK.set(x.b, rank);
}

// ---------------------------------------------------------------------------------------------
// Interpreter B (`0x20926`)

fn interpreter_b(x: &mut Ctx) -> Flow {
    let step = rd_state(x, f::STEP);
    x.push_return(Step::ReturnB);
    match step {
        0 => {
            crate::core::tasks::prompt::stop(x.b);
            x.next_step();
            Obj(mode::BASE).set_u32(x.b, f::SCRIPT, table::SCRIPT_B.addr());
            let t = var::DEMO_TIME.get(x.b);
            var::SAVED_TIME.set(x.b, t);
            let t = var::FRAME_COUNTER.get(x.b);
            var::SAVED_FRAMES.set(x.b, t);
            Obj(mode::BASE).set_u8(x.b, f::ENDING, 0);
            next_operation_b(x)
        }
        2 => {
            crate::core::tasks::prompt::launch(x.b);
            loop_b(x)
        }
        4 => scene_next_b(x),
        _ => Flow::Done,
    }
}

/// `0x20960`: next operation: end of script (negative byte) or scene.
fn next_operation_b(x: &mut Ctx) -> Flow {
    Obj(mode::BASE).set_u8(x.b, f::SUBSTEP, 0);
    let script = Obj(mode::BASE).u32(x.b, f::SCRIPT);
    let op = tables::byte(x.b, script);
    if op & 0x80 != 0 {
        return end_of_script(x);
    }
    scene_start(x, script + 1)
}

fn loop_b(x: &mut Ctx) -> Flow {
    let n = var::FRAME_COUNTER.get(x.b).wrapping_add(1);
    var::FRAME_COUNTER.set(x.b, n);
    let skip = var::SKIP_YIELD.get(x.b) as i8;
    if var::SCREEN_WAIT.get(x.b) != 0 || var::YIELDED.get(x.b) != 0 || skip > 1 {
        var::SKIP_YIELD.set(x.b, 0);
        return x.wait(1, Step::AfterYieldB);
    }
    var::SKIP_YIELD.set(x.b, skip.wrapping_add(3) as u8);
    after_yield_b(x)
}

fn after_yield_b(x: &mut Ctx) -> Flow {
    if var::SCENE.get(x.b) == 0x6f || var::PHASE.get(x.b) == 0xc {
        let timer = Obj(mode::BASE).u16(x.b, f::TIMER);
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
            Obj(mode::BASE).set_u16(x.b, f::TIMER, left);
            if left == 0 {
                let script = Obj(mode::BASE).u32(x.b, f::SCRIPT);
                if tables::word(x.b, script) == 0xffff {
                    Obj(mode::BASE).set_u8(x.b, f::ENDING, 1);
                    let _ = scenery_6(x.b, 8);
                } else {
                    fading::DELAY.set(x.b, 2);
                    fading::PAGES.set(x.b, 15);
                    crate::core::kernel::create_task(x.b, 0x10, crate::screens::scene::fading::DESCENT);
                    crate::services::sound::service(x.b, crate::services::sound::Service::Ff07 { lock: 0, line: 0 });
                }
            }
        }
    }
    run_phase(x, table::PHASES_B, Step::LoopB, Step::AfterPhaseB)
}

/// `0x20abc`: next scene: like `0x1fb20` but also clears `A5+0x59`.
fn scene_next_b(x: &mut Ctx) -> Flow {
    let rank = var::SAVED_RANK.get(x.b);
    var::RANK.set(x.b, rank);
    scene_cleanup_1(x);
    clear::BYTE_59.set(x.b, 0);
    clear::BOSS_ROW.set(x.b, 0);
    scene_end_common(x, Step::SceneNextClearB)
}

/// `0x20b86`: end of the game demonstration: end screen.
fn ending_b(x: &mut Ctx) -> Flow {
    let s = Obj(mode::BASE).u8(x.b, f::SUB).wrapping_add(2);
    Obj(mode::BASE).set_u8(x.b, f::SUB, s);
    restore_clock(x);
    scene_cleanup_1(x);
    stop_recorder(x.b);
    var::PHASE.set(x.b, 0);
    Obj(mode::BASE).set_u16(x.b, f::STEP, 0);
    var::PLAYERS_PRESENT.set(x.b, 0);
    crate::core::kernel::kill_task(x.b, 0x30);
    clear_layer(x.b, Layer::One);
    crate::services::video::clear_layer1_start(x.b);
    palette(1, 0xe).set(x.b, 0xf112);
    Word(sv::SCROLL_2.0).set(x.b, 256);
    Word(sv::SCROLL_1.0).set(x.b, 0);
    x.wait(1, Step::EndingReset)
}

// ---------------------------------------------------------------------------------------------
// Phase 12 of the demonstration (`0x1fa38`)

/// `0x1fa38`: AI inputs, then the world update (an object handler may make the task cede control there; it
/// will resume at [`Step::AfterWorld`]).
pub(crate) fn demo_phase_12(x: &mut Ctx) -> Flow {
    // Original glitch: D2-D4 of the input copy (0x2aec) are re-read by handlers (see game.rs)
    crate::services::inputs::copy_in_game(x.b);
    demo_inputs(x);
    x.push_return(Step::AfterWorld);
    if crate::core::game::update_world(x.b) == Flow::Yield {
        return Flow::Yield;
    }
    x.drop_return();
    demo_after_world(x)
}

/// `0x1fa4a`: like game phase 12 (`game::after_world`); the original ends with `jmp 0x28d8` (the end of
/// the game, then its `rts`).
fn demo_after_world(x: &mut Ctx) -> Flow {
    crate::core::game::after_world(x.b);
    Flow::Done
}

pub const NATIVES: &[Native] = &[
    
    
    RECORDER_STOP,
];

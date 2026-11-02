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

// --- character presentation (operation other than 0 and 2) -----------------------------------

/// `0x1fbd6`: stores the three presentation parameters (`a1`: after the operation) and prepares the display.
fn presentation(x: &mut Ctx, mut a1: u32) -> Flow {
    crate::tasks::prompt::stop(x.b);
    wr8(x.b, mode::BASE + f::STEP, 6);
    wr16(x.b, mode::BASE + f::TIMER, 0);
    let b = rd8(x.b, a1);
    wr8(x.b, mode::BASE + f::PRESENT_9, b);
    let b = rd8(x.b, a1 + 1);
    wr8(x.b, mode::BASE + f::PRESENT_KIND, b);
    let b = rd8(x.b, a1 + 2);
    wr8(x.b, mode::BASE + f::PRESENT_CHARACTER, b);
    a1 += 3;
    wr32(x.b, mode::BASE + f::SCRIPT, a1);
    wr8(x.b, A5 + 0x53c, 0);
    wr8(x.b, A5 + 0x36, 0);
    wr16(x.b, A5 + 0x506, 0);
    wr16(x.b, A5 + 0x50a, 512);
    wr16(x.b, A5 + 0x506, 0);
    wr16(x.b, A5 + 0x55a, 512);
    wr16(x.b, A5 + 0x4b6, 0);
    wr16(x.b, A5 + 0x4ba, 0);
    let v = rd8(x.b, var::VIDEO_FLAGS_B) & !1;
    wr8(x.b, var::VIDEO_FLAGS_B, v);
    wr16(x.b, var::VIDEO, 0x12c0);
    crate::palette::load_palette_page(x.b, 2, 84);
    crate::palette::load_palette_page(x.b, 3, 84);
    fading::PAGES.set(x.b, 12);
    crate::palette::full_brightness(x.b);
    crate::tasks::wait_then_return(x, 1)
}

fn presentation_step(x: &mut Ctx) -> Flow {
    let substep = rd_state(x, f::SUBSTEP);
    match substep {
        0 => p0_images(x),
        2 => p2_show(x),
        4 => p4_end(x),
        6 => p6_text(x),
        // table 0x1fc72 has four entries, all rewritten (code-mort.md, batch "CPU removal")
        _ => unreachable!("presentation: sub-step {substep:#x} outside table 0x1fc72"),
    }
}

/// `0x1fc68`: common end of each sub-state: terrain (SET, SOBJ) and sprites (`jmp 0x1de96`).
fn p_render(x: &mut Ctx) -> Flow {
    crate::dispatch::SET.update_no_next(x.b);
    crate::dispatch::SOBJ.update_no_next(x.b);
    crate::sprites::build_in_game(x.b);
    Flow::Done
}

fn next_substep(x: &mut Ctx) {
    let s = rd_state(x, f::SUBSTEP).wrapping_add(2);
    wr8(x.b, mode::BASE + f::SUBSTEP, s);
}

/// Sub-state 0: loads the character's two tile blocks, creates the frame and portrait objects.
fn p0_images(x: &mut Ctx) -> Flow {
    next_substep(x);
    let character = rd_state(x, f::PRESENT_CHARACTER) as u32 * 4;
    let (first, second) = (x.prog16(table::TILES + character), x.prog16(table::TILES + 2 + character));
    let end = crate::video::load_grid(x.b, crate::video::Grid::Layer2, first, 0x915000);
    crate::video::load_grid(x.b, crate::video::Grid::Layer2, second, end.destination);
    if rd_state(x, f::PRESENT_KIND) != 0 {
        crate::sound::service(x.b, crate::sound::Service::Ff07 { lock: 0, line: 0 });
        if let Ok(Obj(set)) = set_15_pure(x.b, 4) {
            let character = rd_state(x, f::PRESENT_CHARACTER);
            wr8(x.b, set + 0x2f, character);
            wr16(x.b, mode::BASE + f::TIMER, 30);
        }
        return p_render(x);
    }
    let Ok(Obj(set)) = set_15_pure(x.b, 4) else {
        return p_render(x);
    };
    let character = rd_state(x, f::PRESENT_CHARACTER);
    wr8(x.b, set + 0x2f, character);
    portrait_object(x)
}

/// `0x1fce4`: portrait object; on failure or success the original *jumps* to `0x1fc68`.
fn portrait_object(x: &mut Ctx) -> Flow {
    if let Ok(Obj(o)) = scenery_6(x.b, 15) {
        let character = rd_state(x, f::PRESENT_CHARACTER) >> 1;
        wr8(x.b, o + 0x2f, character);
        wr16(x.b, mode::BASE + f::LINK, o as u16);
    }
    p_render(x)
}

/// Sub-state 2: turns the planes on; next sub-state 6 (text) if the presentation has a text, else 4.
fn p2_show(x: &mut Ctx) -> Flow {
    let v = rd16(x.b, var::VIDEO) | 0xe;
    wr16(x.b, var::VIDEO, v);
    let next = if rd_state(x, f::PRESENT_KIND) != 0 { 6 } else { 4 };
    wr8(x.b, mode::BASE + f::SUBSTEP, next);
    p_render(x)
}

/// Sub-state 6: after the timer (word `+8`, counted down by the objects), displays the character's text.
fn p6_text(x: &mut Ctx) -> Flow {
    let left = rd16(x.b, mode::BASE + f::TIMER).wrapping_sub(1);
    wr16(x.b, mode::BASE + f::TIMER, left);
    if left != 0 {
        return p_render(x);
    }
    wr8(x.b, mode::BASE + f::SUBSTEP, 4);
    wr16(x.b, mode::BASE + f::TIMER, 240);
    let entry = table::PRESENT_DATA + ((rd_state(x, f::PRESENT_CHARACTER) as u32) << 3);
    // choice by number of players (A5+0x72: 2, 4, 6...) of a data address (plain)
    let players = rd16(x.b, A5 + 0x72).wrapping_sub(2).wrapping_mul(2) as u32;
    let mut a1 = rd32(x.b, entry + players);
    let a4 = entry + 12;
    let (cx, cy) = (rd16(x.b, a1), rd16(x.b, a1 + 2));
    wr16(x.b, A5 + 0x5ca, cx);
    wr16(x.b, A5 + 0x5cc, cy);
    a1 += 4;
    wr32(x.b, A5 + 0x5c6, a1);
    let offset = rd16(x.b, a4) as i16 as i32 as u32;
    let src = 0x1fe3e_u32.wrapping_add(offset);
    for i in 0..8 {
        let v = rd32(x.b, src + 4 * i);
        wr32(x.b, 0x91c1e0 + 4 * i, v);
    }
    x.push_return(Step::PresentAgain);
    text_screen(x);
    portrait_object(x)
}

/// Sub-state 4: after the timer, closes the presentation and reads the next operation.
fn p4_end(x: &mut Ctx) -> Flow {
    let left = rd16(x.b, mode::BASE + f::TIMER).wrapping_sub(1);
    wr16(x.b, mode::BASE + f::TIMER, left);
    if left != 0 {
        return p_render(x);
    }
    let link = rd16(x.b, mode::BASE + f::LINK) as i16 as i32 as u32 & 0xff_ffff;
    wr32(x.b, link + 4, 0x0600_0000);
    wr8(x.b, mode::BASE + f::STEP, 2);
    wr16(x.b, mode::BASE + 0x30, 300);
    crate::objects::lists::unlink_all(x.b);
    crate::sprites::clear_lists(x.b);
    for pool in pools::POOLS {
        pool.reset(x.b);
    }
    crate::video::clear_objects(x.b);
    x.wait(1, Step::P4Hide)
}
const P4_HIDE: u32 = 0x1ff5e;
const P4_CLEAR: u32 = 0x1ff6c;

/// `0x1ffae`: frame and texts of the presentation, written into plane 1 (GFX RAM `0x912000`).
fn text_screen(x: &mut Ctx) {
    let character = rd_state(x, f::PRESENT_CHARACTER) as u32;
    let frame = x.prog16(table::FRAME_ATTR + character) as i16 as i32 as u32;
    let a4 = 0x912000u32.wrapping_add(frame);
    let attr = x.prog16(table::BOX_ATTR + character);
    frame_box(x, a4, attr);
    text_lines(x, a4 + 128, table::TEXT_1, attr);
    let which = (character & 0xffff) * 2;
    let text = prog32(x, table::TEXT_2 + which);
    text_lines(x, a4 + 0x700, text, attr);
}

/// `0x1ffdc`: the frame (eight edge and corner tiles).
fn frame_box(x: &mut Ctx, a4: u32, attr: u16) {
    let put = |x: &mut Ctx, a: &mut u32, tile: u16| {
        wr16(x.b, *a, tile);
        wr16(x.b, *a + 2, attr);
        *a += 4;
    };
    let mut a1 = a4;
    put(x, &mut a1, 0x145);
    for _ in 0..10 {
        put(x, &mut a1, 0x14b);
    }
    put(x, &mut a1, 0x147);
    a1 += 124;
    for _ in 0..16 {
        put(x, &mut a1, 0x14a);
        a1 += 124;
    }
    a1 = a4 + 128;
    for _ in 0..16 {
        put(x, &mut a1, 0x149);
        a1 += 124;
    }
    put(x, &mut a1, 0x146);
    for _ in 0..10 {
        put(x, &mut a1, 0x14c);
    }
    put(x, &mut a1, 0x148);
}

/// `0x20044`: writes a vertical text (one tile per line); `0x2f` moves to the next column,
/// `0x40` termine.
fn text_lines(x: &mut Ctx, start: u32, text: u32, attr: u16) {
    let mut a3 = start;
    let mut a2 = text;
    let mut a1 = a3 + 4;
    a3 = a1;
    loop {
        let c = rd8(x.b, a2);
        a2 += 1;
        match c {
            0x2f => {
                a1 = a3 + 4;
                a3 = a1;
            }
            0x40 => return,
            _ => {
                wr16(x.b, a1, c as u16);
                wr16(x.b, a1 + 2, attr);
                a1 += 4 + 124;
            }
        }
    }
}

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
// Recorded inputs (`0xe05ec`)

/// Demonstration input recorder (A5 - 0x5ab2...): pointer to the current entry (long), remaining
/// frames of the entry (word), and three flags: recording, replay, active.
mod rec {
    use crate::mem::A5;
    pub const POINTER: u32 = A5 - 0x5ab2;
    pub const LEFT: u32 = A5 - 0x5aae;
    pub const PLAYING: u32 = A5 - 0x5aac;
    pub const RECORDING: u32 = A5 - 0x5aab;
    pub const ACTIVE: u32 = A5 - 0x5aaa;
    /// An entry: duration (word), then the three input bytes of each player.
    pub const ENTRY: u32 = 0xe;
    /// Last possible entry.
    pub const LAST: u32 = 0x93fff2;
}

/// The twelve input bytes of the players (`A5 + 0x64a + 0x200 k`, three bytes each), in the order of an entry.
fn input_byte(i: u32) -> u32 {
    A5 + 0x64a + 0x200 * (i / 3) + i % 3
}

/// `0xe05ec`: records the players' inputs (one entry per change, with its duration) or replays them (a
/// game in progress with bit 15 of the settings suspends the replay).
fn demo_inputs(x: &mut Ctx) {
    if rd8(x.b, rec::ACTIVE) == 0 {
        return;
    }
    if rd8(x.b, rec::RECORDING) != 0 {
        record_inputs(x);
    } else if rd8(x.b, rec::PLAYING) != 0 {
        if rd8(x.b, var::GAME) != 0 && rd16(x.b, var::DSW) & 0x8000 != 0 {
            return;
        }
        let at = rd32(x.b, rec::POINTER);
        if at > rec::LAST {
            wr8(x.b, rec::PLAYING, 0);
            return;
        }
        for i in 0..12 {
            let v = rd8(x.b, at + 2 + i);
            wr8(x.b, input_byte(i), v);
        }
        let left = rd16(x.b, rec::LEFT);
        if left != 0 {
            wr16(x.b, rec::LEFT, left - 1);
        } else {
            let next = at + rec::ENTRY;
            wr32(x.b, rec::POINTER, next);
            let length = rd16(x.b, next);
            wr16(x.b, rec::LEFT, length);
        }
    }
}

/// `0xe0538` (scene entry, outside a game): prepares the recorder. Setting bit 13: random seed and
/// frame counters to zero, then replay active; bit 15: recording at 0x930000; otherwise replay
/// (inactive) of the demonstration pointed to by `A5-0x5aa8`.
pub fn reset_recorder(b: &mut Cps2Bus) {
    reset_recorder_by(b);
}

/// What 0xe0538 leaves in the registers: D5.w = the settings read, A0 = the demonstration replayed.
#[derive(Default)]
pub(crate) struct RecorderLeftovers {
    pub(crate) settings: Option<u16>,
    pub(crate) demo: Option<u32>,
}

pub(crate) fn reset_recorder_by(b: &mut Cps2Bus) -> RecorderLeftovers {
    const DEMO: u32 = A5 - 0x5aa8;
    const RECORD_AREA: u32 = 0x93_0000;
    let mut leftovers = RecorderLeftovers::default();
    if rd8(b, var::GAME) != 0 {
        return leftovers;
    }
    wr16(b, rec::LEFT, 0);
    wr8(b, rec::RECORDING, 0);
    wr8(b, rec::PLAYING, 0);
    wr8(b, rec::ACTIVE, 0);
    let dsw = rd16(b, var::DSW);
    leftovers.settings = Some(dsw);
    if dsw & 0x2000 != 0 {
        crate::rng::seed(b);
        wr16(b, A5 + 0x6c, 0);
        wr16(b, A5 + 0x6e, 0);
        wr16(b, A5 + 0x7f92, 0xe);
        wr8(b, rec::ACTIVE, 1);
    } else if dsw & 0x8000 != 0 {
        wr32(b, rec::POINTER, RECORD_AREA);
        for off in [0, 4, 8] {
            wr32(b, RECORD_AREA + off, 0xffff_ffff);
        }
        wr16(b, RECORD_AREA + 0xc, 0xffff);
        wr8(b, rec::RECORDING, 1);
        return leftovers;
    }
    let demo = rd32(b, DEMO);
    leftovers.demo = Some(demo);
    wr32(b, rec::POINTER, demo);
    let length = rd16(b, demo);
    wr16(b, rec::LEFT, length);
    wr8(b, rec::PLAYING, 1);
    leftovers
}

/// `0xe05d6`: stops the recorder.
pub fn stop_recorder(b: &mut Cps2Bus) {
    wr16(b, rec::LEFT, 0);
    for flag in [rec::RECORDING, rec::PLAYING, rec::ACTIVE] {
        wr8(b, flag, 0);
    }
    wr32(b, A5 - 0x5aa8, 0);
}

pub const RECORDER_STOP: Native = Native {
    entry: 0xe05d6,
    name: "demo_recorder_stop",
    run: crate::Body::Memory(stop_recorder),
    touches_sound: false,
    outputs: 0,
};

/// Recording: the current entry lasts one more frame as long as the inputs do not change (and its duration
/// fits in 16 bits); otherwise its duration is written and a new entry begins.
fn record_inputs(x: &mut Ctx) {
    let length = rd16(x.b, rec::LEFT);
    let at = rd32(x.b, rec::POINTER);
    if length != 0xffff && (0..12).all(|i| rd8(x.b, input_byte(i)) == rd8(x.b, at + 2 + i)) {
        wr16(x.b, rec::LEFT, length + 1);
        return;
    }
    wr16(x.b, at, length);
    wr16(x.b, rec::LEFT, 0);
    let next = at + rec::ENTRY;
    wr32(x.b, rec::POINTER, next);
    if next > rec::LAST {
        wr8(x.b, rec::RECORDING, 0);
        return;
    }
    for i in 0..12 {
        let v = rd8(x.b, input_byte(i));
        wr8(x.b, next + 2 + i, v);
    }
}

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

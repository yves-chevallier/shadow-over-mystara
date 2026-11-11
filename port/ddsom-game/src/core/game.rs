//! Game loop (W1): end of frame (`0x1c4c4`), phase 0 (`0x1c522`, game initialisation) and phase 12 (`0x1c6a0`, the
//! game). See `docs/boucle.md`.
//!
//! The game task (`0x1c45a`) loops forever: one frame = one call of the current phase
//! (`A5+0x2c`), then the clearing of the collision queues, then a cession (`0xa40`): see
//! `tasks/game.rs`. Phase 12 calls the world update (`actors::update`); an object handler can make the task cede
//! control in the middle of it (fade to black of the doors, fireworks): the pending calls of phase 12 and of the world
//! update are then stages ([`Step`]), where the task will resume.

use crate::world::World;
use crate::world::ops::sbcd;

use crate::actors::dispatch;
use crate::actors::update::{self, update_world};
use crate::hw::mem::{rd16, rd8, wr16, wr8, A5};
use crate::world::tasks::Flow;
use crate::world::objects::pools::{self, Queue};
use crate::world::objects::Obj;
use crate::world::tasks::stages::World as Step;
use crate::world::tasks::table;

/// Work RAM variables (offsets from A5).
mod ram {
    /// Game phase (byte, step of 4), step.
    pub const PHASE: u32 = 0x2c;
    pub const STEP: u32 = 0x2d;
    /// "Continue" mode: phase 0 jumps to its last step.
    pub const CONTINUE: u32 = 0x25;
    /// Game active.
    pub const ACTIVE: u32 = 0x30;
    /// Scene number (signed byte).
    pub const SCENE: u32 = 0x34;
    /// Step countdown (word); in game, BCD clock on three bytes.
    pub const TIMER: u32 = 0x38;
    /// Vertical scroll of plane 2 (word).
    pub const SCROLL2_Y: u32 = 0x4ba;
}

// ---------------------------------------------------------------------------------------------
// End of frame (0x1c4c4)

/// Queues of collision candidates, cleared every frame.
const QUEUES: [&Queue; 5] =
    [&pools::QUEUE_PSHL, &pools::QUEUE_ESHL, &pools::QUEUE_ENEMY_BOSS, &pools::QUEUE_BOX, &pools::QUEUE_ITEM];

/// End of frame (`0x1c4c4`), when the phase returns (step `tasks::game::Step::FrameEnd`): the collision
/// queues start empty again. The `rts` that follows returns to the start of the game loop (`0x1c468`).
pub fn frame_end(b: &mut World) {
    for queue in QUEUES {
        queue.reset(b);
    }
}

// ---------------------------------------------------------------------------------------------
// Phase 0: game initialisation (0x1c522)

/// Text rectangles erased by `text::clear_box` (0x1324; descriptors in the code, after the step table). Batch C
/// showed that 0x1324 erases a rectangle: the names "palette scripts" were wrong.
const TEXT_BOXES: [u32; 3] = [0x1c546, 0x1c54a, 0x1c54e];

/// Variant of the three SET objects created at step 0, and their palette animation index (`+0x2f`).
const SET_PALETTES: [(u8, u8); 3] = [(0x3e, 0), (0x3f, 1), (0x40, 2)];
const SET_PALETTE_INDEX: u32 = 0x2f;

/// RAM bytes outside the A5±0x7fff page, reset to zero by the phase.
const CLEARED_A: u32 = 0xff_2472; // A5 - 0x5b8e
const CLEARED_B: u32 = 0xff_19f0; // A5 - 0x6610

/// Players' interface task created at the end of the phase.
const PLAYER_UI_SLOT: u32 = 0x30;
const PLAYER_UI_TASK: crate::world::tasks::Task = crate::world::tasks::Task::Interface;

/// `A5` bytes reset to zero on exit: flag and score.
const EXIT_CLEARED_BYTES: [u32; 2] = [0x37, 0x5d];

fn next_step(b: &mut World) {
    let step = rd8(b, A5 + ram::STEP).wrapping_add(4);
    wr8(b, A5 + ram::STEP, step);
}

/// Step 0: three SET objects (palette animations), prepares the players.
fn create_palette_objects(b: &mut World) {
    next_step(b);
    wr16(b, A5 + ram::TIMER, 0x40);
    wr8(b, A5 + ram::ACTIVE, 1);
    wr8(b, CLEARED_A, 0);
    crate::world::objects::pools::SET.reset(b);
    for (variant, index) in SET_PALETTES {
        // the pool has just been emptied: the allocation succeeds
        let set = pools::SET.alloc(b).expect("SET pool empty");
        set.set_u8(b, Obj::ACTIVE, 1);
        set.set_u8(b, Obj::KIND, 0);
        set.set_u8(b, Obj::VARIANT, variant);
        set.set_u8(b, SET_PALETTE_INDEX, index);
    }
    crate::services::palette::set_flash_color(b, crate::services::palette::FLASH_COLOR_WHITE_VALUE);
    // 0x3525c
    crate::services::video::clear_layer1_start(b);
    let y = rd16(b, A5 + ram::SCROLL2_Y).wrapping_add(0x100);
    wr16(b, A5 + ram::SCROLL2_Y, y);
}

/// Step 4: erases the first two text rectangles (then moves to step 8).
fn clear_text_boxes(b: &mut World) {
    next_step(b);
    crate::services::text::clear_box(b, TEXT_BOXES[0]);
    crate::services::text::clear_box(b, TEXT_BOXES[1]);
    let y = rd16(b, A5 + ram::SCROLL2_Y).wrapping_sub(0x100);
    wr16(b, A5 + ram::SCROLL2_Y, y);
}

/// Start of steps 8 and 12: system objects and animated scenery, sprites, then countdown.
/// Returns true when the countdown has elapsed.
fn animate(b: &mut World) -> bool {
    dispatch::SOBJ.update_no_next(b);
    dispatch::SET.update_no_next(b);
    // 0x1de96
    crate::services::sprites::build_in_game(b);
    let left = rd16(b, A5 + ram::TIMER).wrapping_sub(1);
    wr16(b, A5 + ram::TIMER, left);
    (left as i16) < 0
}

/// Step 8: countdown elapsed, a SET object (type 0xc) appears and step 12 begins.
fn wait_then_spawn(b: &mut World) {
    if !animate(b) {
        return;
    }
    let Some(set) = pools::SET.alloc(b) else { return };
    set.set_u8(b, Obj::ACTIVE, 1);
    set.set_u8(b, Obj::KIND, 0xc);
    next_step(b);
    wr16(b, A5 + ram::TIMER, 0x20);
}

/// Step 12: countdown elapsed, end fade and third palette animation, then step 16.
fn wait_then_fade(b: &mut World) {
    if !animate(b) {
        return;
    }
    // 0x1cf3c
    crate::services::palette::white_out(b);
    crate::services::text::clear_box(b, TEXT_BOXES[2]);
    reset_pools_and_leave(b);
}

/// Step 16: pools made new again, then exit from the phase.
fn reset_pools_and_leave(b: &mut World) {
    wr8(b, A5 + ram::STEP, 0);
    // 0x1de7c
    crate::services::sprites::clear_lists(b);
    crate::world::objects::pools::SOBJ.reset(b);
    crate::world::objects::pools::SET.reset(b);
    leave(b);
}

/// Exit from phase 0 (also in continue mode): next phase, first level, interface task.
fn leave(b: &mut World) {
    let phase = rd8(b, A5 + ram::PHASE).wrapping_add(4);
    wr8(b, A5 + ram::PHASE, phase);
    wr8(b, CLEARED_B, 0);
    wr8(b, A5 + ram::ACTIVE, 1);
    crate::world::objects::rank::load_current_preset(b);
    wr8(b, A5 + ram::SCENE, 1);
    wr8(b, A5 + EXIT_CLEARED_BYTES[0], 0);
    wr16(b, A5 + ram::TIMER, 0);
    wr16(b, A5 + ram::TIMER + 2, 0);
    wr8(b, A5 + EXIT_CLEARED_BYTES[1], 0);
    crate::actors::families::items::clear_picked_up(b);
    crate::world::tasks::traps::create_task(b, PLAYER_UI_SLOT, PLAYER_UI_TASK);
}

/// Phase 0: one step per frame. The step (`A5+0x2d`: 0, 4, 8, 12, 16) chooses what follows.
pub fn phase0(b: &mut World) {
    if rd8(b, A5 + ram::CONTINUE) != 0 {
        return leave(b);
    }
    match rd8(b, A5 + ram::STEP) {
        0 => create_palette_objects(b),
        4 => {
            clear_text_boxes(b);
            wait_then_spawn(b);
        }
        8 => wait_then_spawn(b),
        12 => wait_then_fade(b),
        16 => reset_pools_and_leave(b),
        step => panic!("phase 0: unknown step {step}"),
    }
}

// ---------------------------------------------------------------------------------------------
// Phase 12: the game proper (0x1c6a0)

/// Game clock in BCD: minutes (`+0x38`), seconds, frames (0 to 59).
const CLOCK: u32 = 0x38;
/// Game frame counter (word, capped at 0xffff).
const GAME_FRAMES: u32 = 0x54;


/// Removes a frame from the clock (frames, seconds, minutes in cascade, from 59 to 00): the cascade
/// stops as soon as a byte stays positive. `borrow`: the X flag that the first `sbcd` takes as carry.
fn tick_clock(b: &mut World, mut borrow: bool) {
    for (i, offset) in [CLOCK + 2, CLOCK + 1, CLOCK].into_iter().enumerate() {
        let (value, next_borrow) = sbcd(rd8(b, A5 + offset), 1, borrow);
        borrow = next_borrow;
        wr8(b, A5 + offset, value);
        if value as i8 >= 0 {
            return;
        }
        if i < 2 {
            wr8(b, A5 + offset, 0x59);
        } else {
            // time elapsed: clock at zero
            wr16(b, A5 + CLOCK, 0);
            wr16(b, A5 + CLOCK + 2, 0);
        }
    }
}

/// Phase 12: one game frame. `Flow::Yield`: an object handler made the task cede control (it
/// will resume at [`Step::AfterWorld`] after the world update).
pub fn phase12(b: &mut World) -> Flow {
    // Original glitch: the carry of the clock's first `sbcd` is the X flag left by the previous code: zero after the
    // frame counter's addition, otherwise (counter at 0xFFFF, after 18 minutes of play: never in the scenarios) that
    // of the last arithmetic instruction run, which the dispatch loops leave at zero (their `lsl.w #2`): taken as
    // zero.
    if rd16(b, A5 + GAME_FRAMES) != 0xffff {
        let frames = rd16(b, A5 + GAME_FRAMES) + 1;
        wr16(b, A5 + GAME_FRAMES, frames);
    }
    if rd16(b, A5 + CLOCK) != 0 {
        tick_clock(b, false);
    }
    // 0x2aec. Original glitch: the input bytes that the copy leaves in D2-D4 are reread by a handler of the world
    // update, which writes them into an object (DDSOM_POISON=call_2aec: long_elfe_objets, frame 13133, ram ffd694):
    // leftovers of the game
    crate::services::inputs::copy_in_game(b);
    table::call(b, Step::AfterWorld);
    if update_world(b) == Flow::Yield {
        return Flow::Yield;
    }
    table::pop_return(b);
    phase12_after_world(b);
    Flow::Done
}

/// Return of the world update in phase 12 (`jsr 0x1c86e` at 0x1c6fc).
const WORLD_RETURN: u32 = 0x1c6fc + update::JSR;

impl Step {
    /// Address that the original resumes at: the one following the call (`jsr`).
    pub fn address(self) -> u32 {
        match self {
            Step::World { order_index, step } => update::resume_address(order_index, step),
            Step::AfterWorld => WORLD_RETURN,
        }
    }

    pub fn step(self, x: &mut crate::world::tasks::Ctx) -> Flow {
        match self {
            Step::World { order_index, step } => update::run_steps(x.b, order_index, step as usize + 1),
            Step::AfterWorld => {
                phase12_after_world(x.b);
                Flow::Done
            }
        }
    }
}


/// End of phase 12, after the world update: [`after_world`], then the debug level skip.
fn phase12_after_world(b: &mut World) {
    crate::screens::scene::after_world(b);
    crate::screens::scene::debug_scene_skip(b);
}


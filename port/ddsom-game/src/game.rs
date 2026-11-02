//! Game loop (W1): world update (`0x1c86e`), end of frame (`0x1c4c4`), phase 0
//! (`0x1c522`, game initialisation) and phase 12 (`0x1c6a0`, the game). See `docs/looped.md`.
//!
//! The game task (`0x1c45a`) loops forever: one frame = one call of the current phase
//! (`A5+0x2c`), then the clearing of the collision queues, then a cession (`0xa40`): see
//! `tasks/game.rs`. An object handler can make the task cede control in the middle of the world
//! update (fade to black of the doors, fireworks): the pending calls of phase 12 and of the world update
//! are then steps ([`Step`]), where the task will resume.

use cps2::Cps2Bus;
use m68k::Cpu;

use crate::dispatch::{self, Class};
use crate::mem::{rd16, rd8, rts, wr16, wr8, A5};
use crate::modes::Flow;
use crate::objects::pools::{self, Queue};
use crate::objects::Obj;
use crate::tasks::table;
use crate::{call, Native};

/// Work RAM variables (offsets from A5).
mod ram {
    /// Game phase (byte, step of 4), step, substep.
    pub const PHASE: u32 = 0x2c;
    pub const STEP: u32 = 0x2d;
    pub const SUBSTEP: u32 = 0x2e;
    /// "Continue" mode: phase 0 jumps to its last step.
    pub const CONTINUE: u32 = 0x25;
    /// Game active.
    pub const ACTIVE: u32 = 0x30;
    /// Scene number (signed byte).
    pub const SCENE: u32 = 0x34;
    /// Cutscene in progress (≠ 0).
    pub const CINEMATIC: u32 = 0x44;
    /// Step countdown (word); in game, BCD clock on three bytes.
    pub const TIMER: u32 = 0x38;
    /// Vertical scroll of plane 2 (word).
    pub const SCROLL2_Y: u32 = 0x4ba;
}

/// Natives of other modules still called by their address (`call`).
mod ext {
    pub const PHASE0_END_FADE: u32 = 0x1cf3c;
}

// ---------------------------------------------------------------------------------------------
// World update (0x1c86e)

/// One step of the world update.
#[derive(Clone, Copy)]
enum UpdateStep {
    /// Charge gauge, with its marker (D1).
    Gauge(u32),
    Players,
    PlayerParts,
    /// Dispatch of an object class.
    Objs(&'static Class),
    Cameras,
}
use UpdateStep::{Cameras, Gauge, Objs, PlayerParts, Players};

const G_PLAYER: u32 = 0x50_0001;
const G_ENEMY: u32 = 0x45_0002;
const G_SCENERY: u32 = 0x53_0003;

/// A step and the address of its call in the original code (`jsr`, 6 bytes; the last call of
/// each order is a `jmp`). The return address of a call is that of the resumption step ([`Step::World`]).
type Order = &'static [(u32, UpdateStep)];

/// Normal order (0x1c894): players and their shots, then bosses, enemies, enemy shots, effects, then
/// scenery, crates, items, cameras, system objects.
const ORDER_NORMAL: Order = &[
    (0x1c89a, Gauge(G_PLAYER)), (0x1c8a0, Players), (0x1c8a6, PlayerParts),
    (0x1c8ac, Objs(&dispatch::PSHL)), (0x1c8b2, Objs(&dispatch::PEFF)),
    (0x1c8be, Gauge(G_ENEMY)), (0x1c8c4, Objs(&dispatch::BOSS)), (0x1c8ca, Objs(&dispatch::ENEMY)),
    (0x1c8d0, Objs(&dispatch::ESHL)), (0x1c8d6, Objs(&dispatch::EFFCT)),
    (0x1c8e2, Gauge(G_SCENERY)), (0x1c8e8, Objs(&dispatch::SET)), (0x1c8ee, Objs(&dispatch::BOX)),
    (0x1c8f4, Objs(&dispatch::ITEM)), (0x1c8fa, Cameras),
    (0x1c900, Objs(&dispatch::SOBJ)),
];

/// Cutscene (`A5+0x44` ≠ 0, 0x1c906): the effects come first.
const ORDER_CINEMATIC: Order = &[
    (0x1c90c, Gauge(G_ENEMY)), (0x1c912, Objs(&dispatch::EFFCT)),
    (0x1c91e, Gauge(G_PLAYER)), (0x1c924, Players), (0x1c92a, PlayerParts),
    (0x1c930, Objs(&dispatch::PSHL)), (0x1c936, Objs(&dispatch::PEFF)),
    (0x1c942, Gauge(G_ENEMY)), (0x1c948, Objs(&dispatch::BOSS)), (0x1c94e, Objs(&dispatch::ENEMY)),
    (0x1c954, Objs(&dispatch::ESHL)),
    (0x1c960, Gauge(G_SCENERY)), (0x1c966, Objs(&dispatch::SET)), (0x1c96c, Objs(&dispatch::BOX)),
    (0x1c972, Objs(&dispatch::ITEM)), (0x1c978, Cameras),
    (0x1c97e, Objs(&dispatch::SOBJ)),
];

/// Scenes 0x45 to 0x52 (0x1c984): neither players nor enemies.
const ORDER_REDUCED: Order = &[
    (0x1c98a, Gauge(G_ENEMY)), (0x1c990, Objs(&dispatch::EFFCT)), (0x1c996, Objs(&dispatch::SET)),
    (0x1c99c, Cameras), (0x1c9a2, Objs(&dispatch::PEFF)),
    (0x1c9a8, Objs(&dispatch::SOBJ)),
];

/// Scene 8 (0x1c9ae): like the normal order, but the effects come after the cameras.
const ORDER_SCENE_8: Order = &[
    (0x1c9b4, Gauge(G_PLAYER)), (0x1c9ba, Players), (0x1c9c0, PlayerParts),
    (0x1c9c6, Objs(&dispatch::PSHL)), (0x1c9cc, Objs(&dispatch::PEFF)),
    (0x1c9d8, Gauge(G_ENEMY)), (0x1c9de, Objs(&dispatch::BOSS)), (0x1c9e4, Objs(&dispatch::ENEMY)),
    (0x1c9ea, Objs(&dispatch::ESHL)),
    (0x1c9f6, Gauge(G_SCENERY)), (0x1c9fc, Objs(&dispatch::SET)), (0x1ca02, Objs(&dispatch::BOX)),
    (0x1ca08, Objs(&dispatch::ITEM)), (0x1ca0e, Cameras), (0x1ca14, Objs(&dispatch::EFFCT)),
    (0x1ca1a, Objs(&dispatch::SOBJ)),
];

/// Update order according to the cutscene and the scene (signed comparisons on a byte).
fn world_order(b: &mut Cps2Bus) -> Order {
    let scene = rd8(b, A5 + ram::SCENE) as i8;
    if rd8(b, A5 + ram::CINEMATIC) != 0 {
        ORDER_CINEMATIC
    } else if scene >= 0x53 {
        ORDER_NORMAL
    } else if scene >= 0x45 {
        ORDER_REDUCED
    } else if scene == 8 {
        ORDER_SCENE_8
    } else {
        ORDER_NORMAL
    }
}

/// Updates all the objects: players, then the ten classes in the scene's order. `Flow::Yield`: an object
/// handler made the task cede control; it will resume at the step [`Step::World`], then at the continuation that the
/// caller stored in its pending calls before the call.
pub fn update_world(c: &mut Cpu, b: &mut Cps2Bus) -> Flow {
    let order = world_order(b);
    let order_index = ORDERS.iter().position(|&o| std::ptr::eq(o, order)).expect("update order") as u8;
    run_steps(c, b, order_index, 0)
}

/// The steps `from..` of the order `ORDERS[order_index]`.
fn run_steps(c: &mut Cpu, b: &mut Cps2Bus, order_index: u8, from: usize) -> Flow {
    let order = ORDERS[order_index as usize];
    for (i, &(_, step)) in order.iter().enumerate().skip(from) {
        match step {
            Gauge(marker) => gauge(c, b, marker),
            // 0x35292
            Players => crate::player::update::update_all(&mut crate::player::Ctx::new(c, b)),
            // 0x392e4: sub-objects of the players (front and back weapons)
            PlayerParts => crate::player::overlays::update(&mut crate::player::Ctx::new(c, b)),
            // last call: a `jmp`, the dispatch loop returns directly to the caller
            Objs(class) if i + 1 == order.len() => return class.update(c, b),
            Objs(class) => {
                table::call(b, Step::World { order_index, step: i as u8 });
                if class.update(c, b) == Flow::Yield {
                    return Flow::Yield;
                }
                table::pop_return(c, b);
            }
            // 0x33e20. Original glitch: the D4 of the scenery copy is reread by a following handler
            // (boss13_dragon, frame 7241); the original's A0 (camera 2) is not
            Cameras => {
                for (v, m) in crate::scroll::update_cameras(b).into_iter().flatten() {
                    c.d[4] = c.d[4] & !m | v;
                }
            }
        }
    }
    Flow::Done
}

/// Length of an absolute `jsr`: the return address is that of the call site + 6.
const JSR: u32 = 6;

/// Charge gauge, with its marker (D1).
fn gauge(c: &mut Cpu, b: &mut Cps2Bus, marker: u32) {
    // 0xb3e. Original glitch: the hidden gauge leaves D2.w = 0x400 (DSW test), which a handler writes into an
    // object field (DDSOM_POISON=call_b3e: long_nain_57_86, frame 13398, ram ffd692)
    c.d[1] = marker;
    crate::video::gauge_68k(c, b);
}

/// The four update orders.
const ORDERS: [Order; 4] = [ORDER_NORMAL, ORDER_CINEMATIC, ORDER_REDUCED, ORDER_SCENE_8];

/// Resumptions of the game loop when an object handler made the task cede control.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Step {
    /// Return from the dispatch of an object class, step `step` of the order `ORDERS[order_index]`: the world
    /// update continues at the next step, then returns to its caller.
    World { order_index: u8, step: u8 },
    /// Return of the world update in phase 12 (`0x1c702`): end of the phase.
    AfterWorld,
}

impl Step {
    /// Address that the original resumes at: the one following the call (`jsr`).
    pub const fn address(self) -> u32 {
        match self {
            Step::World { order_index, step } => ORDERS[order_index as usize][step as usize].0 + JSR,
            Step::AfterWorld => WORLD_RETURN,
        }
    }

    pub fn step(self, x: &mut crate::scene::Ctx) -> Flow {
        match self {
            Step::World { order_index, step } => run_steps(x.c, x.b, order_index, step as usize + 1),
            Step::AfterWorld => {
                phase12_after_world(x.c, x.b);
                Flow::Done
            }
        }
    }
}

/// World update called by address (scene sequences, demo): a handler never cedes control there.
pub const WORLD_UPDATE: Native = Native {
    entry: 0x1c86e,
    name: "world_update",
    run: crate::Body::Processeur(|c, b| {
        if update_world(c, b) == Flow::Done {
            rts(c, b);
        }
    }),
    touches_sound: false,
    outputs: 0, // the callers chain to other calls
};

// ---------------------------------------------------------------------------------------------
// End of frame (0x1c4c4)

/// Queues of collision candidates, cleared every frame.
const QUEUES: [&Queue; 5] =
    [&pools::QUEUE_PSHL, &pools::QUEUE_ESHL, &pools::QUEUE_ENEMY_BOSS, &pools::QUEUE_BOX, &pools::QUEUE_ITEM];

/// End of frame (`0x1c4c4`), when the phase returns (step `tasks::game::Step::FrameEnd`): the collision
/// queues start empty again. The `rts` that follows returns to the start of the game loop (`0x1c468`).
pub fn frame_end(c: &mut Cpu, b: &mut Cps2Bus) {
    for queue in QUEUES {
        queue.reset(b);
    }
    // the original leaves A0 on the last queue (the game inherits it on the next frame)
    c.a[0] = A5 + pools::QUEUE_ITEM.array;
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
const PLAYER_UI_TASK: u32 = 0x150ac;

/// `A5` bytes reset to zero on exit: flag and score.
const EXIT_CLEARED_BYTES: [u32; 2] = [0x37, 0x5d];

fn next_step(b: &mut Cps2Bus) {
    let step = rd8(b, A5 + ram::STEP).wrapping_add(4);
    wr8(b, A5 + ram::STEP, step);
}

/// Step 0: three SET objects (palette animations), prepares the players.
fn create_palette_objects(b: &mut Cps2Bus) {
    next_step(b);
    wr16(b, A5 + ram::TIMER, 0x40);
    wr8(b, A5 + ram::ACTIVE, 1);
    wr8(b, CLEARED_A, 0);
    crate::objects::pools::SET.reset(b);
    for (variant, index) in SET_PALETTES {
        // the pool has just been emptied: the allocation succeeds
        let set = pools::SET.alloc(b).expect("SET pool empty");
        set.set_u8(b, Obj::ACTIVE, 1);
        set.set_u8(b, Obj::KIND, 0);
        set.set_u8(b, Obj::VARIANT, variant);
        set.set_u8(b, SET_PALETTE_INDEX, index);
    }
    crate::palette::set_flash_color(b, crate::palette::FLASH_COLOR_WHITE_VALUE);
    // 0x3525c
    crate::video::clear_layer1_start(b);
    let y = rd16(b, A5 + ram::SCROLL2_Y).wrapping_add(0x100);
    wr16(b, A5 + ram::SCROLL2_Y, y);
}

/// Step 4: erases the first two text rectangles (then moves to step 8).
fn clear_text_boxes(b: &mut Cps2Bus) {
    next_step(b);
    crate::text::clear_box(b, TEXT_BOXES[0]);
    crate::text::clear_box(b, TEXT_BOXES[1]);
    let y = rd16(b, A5 + ram::SCROLL2_Y).wrapping_sub(0x100);
    wr16(b, A5 + ram::SCROLL2_Y, y);
}

/// Start of steps 8 and 12: system objects and animated scenery, sprites, then countdown.
/// Returns true when the countdown has elapsed.
fn animate(c: &mut Cpu, b: &mut Cps2Bus) -> bool {
    dispatch::SOBJ.update_no_next(c, b);
    dispatch::SET.update_no_next(c, b);
    // 0x1de96
    crate::sprites::build_68k(c, b);
    let left = rd16(b, A5 + ram::TIMER).wrapping_sub(1);
    wr16(b, A5 + ram::TIMER, left);
    (left as i16) < 0
}

/// Step 8: countdown elapsed, a SET object (type 0xc) appears and step 12 begins.
fn wait_then_spawn(c: &mut Cpu, b: &mut Cps2Bus) {
    if !animate(c, b) {
        return;
    }
    let Some(set) = pools::SET.alloc(b) else { return };
    set.set_u8(b, Obj::ACTIVE, 1);
    set.set_u8(b, Obj::KIND, 0xc);
    next_step(b);
    wr16(b, A5 + ram::TIMER, 0x20);
}

/// Step 12: countdown elapsed, end fade and third palette animation, then step 16.
fn wait_then_fade(c: &mut Cpu, b: &mut Cps2Bus) {
    if !animate(c, b) {
        return;
    }
    call(c, b, ext::PHASE0_END_FADE);
    crate::text::clear_box(b, TEXT_BOXES[2]);
    reset_pools_and_leave(c, b);
}

/// Step 16: pools made new again, then exit from the phase.
fn reset_pools_and_leave(c: &mut Cpu, b: &mut Cps2Bus) {
    wr8(b, A5 + ram::STEP, 0);
    // 0x1de7c
    crate::sprites::clear_lists(b);
    crate::objects::pools::SOBJ.reset(b);
    crate::objects::pools::SET.reset(b);
    leave(c, b);
}

/// Exit from phase 0 (also in continue mode): next phase, first level, interface task.
fn leave(c: &mut Cpu, b: &mut Cps2Bus) {
    let phase = rd8(b, A5 + ram::PHASE).wrapping_add(4);
    wr8(b, A5 + ram::PHASE, phase);
    wr8(b, CLEARED_B, 0);
    wr8(b, A5 + ram::ACTIVE, 1);
    crate::objects::rank::load_current_preset(b);
    wr8(b, A5 + ram::SCENE, 1);
    wr8(b, A5 + EXIT_CLEARED_BYTES[0], 0);
    wr16(b, A5 + ram::TIMER, 0);
    wr16(b, A5 + ram::TIMER + 2, 0);
    wr8(b, A5 + EXIT_CLEARED_BYTES[1], 0);
    crate::families::items::clear_picked_68k(c, b);
    c.d[0] = c.d[0] & 0xffff_0000 | PLAYER_UI_SLOT;
    c.a[0] = PLAYER_UI_TASK;
    crate::kernel::create_task(b, PLAYER_UI_SLOT as u32, PLAYER_UI_TASK);
}

/// Phase 0: one step per frame. The step (`A5+0x2d`: 0, 4, 8, 12, 16) chooses what follows.
pub fn phase0(c: &mut Cpu, b: &mut Cps2Bus) {
    if rd8(b, A5 + ram::CONTINUE) != 0 {
        return leave(c, b);
    }
    match rd8(b, A5 + ram::STEP) {
        0 => create_palette_objects(b),
        4 => {
            clear_text_boxes(b);
            wait_then_spawn(c, b);
        }
        8 => wait_then_spawn(c, b),
        12 => wait_then_fade(c, b),
        16 => reset_pools_and_leave(c, b),
        step => panic!("phase 0: unknown step {step}"),
    }
}

pub const PHASE_0: Native = Native {
    entry: 0x1c522,
    name: "game_phase0",
    run: crate::Body::Processeur(|c, b| {
        phase0(c, b);
        rts(c, b);
    }),
    touches_sound: false,
    outputs: 0,
};

// ---------------------------------------------------------------------------------------------
// Phase 12: the game proper (0x1c6a0)

/// Game clock in BCD: minutes (`+0x38`), seconds, frames (0 to 59).
const CLOCK: u32 = 0x38;
/// Game frame counter (word, capped at 0xffff).
const GAME_FRAMES: u32 = 0x54;
/// Buttons pressed this frame (+1) and the previous one (+7), for the debug level skip.
const BUTTONS_NOW: u32 = 0x01;
const BUTTONS_BEFORE: u32 = 0x07;
/// Words reset to zero by a level skip (status of each player and of its weapons).
const PLAYER_RESET: [u32; 8] = [0x5e2, 0x7e2, 0x9e2, 0xbe2, 0x602, 0x802, 0xa02, 0xc02];
/// X flag of the status register (carry of `sbcd`).
const SR_X: u16 = 0x10;
/// ROM word whose bit 15 enables the debug level skip.
const DEBUG_SWITCH: u32 = 0xc0;
/// Last scene number.
const LAST_SCENE: i8 = 0x6c;
/// Level skip buttons: next scene, previous scene.
const BUTTONS_NEXT: u8 = 0x90;
const BUTTONS_PREVIOUS: u8 = 0x60;

/// `d - s - X` in BCD on a byte: result and carry (that of `sbcd`).
pub(crate) fn sbcd(d: u8, s: u8, x: bool) -> (u8, bool) {
    let (d, s, x) = (d as i32, s as i32, x as i32);
    let unadjusted = d - s - x;
    let mut adjust = 0;
    if (d & 0x0f) - (s & 0x0f) - x < 0 {
        adjust |= 0x06;
    }
    let borrow = unadjusted < 0 || unadjusted - adjust < 0;
    if unadjusted < 0 {
        adjust |= 0x60;
    }
    ((unadjusted - adjust) as u8, borrow)
}

/// Removes a frame from the clock (frames, seconds, minutes in cascade, from 59 to 00): the cascade
/// stops as soon as a byte stays positive. The carry starts from the X flag left by the previous
/// code, hence the read of `SR`.
fn tick_clock(c: &mut Cpu, b: &mut Cps2Bus) {
    let mut borrow = c.sr & SR_X != 0;
    for (i, offset) in [CLOCK + 2, CLOCK + 1, CLOCK].into_iter().enumerate() {
        let (value, next_borrow) = sbcd(rd8(b, A5 + offset), 1, borrow);
        borrow = next_borrow;
        c.sr = c.sr & !SR_X | if borrow { SR_X } else { 0 };
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

/// Debug level skip: players reset to zero, return to phase 8 (scene entry).
fn change_scene(b: &mut Cps2Bus, forward: bool) {
    for offset in PLAYER_RESET {
        wr16(b, A5 + offset, 0);
        wr16(b, A5 + offset + 2, 0);
    }
    wr8(b, A5 + 0x50, 0);
    let scene = rd8(b, A5 + ram::SCENE);
    let scene = if forward {
        let next = scene.wrapping_add(1);
        if next as i8 <= LAST_SCENE { next } else { 1 }
    } else {
        let previous = scene.wrapping_sub(1);
        if previous as i8 > 0 { previous } else { LAST_SCENE as u8 }
    };
    wr8(b, A5 + ram::SCENE, scene);
    wr8(b, A5 + ram::PHASE, 8);
    wr8(b, A5 + ram::STEP, 0);
    wr16(b, A5 + ram::SUBSTEP, 0);
}

/// Phase 12: one game frame. `Flow::Yield`: an object handler made the task cede control (it
/// will resume at [`Step::AfterWorld`] after the world update).
pub fn phase12(c: &mut Cpu, b: &mut Cps2Bus) -> Flow {
    if rd16(b, A5 + GAME_FRAMES) != 0xffff {
        let frames = rd16(b, A5 + GAME_FRAMES) + 1;
        wr16(b, A5 + GAME_FRAMES, frames);
        c.sr &= !SR_X; // the addition has no carry
    }
    if rd16(b, A5 + CLOCK) != 0 {
        tick_clock(c, b);
    }
    // 0x2aec
    // Original glitch: the input bytes that the copy leaves in D0-D4 (and its flags) are reread by a handler of the
    // world update, which writes them into an object (DDSOM_POISON=call_2aec: long_elfe_objets, frame 13133,
    // ram ffd694)
    crate::inputs::copy_68k(c, b);
    table::call(b, Step::AfterWorld);
    if update_world(c, b) == Flow::Yield {
        return Flow::Yield;
    }
    table::pop_return(c, b);
    phase12_after_world(c, b);
    Flow::Done
}

/// Return of the world update in phase 12 (`jsr 0x1c86e` at 0x1c6fc).
const WORLD_RETURN: u32 = 0x1c6fc + JSR;

/// End of phase 12, after the world update: [`after_world`], then the debug level skip.
fn phase12_after_world(c: &mut Cpu, b: &mut Cps2Bus) {
    after_world(c, b);
    debug_scene_skip(b);
}

/// After the world update (game phase 12 `0x1c702` and demo `0x1fa4a`): sprites, collisions,
/// pop-ups, spawn script, rank, game over; each preceded by its charge gauge.
pub fn after_world(c: &mut Cpu, b: &mut Cps2Bus) {
    gauge(c, b, 0x54_0000);
    // 0x1de96. Original glitch: D4 (attributes of the last piece) reread later (DDSOM_POISON=call_1de96:
    // long_28_scenes, frame 10431), see `sprites::build`
    crate::sprites::build_68k(c, b);
    gauge(c, b, 0x48_0002);
    // 0x260ee
    crate::combat::combat_update(&mut crate::combat::World { c, b, a2: 0 });
    gauge(c, b, 0x4f_001e);
    // 0x19e20: pop-ups; 0xf2e5c: spawn script reader
    crate::popups::animer(b);
    crate::scene::script::step(c, b);
    // 0x1f27c (registers of the original: the task keeps them until the next frame)
    crate::objects::rank::tick(b);
    // 0x28d8
    crate::aids::game_over(c, b);
}

/// Debug level skip (end of phase 12 and of phase 8, `0x1c74a`): inactive in the production
/// game (bit 15 of the ROM word `0xc0` is zero).
pub fn debug_scene_skip(b: &mut Cps2Bus) {
    if rd16(b, DEBUG_SWITCH) & 0x8000 != 0 {
        let pressed = !rd8(b, A5 + BUTTONS_BEFORE) & rd8(b, A5 + BUTTONS_NOW);
        if pressed & BUTTONS_NEXT == BUTTONS_NEXT {
            change_scene(b, true);
        } else if pressed & BUTTONS_PREVIOUS == BUTTONS_PREVIOUS {
            change_scene(b, false);
        }
    }
}

pub const PHASE_12: Native = Native {
    entry: 0x1c6a0,
    name: "game_phase12",
    run: crate::Body::Processeur(|c, b| {
        if phase12(c, b) == Flow::Done {
            rts(c, b);
        }
    }),
    touches_sound: false,
    outputs: 0,
};

/// Native routines of the game loop.
pub const NATIVES: &[Native] = &[WORLD_UPDATE, PHASE_0, PHASE_12];

#[cfg(test)]
mod tests {
    use super::*;

    /// Each class is updated only once per frame, in all the complete orders.
    #[test]
    fn full_orders_without_duplicates() {
        for order in [ORDER_NORMAL, ORDER_CINEMATIC, ORDER_SCENE_8] {
            let mut entries: Vec<u32> =
                order.iter().filter_map(|(_, s)| if let Objs(class) = s { Some(class.entry) } else { None }).collect();
            entries.sort();
            entries.dedup();
            assert_eq!(entries.len(), 10);
        }
    }

    /// The call sites follow one another: a `jsr` (6 bytes), or a `move.l #,D1` (6 bytes) then the
    /// `jsr` of the gauge.
    #[test]
    fn increasing_sites() {
        for order in [ORDER_NORMAL, ORDER_CINEMATIC, ORDER_REDUCED, ORDER_SCENE_8] {
            for w in order.windows(2) {
                let gap = w[1].0 - w[0].0;
                assert!(gap == JSR || gap == 2 * JSR && matches!(w[1].1, Gauge(_)), "{:x}", w[1].0);
            }
        }
    }
}

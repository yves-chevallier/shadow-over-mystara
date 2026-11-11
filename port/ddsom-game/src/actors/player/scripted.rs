//! Scripted state of the players (`+4` = 0): scene entries, revival, cinematics (P3).
//!
//! In this state the player obeys a small script rather than the controllers. Byte `+5` selects the
//! kind of script; `+6` is then the script *step* (and `+7` a sub-step):
//!
//! | `+5` | kind | dispatch |
//! |---|---|---|
//! | 0 | scene cinematic | scene number (`A5+0x34`) -> routine (table 0x23826) |
//! | 4 | revival | step `+6` -> routine (offsets, table 0x23b98) |
//! | 8 | wait for controls to open | step `+6` -> routine (table 0x23d66) |
//! | 0xc | end cinematic | scene number -> routine (table 0x23f4e), without reinsertion |
//!
//! Like `hurt`, the routines are reached through the code tables of the original; `HANDLERS`
//! gives their Rust version (all the reachable ones are rewritten).

use super::actions::start_stand;
use super::checks::{special_attack, throw_button};
use super::effects::restore_palette;
use super::states::State;
use super::walk::{move_x, move_xz, move_z};
use super::{animation, cinematics, ram, Ctx, Player};
use crate::world::objects::lists::ACTORS;
use crate::world::objects::Obj;
use crate::actors::player::external as ext;
use crate::actors::player::tables::{player_data, pc16, Aligned};

type Handler = fn(&mut Ctx, Player);

/// Identifiers (original addresses) of the routines reached (sounds).
mod rom {
    pub const INTRO_ACTION: crate::services::sound::Sound = crate::services::sound::CMD_20;
}

/// Code tables.
mod tables {
    /// Revival: offsets (words) relative to the table, indexed by step `+6`.
    pub const REVIVE_STEPS: u32 = 0x23b98;
    /// Steps of the scene 1 cinematic.
    pub const SCENE1_STEPS: u32 = 0x24eae;
    /// Walk-in: steps.
    pub const WALK_IN_STEPS: u32 = 0x23a00;
    /// Steps of the wait for the controls (`+7`).
    pub const WAIT_SUBSTEPS: u32 = 0x23d7a;
    /// Durations (words) of the scene entry by player.
    pub const WALK_IN_WAIT: u32 = 0x23a56;
    /// Waiting for the controls, step 4 (`+7`): walks to the spot, then thrown by the boss.
    pub const THROWN_SUBSTEPS: u32 = 0x23e0e;
    /// Target depth (words) by player.
    pub const THROWN_ROWS: u32 = 0x23e58;
    /// Revival while the inputs are frozen: x offsets (words) by player.
    pub const FROZEN_OFFSETS: u32 = 0x23d04;
}

// Cinematic signal shared with boss 21 (`ram::BOSS_SIGNAL`): 0x5c when it throws the
// players, 4 once the first has landed.
const SIGNAL_THROW: u16 = 0x5c;
const SIGNAL_LANDED: u16 = 4;
/// Depth aimed at by the player (word).
const TARGET_ROW: u32 = 0x1e2;

/// Rewritten: original address -> Rust routine.
const HANDLERS: &[(u32, Handler)] = &[
    (0x24ea0, |g, p| step_table(g, p, tables::SCENE1_STEPS)),
    (0x24ebc, scene1_start),
    (0x24eea, scene1_walk),
    (0x24f12, scene1_wait_progress),
    (0x24f40, scene1_wait),
    (0x24f60, scene1_pose),
    (0x24f98, scene1_dash_start),
    (0x24fc8, scene1_dash),
    (0x239f2, |g, p| step_table(g, p, tables::WALK_IN_STEPS)),
    (0x23a0a, walk_in_start),
    (0x23a2e, walk_in_walk),
    (0x23a5e, walk_in_wait),
    (0x23a7c, walk_in_pose),
    (0x23ab2, walk_in_end),
    (0x24102, idle),
    (0x24118, |_, _| {}),
    (0x23ba6, revive_start),
    (0x23c16, revive_fall),
    (0x23c3c, revive_land),
    (0x23c58, revive_end),
    (0x23c7a, revive_frozen),
    (0x23d0c, revive_in_place),
    (0x23d6e, |g, p| substep_table(g, p, tables::WAIT_SUBSTEPS)),
    (0x23d82, wait_start),
    (0x23db4, wait_delay),
    (0x23dde, wait_pose),
    (0x23df2, wait_inputs),
    (0x23e02, |g, p| substep_table(g, p, tables::THROWN_SUBSTEPS)),
    (0x23e20, thrown_turn),
    (0x23e60, thrown_turned),
    (0x23e76, thrown_align),
    (0x23e98, thrown_walk),
    (0x23eb4, thrown_wait_signal),
    (0x23ee4, thrown_first_fall),
    (0x23ede, |g, p| {
        animation::step(g, p);
    }),
    (0x23f0a, thrown_fall),
    (0x23f2a, thrown_end),
];

/// Rust version of the original routine `addr`, if it is rewritten.
fn handler(addr: u32) -> Option<Handler> {
    HANDLERS.iter().chain(cinematics::HANDLERS).chain(cinematics::INTRO_HANDLERS).find(|h| h.0 == addr).map(|h| h.1)
}

fn run_at(g: &mut Ctx, p: Player, addr: u32) {
    cov!();
    match handler(addr) {
        Some(f) => f(g, p),
        // every entry of the tables by scene is rewritten, and there is no native at the addresses of the step
        // tables: the game without 68000 already stopped there (`docs/code-mort.md`, "Sortie du Cpu, partie 6")
        None => unreachable!("scripted routine {addr:06x} not rewritten"),
    }
}

/// Jump by offset table indexed by step `+6`.
fn step_table(g: &mut Ctx, p: Player, base: u32) {
    cov!();
    let step = p.u8(g, Player::ACTION) as u32;
    let offset = pc16(g.b, base + step) as i16 as i32 as u32;
    run_at(g, p, base.wrapping_add(offset));
}

/// Same with sub-step `+7`.
fn substep_table(g: &mut Ctx, p: Player, base: u32) {
    cov!();
    let step = p.u8(g, Player::STEP) as u32;
    let offset = pc16(g.b, base + step) as i16 as i32 as u32;
    run_at(g, p, base.wrapping_add(offset));
}

/// 0x352da: dispatches according to `+5`.
pub fn update(g: &mut Ctx, p: Player) {
    cov!();
    match p.u8(g, Player::SUB) {
        0 => {
            let scene = ram::SCENE.get(g.b) as u32;
            let routine = player_data().scripted_by_scene.long_at(g.b, 4 * scene);
            run_at(g, p, routine);
            ACTORS.reinsert(g.b, p.obj());
        }
        4 => {
            step_table(g, p, tables::REVIVE_STEPS);
            ACTORS.reinsert(g.b, p.obj());
        }
        8 => {
            let step = p.u8(g, Player::ACTION) as u32;
            let routine = player_data().scripted_wait_steps.long_at(g.b, step);
            run_at(g, p, routine);
            if !throw_button(g, p) {
                special_attack(g, p);
            }
            ACTORS.reinsert(g.b, p.obj());
        }
        0xc => {
            let scene = ram::SCENE.get(g.b) as u32;
            // the original dispatch (0x23f40) jumps with D0 = 4 × scene and A1 = routine: only the step out of the
            // table of 0x24aba reads A1 (`cinematics::stray_hex_dump`)
            let routine = end_routine(g, scene);
            run_at(g, p, routine);
        }
        _ => {}
    }
}

// ---- end cinematic ------------------------------------------------------------------------

/// Start of the original dispatch of the end cinematic (indexed by scene).
pub const END_BY_SCENE_ENTRY: crate::world::data::RomAddr = crate::world::data::RomAddr(0x23f40);
/// Last scene of the table 0x23f4e; beyond it (demo scenes 109 to 114), the original reads code as
/// addresses and jumps at random (scene 110 ends up at animation 0x1a32 with the registers left by the
/// previous routine, hence sprite pieces written by tens of thousands). These scenes have no
/// cinematic: the inactive routine 0x24102.
pub(crate) const LAST_END_SCENE: u32 = 108;
pub(crate) const NO_END: crate::world::data::RomAddr = crate::world::data::RomAddr(0x24102);

fn end_routine(g: &mut Ctx, scene: u32) -> u32 {
    if scene > LAST_END_SCENE {
        NO_END.0
    } else {
        player_data().scripted_end_by_scene.long_at(g.b, 4 * scene)
    }
}

// ---- tools -----------------------------------------------------------------------------------

fn next_action(g: &mut Ctx, p: Player) {
    cov!();
    let step = p.u8(g, Player::ACTION).wrapping_add(2);
    p.set_u8(g, Player::ACTION, step);
}

fn next_step(g: &mut Ctx, p: Player) {
    cov!();
    let step = p.u8(g, Player::STEP).wrapping_add(2);
    p.set_u8(g, Player::STEP, step);
}

/// Countdown at `+0x1e0` (word); true when it reaches zero.
fn countdown(g: &mut Ctx, p: Player) -> bool {
    cov!();
    let left = p.u16(g, Player::SCRIPT_TIMER).wrapping_sub(1);
    p.set_u16(g, Player::SCRIPT_TIMER, left);
    left == 0
}

/// The player gets control back: normal game state.
fn give_control(g: &mut Ctx, p: Player) {
    cov!();
    p.set_state(g, State::pack(2, 0, 0, 0).0);
}

/// Word of the table indexed by the player number.
fn per_player(g: &mut Ctx, p: Player, table: u32) -> u16 {
    cov!();
    let k = p.u8(g, Player::INDEX) as u32;
    pc16(g.b, table + 2 * k)
}

// ---- scene 1 cinematic: walking in, pose, final run ------------------------

/// 0x24ebc: start of the walk-in (speed 2 pixels per frame).
fn scene1_start(g: &mut Ctx, p: Player) {
    cov!();
    next_action(g, p);
    p.set_u8(g, Player::ACTIVE, 1);
    p.set_u8(g, Player::SPECIAL_ACTIVE, 0);
    p.set_u8(g, Player::ARMED, 0);
    p.set_u16(g, Player::SCRIPT_TIMER, 0x120);
    p.set_u32(g, Obj::VX, 0x0200_0000);
    p.set_u32(g, Obj::VZ, 0);
    p.set_u32(g, Obj::VY, 0);
    animation::start(g, p, 2);
}

/// 0x24eea: walk-in; at the end of the countdown, standing.
fn scene1_walk(g: &mut Ctx, p: Player) {
    cov!();
    if countdown(g, p) {
        next_action(g, p);
        animation::start(g, p, 0);
        return;
    }
    if p.u16(g, Player::SCRIPT_TIMER) > 0x50 {
        return;
    }
    animation::step(g, p);
    move_x(g, p);
}

/// 0x24f12: waits for the scene (`A5+0x2e`) to reach step 6.
fn scene1_wait_progress(g: &mut Ctx, p: Player) {
    cov!();
    if ram::SUB_STEP.get(g.b) == 6 {
        next_action(g, p);
        let k = p.u8(g, Player::RANK) as u32;
        let delay = player_data().scripted_scene1_wait.word_at(g.b, 2 * k);
        p.set_u16(g, Player::SCRIPT_TIMER, delay);
        crate::actors::families::effects_creators::for_players::halo(&mut g.leftovers, g.b, p.obj());
    }
    animation::step(g, p);
}

/// 0x24f40: waits, then strikes the pose (animation 0x33, weapon in hand).
fn scene1_wait(g: &mut Ctx, p: Player) {
    cov!();
    if countdown(g, p) {
        next_action(g, p);
        p.set_u8(g, Player::ARMED, 1);
        animation::start(g, p, 0x33);
        return;
    }
    animation::step(g, p);
}

/// 0x24f60: pose, then the hero's cry at the moment of the gesture (according to the class), return to standing.
fn scene1_pose(g: &mut Ctx, p: Player) {
    cov!();
    if animation::step(g, p) {
        next_action(g, p);
        animation::start(g, p, 0);
        return;
    }
    if p.u8(g, Player::ANIM_EVENT) == 0 {
        return;
    }
    p.set_u8(g, Player::ANIM_EVENT, 0);
    let class = p.class(g);
    if class == 0 || class == 2 {
        super::spawn::swap(g, p, 0);
    }
}

/// 0x24f98: waits for step 8, then the final run (animation 7, speeds by table 0x21).
fn scene1_dash_start(g: &mut Ctx, p: Player) {
    cov!();
    if ram::SUB_STEP.get(g.b) != 8 {
        animation::step(g, p);
        return;
    }
    next_action(g, p);
    p.set_u16(g, Player::SCRIPT_TIMER, 0x70);
    p.set_u8(g, Obj::SPEED_INDEX, 0xa);
    crate::world::objects::motion::select_speed(g.b, p.obj(), &crate::world::objects::motion::SPEEDS_B, 0x21);
    animation::start(g, p, 7);
}

/// 0x24fc8: final run; at the end, the player gets control back.
fn scene1_dash(g: &mut Ctx, p: Player) {
    cov!();
    crate::world::objects::motion::advance_by_table(g.b, p.obj(), crate::world::objects::motion::AXIS_X, 0);
    crate::world::objects::motion::advance_by_table(g.b, p.obj(), crate::world::objects::motion::AXIS_Y, 2);
    if countdown(g, p) {
        give_control(g, p);
    }
    animation::step(g, p);
}

// ---- walk-in of scenes 2, 15, 16... ----------------------------------------------------

/// 0x23a0a.
fn walk_in_start(g: &mut Ctx, p: Player) {
    cov!();
    next_action(g, p);
    p.set_u16(g, Player::ACTIVE, 0x0101);
    p.set_u8(g, Player::SPECIAL_ACTIVE, 0);
    p.set_u8(g, Player::ARMED, 0);
    p.set_u16(g, Player::SCRIPT_TIMER, 0x50);
    animation::start(g, p, 2);
}

/// 0x23a2e: the player advances 2 pixels per frame.
fn walk_in_walk(g: &mut Ctx, p: Player) {
    cov!();
    let x = p.u16(g, Obj::X).wrapping_add(2);
    p.set_u16(g, Obj::X, x);
    if countdown(g, p) {
        next_action(g, p);
        let wait = per_player(g, p, tables::WALK_IN_WAIT);
        p.set_u16(g, Player::SCRIPT_TIMER, wait);
        start_stand(g, p);
        return;
    }
    animation::step(g, p);
}

/// 0x23a5e: waits, then strikes the pose.
fn walk_in_wait(g: &mut Ctx, p: Player) {
    cov!();
    if countdown(g, p) {
        next_action(g, p);
        p.set_u8(g, Player::ARMED, 1);
        animation::start(g, p, 0x33);
        return;
    }
    animation::step(g, p);
}

/// 0x23a7c: pose; the hero's cry; return to standing.
fn walk_in_pose(g: &mut Ctx, p: Player) {
    cov!();
    if animation::step(g, p) {
        next_action(g, p);
        start_stand(g, p);
        return;
    }
    if p.u8(g, Player::ANIM_EVENT) == 0 {
        return;
    }
    p.set_u8(g, Player::ANIM_EVENT, 0);
    let class = p.class(g);
    if class == 0 || class == 2 {
        super::spawn::swap(g, p, 0);
    }
}

/// 0x23ab2: in the game phase, the player gets control back.
fn walk_in_end(g: &mut Ctx, p: Player) {
    cov!();
    if ram::PHASE.get(g.b) == 0xc {
        give_control(g, p);
    }
    animation::step(g, p);
}

/// 0x24102: empty cinematic: a still pose.
pub fn idle(g: &mut Ctx, p: Player) {
    cov!();
    if p.u8(g, Player::ACTION) == 0 {
        next_action(g, p);
        start_stand(g, p);
    } else {
        animation::step(g, p);
    }
}

// ---- revival ------------------------------------------------------------------------------------

/// 0x23ba6: the player falls from the sky at its spot.
fn revive_start(g: &mut Ctx, p: Player) {
    cov!();
    next_action(g, p);
    p.set_u8(g, Player::ACTIVE, 2);
    p.set_u8(g, Player::ARMED, 1);
    p.set_u8(g, Player::FACING, 0x20);
    p.set_u16(g, Obj::VZ, (-0x300i16) as u16);
    p.set_u16(g, Obj::AZ, (-0x80i16) as u16);
    let flags = p.u8(g, Player::FLAGS) & !2;
    p.set_u8(g, Player::FLAGS, flags);
    for field in [Player::SLOW, 0x15c, Player::POWERUP, Player::AURA, Player::AURA_B, Player::AURA_C] {
        p.set_u8(g, field, 0);
    }
    crate::world::placement::place_on_free_ground(g.b, p.obj());
    crate::actors::families::small::revive(&mut g.leftovers, g.b, p.obj());
    animation::start(g, p, 0xd);
    restore_palette(g, p);
    ram::FADE_PAGES.set(g.b, 0xf);
    crate::services::palette::full_brightness(g.b);
    crate::world::objects::rank::raise(g.b);
    crate::world::terrain::push(g.b, p.obj());
}

/// 0x23c16: falls to the ground.
fn revive_fall(g: &mut Ctx, p: Player) {
    cov!();
    move_z(g, p);
    if (p.u16(g, Obj::Z) as i16) >= 0 {
        animation::step(g, p);
        return;
    }
    next_action(g, p);
    p.set_u32(g, Obj::Z, 0);
    p.set_u32(g, Obj::VZ, 0);
    animation::start(g, p, 0xe);
}

/// 0x23c3c: landing, then animation 0x23 and the revival sound.
fn revive_land(g: &mut Ctx, p: Player) {
    cov!();
    if !animation::step(g, p) {
        return;
    }
    next_action(g, p);
    animation::start(g, p, 0x23);
    g.sound(ext::sound::FANFARE, p);
}

/// 0x23c58: end of the animation: the player gets control back, invulnerable for 150 frames.
fn revive_end(g: &mut Ctx, p: Player) {
    cov!();
    if !animation::step(g, p) {
        return;
    }
    give_control(g, p);
    p.set_u8(g, Player::ACTIVE, 1);
    p.set_u8(g, Player::INVULN, 0x96);
    start_stand(g, p);
}

/// 0x23c7a (step 8, state 0x00040800: the player joins the game while the inputs are frozen, `ram::INPUT_RAW`):
/// appears on the ground in front of the object followed by the camera, shifted by its number, and directly plays
/// the revival animation (0x23) with its fanfare; step 10 (0x23c58) gives control back.
fn revive_frozen(g: &mut Ctx, p: Player) {
    cov!();
    next_action(g, p);
    p.set_u8(g, Player::ACTIVE, 2);
    p.set_u8(g, Player::ARMED, 1);
    p.set_u8(g, Player::INVULN, 0x96);
    let flags = p.u8(g, Player::FLAGS) & !2;
    p.set_u8(g, Player::FLAGS, flags);
    for field in [Player::SLOW, 0x15c, Player::POWERUP, Player::AURA, Player::AURA_B, Player::AURA_C, Player::FACING] {
        p.set_u8(g, field, 0);
    }
    cinematics::place_by_focus(g, p, tables::FROZEN_OFFSETS);
    let y = p.u16(g, Obj::Y).wrapping_sub(1);
    p.set_u16(g, Obj::Y, y);
    crate::actors::families::small::revive(&mut g.leftovers, g.b, p.obj());
    animation::start(g, p, 0x23);
    g.sound(ext::sound::FANFARE, p);
    restore_palette(g, p);
    ram::FADE_PAGES.set(g.b, 0xf);
    crate::services::palette::full_brightness(g.b);
    crate::world::objects::rank::raise(g.b);
}

/// 0x23d0c (step 12): the player reappears directly at its spot, standing and empty-handed: it gets
/// control back (normal game state) with animation 0x35.
fn revive_in_place(g: &mut Ctx, p: Player) {
    cov!();
    p.set_state(g, State::pack(2, 0, 0, 0).0);
    p.set_u8(g, Player::ACTIVE, 1);
    let flags = p.u8(g, Player::FLAGS) & !2;
    p.set_u8(g, Player::FLAGS, flags);
    p.set_u8(g, Player::FACING, 0x20);
    p.set_u8(g, Player::ARMED, 0);
    crate::world::placement::place_on_free_ground(g.b, p.obj());
    p.set_u32(g, Obj::Z, 0);
    crate::actors::families::small::revive(&mut g.leftovers, g.b, p.obj());
    animation::start(g, p, 0x35);
    crate::world::objects::rank::raise(g.b);
}

// ---- wait for the controls to open (start of scene) ------------------------------------------

/// 0x23d82.
fn wait_start(g: &mut Ctx, p: Player) {
    cov!();
    next_step(g, p);
    let flags = p.u8(g, Player::FLAGS) & !2;
    p.set_u8(g, Player::FLAGS, flags);
    let k = p.u8(g, Player::RANK) as u32;
    let delay = player_data().scripted_wait_delays.word_at(g.b, 2 * k);
    p.set_u16(g, Player::SCRIPT_TIMER, delay);
    crate::actors::families::effects_creators::for_players::halo(&mut g.leftovers, g.b, p.obj());
    animation::step(g, p);
    g.sound(rom::INTRO_ACTION, p);
}

/// 0x23db4: waits, then turns to the right if necessary.
fn wait_delay(g: &mut Ctx, p: Player) {
    cov!();
    if !countdown(g, p) {
        animation::step(g, p);
        return;
    }
    next_step(g, p);
    if p.u8(g, Player::FACING) == 0 {
        next_step(g, p);
        animation::step(g, p);
        return;
    }
    p.flip(g);
    animation::start(g, p, 5);
}

/// 0x23dde: turn pose.
fn wait_pose(g: &mut Ctx, p: Player) {
    cov!();
    if animation::step(g, p) {
        next_step(g, p);
        start_stand(g, p);
    }
}

/// 0x23df2: as soon as the controls are open, the player gets control back.
fn wait_inputs(g: &mut Ctx, p: Player) {
    cov!();
    if ram::INPUT_MASK.get(g.b) != 0 {
        give_control(g, p);
    }
}

// ---- wait for the controls, step 4: entry thrown by the boss (0x23e02) -----------------------

/// 0x23e20: aims for their depth and turns to the right (turn if needed), then walks.
fn thrown_turn(g: &mut Ctx, p: Player) {
    cov!();
    next_step(g, p);
    let flags = p.u8(g, Player::FLAGS) & !1;
    p.set_u8(g, Player::FLAGS, flags);
    let row = per_player(g, p, tables::THROWN_ROWS);
    p.set_u16(g, TARGET_ROW, row);
    if p.u8(g, Player::FACING) == 0 {
        p.set_u8(g, Player::FACING, 0x20);
        animation::start(g, p, 5);
    } else {
        next_step(g, p);
        animation::start(g, p, 2);
    }
}

/// 0x23e60: end of the turn, then the walk.
fn thrown_turned(g: &mut Ctx, p: Player) {
    cov!();
    if animation::step(g, p) {
        next_step(g, p);
        animation::start(g, p, 2);
    }
}

/// 0x23e76: reaches their depth, one pixel per frame.
fn thrown_align(g: &mut Ctx, p: Player) {
    cov!();
    let (y, row) = (p.u16(g, Obj::Y), p.u16(g, TARGET_ROW));
    if y == row {
        next_step(g, p);
        return;
    }
    let dy = if (y.wrapping_sub(row) as i16) < 0 { 1 } else { 0xffff };
    p.set_u16(g, Obj::Y, y.wrapping_add(dy));
    animation::step(g, p);
}

/// 0x23e98: advances 2 pixels per frame until bumping into the terrain, then standing.
fn thrown_walk(g: &mut Ctx, p: Player) {
    cov!();
    let x = p.u16(g, Obj::X).wrapping_add(2);
    p.set_u16(g, Obj::X, x);
    if crate::world::terrain::push(g.b, p.obj()) {
        animation::step(g, p);
    } else {
        next_step(g, p);
        start_stand(g, p);
    }
}

/// 0x23eb4: waits for the boss's signal, then is thrown into the air (x and z speeds, gravity).
fn thrown_wait_signal(g: &mut Ctx, p: Player) {
    cov!();
    if ram::BOSS_SIGNAL.get(g.b) != SIGNAL_THROW {
        animation::step(g, p);
        return;
    }
    next_step(g, p);
    p.set_u16(g, Obj::VX, 0x200);
    p.set_u16(g, Obj::AX, 0);
    p.set_u16(g, Obj::VZ, 0x580);
    p.set_u16(g, Obj::AZ, 0xffc8);
    crate::world::objects::anim::start_by_character(g.b, p.obj(), &crate::world::objects::anim::SETS_BY_VARIANT, 2);
}

/// Glide: true on landing (altitude back below zero, reset).
fn thrown_flight(g: &mut Ctx, p: Player) -> bool {
    move_xz(g, p);
    if (p.u16(g, Obj::Z) as i16) >= 0 {
        animation::step(g, p);
        return false;
    }
    next_step(g, p);
    p.set_u16(g, Obj::Z, 0);
    true
}

/// 0x23ee4: flight; on landing, notifies the boss (`SIGNAL_LANDED`) and resumes the still pose.
fn thrown_first_fall(g: &mut Ctx, p: Player) {
    cov!();
    if thrown_flight(g, p) {
        ram::BOSS_SIGNAL.set(g.b, SIGNAL_LANDED);
        start_stand(g, p);
    }
}

/// 0x23f0a: same, without signal.
fn thrown_fall(g: &mut Ctx, p: Player) {
    cov!();
    if thrown_flight(g, p) {
        start_stand(g, p);
    }
}

/// 0x23f2a: in the game phase, the player gets control back.
fn thrown_end(g: &mut Ctx, p: Player) {
    cov!();
    if ram::PHASE.get(g.b) == 0xc {
        give_control(g, p);
    }
    animation::step(g, p);
}

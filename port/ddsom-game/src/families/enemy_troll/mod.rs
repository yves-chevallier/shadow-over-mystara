//! "Troll" enemy (ENEMY type 14, 0x6c4d6). See `docs/families/enemy.md`.
//!
//! Same framework as the Goblin (`enemy_goblin`): it walks toward the player, chooses a move from rows of 32 bytes (one
//! per difficulty level `A5+0x7f92 / 2`) and positions itself next to its target. It jumps instead of crouching, throws
//! an object in its series of attacks (`+6` = 4) and runs away walking when a player grabs it.
//!
//! | `+5` | File | Role (h) |
//! |---|---|---|
//! | 0 | `arrival` | arrival according to variant `+3` (0 walk, 2 and 4 fall) |
//! | 2 | `stance` | standing: approach, turn-around, wait, screen exit |
//! | 4, 6, 8 | `attacks` | moves (`+6` = 0 to 20), throw, jumps, teleportation |
//! | 10 | `stance` | waits as long as no player is alive |
//! | state 4 | `hit` | reactions to hits, recoil, fall, flight |
//!
//! Animations 0x16570e (movements), 0x16571c (throw) and 0x165726 (fall). The initialization state chains directly into
//! the body of the active state (the same frame).

use crate::families::cx::Bench;
use super::cx::{handlers, Cx};
use super::enemy_common::*;
use crate::objects::Obj;

mod arrival;
mod attacks;
mod hit;
mod stance;

mod g {
    pub const VULNERABLE: u32 = 0x67;
    pub const AI_WANDER: u32 = 0x6b;
    pub const AI_WAIT: u32 = 0x6e;
    pub const AI_CHARGE: u32 = 0x71;
    pub const AI_LUNGE: u32 = 0x77;
    pub const AI_SHOT: u32 = 0x78;
    pub const SHOT_KIND: u32 = 0x79;
    pub const LOOK_PREV: u32 = 0xa1;
    /// Timer (byte) of a gesture.
    pub const TIMER: u32 = 0xa2;
    pub const COOLDOWN: u32 = 0xa4;
    /// Chosen move (value of `+6` to resume).
    pub const CHOSEN: u32 = 0xaa;
    /// Non-zero: no zone test (arrival by the run-in).
    pub const NO_ZONE: u32 = 0xae;
    /// Remaining throw charges (byte, in steps of 0x10).
    pub const AMMO: u32 = 0xab;
    /// Thrown object (full address).
    pub const THROWN: u32 = 0xa6;
    /// Countdown (high part) of the wait after the throw.
    pub const AFTER_THROW: u32 = 0xac;
}
use g::*;
use crate::objects::pools;
use crate::objects::Current;

const SET_MOVE: u32 = 0x16570e;
const SET_THROW: u32 = 0x16571c;
const SET_HURT: u32 = 0x165726;
const STANCE: u32 = 0x0202_0000;
const HELD: u32 = 0x0408_0000;
const IDLE: u32 = 0x020a_0000;

pub fn run<R: Bench>(x: &mut Cx<'_, R>) {
    if suspended(x) {
        return;
    }
    if var::GAME_ON.get(x.b) != 0 {
        x.tick_counters();
    }
    match x.u8(f::STATE) {
        st::INIT => {
            if init(x) {
                active(x);
            }
        }
        st::ACTIVE => active(x),
        st::HIT => hit::run(x),
        st::DEAD => x.free(&pools::ENEMY),
        _ => {}
    }
}

// --- aides --------------------------------------------------------------------------------------

/// Starts animation `n` of the movement set.
fn mv<R: Bench>(x: &mut Cx<'_, R>, n: u32) {
    x.anim_start(SET_MOVE, n as u16);
}

/// Starts animation `n` of the throw set.
fn throw_anim<R: Bench>(x: &mut Cx<'_, R>, n: u32) {
    x.anim_start(SET_THROW, n as u16);
}

fn set_speed<R: Bench>(x: &mut Cx<'_, R>, n: u32) {
    x.select_speed(&crate::objects::motion::SPEEDS_B, n as u16);
}

/// Byte drawn at random from the difficulty row of `table` (rows of 32 bytes).
fn row_byte<R: Bench>(x: &mut Cx<'_, R>, table: u32) -> u8 {
    let row = ((var::RANK.get(x.b) >> 1) << 5) as u32;
    let at = table.wrapping_add(row as u16 as i16 as i32 as u32);
    let r = rand_and(x, 0x1f);
    x.data8(at + r)
}

/// 0x6eb04: end of a move; standing, and if the draw says so, wait (`+5 = 10`).
fn done_attack<R: Bench>(x: &mut Cx<'_, R>) {
    set_state_long(x, STANCE);
    if lib::maybe_draw_6e(x) {
        x.set8(f::SUB, 0xa);
    }
}

/// 0x6eb34: is a player in box `(d0, d1, d2)` and accepted by the AI gates (0xa4b4, and 0xa4c2 if it looks in the same
/// direction)?
fn box_check<R: Bench>(x: &mut Cx<'_, R>, d0: i16, d1: u16, d2: u16) -> bool {
    if !lib::target_in_box(x, d0, d1, d2) {
        return false;
    }
    let target = load_target(x);
    let facing = target.u8(x.b, 0x2c);
    if facing == x.u8(f::FLIP) {
        if !lib::decision_b(x) {
            return false;
        }
    }
    if !lib::decision_a(x) {
        return false;
    }
    true
}

/// 0x6eb2a: player very close.
fn near_box<R: Bench>(x: &mut Cx<'_, R>) -> bool {
    box_check(x, 0, 0x70, 0x10)
}

/// 0x6eb1c: player at a short distance, in front.
fn far_box<R: Bench>(x: &mut Cx<'_, R>) -> bool {
    box_check(x, -0xd0, 0x60, 0x10)
}

/// 0x6eb62: the Troll has its back to its target.
fn faces_wrong<R: Bench>(x: &mut Cx<'_, R>) -> bool {
    if x.u16(f::TARGET) == 0 {
        return false;
    }
    let target = load_target(x);
    let dx = x.u16(Obj::X).wrapping_sub(target.u16(x.b, Obj::X));
    let want = if (dx as i16) < 0 { 0x20 } else { 0 };
    let wrong = want as u8 != x.u8(f::FLIP);
    wrong
}

const NEAR_TABLE: u32 = 0x6d620;
const FAR_TABLE: u32 = 0x6d820;
/// Rows of moves that follow a series (substate 8).
const FOLLOW_TABLE: u32 = 0x6d364;

/// 0x6eb8a (far series) / 0x6eb90 (near series): chooses the move (`+5 = 4`, `+6 = 0`).
fn pick_attack<R: Bench>(x: &mut Cx<'_, R>, table: u32) -> u8 {
    let b = row_byte(x, table);
    x.set8(f::SUB, 4);
    x.set16(f::SUB2, 0);
    x.set8(CHOSEN, b);
    b
}

/// 0x6e8d6: after a series, chooses the fallback move (substate 8).
fn follow_up<R: Bench>(x: &mut Cx<'_, R>) {
    x.set8(AI_SHOT, 0xff);
    let b = row_byte(x, FOLLOW_TABLE);
    x.set8(f::SUB, 8);
    x.set8(f::SUB2, b);
    x.set8(f::SUB3, 0);
}

/// 0x6d08c: the Troll is grabbed by a player: it runs away walking off the screen, then disappears.
fn grab<R: Bench>(x: &mut Cx<'_, R>) {
    if paused(x) {
        return;
    }
    if x.u8(f::SUB2) == 0 {
        x.add8(f::SUB2, 2);
        let angle = if x.u8(f::FLIP) != 0 { 8 } else { 0x18 };
        x.set8(Obj::SPEED_INDEX, angle);
        set_speed(x, 0x21);
        return mv(x, 1);
    }
    if !x.in_arena_default() {
        detach_from_host(x);
        x.add8(f::STATE, 2);
        return;
    }
    x.move_by_table_xy();
    x.anim_step();
}

// --- state 0: creation --------------------------------------------------------------------------

/// False if the game is paused (nothing is done); otherwise the body of the active state follows within the same frame.
fn init<R: Bench>(x: &mut Cx<'_, R>) -> bool {
    if paused(x) {
        return false;
    }
    x.add8(f::STATE, 2);
    x.set8(f::BOX_SET, 0x1a);
    lib::initial_hp(x);
    lib::set_immunities(x);
    for (off, v) in [
        (0x68, 2),
        (0x69, 4),
        (0x70, 0),
        (0x6a, 1),
        (0x6c, 3),
        (0x6d, 3),
        (0x72, 3),
        (0x73, 2),
        (0x74, 3),
        (0x75, 0x38),
        (0x76, 3),
        (TIMER, 0x20),
        (LOOK_PREV, 0),
        (COOLDOWN, 2),
        (VULNERABLE, 1),
        (0x8e, 0),
        (0x7f, 0),
        (0x7e, 0xe),
    ] {
        x.set8(off, v);
    }
    x.set32(0x8a, 0);
    x.set8(AI_SHOT, 0xff);
    x.set8(0x80, 0xf);
    x.set8(0x81, 0xc);
    x.set8(0xab, 0x20);
    x.set8(NO_ZONE, 0);
    true
}

// --- state 2: active -----------------------------------------------------------------------------

fn active<R: Bench>(x: &mut Cx<'_, R>) {
    let freeze = x.u8(f::FREEZE) as i8;
    if freeze > 0 {
        x.set8(f::FREEZE, (freeze - 1) as u8);
    } else if !paused(x) {
        match x.u8(f::SUB) {
            0 => arrival::run(x),
            2 => stance::run(x),
            4 | 6 | 8 => attacks::run(x),
            10 => stance::idle(x),
            _ => {}
        }
    }
    if x.u8(f::SUB) != 0 {
        zone_and_terrain(x);
    }
    x.reinsert();
}

handlers! {
    0x6c4d6 "enemy_14" run,
}

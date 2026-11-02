//! "Bow Gnoll" enemy (ENEMY type 3, 0x5bda6). See `docs/families/enemy.md`.
//!
//! An archer: it walks toward its target, shoots (`+0x44` = 0xa, `+0x45` = 4 for the arrow, the shot itself is created
//! by the animation key with 0x9dbd6), backs off, dodges and jumps. A Bow Gnoll created by another object (`+0x42`)
//! does not follow the AI: it shoots in place (`spawned`). A non-zero `+0x43` gives a lifetime (`+0xa8`).
//!
//! | `+5` | File | Role (h) |
//! |---|---|---|
//! | 0 | `arrival` | arrival according to variant `+3` (0 and 2 walk, 4 fall, 6 appear, 8 run) |
//! | 2 | `stance` | standing: approach, walk, choice of a move |
//! | 4, 6 | `shoot` | shots, side steps, jumps |
//! | 8, 10, 12 | `stance` | rest, wait, turn around |
//! | state 4 | `hit` | reactions to hits, fall, grab by a player |
//!
//! Animations 0x16032c (movements) and 0x160350 (shots). The Bow Gnoll has no "player alive" test: the wait (`+5` = 10)
//! goes back to substate 2 when count `+0x6e` has elapsed.

use crate::families::cx::Bench;
use crate::record::record;
use crate::data::RomAddr;
use super::cx::{handlers, Cx};
use super::enemy_common::*;
use crate::objects::Obj;

mod arrival;
mod hit;
mod shoot;
mod spawned;
mod stance;

/// Fields of the family beyond the common ones of `Enemy` (`record!`). A meaning guessed from the code alone is
/// marked (h).
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub struct Bow(pub Obj);

impl Bow {
    pub fn at(self, off: u32) -> u32 {
        self.0.at(off)
    }
}

record! {
    impl Bow {
        /// Non-zero: created by another object, shoots in place (`spawned`).
        spawned, set_spawned, SPAWNED: u8 @ 0x42;
        /// Non-zero: lifetime limited by `Bow::LIFETIME`.
        timed, set_timed, TIMED: u8 @ 0x43;
        /// State to resume after a turn-around (long).
        resume_state, set_resume_state, RESUME_STATE: u32 @ 0xa0;
        resume_sub2, set_resume_sub2, RESUME_SUB2: u16 @ 0xa2;
        timer, set_timer, TIMER: u8 @ 0xa4;
        repeat, set_repeat, REPEAT: u8 @ 0xa5;
        saved_step, set_saved_step, SAVED_STEP: u8 @ 0xa6;
        /// Non-zero: the Bow Gnoll is grabbed (lifetime elapsed or created to be grabbed).
        expired, set_expired, EXPIRED: u8 @ 0xa7;
        lifetime, set_lifetime, LIFETIME: u16 @ 0xa8;
        /// Non-zero: the hit state draws neither zone nor shadow.
        no_zone, set_no_zone, NO_ZONE: u8 @ 0xac;
    }
}
use crate::objects::pools;
use crate::objects::Current;

const SET_MOVE: RomAddr = RomAddr(0x16032c);
const SET_SHOT: RomAddr = RomAddr(0x160350);
const STANCE: u32 = 0x0202_0000;
const HELD: u32 = 0x0408_0000;
const IDLE: u32 = 0x020a_0000;

pub fn run<R: Bench>(x: &mut Cx<'_, R>) {
    if suspended(x) {
        return;
    }
    x.tick_counters();
    if x.u16(f::TARGET) == 0 {
        pick_target(x);
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
    x.anim_start(SET_MOVE.0, n as u16);
}

/// Starts animation `n` of the shot set.
fn shot<R: Bench>(x: &mut Cx<'_, R>, n: u32) {
    x.anim_start(SET_SHOT.0, n as u16);
}

fn set_speed<R: Bench>(x: &mut Cx<'_, R>, n: u32) {
    x.select_speed(&crate::objects::motion::SPEEDS_B, n as u16);
}

/// 0x5ce22: walking direction according to orientation (0xc left, 4 right).
fn face_angle<R: Bench>(x: &mut Cx<'_, R>) {
    let angle = if x.u8(f::FLIP) != 0 { 4 } else { 0xc };
    x.set8(Obj::SPEED_INDEX, angle);
}

/// Walking direction: 8 (looks left) or 0x18.
fn walk_angle<R: Bench>(x: &mut Cx<'_, R>) {
    let angle = if x.u8(f::FLIP) != 0 { 8 } else { 0x18 };
    x.set8(Obj::SPEED_INDEX, angle);
}

/// Difficulty row tables: first moves, number of shots, second moves, follow-up.
const FIRST_ACTIONS: &str = "bow/first_actions";
const SHOT_REPEATS: &str = "bow/shot_repeats";
const SECOND_ACTIONS: &str = "bow/second_actions";
const FOLLOW_TABLE: &str = "bow/follow_moves";

/// 0x5cdc6: random draw (ten chances out of sixteen) that decides on a shot; D0 = 0 or 1.
fn shoot_roll<R: Bench>(x: &mut Cx<'_, R>) -> bool {
    let r = rand_and(x, 0xf);
    let mask = tables::table("bow/shot_mask").at16(x.b, 0);
    (mask >> r) & 1 != 0
}

/// Range boxes (0xb5e0); D5 is set as in the original.
fn range_box<R: Bench>(x: &mut Cx<'_, R>, d5: u32, d0: i16, d1: u16, d2: u16) -> bool {
    *x.c.d5() = d5; // re-read: D5 re-read afterwards (as for in_front)
    lib::target_in_box(x, d0, d1, d2)
}

/// 0x5ce14: player very close (close enough for a point-blank shot).
fn point_blank<R: Bench>(x: &mut Cx<'_, R>) -> bool {
    range_box(x, 2, -0x28, 0x28, 0xc)
}

/// 0x5cdf6: player in front, at short distance.
fn in_front<R: Bench>(x: &mut Cx<'_, R>) -> bool {
    *x.c.d5() = 0; // re-read: D5 re-read afterwards (DDSOM_POISON: covb_long3p_02)
    lib::target_in_box(x, -0xd0, 0x60, 0x18)
}

/// 0x5ce06: player close, on the same line.
fn on_line<R: Bench>(x: &mut Cx<'_, R>) -> bool {
    range_box(x, 2, 0, 0x38, 0x10)
}

/// 0x5ce36: end of a shot, choice of the fallback move (substate 8); grabbed if `+0xa7`.
fn follow_up<R: Bench>(x: &mut Cx<'_, R>) {
    x.set8(Enemy::AI_SHOT, 0xff);
    if x.u8(Bow::EXPIRED) != 0 {
        set_state_long(x, HELD);
    }
    let b = rank_byte(x, FOLLOW_TABLE);
    x.set8(f::SUB, 8);
    x.set8(f::SUB2, b);
    x.set8(f::SUB3, 0);
}

// --- state 0: creation --------------------------------------------------------------------------

/// False if the game is paused (nothing is done); otherwise the body of the active state follows within the same frame.
fn init<R: Bench>(x: &mut Cx<'_, R>) -> bool {
    if paused(x) {
        return false;
    }
    x.add8(f::STATE, 2);
    x.set8(f::BOX_SET, 0xf);
    lib::initial_hp(x);
    for (off, v) in [
        (Enemy::LOOT_ROW, 8),
        (Enemy::VULNERABLE, 0),
        (Enemy::AI_STANCE_ROW, 0),
        (Enemy::AI_68, 2),
        (Enemy::AI_69, 2),
        (Enemy::AI_WANDER_ROW, 1),
        (Enemy::AI_WAIT_MASKS, 2),
        (Enemy::AI_WAIT_ROW, 4),
        (Enemy::AI_DECISION_A, 2),
        (Enemy::AI_DECISION_B, 2),
        (Enemy::AI_BOX_CHANCE, 2),
        (Enemy::AI_BYTE_ROW, 3),
        (Enemy::AI_SHOT, 0xff),
        (Enemy::AI_BOX_ANIM, 0),
    ] {
        x.set8(off, v);
    }
    lib::set_immunities(x);
    x.set32(Enemy::RESUME_STATE, 0);
    x.set8(Enemy::EVENT_LOW, 0);
    x.set8(Enemy::EVENT_KIND, 0xc);
    x.set8(Enemy::LOOT, 7);
    if x.u8(Bow::TIMED) != 0 {
        x.set16(Bow::LIFETIME, 0x80);
    }
    face_angle(x);
    true
}

// --- state 2: active -----------------------------------------------------------------------------

fn active<R: Bench>(x: &mut Cx<'_, R>) {
    let freeze = x.u8(f::FREEZE) as i8;
    if freeze > 0 {
        x.set8(f::FREEZE, (freeze - 1) as u8);
    } else if !paused(x) {
        if x.u8(Bow::TIMED) != 0 {
            let n = x.u16(Bow::LIFETIME).wrapping_sub(1);
            x.set16(Bow::LIFETIME, n);
            if (n as i16) < 0 {
                x.set16(Bow::LIFETIME, 0xffff);
                x.set8(Bow::EXPIRED, 1);
            }
        }
        if x.u8(Bow::SPAWNED) != 0 {
            return spawned::run(x);
        }
        match x.u8(f::SUB) {
            0 => arrival::run(x),
            2 => stance::run(x),
            4 => shoot::run(x),
            6 => shoot::run_b(x),
            8 => stance::rest(x),
            10 => stance::idle(x),
            12 => stance::turn_around(x),
            _ => {}
        }
    }
    if x.u8(f::SUB) != 0 {
        zone_and_terrain(x);
    }
    x.reinsert();
}

handlers! {
    0x5bda6 "enemy_3" run,
}

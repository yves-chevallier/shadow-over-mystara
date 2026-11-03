//! Enemy "Riyakaa" (ENEMY type 16, 0x6f1c0). See `docs/families/enemy.md`.
//!
//! Four-level state machine: `+4` main state (0 creation, 2 active, 4 hit, 6 release), `+5` substate (active state: 0
//! arrival according to the variant `+3`, 2 choice of an attack, 4 paw strikes, 6 special hits, 8 pursuit, 10 rest),
//! `+6` and `+7` sub-substates. Each function below is a substate of the original, named after its role; the original
//! address is noted when it helps to find the code.

use crate::families::cx::Bench;
use crate::data::RomAddr;
use super::cx::{handlers, Cx};
use super::enemy_common::*;
use crate::record::record;
use crate::objects::Obj;
use crate::objects::Current;

mod attacks;
mod decide;
mod hit;

use attacks::*;
use decide::*;
use hit::*;

/// Fields of the Riyakaa beyond the common ones of `Enemy` (`record!`). A meaning guessed from the code alone is marked
/// (h).
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub struct Riyakaa(pub Obj);

impl Riyakaa {
    pub fn at(self, off: u32) -> u32 {
        self.0.at(off)
    }
}

record! {
    impl Riyakaa {
        /// Shift of the gaze for the strike to come (0xf0 high, 0xf2 middle, 0xf4 low, 0xf6 leap) (h).
        gaze, set_gaze, GAZE: u8 @ 0xa0;
        /// Pose of the current gesture, also the previous gaze (h).
        pose, set_pose, POSE: u8 @ 0xa1;
        /// Countdown of the current gesture; extra strikes of a series.
        timer, set_timer, TIMER: u8 @ 0xa2;
        /// Step (`+6`) to resume after the wait for a target.
        resume_step, set_resume_step, RESUME_STEP: u8 @ 0xa3;
        /// State long taken at the end of a dash.
        next_state, set_next_state, NEXT_STATE: u32 @ 0xa8;
        /// Tries of a pounce with the same gaze (three: the Riyakaa looks again).
        pounce_tries, set_pounce_tries, POUNCE_TRIES: u8 @ 0xac;
        /// Set when the search began far from the target: the walk animation starts with the next attack (h).
        walk_pending, set_walk_pending, WALK_PENDING: u8 @ 0xad;
        /// Set when a player grabs the Riyakaa; not read by the family (h).
        grabbed, set_grabbed, GRABBED: u8 @ 0xae;
    }
}

/// Animation sets of the body and of the hits.
const BODY: RomAddr = RomAddr(0x1665d6);
const STRIKE: RomAddr = RomAddr(0x1665f8);
/// Address tables of the reactions to hits (state 4: hit while standing / hit in the middle of an action).
const REACT_STANDING: RomAddr = RomAddr(0x6fe22);
const REACT_ACTING: RomAddr = RomAddr(0x70018);
/// Bytes for choosing the next gesture, 32 per difficulty level (`A5+0x7f92` / 2), 5-bit draw.
/// (The first is the table `follow_moves` of the bow gnolls.)
const NEXT_MOVES: &str = "bow/follow_moves";
const NEXT_ATTACKS: &str = "riyakaa/next_attacks";
/// Decision masks of the counter-attack (one long per difficulty level).
const MASKS: &str = "riyakaa/counter_masks";
/// Long state "active, substate 0": return to walking.
const WALK: u32 = 0x0202_0000;

pub fn run<R: Bench>(x: &mut Cx<'_, R>) {
    if suspended(x) {
        return;
    }
    x.tick_counters();
    match x.u8(f::STATE) {
        st::INIT => {
            if paused(x) {
                return;
            }
            init(x);
            active(x);
        }
        st::ACTIVE => active(x),
        st::HIT => hit(x),
        st::DEAD => free(x),
        _ => {}
    }
}

fn init<R: Bench>(x: &mut Cx<'_, R>) {
    x.add8(f::STATE, 2);
    x.set8(f::BOX_SET, 0xf);
    lib::initial_hp(x);
    lib::set_immunities(x);
    // AI parameters read by the decision tables, and starting values
    for (off, v) in [
        (Enemy::AI_68, 2),
        (Enemy::AI_69, 2),
        (Enemy::AI_STANCE_ROW, 0),
        (Enemy::AI_WANDER_ROW, 1),
        (Enemy::AI_WAIT_MASKS, 2),
        (Enemy::AI_WAIT_ROW, 4),
        (Enemy::AI_DECISION_A, 2),
        (Enemy::AI_DECISION_B, 2),
        (Enemy::AI_BOX_CHANCE, 2),
        (Enemy::AI_BOX_ANIM, 0),
        (Enemy::AI_BYTE_ROW, 3),
        (Riyakaa::TIMER, 0x20),
        (Riyakaa::POSE, 2),
        (Enemy::VULNERABLE, 1),
        (Enemy::AI_ROW_8E, 0),
        (Enemy::EVENT_LOW, 0),
        (Enemy::EVENT_KIND, 0x44),
    ] {
        x.set8(off, v);
    }
    x.set32(Enemy::RESUME_STATE, 0);
    x.set8(Riyakaa::WALK_PENDING, 0);
    x.set8(Enemy::AI_POINT_SET, 6);
    x.set8(Enemy::LOOT, 6);
    x.set8(Enemy::LOOT_ROW, 7);
}

/// Plays animation `n` of the movement set.
fn body<R: Bench>(x: &mut Cx<'_, R>, n: u32) {
    x.anim_start(BODY.0, n as u16);
}

/// Plays animation `n` of the hit set.
fn strike<R: Bench>(x: &mut Cx<'_, R>, n: u32) {
    x.anim_start(STRIKE.0, n as u16);
}

/// Chooses speed table `n`.
fn speed<R: Bench>(x: &mut Cx<'_, R>, n: u32) {
    x.select_speed(&crate::objects::motion::SPEEDS_B, n as u16);
}

/// Return to the active state, substate 0.
fn walk<R: Bench>(x: &mut Cx<'_, R>) {
    x.set32(f::STATE, WALK);
}

fn toggle_flip<R: Bench>(x: &mut Cx<'_, R>) {
    let flip = x.u8(f::FLIP) ^ 0x20;
    x.set8(f::FLIP, flip);
}

/// Decrements a byte and returns its new value.
fn countdown<R: Bench>(x: &mut Cx<'_, R>, off: u32) -> u8 {
    let n = x.u8(off).wrapping_sub(1);
    x.set8(off, n);
    n
}

// --- state 2: active -----------------------------------------------------------------------

fn active<R: Bench>(x: &mut Cx<'_, R>) {
    active_loop(x, |x| {
        match x.u8(f::SUB) {
            0 => return arrive(x),
            2 => decide(x),
            4 => swipe(x),
            6 => special(x),
            8 => chase(x),
            10 => rest(x),
            _ => {}
        }
        false
    });
}

/// Substate 0: arrival according to the variant `+3`. Returns true when the original abandons the frame without
/// finishing it (variant 8, `+6 = 2`: it pops the return address of the substate function).
fn arrive<R: Bench>(x: &mut Cx<'_, R>) -> bool {
    match x.u8(Enemy::VARIANT) {
        0 | 4 => charge_in(x),
        2 => rise(x),
        6 => leap_in(x),
        8 => return approach(x),
        10 => hover(x),
        _ => {}
    }
    false
}

/// Movement common to the arrivals: the duration `+0xa2` counted down, then return to walking.
fn fly_step<R: Bench>(x: &mut Cx<'_, R>) {
    if countdown(x, Riyakaa::TIMER) == 0 {
        walk(x);
        x.set8(Enemy::ACTIVE, 1);
    }
    x.move_by_table_xy();
    x.anim_step();
}

/// 0x6f2e0: enters running (variants 0 and 4).
fn charge_in<R: Bench>(x: &mut Cx<'_, R>) {
    if x.u8(f::SUB2) != 0 {
        return fly_step(x);
    }
    x.add8(f::SUB2, 2);
    x.set8(Enemy::VULNERABLE, 1);
    x.set16(Riyakaa::GAZE, 0);
    x.set8(Riyakaa::TIMER, 0x18);
    x.set8(Enemy::HEADING, 0x18);
    if x.u8(f::FLIP) != 0 {
        x.set8(Enemy::HEADING, 8);
    }
    x.set16(Riyakaa::GAZE, 0x202);
    speed(x, 0x17);
    let mut n = 3;
    if x.u8(Enemy::VARIANT) == 0 {
        x.set16(Riyakaa::GAZE, 0);
        speed(x, 0xd);
        n = 2;
    }
    body(x, n);
}

/// 0x6f35e: enters leaping (variant 6).
fn leap_in<R: Bench>(x: &mut Cx<'_, R>) {
    if x.u8(f::SUB2) != 0 {
        return fly_step(x);
    }
    x.add8(f::SUB2, 2);
    x.set8(Enemy::VULNERABLE, 1);
    x.set8(Riyakaa::TIMER, 0x20);
    x.set8(Riyakaa::POSE, 0xd);
    x.set8(Enemy::HEADING, 0x10);
    speed(x, 0xd);
    body(x, 6);
}

/// 0x6f3aa: start of the fall (springs upward then falls back).
fn rise_start<R: Bench>(x: &mut Cx<'_, R>) {
    x.add8(f::SUB2, 2);
    x.set8(Enemy::VULNERABLE, 0);
    x.set8(Riyakaa::POSE, 0xe);
    x.set16(Enemy::VZ, 0xfd00);
    x.set16(Enemy::GRAVITY, 0xff80);
    body(x, 0xb);
}

/// 0x6f3d2: falls back to the ground.
fn rise_fall<R: Bench>(x: &mut Cx<'_, R>) {
    x.move_z();
    if (x.u16(Obj::Z) as i16) >= 0 {
        return;
    }
    x.add8(f::SUB2, 2);
    x.set32(Obj::Z, 0);
    body(x, 0x10);
}

/// 0x6f3f6: end of the landing animation.
fn rise_land<R: Bench>(x: &mut Cx<'_, R>) {
    if x.anim_step() {
        walk(x);
        x.set8(Enemy::ACTIVE, 1);
    }
}

/// Variant 2: falls from the top of the screen.
fn rise<R: Bench>(x: &mut Cx<'_, R>) {
    match x.u8(f::SUB2) {
        0 => rise_start(x),
        2 => rise_fall(x),
        4 => rise_land(x),
        _ => {}
    }
}

/// Variant 10: appears after 30 frames, then falls like variant 2.
fn hover<R: Bench>(x: &mut Cx<'_, R>) {
    match x.u8(f::SUB2) {
        0 => {
            x.set8(Enemy::COUNTER, 0x1e);
            rise_start(x);
        }
        2 => {
            if countdown(x, Enemy::COUNTER) == 0 {
                x.add8(f::SUB2, 2);
            }
        }
        4 => rise_fall(x),
        6 => rise_land(x),
        _ => {}
    }
}

/// Variant 8: approaches until it sees a player; at `+6 = 2` the original abandons the frame.
fn approach<R: Bench>(x: &mut Cx<'_, R>) -> bool {
    match x.u8(f::SUB2) {
        0 => {
            x.add8(f::SUB2, 2);
            x.set8(Enemy::VULNERABLE, 0);
            x.set8(Riyakaa::POSE, 0xb);
            body(x, 0xd);
        }
        2 => {
            x.anim_step();
            if lib::pick_target(x).is_some() && lib::target_in_box(x, 0, 0x80, 0x20) {
                    x.add8(f::SUB2, 2);
                }
            return true;
        }
        4 => {
            x.add8(f::SUB2, 2);
            x.set8(Enemy::VULNERABLE, 1);
            x.set8(Riyakaa::POSE, 0xc);
            body(x, 0xa);
        }
        6 => {
            if x.anim_step() {
                x.set8(f::SUB, 0xa);
                x.set16(f::SUB2, 0);
            }
        }
        _ => {}
    }
    false
}

/// Substate 10: waits, standing, until a player is alive.
fn rest<R: Bench>(x: &mut Cx<'_, R>) {
    if x.u8(f::SUB2) == 0 {
        x.add8(f::SUB2, 2);
        x.set8(Enemy::VULNERABLE, 1);
        x.set8(Riyakaa::POSE, 1);
        return body(x, 0);
    }
    if var::PLAYERS_ALIVE.get(x.b) == 0 {
        x.anim_step();
    } else {
        walk(x);
        lib::pick_target(x);
    }
}

handlers! {
    ENEMY_16 0x6f1c0 "enemy_16" run,
}

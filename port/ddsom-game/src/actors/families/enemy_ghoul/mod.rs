//! "Ghoul" enemy (ENEMY type 5, 0x5f792). See `docs/families/enemy.md`.
//!
//! Common framework (`active_loop`: freeze, substate, zone and shadow, re-queuing). In the active state:
//!
//! | `+5` | File | Role (h) |
//! |---|---|---|
//! | 0 | `mod` | arrival according to variant `+3` |
//! | 2 | `stance` | standing: walk, turn-around, wait, choice of an attack |
//! | 4 | `attacks` | attacks and leaps (`+6` = the attack) |
//! | 6 | `moves` | fallback movements, side steps, screen exit |
//! | 8 | `moves` | turn-arounds and resume |
//! | 10 | `stance` | patrols as long as no player is alive |
//! | state 4 | `hit` | reactions to hits |
//!
//! Animations: 0x16191c (movements) and 0x161930 (attacks). `+0xa0/+0xa1` are look bytes.
//! Substates 2, 4, 6 and 8 begin with the reaction to hit flags `+0x8f` (0x60c88).

use crate::actors::families::cx::Bench;
use crate::world::record::record;
use crate::world::data::RomAddr;
use super::cx::{handlers, Cx};
use super::enemy_common::*;
use crate::world::objects::Obj;

mod attacks;
mod hit;
mod moves;
mod stance;

/// Fields of the family beyond the common ones of `Enemy` (`record!`). A meaning guessed from the code alone is
/// marked (h).
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub struct Ghoul(pub Obj);

impl Ghoul {
    pub fn at(self, off: u32) -> u32 {
        self.0.at(off)
    }
}

record! {
    impl Ghoul {
        special_hit, set_special_hit, SPECIAL_HIT: u8 @ 0x8f;
        /// Current and previous look.
        look, set_look, LOOK: u8 @ 0xa0;
        look_prev, set_look_prev, LOOK_PREV: u8 @ 0xa1;
        /// Timer of a step.
        timer, set_timer, TIMER: u8 @ 0xa2;
        /// Object that spawned the ghoul (word, RAM address), when `SPAWNED`.
        spawner, set_spawner, SPAWNER: u16 @ 0xa4;
        /// Non-zero: spawned by an object (`SPAWNER`), told of its death.
        spawned, set_spawned, SPAWNED: u8 @ 0xa6;
        /// Second byte cleared with `SPAWNED` at the creation; not read (h).
        spawned_low, set_spawned_low, SPAWNED_LOW: u8 @ 0xa7;
        /// Saved position (flight).
        saved_x, set_saved_x, SAVED_X: u16 @ 0xa8;
    }
}

use crate::world::objects::Current;

const SET_BODY: RomAddr = RomAddr(0x16191c);
const SET_STRIKE: RomAddr = RomAddr(0x161930);
const STANCE: u32 = 0x0202_0000;
const IDLE: u32 = 0x020a_0000;
const KNOCKED: u32 = 0x0408_0000;
/// ROM tables: attacks to choose per difficulty level (`A5+0x7f92`), 32 bytes per level.
const CLOSE_ATTACKS: &str = "ghoul/close_attacks";
const FAR_ATTACKS: &str = "ghoul/far_attacks";
const FALLBACK_MOVES: &str = "ghoul/fallback_moves";

pub fn run<R: Bench>(x: &mut Cx<'_, R>) {
    if suspended(x) {
        return;
    }
    x.tick_counters();
    match x.u8(f::STATE) {
        st::INIT => init(x),
        st::ACTIVE => active(x),
        st::HIT => hit::run(x),
        st::DEAD => dead(x),
        _ => {}
    }
}

// --- aides ------------------------------------------------------------------------------------

fn body<R: Bench>(x: &mut Cx<'_, R>, n: u32) {
    x.anim_start(SET_BODY.0, n as u16);
}

fn strike<R: Bench>(x: &mut Cx<'_, R>, n: u32) {
    x.anim_start(SET_STRIKE.0, n as u16);
}

fn step<R: Bench>(x: &mut Cx<'_, R>) -> bool {
    x.anim_step()
}

fn move_xy<R: Bench>(x: &mut Cx<'_, R>) {
    x.move_by_table_xy();
}

/// The wanted look (`Ghoul::LOOK` and `Ghoul::LOOK_PREV` identical).
fn look<R: Bench>(x: &mut Cx<'_, R>, v: u8) {
    x.set8(Ghoul::LOOK, v);
    x.set8(Ghoul::LOOK_PREV, v);
}

/// Walking direction according to the orientation: `flipped` if `+0x2c` is non-zero, `otherwise` if not.
fn set_angle<R: Bench>(x: &mut Cx<'_, R>, flipped: u8, otherwise: u8) {
    let angle = if x.u8(f::FLIP) != 0 { flipped } else { otherwise };
    x.set8(Obj::SPEED_INDEX, angle);
}

/// Speed table 0xd then walk: `ANGLE` then 0x1b64 with D0 = `speed`.
fn set_speed<R: Bench>(x: &mut Cx<'_, R>, speed: u32) {
    x.select_speed(&crate::world::objects::motion::SPEEDS_B, speed as u16);
}


/// 0x61138: end of an attack, the next one is drawn from the difficulty row.
fn end_attack<R: Bench>(x: &mut Cx<'_, R>) {
    let mv = rank_byte(x, FALLBACK_MOVES);
    x.set8(f::SUB, 8);
    x.set8(f::SUB2, mv);
    x.set8(f::SUB3, 0);
}

/// 0x60d04 / 0x60d0c: chooses the attack move (`+6` = table byte + `bias`).
fn pick_attack<R: Bench>(x: &mut Cx<'_, R>, table: &str, bias: u8) {
    let mv = rank_byte(x, table);
    x.set8(f::SUB2, mv.wrapping_add(bias));
    x.set8(f::SUB3, 0);
}

/// 0x5fdf8: the target is on the other side of the orientation (D0 = 1 or 0).
fn needs_turn_around<R: Bench>(x: &mut Cx<'_, R>) -> bool {
    if x.u16(f::TARGET) == 0 {
        return false;
    }
    let target = load_target(x);
    let dx = x.u16(Obj::X).wrapping_sub(target.u16(x.b, Obj::X));
    let want = if (dx as i16) < 0 { 0x20 } else { 0 };
    want != x.u8(f::FLIP) as u32
}

/// Decision by mask (0xa4c2 for the same orientation, 0xa4b4 otherwise) for the current target.
fn decision<R: Bench>(x: &mut Cx<'_, R>) -> bool {
    let target = load_target(x);
    let theirs = target.u8(x.b, f::FLIP);
    let same = theirs == x.u8(f::FLIP);
    lib::decision(x, same)
}

/// 0x5fe22 / 0x5fe70: a player in the box and the decision accepts it; the attack is then chosen.
fn box_attack<R: Bench>(x: &mut Cx<'_, R>, d0: i16, d1: u16, d2: u16, table: &str, bias: u8) -> bool {
    let ok = lib::target_in_box(x, d0, d1, d2) && decision(x);
    if ok {
        set_state_long(x, 0x0204_0000);
        pick_attack(x, table, bias);
    }
    ok
}

/// Reaction to hit flags `+0x8f` (0x60c88): true if it took over.
fn special_hit<R: Bench>(x: &mut Cx<'_, R>) -> bool {
    let flag = x.u8(Ghoul::SPECIAL_HIT);
    match flag {
        1 => {
            if x.u16(Obj::Z) != 0 {
                return false;
            }
            x.set8(Ghoul::SPECIAL_HIT, 0);
            set_state_long(x, 0x0400_0000);
            x.set8(Enemy::REACTION, 0x58);
            hit::run(x);
            true
        }
        2 => {
            set_state_long(x, 0x0400_0000);
            x.set8(Ghoul::SPECIAL_HIT, 0);
            let code = if x.u16(Obj::Z) == 0 { 0xc } else { 0x38 };
            x.set8(Enemy::REACTION, code);
            hit::run(x);
            true
        }
        3 => {
            if x.u16(Obj::Z) != 0 || x.u8(Enemy::DOWNED) != 0 {
                return false;
            }
            x.set8(Ghoul::SPECIAL_HIT, 0);
            set_state_long(x, KNOCKED);
            moves::run_b(x);
            true
        }
        _ => false,
    }
}

// --- state 0: creation --------------------------------------------------------------------------

fn init<R: Bench>(x: &mut Cx<'_, R>) {
    if paused(x) {
        return;
    }
    x.add8(f::STATE, 2);
    x.set8(Enemy::ACTIVE, 2);
    x.set8(f::BOX_SET, 0x11);
    lib::initial_hp(x);
    lib::set_immunities(x);
    for (off, v) in [
        (Enemy::AI_68, 2),
        (Enemy::AI_69, 2),
        (Enemy::AI_STANCE_ROW, 0),
        (Enemy::AI_WANDER_ROW, 3),
        (Enemy::AI_WAIT_MASKS, 3),
        (Enemy::AI_WAIT_ROW, 3),
        (Enemy::AI_DECISION_A, 3),
        (Enemy::AI_DECISION_B, 2),
        (Enemy::AI_BOX_CHANCE, 3),
        (Enemy::AI_BOX_ANIM, 4),
        (Enemy::AI_BYTE_ROW, 3),
        (Ghoul::TIMER, 0x20),
        (Ghoul::LOOK, 6),
        (Enemy::VULNERABLE, 1),
        (Enemy::AI_ROW_8E, 0),
        (Enemy::LOOT, 8),
        (Enemy::LOOT_ROW, 9),
        (Ghoul::SPAWNED, 0),
        (Ghoul::SPAWNED_LOW, 0),
        (Enemy::WEAK_TO_C, 1),
        (Enemy::EVENT_LOW, 0x44),
        (Enemy::AI_SHOT, 0xff),
    ] {
        x.set8(off, v);
    }
    // creation chains directly into the active state
    active(x);
}

// --- state 2: active -----------------------------------------------------------------------------

fn active<R: Bench>(x: &mut Cx<'_, R>) {
    active_loop(x, |x| {
        match x.u8(f::SUB) {
            0 => arrival(x),
            2 => stance::run(x),
            4 => attacks::run(x),
            6 => moves::run_b(x),
            8 => moves::run_c(x),
            10 => stance::patrol(x),
            _ => {}
        }
        false
    });
}

/// Substate 0: arrival according to variant `+3` (0 walk, 1 jump, 2 fall, 3 wake-up, 4 appearance, 5 blinking).
fn arrival<R: Bench>(x: &mut Cx<'_, R>) {
    match x.u8(Obj::VARIANT) {
        0 => {
            if x.u8(f::SUB2) != 0 {
                let t = x.u8(Ghoul::TIMER).wrapping_sub(1);
                x.set8(Ghoul::TIMER, t);
                if t == 0 {
                    set_state_long(x, STANCE);
                    x.set8(Enemy::ACTIVE, 1);
                }
                move_xy(x);
                step(x);
                return;
            }
            x.add8(f::SUB2, 2);
            x.set8(Enemy::VULNERABLE, 1);
            look(x, 0xff);
            x.set8(Ghoul::TIMER, 0x18);
            set_angle(x, 8, 0x18);
            set_speed(x, 0xd);
            body(x, 2);
        }
        1 => match x.u8(f::SUB2) {
            0 => {
                x.add8(f::SUB2, 2);
                x.set8(Enemy::VULNERABLE, 1);
                look(x, 0xff);
                x.set8(Ghoul::TIMER, 4);
                body(x, 3);
            }
            2 => {
                let t = x.u8(Ghoul::TIMER).wrapping_sub(1);
                x.set8(Ghoul::TIMER, t);
                if t != 0 {
                    return;
                }
                x.add8(f::SUB2, 2);
                x.set16(Obj::VX, -0x500i16 as u16);
                if x.u8(f::FLIP) != 0 {
                    negate(x, Obj::VX);
                }
                x.set16(Obj::VZ, 0x380);
                x.set16(Obj::AZ, -0x38i16 as u16);
                x.set16(Obj::AX, 0);
                body(x, 4);
            }
            4 => {
                if x.move_xz().gt() {
                    return;
                }
                x.add8(f::SUB2, 2);
                x.set32(Obj::Z, 0);
                body(x, 5);
            }
            6 => end_arrival(x),
            _ => {}
        },
        2 => match x.u8(f::SUB2) {
            0 => {
                x.add8(f::SUB2, 2);
                x.set8(Enemy::VULNERABLE, 1);
                look(x, 0xff);
                x.set16(Obj::VZ, -0x300i16 as u16);
                x.set16(Obj::AZ, -0x80i16 as u16);
                let h = var::SCROLL_Y.get(x.b).wrapping_add(0x100).wrapping_sub(x.u16(Obj::Y));
                x.set16(Obj::Z, h);
                body(x, 4);
            }
            2 => {
                x.move_z();
                if (x.u16(Obj::Z) as i16) >= 0 {
                    return;
                }
                x.add8(f::SUB2, 2);
                x.set32(Obj::Z, 0);
                body(x, 5);
            }
            4 => end_arrival(x),
            _ => {}
        },
        3 => {
            if special_hit(x) {
                return;
            }
            match x.u8(f::SUB2) {
                0 => {
                    x.add8(f::SUB2, 2);
                    x.set8(Enemy::VULNERABLE, 1);
                    look(x, 0xff);
                    body(x, 5);
                }
                2 => {
                    // a player close enough wakes the ghoul up
                    let mut near = 0;
                    for k in 0..4u32 {
                        let p = 0xff85de + 0x200 * k;
                        let dx = x.u16(Obj::X).wrapping_sub(crate::hw::mem::rd16(x.b, p + Obj::X)).wrapping_add(0x60);
                        let dy = x.u16(Obj::Y).wrapping_sub(crate::hw::mem::rd16(x.b, p + Obj::Y)).wrapping_add(0x30);
                        if dx <= 0xc0 && dy <= 0x60 {
                            near += 1;
                        }
                    }
                    if near != 0 {
                        x.add8(f::SUB2, 2);
                    }
                }
                4 => end_arrival(x),
                _ => {}
            }
        }
        4 => {
            if x.u8(f::SUB2) == 0 {
                x.add8(f::SUB2, 2);
                x.set8(Enemy::VULNERABLE, 1);
                look(x, 0xff);
                body(x, 8);
            } else if step(x) {
                set_state_long(x, STANCE);
                x.set8(Enemy::ACTIVE, 1);
            }
            zone_and_terrain(x);
        }
        5 => match x.u8(f::SUB2) {
            0 => {
                x.add8(f::SUB2, 2);
                x.set8(Enemy::LOOT, 0);
                x.set8(Enemy::LOOT_ROW, 0);
                x.set8(Ghoul::SPAWNED, 1);
                x.set8(Enemy::VULNERABLE, 1);
                look(x, 0xff);
                x.set16(Enemy::CREATOR, 0x703);
                let d7 = x.counter() as u8;
                x.set8(Ghoul::TIMER, d7);
                body(x, 0);
            }
            2 => blink(x, 0x307),
            4 | 6 => blink(x, 0x110),
            8 => {
                set_state_long(x, STANCE);
                x.set8(Enemy::ACTIVE, 1);
            }
            _ => {}
        },
        _ => {}
    }
}

/// 0x5fa56: once the animation is over, the ghoul stands up.
fn end_arrival<R: Bench>(x: &mut Cx<'_, R>) {
    if step(x) {
        set_state_long(x, STANCE);
        x.set8(Enemy::ACTIVE, 1);
    }
}

/// Arrival blinking (0x5fbbe), `next`: value of word `+0x42` when a series is over.
fn blink<R: Bench>(x: &mut Cx<'_, R>, next: u16) {
    x.set_bit(Enemy::DISPLAY, 1, true);
    let t = x.u8(Ghoul::TIMER).wrapping_sub(1);
    x.set8(Ghoul::TIMER, t);
    if t & x.u8(Enemy::BLINK_MASK) != 0 {
        return;
    }
    x.set_bit(Enemy::DISPLAY, 1, false);
    let n = x.u8(Enemy::BLINK_COUNT).wrapping_sub(1);
    x.set8(Enemy::BLINK_COUNT, n);
    if n != 0 {
        return;
    }
    x.add8(f::SUB2, 2);
    x.set16(Enemy::CREATOR, next);
}

// --- state 6: release ------------------------------------------------------------------------

fn dead<R: Bench>(x: &mut Cx<'_, R>) {
    if x.u8(Ghoul::SPAWNED) != 0 {
        let parent = x.linked(Ghoul::SPAWNER);
        let n = parent.u8(x.b, Enemy::CHILDREN).wrapping_sub(1);
        parent.set_u8(x.b, Enemy::CHILDREN, n);
    }
    free(x);
}

handlers! {
    ENEMY_5 0x5f792 "enemy_5" run,
}

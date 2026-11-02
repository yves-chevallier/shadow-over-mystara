//! "Shadow Elf" enemy (ENEMY type 12, 0x69384). See `docs/families/enemy.md`.
//!
//! Dark elf that strikes with a sword, disappears and falls back onto its target. Common framework: `+4` state (0
//! creation, 2 active, 4 hit, 6 release), `+5` group of behaviours, `+6` and `+7` steps. In the active state:
//!
//! | `+5` | File | Role (h) |
//! |---|---|---|
//! | 0 | `mod` | arrival according to variant `+3` |
//! | 2 | `stance` | standing: walk, turn-around, wait, choice of an attack |
//! | 4 | `attacks` | attacks (`+6` = the attack) |
//! | 6 | `attacks` | post-leap attacks |
//! | 8 | `attacks` | fallback attacks |
//! | 10 | `stance` | rest: waits for a player to be alive |
//! | state 4 | `hit` | reactions to hits |
//!
//! Animations: 0x164174 (movements) and 0x164184 (attacks). Except when frozen, the active state always runs the
//! proximity test 0x81aa4, the zone test, the shadow and the re-queuing.

use crate::families::cx::Bench;
use crate::data::RomAddr;
use super::cx::{handlers, Cx};
use super::enemy_common::*;
use crate::objects::Obj;

mod attacks;
mod hit;
mod stance;

/// Fields specific to this type (the common fields are in `enemy_common::f`).
mod g {
    /// Damage of the current attack: type and strength.
    pub const STRIKE_KIND: u32 = 0x44;
    pub const STRIKE_POWER: u32 = 0x45;
    /// Vulnerable (1) or not.
    pub const VULNERABLE: u32 = 0x67;
    /// AI counters: remaining attacks, delay before a new choice, wait, walk.
    pub const AI_CHOICE: u32 = 0x6b;
    pub const AI_WAIT: u32 = 0x6e;
    pub const AI_WALK: u32 = 0x71;
    pub const AI_KIND: u32 = 0x78;
    /// Repetition counter of an attack and timer.
    pub const REPEAT: u32 = 0xa2;
    /// Last AI direction (walking animation) and its countdown.
    pub const WALK_ANIM: u32 = 0xa1;
    pub const WALK_ANIM_TIMER: u32 = 0xa4;
    /// Attack planned after the approach and movement animation of that attack.
    pub const NEXT_ATTACK: u32 = 0xa5;
    pub const APPROACH_ANIM: u32 = 0xa6;
    /// Remaining wait before the next step.
    pub const COOLDOWN: u32 = 0x77;
}

use g::*;
use crate::objects::Current;

/// Animation sets: movements and attacks.
const SET_BODY: RomAddr = RomAddr(0x164174);
const SET_STRIKE: RomAddr = RomAddr(0x164184);
/// Long states (`+4..+7`).
const STANCE: u32 = 0x0202_0000;
const IDLE: u32 = 0x020a_0000;
/// ROM tables: attacks to choose per difficulty level (`A5+0x7f92`), 32 bytes per level.
const FALLBACK_MOVES: &str = "elf/fallback_moves";
const CLOSE_ATTACKS: &str = "elf/close_attacks";
const FAR_ATTACKS: &str = "elf/far_attacks";

pub fn run<R: Bench>(x: &mut Cx<'_, R>) {
    if suspended(x) {
        return;
    }
    if var::GAME_ON.get(x.b) != 0 {
        x.tick_counters();
    }
    match x.u8(f::STATE) {
        st::INIT => init(x),
        st::ACTIVE => active(x),
        st::HIT => hit::run(x),
        st::DEAD => free(x),
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

/// 0x69528: jump — effect 0xecfe4 and animation 3.
fn launch<R: Bench>(x: &mut Cx<'_, R>) {
    lib::sound(x, crate::sound::CMD_335_435);
    body(x, 3);
}

/// 0x6954a : accroupi (animation 4).
fn crouch<R: Bench>(x: &mut Cx<'_, R>) {
    body(x, 4);
}

/// 0x6953c: waits for the end of the animation, then moves on to the next `+6`.
fn wait_anim<R: Bench>(x: &mut Cx<'_, R>) {
    if step(x) {
        x.add8(f::SUB2, 2);
    }
}

/// 0x6957c: the animation over, back to standing.
fn end_to_stance<R: Bench>(x: &mut Cx<'_, R>) {
    if step(x) {
        set_state_long(x, STANCE);
    }
}

/// 0x6a680: back to standing; if decision 0x79ce wants it, to rest.
fn back_to_stance<R: Bench>(x: &mut Cx<'_, R>) {
    set_state_long(x, STANCE);
    if lib::maybe_draw_6e(x) {
        x.set8(f::SUB, 0xa);
    }
}

/// 0x6a452: end of an attack — the next one is drawn from the difficulty row (32 bytes).
fn end_attack<R: Bench>(x: &mut Cx<'_, R>) {
    x.set8(AI_KIND, 0xff);
    let mv = rank_byte(x, FALLBACK_MOVES);
    x.set8(f::SUB, 8);
    x.set8(f::SUB2, mv);
    x.set8(f::SUB3, 0);
}

// --- state 0: creation --------------------------------------------------------------------------

fn init<R: Bench>(x: &mut Cx<'_, R>) {
    if paused(x) {
        return;
    }
    x.add8(f::STATE, 2);
    x.set8(f::BOX_SET, 0x18);
    lib::initial_hp(x);
    lib::set_immunities(x);
    for (off, v) in [(Enemy::AI_68, 4), (Enemy::AI_69, 1), (Enemy::AI_STANCE_ROW, 0), (Enemy::AI_WANDER_ROW, 1), (Enemy::AI_WAIT_MASKS, 3), (Enemy::AI_WAIT_ROW, 3), (Enemy::AI_DECISION_A, 3), (Enemy::AI_DECISION_B, 2), (Enemy::AI_BOX_CHANCE, 3), (Enemy::AI_BOX_ANIM, 0x38), (Enemy::AI_BYTE_ROW, 3), (REPEAT, 0x20), (WALK_ANIM, 0), (WALK_ANIM_TIMER, 2)] {
        x.set8(off, v);
    }
    if var::GAME_ON.get(x.b) != 0 {
        x.set8(VULNERABLE, 1);
    }
    for (off, v) in [(Enemy::AI_ROW_8E, 0), (Enemy::EVENT_LOW, 0), (Enemy::EVENT_KIND, 0xe)] {
        x.set8(off, v);
    }
    x.set32(Enemy::RESUME_STATE, 0);
    x.set8(APPROACH_ANIM, 0);
    x.set8(AI_KIND, 0xff);
    x.set8(Enemy::LOOT_ROW, 0xc);
    x.set8(Enemy::LOOT, 0xb);
    // creation chains directly into the active state
    active(x);
}

// --- state 2: active -----------------------------------------------------------------------------

fn active<R: Bench>(x: &mut Cx<'_, R>) {
    let freeze = x.u8(f::FREEZE) as i8;
    if freeze > 0 {
        x.set8(f::FREEZE, (freeze - 1) as u8);
    } else if !paused(x) {
        match x.u8(f::SUB) {
            0 => arrival(x),
            2 => stance::run(x),
            4 => attacks::run(x),
            6 => attacks::run_b(x),
            8 => attacks::run_fallback(x),
            10 => stance::rest(x),
            _ => {}
        }
    }
    super::boss_common::push_out_of_boss(x);
    zone_and_terrain(x);
    x.reinsert();
}

/// Substate 0: arrival according to variant `+3` (even byte, table index: 0 walk, 2 fall, 4 leap, 6 slow fall).
fn arrival<R: Bench>(x: &mut Cx<'_, R>) {
    match x.u8(Obj::VARIANT) {
        0 => {
            if x.u8(f::SUB2) != 0 {
                move_xy(x);
                step(x);
                let t = x.u8(REPEAT).wrapping_sub(1);
                x.set8(REPEAT, t);
                if (t as i8) < 0 {
                    set_state_long(x, STANCE);
                }
                return;
            }
            x.add8(f::SUB2, 2);
            x.set8(REPEAT, 0x14);
            let angle = if x.u8(f::FLIP) != 0 { 8 } else { 0x18 };
            x.set8(Obj::SPEED_INDEX, angle);
            x.select_speed(&crate::objects::motion::SPEEDS_B, 0x12);
            body(x, 1);
        }
        2 => match x.u8(f::SUB2) {
            0 => {
                x.set16(Obj::VZ, 0);
                x.set16(Obj::AZ, -0x40i16 as u16);
                x.add8(f::SUB2, 2);
                launch(x);
            }
            2 => wait_anim(x),
            4 => land(x),
            6 => end_to_stance(x),
            _ => {}
        },
        4 => match x.u8(f::SUB2) {
            0 => {
                x.add8(f::SUB2, 2);
                x.set16(Obj::VZ, 0);
                x.set16(Obj::AZ, -0x38i16 as u16);
                x.set16(Obj::VX, -0x280i16 as u16);
                x.set16(Obj::AX, 0);
                if x.u8(f::FLIP) != 0 {
                    negate(x, Obj::VX);
                }
                launch(x);
            }
            2 => wait_anim(x),
            4 => {
                x.add8(f::SUB2, 2);
                x.anim_start(SET_STRIKE.0, 1);
            }
            6 => {
                x.move_xz();
                if (x.u16(Obj::Z) as i16) <= 0 {
                    x.add8(f::SUB2, 2);
                    x.set32(Obj::Z, 0);
                    body(x, 5);
                } else if (x.u16(Obj::Z) as i16) <= 0x30 {
                    step(x);
                }
            }
            8 => end_to_stance(x),
            _ => {}
        },
        6 => match x.u8(f::SUB2) {
            0 => {
                x.set16(Obj::VZ, 0);
                x.set16(Obj::AZ, -0x40i16 as u16);
                x.add8(f::SUB2, 2);
                launch(x);
            }
            2 => wait_anim(x),
            4 => {
                x.add8(f::SUB2, 2);
                x.set32(Obj::Z, 0);
                body(x, 5);
            }
            6 => end_to_stance(x),
            _ => {}
        },
        _ => {}
    }
}

/// 0x69558: descent (movement in height) then landing.
fn land<R: Bench>(x: &mut Cx<'_, R>) {
    x.move_z();
    if (x.u16(Obj::Z) as i16) > 0 {
        return;
    }
    x.add8(f::SUB2, 2);
    x.set32(Obj::Z, 0);
    body(x, 5);
}

handlers! {
    0x69384 "enemy_12" run,
}

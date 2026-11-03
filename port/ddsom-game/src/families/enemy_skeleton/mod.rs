//! "Skeleton" enemy (ENEMY type 1, 0x57cc4). See `docs/families/enemy.md`.
//!
//! Sword skeleton (series `+0x2f` = 0) or armed skeleton (series 1). Common framework: `+4` state (0 creation, 2
//! active, 4 hit, 6 release), `+5` group of behaviours, `+6` and `+7` steps. In the active state:
//!
//! | `+5` | File | Role (h) |
//! |---|---|---|
//! | 0 | `mod` | arrival according to variant `+3` |
//! | 2 | `stance` | standing: idea, walk, wait, turn-around, choice of an attack |
//! | 4 | `attacks` | melee attacks (`+6` = the attack) |
//! | 6 | `leaps` | leaps, guard, screen exit |
//! | 8 | `leaps` | lateral dodge and side step |
//! | 10 | `leaps` | arrival fall |
//! | state 4 | `hit` | reactions to hits |
//!
//! Animations: 0x15eed8 (movements) and 0x15eef4 (attacks).

use crate::families::cx::Bench;
use crate::record::record;
use crate::data::RomAddr;
use super::cx::{handlers, Cx};
use super::enemy_common::*;
use crate::objects::Obj;

mod attacks;
mod hit;
mod leaps;
mod stance;

/// Fields specific to this type (the common fields are in `enemy_common::f`).
/// Fields of the family beyond the common ones of `Enemy` (`record!`). A meaning guessed from the code alone is
/// marked (h).
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub struct Skeleton(pub Obj);

impl Skeleton {
    pub fn at(self, off: u32) -> u32 {
        self.0.at(off)
    }
}

record! {
    impl Skeleton {
        /// Walking box sets.
        walk_args_y, set_walk_args_y, WALK_ARGS_Y: u16 @ 0x7a;
        walk_args_z, set_walk_args_z, WALK_ARGS_Z: u16 @ 0x7c;
        /// Special reaction flags (1 fall, 2 recoil, 3 throw).
        special_hit, set_special_hit, SPECIAL_HIT: u8 @ 0x8f;
        /// Timer (word) and saved position.
        timer, set_timer, TIMER: u16 @ 0xa2;
        saved_pos, set_saved_pos, SAVED_POS: u16 @ 0xa4;
        /// Side where the target is (0 in front, 2 behind), set by the detection.
        side, set_side, SIDE: u8 @ 0xa6;
        /// "Grabbed" flag: the enemy goes into a fall.
        caught, set_caught, CAUGHT: u8 @ 0xa7;
        /// Counter-attack: flag (`+0xa8`, 0xff = at random) and attack to play (`+0xa9`).
        counter, set_counter, COUNTER: u16 @ 0xa8;
        counter_move, set_counter_move, COUNTER_MOVE: u8 @ 0xa9;
        /// Object that spawned the skeleton (word, RAM address), when `SPAWNED`.
        spawner, set_spawner, SPAWNER: u16 @ 0xaa;
        /// Non-zero: spawned by an object (`SPAWNER`), told of its death.
        spawned, set_spawned, SPAWNED: u8 @ 0xac;
    }
}

use crate::objects::Current;

/// Animation sets: movements and attacks.
const SET_BODY: RomAddr = RomAddr(0x15eed8);
const SET_STRIKE: RomAddr = RomAddr(0x15eef4);
/// Long states (`+4..+7`).
const STANCE: u32 = 0x0202_0000;
const STANCE_ALT: u32 = 0x0202_0600;
const AFTER_TURN: u32 = 0x0202_0002;
const IDLE: u32 = 0x020a_0000;
const DASH: u32 = 0x0206_0800;
const KNOCKED: u32 = 0x0408_0000;
/// ROM tables: attacks to choose per difficulty level (`A5+0x7f92`), 32 bytes per level.
const CLOSE_ATTACKS: &str = "skeleton/close_attacks";
const FAR_ATTACKS: &str = "skeleton/far_attacks";
const NEXT_MOVES: &str = "skeleton/next_moves";

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

/// End of the attacks: next step drawn from the table by difficulty, or back to standing.
fn end_attack<R: Bench>(x: &mut Cx<'_, R>) {
    x.set8(Enemy::AI_SHOT, 0xff);
    if x.u16(f::TARGET) != 0 {
        load_target(x);
        let side = match lib::target_in_front(x, -8, 0x68, 8) {
            lib::InBox::No => lib::target_in_front(x, -0xa0, 0x30, 8),
            near => near,
        };
        if side.found() {
            x.set8(Skeleton::SIDE, side as u8);
            pick_move(x, NEXT_MOVES, 0x208);
            return;
        }
    }
    set_state_long(x, STANCE);
}

/// Chooses attack `+6` from the difficulty row (32 bytes) of `table`, and sets `+4/+5` to `state`.
fn pick_move<R: Bench>(x: &mut Cx<'_, R>, table: &str, state: u16) {
    let mv = rank_byte(x, table);
    x.set8(f::SUB2, mv);
    x.set16(f::STATE, state);
    x.set8(f::SUB3, 0);
}

/// Reaction to hit flags `+0x8f` (0x59146): true if it took over (the resume address that the original passed in A6 is
/// read by nobody).
fn special_hit<R: Bench>(x: &mut Cx<'_, R>) -> bool {
    let flag = x.u8(Skeleton::SPECIAL_HIT);
    match flag {
        1 => {
            if x.u16(Obj::Z) != 0 {
                return false;
            }
            x.set8(Skeleton::SPECIAL_HIT, 0);
            x.set8(Enemy::REACTION, 0x58);
            if x.u32(f::STATE) == 0x0404_0200 {
                x.set16(Enemy::ANIM_END, 0xffff);
                set_state_long(x, 0x0406_0200);
            } else {
                set_state_long(x, 0x0400_0000);
            }
            true
        }
        2 => {
            set_state_long(x, 0x0400_0000);
            x.set8(Skeleton::SPECIAL_HIT, 0);
            let code = if x.u16(Obj::Z) == 0 { 0xc } else { 0x38 };
            x.set8(Enemy::REACTION, code);
            true
        }
        3 => {
            if x.u16(Obj::Z) != 0 || x.u8(Enemy::DOWNED) != 0 {
                return false;
            }
            x.set8(Skeleton::SPECIAL_HIT, 0);
            set_state_long(x, KNOCKED);
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
    if x.u8(Obj::VARIANT) == 4 {
        let t = x.u16(Enemy::CREATOR).wrapping_sub(1);
        x.set16(Enemy::CREATOR, t);
        if t != 0 {
            return;
        }
    }
    x.add8(f::STATE, 2);
    x.set8(f::BOX_SET, 0xd);
    lib::initial_hp(x);
    for (off, v) in [(Enemy::AI_STANCE_ROW, 0), (Enemy::EVENT_KIND, 0x44), (Enemy::EVENT_LOW, 0), (Enemy::AI_WANDER_ROW, 2), (Enemy::AI_WAIT_MASKS, 3), (Enemy::AI_BOX_CHANCE, 3), (Enemy::AI_BYTE_ROW, 3), (Enemy::AI_69, 2), (Enemy::AI_SHOT, 0xff)] {
        x.set8(off, v);
    }
    x.set16(Skeleton::COUNTER, 0);
    x.set8(Enemy::AI_ROW_8E, 0);
    x.set8(Enemy::LOOT, 0);
    x.set8(Enemy::LOOT_ROW, 0);
    x.set32(Enemy::RESUME_STATE, 0);
    x.set8(Skeleton::CAUGHT, 0);
    x.set8(Skeleton::SPAWNED, 0);
    x.set8(Enemy::WEAK_TO_C, 1);
    x.set32(f::IMMUNITY, 0x1001);
    lib::set_immunities(x);
    let series = x.u8(Enemy::GROUP) as u32;
    if series != 0 {
        x.set8(Enemy::OWN_PALETTE, 1);
        x.set8(Enemy::PALETTE, 0x1a);
    }
    // four bytes per series: the AI rows of the wait, of the two decisions and of the first parry
    let row = 4 * series;
    for (i, off) in [Enemy::AI_WAIT_ROW, Enemy::AI_DECISION_A, Enemy::AI_DECISION_B, Enemy::AI_68].into_iter().enumerate() {
        let v = tables::table("skeleton/series_ai").at8(x.b, row + i as u32);
        x.set8(off, v);
    }
    x.set16(f::SUB2, 0);
    match x.u8(Obj::VARIANT) {
        0 => {
            x.set8(f::SUB, 2);
            body(x, 0);
        }
        1 | 4 => {
            if series != 0 {
                let v = x.u8(Enemy::PALETTE) | 0x80;
                x.set8(Enemy::PENDING_PALETTE, v);
            }
            x.set8(f::SUB, 0);
            x.set8(Enemy::OWN_PALETTE, 0);
            body(x, 5 + series);
        }
        2 => {
            x.set8(f::SUB, 0);
            x.set8(f::SUB2, 4);
            let cam = if var::CAMERA_SELECT.get(x.b) != 0 { var::CAMERA_ALT_Y.get(x.b) } else { var::SCROLL_Y.get(x.b) };
            let h = cam.wrapping_add(0x100).wrapping_sub(x.u16(Obj::Y));
            x.set16(Obj::Z, h);
            x.set16(Obj::VX, 0);
            x.set16(Obj::AX, 0);
            x.set16(Obj::VZ, 0);
            x.set16(Obj::AZ, -0x40i16 as u16);
            body(x, 0xb);
        }
        3 => {
            x.set8(f::SUB, 0);
            body(x, 10);
        }
        5 => {
            x.set8(Enemy::ACTIVE, 2);
            x.set8(f::SUB, 0);
            x.set8(f::SUB2, 6);
            body(x, 0);
        }
        _ => return,
    }
    x.reinsert();
    zone_and_terrain(x);
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
            6 => leaps::run(x),
            8 => leaps::run_dash(x),
            10 => leaps::run_land(x),
            _ => {}
        }
    }
    x.reinsert();
}

/// Substate 0: arrival (`+6`: 0 and 2 wait for the animation, 4 fall, 6 blinking).
fn arrival<R: Bench>(x: &mut Cx<'_, R>) {
    match x.u8(f::SUB2) {
        0 | 2 => {
            step(x);
            if (x.u16(Enemy::ANIM_END) as i16) >= 0 {
                return;
            }
            x.set8(f::SUB, 2);
            x.set16(f::SUB2, 0);
            if x.u8(Enemy::GROUP) != 0 {
                x.set8(Enemy::PENDING_PALETTE, 0);
                x.set8(Enemy::OWN_PALETTE, 1);
                body(x, 0);
            }
        }
        4 => {
            if x.u8(f::SUB3) == 0 {
                x.move_xz();
                if (x.u16(Obj::Z) as i16) > 0 {
                    return;
                }
                x.add8(f::SUB3, 2);
                x.set32(Obj::Z, 0);
                x.set16(Obj::ANIM_TIMER, 1);
            } else {
                step(x);
                if (x.u16(Enemy::ANIM_END) as i16) >= 0 {
                    return;
                }
                x.set8(f::SUB, 2);
                x.set16(f::SUB2, 0);
            }
        }
        6 => blink(x),
        _ => {}
    }
}

/// Blinking arrival (`+6 = 6`): three series of blinks then the standing state.
fn blink<R: Bench>(x: &mut Cx<'_, R>) {
    let d1 = match x.u8(f::SUB3) {
        0 => {
            x.add8(f::SUB3, 2);
            x.set8(Enemy::ACTIVE, 2);
            x.set8(Skeleton::SPAWNED, 1);
            x.set8(Enemy::VULNERABLE, 1);
            x.set16(Enemy::CREATOR, 0x703);
            let d7 = x.counter() as u8;
            x.set8(Skeleton::TIMER, d7);
            return;
        }
        2 => 0x307,
        4 | 6 => 0x110,
        8 => {
            set_state_long(x, STANCE);
            x.set8(Enemy::ACTIVE, 1);
            return;
        }
        _ => return,
    };
    x.set_bit(Enemy::DISPLAY, 1, true);
    let t = x.u8(Skeleton::TIMER).wrapping_sub(1);
    x.set8(Skeleton::TIMER, t);
    if t & x.u8(Enemy::BLINK_MASK) != 0 {
        return;
    }
    x.set_bit(Enemy::DISPLAY, 1, false);
    let n = x.u8(Enemy::BLINK_COUNT).wrapping_sub(1);
    x.set8(Enemy::BLINK_COUNT, n);
    if n != 0 {
        return;
    }
    x.add8(f::SUB3, 2);
    x.set16(Enemy::CREATOR, d1);
}

// --- state 6: release ------------------------------------------------------------------------

fn dead<R: Bench>(x: &mut Cx<'_, R>) {
    if x.u8(Skeleton::SPAWNED) != 0 {
        let parent = x.linked(Skeleton::SPAWNER);
        let n = parent.u8(x.b, Enemy::CHILDREN).wrapping_sub(1);
        parent.set_u8(x.b, Enemy::CHILDREN, n);
    }
    free(x);
}

handlers! {
    ENEMY_1 0x57cc4 "enemy_1" run,
}

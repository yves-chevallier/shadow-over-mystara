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
use super::cx::{handlers, Cx};
use super::enemy_common::*;
use crate::objects::Obj;

mod attacks;
mod hit;
mod leaps;
mod stance;

/// Fields specific to this type (the common fields are in `enemy_common::f`).
mod g {
    /// Series: 0 skeleton, 1 armed skeleton.
    pub const SERIES: u32 = 0x2f;
    /// Frame counter of the animation (word) and its current value.
    pub const ANIM_COUNT: u32 = 0x30;
    pub const ANIM_STATE: u32 = 0x32;
    /// Damage of the current attack: type and strength.
    pub const STRIKE_KIND: u32 = 0x44;
    pub const STRIKE_POWER: u32 = 0x45;
    /// Movement speed table (long).
    pub const MOVE_TABLE: u32 = 0x4c;
    /// Vulnerable (1) or not.
    pub const VULNERABLE: u32 = 0x67;
    /// AI posture: 9 = on guard (counter-attack to a hit).
    pub const POSTURE: u32 = 0x68;
    /// AI counters: delay before a new choice, before a wait, turn-arounds.
    pub const AI_CHOICE: u32 = 0x6b;
    pub const AI_WAIT: u32 = 0x6e;
    pub const AI_WALK: u32 = 0x71;
    /// Remaining wait before the next step of an attack.
    pub const COOLDOWN: u32 = 0x77;
    pub const AI_KIND: u32 = 0x78;
    pub const AI_SHOT: u32 = 0x79;
    /// Walking box sets.
    pub const WALK_ARGS_Y: u32 = 0x7a;
    pub const WALK_ARGS_Z: u32 = 0x7c;
    /// Timer (word) and saved position.
    pub const TIMER: u32 = 0xa2;
    pub const SAVED_POS: u32 = 0xa4;
    /// Side where the target is (0 in front, 2 behind), set by the detection.
    pub const SIDE: u32 = 0xa6;
    /// "Grabbed" flag: the enemy goes into a fall.
    pub const CAUGHT: u32 = 0xa7;
    /// Counter-attack: flag (`+0xa8`, 0xff = at random) and attack to play (`+0xa9`).
    pub const COUNTER: u32 = 0xa8;
    pub const COUNTER_MOVE: u32 = 0xa9;
    /// Special reaction flags (1 fall, 2 recoil, 3 throw).
    pub const SPECIAL_HIT: u32 = 0x8f;
}

use g::*;
use crate::objects::Current;

/// Animation sets: movements and attacks.
const SET_BODY: u32 = 0x15eed8;
const SET_STRIKE: u32 = 0x15eef4;
/// Long states (`+4..+7`).
const STANCE: u32 = 0x0202_0000;
const STANCE_ALT: u32 = 0x0202_0600;
const AFTER_TURN: u32 = 0x0202_0002;
const IDLE: u32 = 0x020a_0000;
const DASH: u32 = 0x0206_0800;
const KNOCKED: u32 = 0x0408_0000;
/// ROM tables: attacks to choose per difficulty level (`A5+0x7f92`), 32 bytes per level.
const CLOSE_ATTACKS: u32 = 0x5932a;
const FAR_ATTACKS: u32 = 0x5952a;
const NEXT_MOVES: u32 = 0x59982;

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
    x.anim_start(SET_BODY, n as u16);
}

fn strike<R: Bench>(x: &mut Cx<'_, R>, n: u32) {
    x.anim_start(SET_STRIKE, n as u16);
}

fn step<R: Bench>(x: &mut Cx<'_, R>) -> bool {
    x.anim_step()
}

fn move_xy<R: Bench>(x: &mut Cx<'_, R>) {
    x.move_by_table_xy();
}

/// End of the attacks: next step drawn from the table by difficulty, or back to standing.
fn end_attack<R: Bench>(x: &mut Cx<'_, R>) {
    x.set8(AI_KIND, 0xff);
    if x.u16(f::TARGET) != 0 {
        load_target(x);
        let side = match lib::target_in_front(x, -8, 0x68, 8) {
            lib::InBox::No => lib::target_in_front(x, -0xa0, 0x30, 8),
            near => near,
        };
        if side.found() {
            x.set8(SIDE, side as u8);
            pick_move(x, NEXT_MOVES, 0x208);
            return;
        }
    }
    set_state_long(x, STANCE);
}

/// Chooses attack `+6` from the difficulty row (32 bytes) of `table`, and sets `+4/+5` to `state`.
fn pick_move<R: Bench>(x: &mut Cx<'_, R>, table: u32, state: u16) {
    let row = (var::RANK.get(x.b) >> 1) << 5;
    let n = rand_and(x, 0x1f);
    let mv = x.data8(table + row as u32 + n);
    x.set8(f::SUB2, mv);
    x.set16(f::STATE, state);
    x.set8(f::SUB3, 0);
}

/// Reaction to hit flags `+0x8f` (0x59146): true if it took over (the resume address that the original passed in A6 is
/// read by nobody).
fn special_hit<R: Bench>(x: &mut Cx<'_, R>) -> bool {
    let flag = x.u8(SPECIAL_HIT);
    match flag {
        1 => {
            if x.u16(Obj::Z) != 0 {
                return false;
            }
            x.set8(SPECIAL_HIT, 0);
            x.set8(0x49, 0x58);
            if x.u32(f::STATE) == 0x0404_0200 {
                x.set16(ANIM_STATE, 0xffff);
                set_state_long(x, 0x0406_0200);
            } else {
                set_state_long(x, 0x0400_0000);
            }
            true
        }
        2 => {
            set_state_long(x, 0x0400_0000);
            x.set8(SPECIAL_HIT, 0);
            let code = if x.u16(Obj::Z) == 0 { 0xc } else { 0x38 };
            x.set8(0x49, code);
            true
        }
        3 => {
            if x.u16(Obj::Z) != 0 || x.u8(0x86) != 0 {
                return false;
            }
            x.set8(SPECIAL_HIT, 0);
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
        let t = x.u16(0x42).wrapping_sub(1);
        x.set16(0x42, t);
        if t != 0 {
            return;
        }
    }
    x.add8(f::STATE, 2);
    x.set8(f::BOX_SET, 0xd);
    lib::initial_hp(x);
    for (off, v) in [(0x70, 0), (0x7e, 0x44), (0x7f, 0), (0x6a, 2), (0x6c, 3), (0x74, 3), (0x76, 3), (0x69, 2), (0x78, 0xff)] {
        x.set8(off, v);
    }
    x.set16(COUNTER, 0);
    x.set8(0x8e, 0);
    x.set8(0x81, 0);
    x.set8(0x80, 0);
    x.set32(0x8a, 0);
    x.set8(CAUGHT, 0);
    x.set8(0xac, 0);
    x.set8(0x83, 1);
    x.set32(f::IMMUNITY, 0x1001);
    lib::set_immunities(x);
    let series = x.u8(SERIES) as u32;
    if series != 0 {
        x.set8(0x2a, 1);
        x.set8(0x2b, 0x1a);
    }
    // four bytes per series: 0x6d, 0x72, 0x73, 0x68
    let row = 0x57de6 + 4 * series;
    for (i, off) in [0x6d, 0x72, 0x73, POSTURE].into_iter().enumerate() {
        let v = x.data8(row + i as u32);
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
                let v = x.u8(0x2b) | 0x80;
                x.set8(0x91, v);
            }
            x.set8(f::SUB, 0);
            x.set8(0x2a, 0);
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
            x.set8(0, 2);
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
            if (x.u16(ANIM_STATE) as i16) >= 0 {
                return;
            }
            x.set8(f::SUB, 2);
            x.set16(f::SUB2, 0);
            if x.u8(SERIES) != 0 {
                x.set8(0x91, 0);
                x.set8(0x2a, 1);
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
                x.set16(ANIM_COUNT, 1);
            } else {
                step(x);
                if (x.u16(ANIM_STATE) as i16) >= 0 {
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
            x.set8(0, 2);
            x.set8(0xac, 1);
            x.set8(VULNERABLE, 1);
            x.set16(0x42, 0x703);
            let d7 = x.counter() as u8;
            x.set8(TIMER, d7);
            return;
        }
        2 => 0x307,
        4 | 6 => 0x110,
        8 => {
            set_state_long(x, STANCE);
            x.set8(0, 1);
            return;
        }
        _ => return,
    };
    x.set_bit(1, 1, true);
    let t = x.u8(TIMER).wrapping_sub(1);
    x.set8(TIMER, t);
    if t & x.u8(0x42) != 0 {
        return;
    }
    x.set_bit(1, 1, false);
    let n = x.u8(0x43).wrapping_sub(1);
    x.set8(0x43, n);
    if n != 0 {
        return;
    }
    x.add8(f::SUB3, 2);
    x.set16(0x42, d1);
}

// --- state 6: release ------------------------------------------------------------------------

fn dead<R: Bench>(x: &mut Cx<'_, R>) {
    if x.u8(0xac) != 0 {
        let parent = x.linked(0xaa);
        let n = parent.u8(x.b, 0xaf).wrapping_sub(1);
        parent.set_u8(x.b, 0xaf, n);
    }
    free(x);
}

handlers! {
    0x57cc4 "enemy_1" run,
}

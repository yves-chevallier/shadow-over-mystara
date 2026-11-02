//! "Hellhound" enemy (ENEMY type 6, 0x61364). See `docs/families/enemy.md`.
//!
//! Hell hound with eight arrival variants (`+3`) and two sets of AI parameters (`+0x2f`). `active_loop` framework;
//! animations 0x162126 (movements) and 0x162148 (attacks).
//!
//! | `+5` | File | Role (h) |
//! |---|---|---|
//! | 0 | `arrival` | arrival according to `+6` (run, jump, fall from above, appearance) |
//! | 2 | `stance` | standing: approach, turn-around, wait, spit, jump (`+6` = 0 to 10) |
//! | 4 | `attacks` | attacks and attack jumps (`+6` = 0 to 16) |
//! | 6, 8 | `attacks` | leaps, special moves (`+6` = 0 to 12) |
//! | 10 | `attacks` | rest as long as no player is alive |
//! | 12 | `attacks` | grab of a player |
//! | state 4 | `hit` | reactions to hits, recoil, fall, grab by a player |

use crate::families::cx::Bench;
use super::cx::{handlers, Cx};
use super::enemy_common::*;
use crate::objects::Obj;

mod arrival;
mod attacks;
mod hit;
mod stance;

pub(self) mod g {
    pub const GROUP: u32 = 0x2f;
    pub const STRIKE_KIND: u32 = 0x44;
    pub const STRIKE_POWER: u32 = 0x45;
    pub const VULNERABLE: u32 = 0x67;
    pub const AI_WAIT: u32 = 0x6e;
    pub const AI_CHARGE: u32 = 0x71;
    pub const AI_WANDER: u32 = 0x6b;
    pub const AI_LUNGE: u32 = 0x77;
    pub const SHOT_KIND: u32 = 0x79;
    pub const TIMER: u32 = 0xa0;
    pub const LOOK: u32 = 0xa1;
    pub const LOOK_PREV: u32 = 0xa2;
    /// Substate to resume after a turn-around or a move.
    pub const RESUME_SUB2: u32 = 0xa4;
    pub const RESUME_STATE: u32 = 0xa6;
    pub const TIMER_W: u32 = 0xaa;
    pub const SCRIPT: u32 = 0xac;
    pub const GUARD: u32 = 0xad;
    pub const NO_ZONE: u32 = 0xaf;
    pub const BITES: u32 = 0xb0;
}
use g::*;
use crate::objects::pools;
use crate::objects::Current;

pub(self) const SET_MOVE: u32 = 0x162126;
pub(self) const SET_STRIKE: u32 = 0x162148;

pub fn run<R: Bench>(x: &mut Cx<'_, R>) {
    if suspended(x) {
        return;
    }
    x.tick_counters();
    match x.u8(f::STATE) {
        st::INIT => init(x),
        st::ACTIVE => active(x),
        st::HIT => hit::run(x),
        st::DEAD => x.free(&pools::ENEMY),
        _ => {}
    }
}

// --- aides --------------------------------------------------------------------------------------

pub(self) fn mv<R: Bench>(x: &mut Cx<'_, R>, n: u32) {
    x.anim_start(SET_MOVE, n as u16);
}

pub(self) fn strike<R: Bench>(x: &mut Cx<'_, R>, n: u32) {
    x.anim_start(SET_STRIKE, n as u16);
}

/// The wanted look: `+0xa1` and its twin `+0xa2`.
pub(self) fn look<R: Bench>(x: &mut Cx<'_, R>, v: u8) {
    x.set8(LOOK, v);
    x.set8(LOOK_PREV, v);
}

pub(self) fn set_speed<R: Bench>(x: &mut Cx<'_, R>, n: u32) {
    x.select_speed(&crate::objects::motion::SPEEDS_B, n as u16);
}

/// Walking direction: 8 (looks left) or 0x18.
pub(self) fn set_angle<R: Bench>(x: &mut Cx<'_, R>) {
    let angle = if x.u8(f::FLIP) != 0 { 8 } else { 0x18 };
    x.set8(Obj::SPEED_INDEX, angle);
}

/// Common step of the moves: waits for animation key `+0x33` then moves on to the next step (0x6244a).
pub(self) fn wait_key<R: Bench>(x: &mut Cx<'_, R>) {
    if x.u8(f::KEY_FRAME) != 0 {
        x.add8(f::SUB3, 2);
        x.set8(VULNERABLE, 0);
    }
    x.anim_step();
}

/// Landing of a jump (0x62460): the move ends with dust and animation 0xc.
pub(self) fn land<R: Bench>(x: &mut Cx<'_, R>) {
    if x.move_xz().gt() {
        x.anim_step();
        return;
    }
    land_dust(x);
}

/// 0x62478: dust, back to the ground, next step.
pub(self) fn land_dust<R: Bench>(x: &mut Cx<'_, R>) {
    dust(x);
    lib::sound(x, 0xec820);
    x.set32(Obj::Z, 0);
    x.add8(f::SUB3, 2);
    x.set8(VULNERABLE, 1);
    look(x, 0xc);
    mv(x, 0xc);
}

/// 0x63a50: four dust clouds around the enemy.
pub(self) fn dust<R: Bench>(x: &mut Cx<'_, R>) {
    for d2 in [-0x1c, -8, 8, 0x1c] {
        lib::shock(x, 0, 0, d2 as u16, 4);
    }
}

/// One smoke step every `mask + 1` frames (`A5+0x6e` is the frame counter, which the original masks in place).
pub(self) fn puff<R: Bench>(x: &mut Cx<'_, R>, mask: u16, d0: u32, d1: u32, d2: i16, d3: i16) {
    let v = x.a5_u16(0x6e) & mask;
    x.set_a5_u16(0x6e, v);
    if v == 0 {
        lib::shock(x, d0, d1 as u16, d2 as u16, d3 as u16);
    }
}

/// 0x639fe: the enemy must turn around to face its target (false without a target or a saved look).
pub(self) fn must_turn<R: Bench>(x: &mut Cx<'_, R>) -> bool {
    if x.u8(LOOK_PREV) == 0 || x.u16(f::TARGET) == 0 {
        return false;
    }
    let target = x.linked(f::TARGET);
    let dx = target.u16(x.b, Obj::X).wrapping_sub(x.u16(Obj::X)) as i16;
    let want = if dx > 0 { 0x20 } else { 0 };
    let turn = want != x.u8(f::FLIP);
    turn
}

/// 0x63a98: the target is in the same row (depth within 0x30) but far in X (more than 0xf0).
pub(self) fn target_far_in_line<R: Bench>(x: &mut Cx<'_, R>) -> bool {
    if x.u16(f::TARGET) == 0 {
        return false;
    }
    let target = x.linked(f::TARGET);
    let dz = target.u16(x.b, Obj::Y).wrapping_sub(x.u16(Obj::Y)).wrapping_add(0x30);
    if dz > 0x60 {
        return false;
    }
    let dx = target.u16(x.b, Obj::X).wrapping_sub(x.u16(Obj::X)).wrapping_add(0xf0);
    let far = dx > 0x1e0;
    far
}

/// 0x62f1e / 0x62f26: chooses the next attack (`+6`) from the difficulty row of profile `+0x2f`; `bias` = 8 for the far
/// series.
pub(self) fn pick_attack<R: Bench>(x: &mut Cx<'_, R>, bias: u8) {
    let profile = x.u8(GROUP) as u32;
    let list = x.data32(0x62f80 + bias as u32 + 4 * profile);
    let row = ((var::RANK.get(x.b) >> 1) << 5) as u32;
    let r = rand_and(x, 0x1f);
    let mv = x.data8(list + row + r);
    x.set8(f::SUB2, mv.wrapping_add(bias));
    x.set16(f::STATE, 0x0204);
    x.set8(f::SUB3, 0);
    if !x.in_arena(0) && x.u8(f::SUB2) == 0x10 {
        x.set8(f::SUB2, 8);
    }
}

/// 0x63790: end of a move: if the target is in a range box, an attack of series `0x637fe` follows; otherwise standing.
pub(self) fn after_strike<R: Bench>(x: &mut Cx<'_, R>) {
    if x.u16(f::TARGET) == 0 {
        return set_state_long(x, STANCE);
    }
    let close = lib::target_in_box(x, -0x30, 0x30, 8) || lib::target_in_box(x, -0xd0, 0x70, 8);
    if !close {
        return set_state_long(x, STANCE);
    }
    let row = ((var::RANK.get(x.b) >> 1) << 5) as u32;
    let r = rand_and(x, 0x1f);
    let mv = x.data8(0x637fe + row + r);
    x.set8(f::SUB2, mv);
    x.set16(f::STATE, 0x0208);
    x.set8(f::SUB3, 0);
}

pub(self) const STANCE: u32 = 0x0202_0000;
pub(self) const HELD: u32 = 0x0408_0000;
pub(self) const IDLE: u32 = 0x020a_0000;

// --- state 0: creation --------------------------------------------------------------------------

fn init<R: Bench>(x: &mut Cx<'_, R>) {
    if paused(x) {
        return;
    }
    x.add8(f::STATE, 2);
    x.set8(0x6f, 1);
    x.set8(f::BOX_SET, 0x12);
    lib::initial_hp(x);
    for (off, v) in [(0x70, 0), (0x7e, 0xf), (0x7f, 0), (0x8e, 0), (0x78, 0xff), (0x81, 0xf), (0x80, 0x10)] {
        x.set8(off, v);
    }
    x.set32(f::IMMUNITY, 0);
    lib::set_immunities(x);
    let params: [(u32, u8); 10] = if x.u8(GROUP) == 0 {
        [(0x68, 2), (0x69, 2), (0x6a, 0), (0x6c, 3), (0x6d, 2), (0x72, 4), (0x73, 3), (0x74, 3), (0x75, 0x24), (0x76, 4)]
    } else {
        [(0x68, 2), (0x69, 2), (0x6a, 0), (0x6c, 4), (0x6d, 2), (0x72, 5), (0x73, 4), (0x74, 2), (0x75, 0x28), (0x76, 6)]
    };
    for (off, v) in params {
        x.set8(off, v);
    }
    if !pick_target(x) {
        return set_state_long(x, IDLE);
    }
    lib::draw_stance(x);
    lib::draw_6b(x);
    // starting state, look and animation according to the variant
    let (state, look_v, anim) = match x.u8(Obj::VARIANT) {
        0 => (STANCE, 0xc, 2),
        1 => (0x0202_0600, 3, 3),
        2 => (0x0200_0400, 0, 0),
        3 => {
            x.set8(f::FLIP, 0x20);
            (0x0200_0600, 0, 0)
        }
        4 => (0x0200_0800, 0xf, 0xf),
        5 => (0x0200_0000, 0x17, 0x10),
        6 => (0x0200_0200, 0, 0),
        7 => (0x0200_0a00, 0x1a, 9),
        _ => return,
    };
    set_state_long(x, state);
    look(x, look_v);
    x.set8(VULNERABLE, 1);
    mv(x, anim);
}

// --- state 2: active -----------------------------------------------------------------------------

fn active<R: Bench>(x: &mut Cx<'_, R>) {
    active_loop(x, |x| {
        match x.u8(f::SUB) {
            0 => arrival::run(x),
            2 => stance::run(x),
            4 => attacks::run(x),
            6 => attacks::run_b(x),
            8 => attacks::run_c(x),
            10 => attacks::idle(x),
            12 => attacks::grab(x),
            _ => {}
        }
        false
    });
}

handlers! {
    0x61364 "enemy_6" #[without] run,
}

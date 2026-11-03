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
use crate::record::record;
use crate::data::RomAddr;
use super::cx::{handlers, Cx};
use super::enemy_common::*;
use crate::objects::Obj;

mod arrival;
mod attacks;
mod hit;
mod stance;

/// Fields of the family beyond the common ones of `Enemy` (`record!`). A meaning guessed from the code alone is
/// marked (h).
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub struct Hellhound(pub Obj);

impl Hellhound {
    pub fn at(self, off: u32) -> u32 {
        self.0.at(off)
    }
}

record! {
    impl Hellhound {
        /// Second value of the current step of the flame script (low byte of the word `Enemy::CREATOR`) (h).
        flame_arg, set_flame_arg, FLAME_ARG: u8 @ 0x43;
        timer, set_timer, TIMER: u8 @ 0xa0;
        look, set_look, LOOK: u8 @ 0xa1;
        look_prev, set_look_prev, LOOK_PREV: u8 @ 0xa2;
        /// Substate to resume after a turn-around or a move.
        resume_sub2, set_resume_sub2, RESUME_SUB2: u8 @ 0xa4;
        resume_state, set_resume_state, RESUME_STATE: u32 @ 0xa6;
        timer_w, set_timer_w, TIMER_W: u8 @ 0xaa;
        script, set_script, SCRIPT: u8 @ 0xac;
        guard, set_guard, GUARD: u8 @ 0xad;
        no_zone, set_no_zone, NO_ZONE: u8 @ 0xaf;
        bites, set_bites, BITES: u8 @ 0xb0;
    }
}
use crate::objects::pools;
use crate::objects::Current;

const SET_MOVE: RomAddr = RomAddr(0x162126);
const SET_STRIKE: RomAddr = RomAddr(0x162148);

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

fn mv<R: Bench>(x: &mut Cx<'_, R>, n: u32) {
    x.anim_start(SET_MOVE.0, n as u16);
}

fn strike<R: Bench>(x: &mut Cx<'_, R>, n: u32) {
    x.anim_start(SET_STRIKE.0, n as u16);
}

/// The wanted look: `+0xa1` and its twin `+0xa2`.
fn look<R: Bench>(x: &mut Cx<'_, R>, v: u8) {
    x.set8(Hellhound::LOOK, v);
    x.set8(Hellhound::LOOK_PREV, v);
}

fn set_speed<R: Bench>(x: &mut Cx<'_, R>, n: u32) {
    x.select_speed(&crate::objects::motion::SPEEDS_B, n as u16);
}

/// Walking direction: 8 (looks left) or 0x18.
fn set_angle<R: Bench>(x: &mut Cx<'_, R>) {
    let angle = if x.u8(f::FLIP) != 0 { 8 } else { 0x18 };
    x.set8(Obj::SPEED_INDEX, angle);
}

/// Common step of the moves: waits for animation key `+0x33` then moves on to the next step (0x6244a).
fn wait_key<R: Bench>(x: &mut Cx<'_, R>) {
    if x.u8(f::KEY_FRAME) != 0 {
        x.add8(f::SUB3, 2);
        x.set8(Enemy::VULNERABLE, 0);
    }
    x.anim_step();
}

/// Landing of a jump (0x62460): the move ends with dust and animation 0xc.
fn land<R: Bench>(x: &mut Cx<'_, R>) {
    if x.move_xz().gt() {
        x.anim_step();
        return;
    }
    land_dust(x);
}

/// 0x62478: dust, back to the ground, next step.
fn land_dust<R: Bench>(x: &mut Cx<'_, R>) {
    dust(x);
    lib::sound(x, crate::sound::CMD_35F_45F);
    x.set32(Obj::Z, 0);
    x.add8(f::SUB3, 2);
    x.set8(Enemy::VULNERABLE, 1);
    look(x, 0xc);
    mv(x, 0xc);
}

/// 0x63a50: four dust clouds around the enemy.
fn dust<R: Bench>(x: &mut Cx<'_, R>) {
    for d2 in [-0x1c, -8, 8, 0x1c] {
        lib::shock(x, 0, 0, d2 as u16, 4);
    }
}

/// One smoke step every `mask + 1` frames (`A5+0x6e` is the frame counter, which the original masks in place).
fn puff<R: Bench>(x: &mut Cx<'_, R>, mask: u16, d0: u32, d1: u32, d2: i16, d3: i16) {
    let v = var::FRAME_COUNTER.get(x.b) & mask;
    var::FRAME_COUNTER.set(x.b, v);
    if v == 0 {
        lib::shock(x, d0, d1 as u16, d2 as u16, d3 as u16);
    }
}

/// 0x639fe: the enemy must turn around to face its target (false without a target or a saved look).
fn must_turn<R: Bench>(x: &mut Cx<'_, R>) -> bool {
    if x.u8(Hellhound::LOOK_PREV) == 0 || x.u16(f::TARGET) == 0 {
        return false;
    }
    let target = x.linked(f::TARGET);
    let dx = target.u16(x.b, Obj::X).wrapping_sub(x.u16(Obj::X)) as i16;
    let want = if dx > 0 { 0x20 } else { 0 };
    want != x.u8(f::FLIP)
}

/// 0x63a98: the target is in the same row (depth within 0x30) but far in X (more than 0xf0).
fn target_far_in_line<R: Bench>(x: &mut Cx<'_, R>) -> bool {
    if x.u16(f::TARGET) == 0 {
        return false;
    }
    let target = x.linked(f::TARGET);
    let dz = target.u16(x.b, Obj::Y).wrapping_sub(x.u16(Obj::Y)).wrapping_add(0x30);
    if dz > 0x60 {
        return false;
    }
    let dx = target.u16(x.b, Obj::X).wrapping_sub(x.u16(Obj::X)).wrapping_add(0xf0);
    dx > 0x1e0
}

/// 0x62f1e / 0x62f26: chooses the next attack (`+6`) from the difficulty row of profile `+0x2f`; `bias` = 8 for the far
/// series.
fn pick_attack<R: Bench>(x: &mut Cx<'_, R>, bias: u8) {
    let profile = x.u8(Enemy::GROUP) as u32;
    let list = tables::table("hellhound/move_lists").at32(x.b, bias as u32 + 4 * profile);
    let row = rank_row(x);
    let r = rand_and(x, 0x1f);
    let mv = tables::held8(x.b, list + row + r);
    x.set8(f::SUB2, mv.wrapping_add(bias));
    x.set16(f::STATE, 0x0204);
    x.set8(f::SUB3, 0);
    if !x.in_arena(0) && x.u8(f::SUB2) == 0x10 {
        x.set8(f::SUB2, 8);
    }
}

/// 0x63790: end of a move: if the target is in a range box, an attack of the table `follow_moves` follows; otherwise standing.
fn after_strike<R: Bench>(x: &mut Cx<'_, R>) {
    if x.u16(f::TARGET) == 0 {
        return set_state_long(x, STANCE);
    }
    let close = lib::target_in_box(x, -0x30, 0x30, 8) || lib::target_in_box(x, -0xd0, 0x70, 8);
    if !close {
        return set_state_long(x, STANCE);
    }
    let mv = rank_byte(x, "hellhound/follow_moves");
    x.set8(f::SUB2, mv);
    x.set16(f::STATE, 0x0208);
    x.set8(f::SUB3, 0);
}

const STANCE: u32 = 0x0202_0000;
const HELD: u32 = 0x0408_0000;
const IDLE: u32 = 0x020a_0000;

// --- state 0: creation --------------------------------------------------------------------------

fn init<R: Bench>(x: &mut Cx<'_, R>) {
    if paused(x) {
        return;
    }
    x.add8(f::STATE, 2);
    x.set8(Enemy::AI_SPARE_ROW, 1);
    x.set8(f::BOX_SET, 0x12);
    lib::initial_hp(x);
    for (off, v) in [(Enemy::AI_STANCE_ROW, 0), (Enemy::EVENT_KIND, 0xf), (Enemy::EVENT_LOW, 0), (Enemy::AI_ROW_8E, 0), (Enemy::AI_SHOT, 0xff), (Enemy::LOOT, 0xf), (Enemy::LOOT_ROW, 0x10)] {
        x.set8(off, v);
    }
    x.set32(f::IMMUNITY, 0);
    lib::set_immunities(x);
    let params: [(u32, u8); 10] = if x.u8(Enemy::GROUP) == 0 {
        [(Enemy::AI_68, 2), (Enemy::AI_69, 2), (Enemy::AI_WANDER_ROW, 0), (Enemy::AI_WAIT_MASKS, 3), (Enemy::AI_WAIT_ROW, 2), (Enemy::AI_DECISION_A, 4), (Enemy::AI_DECISION_B, 3), (Enemy::AI_BOX_CHANCE, 3), (Enemy::AI_BOX_ANIM, 0x24), (Enemy::AI_BYTE_ROW, 4)]
    } else {
        [(Enemy::AI_68, 2), (Enemy::AI_69, 2), (Enemy::AI_WANDER_ROW, 0), (Enemy::AI_WAIT_MASKS, 4), (Enemy::AI_WAIT_ROW, 2), (Enemy::AI_DECISION_A, 5), (Enemy::AI_DECISION_B, 4), (Enemy::AI_BOX_CHANCE, 2), (Enemy::AI_BOX_ANIM, 0x28), (Enemy::AI_BYTE_ROW, 6)]
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
    x.set8(Enemy::VULNERABLE, 1);
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
    ENEMY_6 0x61364 "enemy_6" run,
}
